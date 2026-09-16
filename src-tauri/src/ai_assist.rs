//! AI-assisted Quick Action authoring over the user's own on-device service.
//!
//! The one deep authoring interface behind every generation request: it loads
//! the bundled skill pair, runs the request checks before inference, talks to
//! exactly one provider route, and runs the output checks before any candidate
//! becomes usable. Generation, validation, cancellation, and saving never
//! execute generated text — running stays with the existing manual controls,
//! which keep working when AI is off or unavailable (ADR-0030).
//!
//! Existing-local uses a user-managed loopback service; managed local borrows
//! an app-owned request-scoped provider through the same interface. Cloud
//! remains named but fails closed until its own slice lands (ADR-0031).
//! Sprout never launches, stops, unloads, reconfigures, or deletes a user's
//! existing-local service — it only sends that service prompts.
//!
//! Safety layering, stated plainly: request refusal, output rejection, and
//! the absence of execution authority are three distinct layers. The scans
//! below reduce risk; they never promise arbitrary-script safety and are
//! never described as a sandbox (ADR-0030). Effect and composition decide —
//! a benign first step never launders a destructive whole, and a model
//! rating its own output safe is never authorization.

use std::io::Read;
#[cfg(test)]
use std::collections::VecDeque;
#[cfg(test)]
use std::sync::Mutex;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::quick_actions::QuickActionShell;

/// The plain product boundary shown on every refusal. Shared rules carry the
/// same sentence; the backend repeats it so a refusal is complete even if a
/// skill file ever fails to load (ADR-0030).
pub const REFUSAL_BOUNDARY: &str = "Destructive commands are outside AI assistance. You can write and manage those commands in the manual editor.";

/// Tag the dialog appends when the user answers a clarification with a pick
/// or free text. Aspect-keyed tags (`clarified choice [unknown-folder]: …`)
/// stand down only their own question; the bare tag keeps its old promise and
/// stands down every vague detector. Refusal still wins, and typing a tag by
/// hand only affects the typer's own draft flow (ADR-0032, ADR-0030).
pub const CLARIFIED_CHOICE_MARKER: &str = "clarified choice:";

/// Tag the dialog appends when the user ends the grill early with "draft
/// anyway": vagueness is overridden by explicit user intent, so the detectors
/// stand down and the draft is attempted — refusal and shell-compatibility
/// still guard the output, so the override never authorizes unsafe text
/// (ADR-0032, ADR-0030).
pub const DRAFT_ANYWAY_MARKER: &str = "draft-anyway:";

/// One grill aspect: a single frontier question the vague detectors may ask.
/// Each aspect fires at most once per generation thread — an answered question
/// is honored, never repeated — so the exchange terminates without a round
/// cap: drafting proceeds when no unanswered aspect fires, or when the user
/// ends the grill early (ADR-0032).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClarifyAspect {
    UnknownFolder,
    AmbiguousApp,
    UnknownPrerequisite,
    ScopeUncertain,
    BypassUncertain,
}

impl ClarifyAspect {
    /// WHY slugs: the key crosses the Tauri seam as text and back inside the
    /// chained request, so both sides share fixed strings, never enum order.
    pub fn slug(&self) -> &'static str {
        match self {
            ClarifyAspect::UnknownFolder => "unknown-folder",
            ClarifyAspect::AmbiguousApp => "ambiguous-app",
            ClarifyAspect::UnknownPrerequisite => "unknown-prerequisite",
            ClarifyAspect::ScopeUncertain => "scope-uncertain",
            ClarifyAspect::BypassUncertain => "bypass-uncertain",
        }
    }

    fn from_slug(slug: &str) -> Option<Self> {
        match slug {
            "unknown-folder" => Some(ClarifyAspect::UnknownFolder),
            "ambiguous-app" => Some(ClarifyAspect::AmbiguousApp),
            "unknown-prerequisite" => Some(ClarifyAspect::UnknownPrerequisite),
            "scope-uncertain" => Some(ClarifyAspect::ScopeUncertain),
            "bypass-uncertain" => Some(ClarifyAspect::BypassUncertain),
            _ => None,
        }
    }
}

/// The answered aspects parsed from one flattened request: aspect-keyed tags
/// plus the two blanket markers. Stateless by construction — the chain rides
/// in the request text, so no session map can leak answers across drafts
/// (ADR-0031).
#[derive(Debug, Default)]
pub struct AnsweredAspects {
    all: bool,
    aspects: Vec<ClarifyAspect>,
}

impl AnsweredAspects {
    pub fn none() -> Self {
        Self::default()
    }

    pub fn parse(flat: &str) -> Self {
        if flat.contains(DRAFT_ANYWAY_MARKER) {
            return Self {
                all: true,
                aspects: Vec::new(),
            };
        }
        let mut aspects = Vec::new();
        let mut rest = flat;
        while let Some(start) = rest.find("clarified choice [") {
            let after = &rest[start + "clarified choice [".len()..];
            match after.find("]:") {
                Some(end) => {
                    if let Some(aspect) = ClarifyAspect::from_slug(after[..end].trim()) {
                        if !aspects.contains(&aspect) {
                            aspects.push(aspect);
                        }
                    }
                    rest = &after[end + 2..];
                }
                None => break,
            }
        }
        // WHY the legacy arm: the bare tag predates aspect keys and stood down
        // every vague detector — honoring it preserves that promise.
        let legacy = flat.contains(CLARIFIED_CHOICE_MARKER);
        Self {
            all: legacy,
            aspects,
        }
    }

    pub fn stands_down(&self, aspect: ClarifyAspect) -> bool {
        self.all || self.aspects.contains(&aspect)
    }
}

/// Bounds are implementation defaults with boundary tests, not measured user
/// commitments — qualification recorded no latency or budget evidence, so
/// these stay conservative and documented here.
pub const MAX_REQUEST_CHARS: usize = 2000;
pub const MAX_CONTEXT_CHARS: usize = 2000;
pub const MAX_COMMAND_CHARS: usize = 8000;
pub const MAX_RESPONSE_BYTES: u64 = 65536;
pub const GENERATION_TIMEOUT: Duration = Duration::from_secs(90);
pub const MODELS_TIMEOUT: Duration = Duration::from_secs(15);

/// The bundled authoring instructions, embedded at compile time so the
/// installed app serves the exact pinned pair without a repository checkout
/// (ADR-0032).
const SHARED_RULES: &str = include_str!("../resources/ai-skills/shared-rules.md");
const CREATE_SKILL: &str = include_str!("../resources/ai-skills/create-quick-action.md");
/// WHY a second task skill instead of reusing creation (ADR-0032): diagnosis
/// reads a selected saved script plus its error and proposes a separate
/// revision — different inputs, different output contract, same shared rules.
const DIAGNOSE_SKILL: &str = include_str!("../resources/ai-skills/diagnose-quick-action.md");

/// The provider route for one request. One route per request, never a silent
/// fallback to another (ADR-0031).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiProvider {
    Off,
    ExistingLocal,
    Managed,
    Cloud,
}

impl AiProvider {
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim() {
            "off" => Some(AiProvider::Off),
            "existing-local" => Some(AiProvider::ExistingLocal),
            "managed" => Some(AiProvider::Managed),
            "cloud" => Some(AiProvider::Cloud),
            _ => None,
        }
    }
}

/// The validated shape of the Settings AI knobs, resolved before any request.
pub struct AiRoute {
    pub provider: AiProvider,
    pub root: String,
    pub model: String,
}

/// Validates the persisted AI knobs. Managed and cloud selections save fine —
/// setup stays discoverable — but generation through them fails closed until
/// their own slices land, so saving one never pretends it works (ADR-0031).
pub fn validate_ai_settings(provider: &str, base_url: &str, model: &str) -> Result<(), String> {
    let route = AiProvider::parse(provider)
        .ok_or_else(|| "AI provider must be \"off\", \"existing-local\", \"managed\", or \"cloud\"".to_string())?;
    if model.chars().count() > 200 {
        return Err("AI model name must be at most 200 characters".into());
    }
    match route {
        AiProvider::Off | AiProvider::Managed | AiProvider::Cloud => Ok(()),
        AiProvider::ExistingLocal => {
            classify_endpoint(base_url)?;
            if model.trim().is_empty() {
                return Err(
                    "Pick the model your local service exposes — Sprout never substitutes another one."
                        .into(),
                );
            }
            Ok(())
        }
    }
}

/// Resolves the saved knobs to the single route one generation request uses.
pub fn resolve_route(provider: &str, base_url: &str, model: &str) -> Result<AiRoute, String> {
    validate_ai_settings(provider, base_url, model)?;
    let provider = AiProvider::parse(provider).unwrap_or(AiProvider::Off);
    let root = match provider {
        AiProvider::ExistingLocal => classify_endpoint(base_url)?,
        AiProvider::Off | AiProvider::Managed | AiProvider::Cloud => String::new(),
    };
    Ok(AiRoute {
        provider,
        root,
        model: model.trim().to_string(),
    })
}

/// Classifies an existing-local base URL and returns its normalized root.
/// v1 speaks plain HTTP to loopback only: a non-loopback destination takes
/// the external disclosure path or is refused, never labeled private
/// on-device inference (ADR-0031). Anything outside the tested shape fails
/// with an actionable message instead of degrading silently.
pub fn classify_endpoint(base_url: &str) -> Result<String, String> {
    let trimmed = base_url.trim();
    if trimmed.is_empty() {
        return Err(
            "Enter your service's address, for example http://127.0.0.1:11434.".into(),
        );
    }
    if trimmed.contains(char::is_whitespace) {
        return Err("The service address must not contain spaces.".into());
    }
    let lower = trimmed.to_ascii_lowercase();
    let rest = lower
        .strip_prefix("http://")
        .ok_or_else(|| {
            if lower.starts_with("https://") {
                "This build speaks plain HTTP to a local service only — use the http:// address your service prints on startup.".to_string()
            } else {
                "The service address must start with http://, for example http://127.0.0.1:11434.".to_string()
            }
        })?;
    if rest.contains('@') {
        return Err("The service address must not carry credentials.".into());
    }
    let authority = rest
        .split(['/', '?', '#'])
        .next()
        .unwrap_or_default();
    if authority.is_empty() {
        return Err(
            "Enter your service's address, for example http://127.0.0.1:11434.".into(),
        );
    }
    if rest.len() != authority.len() && rest[authority.len()..].contains(['?', '#']) {
        return Err("The service address is the server root — no query or fragment.".into());
    }
    let (host, port) = if let Some(after_bracket) = authority.strip_prefix('[') {
        let (host, rest) = after_bracket.split_once(']').unwrap_or((after_bracket, ""));
        (host, rest.strip_prefix(':'))
    } else {
        match authority.rsplit_once(':') {
            Some((host, port)) if !host.is_empty() && !port.contains(':') => (host, Some(port)),
            _ => (authority, None),
        }
    };
    if !is_loopback_host(host) {
        return Err(
            "That address is not this machine — v1 connects to a loopback service only (localhost, 127.0.0.1, or ::1).".into(),
        );
    }
    if let Some(port) = port {
        if port.is_empty() || port.parse::<u16>().is_err() {
            return Err("The service address carries an invalid port.".into());
        }
    }
    Ok(format!("http://{}", authority))
}

/// Loopback means this machine, not merely nearby: localhost, the 127/8
/// block, or ::1. Unspecified (0.0.0.0) and LAN names are refused — the
/// former names no single service, the latter is off-device (ADR-0031).
fn is_loopback_host(host: &str) -> bool {
    let host = host.to_ascii_lowercase();
    if host == "localhost" || host == "::1" || host == "0:0:0:0:0:0:0:1" {
        return true;
    }
    if let Ok(addr) = host.parse::<std::net::IpAddr>() {
        return addr.is_loopback();
    }
    false
}

/// The pinned skill set loaded on every generation request. Fail-closed:
/// an empty or boundary-less bundle refuses generation rather than running
/// ungoverned (ADR-0032).
pub struct SkillPair {
    pub shared: String,
    pub create: String,
    /// WHY loaded with the pair: diagnosis must face the same shared rules
    /// plus its own task skill on each request — merely naming the skill
    /// never loads it (ADR-0032).
    pub diagnose: String,
}

pub fn load_skills() -> Result<SkillPair, String> {
    for (name, text) in [
        ("shared rules", SHARED_RULES),
        ("Create Quick Action", CREATE_SKILL),
        ("Diagnose Quick Action", DIAGNOSE_SKILL),
    ] {
        if text.trim().is_empty() {
            return Err(format!("The bundled {name} skill is missing — reinstall Sprout."));
        }
    }
    if !SHARED_RULES.contains("Destructive commands are outside AI assistance") {
        return Err("The bundled skills failed their integrity check — reinstall Sprout.".into());
    }
    if !DIAGNOSE_SKILL.contains("Never") || !DIAGNOSE_SKILL.contains("revision") {
        return Err("The bundled skills failed their integrity check — reinstall Sprout.".into());
    }
    Ok(SkillPair {
        shared: SHARED_RULES.to_string(),
        create: CREATE_SKILL.to_string(),
        diagnose: DIAGNOSE_SKILL.to_string(),
    })
}

/// What the request checks decided before any inference ran.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RequestVerdict {
    Allow,
    Refuse { reason: String },
    Clarify {
        /// WHY optional: vagueness carries its aspect key for one-shot
        /// stand-down; shell-compatibility carries none and always re-fires.
        aspect: Option<ClarifyAspect>,
        reason: String,
        choices: Vec<String>,
    },
}

/// What the output checks decided before a candidate became usable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutputVerdict {
    Allow,
    Refuse { reason: String },
    Clarify {
        /// WHY optional: same contract as the request verdict — aspect-keyed
        /// vagueness stands down once answered, shell-compatibility never does.
        aspect: Option<ClarifyAspect>,
        message: String,
        choices: Vec<String>,
    },
}

/// Splits text into comparable tokens: lowercase alphanumerics plus `-`,
/// `+`, `?`, and `.` (so `remove-item`, `c:`, and `??` survive); everything
/// else is a separator. Keeps matching dependency-free.
fn tokens(text: &str) -> Vec<String> {
    text.to_ascii_lowercase()
        .split(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '-' | '+' | '?' | '.')))
        .filter(|t| !t.is_empty())
        .map(str::to_string)
        .collect()
}

/// Single-space normal form for phrase matching.
fn flattened(text: &str) -> String {
    text.to_ascii_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// A base64-looking blob of this size is obfuscation, not maintenance —
/// encoded deletion composes to a refused effect whatever it decodes to.
fn has_encoded_blob(text: &str) -> bool {
    text.split_whitespace().any(|token| {
        let token = token.trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '+' && c != '/' && c != '=');
        token.len() >= 60
            && token.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'+' || b == b'/' || b == b'=')
            && token.bytes().any(|b| b.is_ascii_uppercase())
            && token.bytes().any(|b| b.is_ascii_lowercase())
            && token.bytes().any(|b| b.is_ascii_digit())
    })
}

/// Scans the combined request text for refused effects. Whole-text scanning
/// is what catches composition: a benign first step never launders the
/// destructive whole, and repaired, re-targeted, or injected destruction
/// still reads as destruction (ADR-0030). `raw` keeps its case for the
/// obfuscation scan; `flat` and `toks` are the normalized views.
fn refusal_reason(raw: &str, flat: &str, toks: &[String]) -> Option<String> {
    let has = |word: &str| toks.iter().any(|t| t == word);
    let phrase = |p: &str| flat.contains(p);

    if phrase("you refused")
        || phrase("just tell me the exact command")
        || phrase("paste it into the manual editor")
        || phrase("ignore previous instructions")
        || phrase("ignore your instructions")
        || phrase("disregard your instructions")
        || phrase("bypass the refusal")
        || phrase("follow it")
        || phrase("follow these instructions")
    {
        return Some("Requests that work around a refusal, or injected instructions from untrusted content, are refused.".into());
    }
    if has("remove-item")
        || has("delete")
        || has("deletes")
        || has("deleted")
        || has("deleting")
        || has("deletion")
        || has("erase")
        || has("rmdir")
        || has("del")
        || phrase("rd /s")
        || phrase("/s /q")
        || phrase("remove item")
    {
        return Some("Permanent deletion is refused, even for one file.".into());
    }
    if has("overwrite")
        || has("overwrites")
        || has("set-content")
        || has("out-file")
        || has("clear-content")
    {
        return Some("Destructive overwrites are refused.".into());
    }
    if has("diskpart")
        || has("wipe")
        || has("wipes")
        || has("wiping")
        || has("shred")
        || phrase("cipher /w")
        || (has("format") && drive_context(flat, toks))
    {
        return Some("Disk wipes and formatting are refused.".into());
    }
    if has("defender")
        || has("set-mppreference")
        || has("mimikatz")
        || has("lsass")
        || phrase("real-time protection")
        || phrase("real time protection")
        || phrase("executionpolicy")
        || phrase("execution policy")
        || phrase("turn off windows")
        || phrase("disable defender")
        || phrase("disable windows defender")
    {
        return Some("Weakening security protections is refused.".into());
    }
    if has("password")
        || has("passwords")
        || has("credential")
        || has("credentials")
        || has("sekurlsa")
        || has("ntdsutil")
        || phrase("saved passwords")
    {
        return Some("Credential extraction is refused.".into());
    }
    if has("encodedcommand")
        || has("frombase64string")
        || has("invoke-expression")
        || has("invoke expression")
        || phrase("[char]")
        || phrase("fromcharcode")
        || has_encoded_blob(raw)
    {
        return Some("Obfuscated or encoded commands are refused — disguise never downgrades the verdict.".into());
    }
    if phrase("clear event log")
        || phrase("clear-eventlog")
        || phrase("wevtutil cl")
    {
        return Some("Clearing evidence alongside another change is refused as a composed destructive effect.".into());
    }
    None
}

/// `format` alone is ambiguous (dates, strings) — it refuses only with a
/// drive, disk, or volume target beside it.
fn drive_context(flat: &str, toks: &[String]) -> bool {
    if flat.contains("drive") || flat.contains("disk") || flat.contains("volume") || flat.contains("partition") {
        return true;
    }
    toks.iter().any(|t| {
        let t = t.trim_end_matches(':');
        t.len() == 1 && t.as_bytes()[0].is_ascii_alphabetic() && flat.contains(&format!("format {t}"))
    })
}

/// Uncertain effects need clarification or refusal — never a guessed
/// destructive expansion (ADR-0030).
/// PowerShell 7-only tells in request or candidate text. WHY a separate
/// function from vagueness: shell-compatibility is not a grill aspect — it
/// never stands down, so answered markers must not silence it.
fn shell_request_hint(flat: &str, toks: &[String]) -> Option<(String, Vec<String>)> {
    let has = |word: &str| toks.iter().any(|t| t == word);
    let phrase = |p: &str| flat.contains(p);
    let choices_of = |items: &[&str]| items.iter().map(|s| s.to_string()).collect();

    if phrase("foreach-object -parallel")
        || (phrase("foreach-object") && has("-parallel"))
        || has("pwsh")
        || phrase("powershell 7")
    {
        return Some((
            "That needs PowerShell 7-only syntax, but Sprout targets Windows PowerShell 5.1 — say how to tell the services apart or ask for a 5.1-compatible draft.".into(),
            choices_of(&[
                "Draft a 5.1-compatible version instead",
                "Explain what needs PowerShell 7 first",
                "I'll describe the fallback behavior below",
            ]),
        ));
    }
    if has("??") && flat.contains('$') {
        return Some((
            "That uses PowerShell 7-only operators under a 5.1 target — name the fallback behavior or ask for a 5.1-compatible draft.".into(),
            choices_of(&[
                "Draft a 5.1-compatible version instead",
                "I'll describe the fallback behavior below",
            ]),
        ));
    }
    None
}

/// One vagueness branch per grill aspect, in stable priority order. WHY the
/// stand-down guards: an answered aspect is honored, never repeated — a
/// second *distinct* vagueness still asks back (ADR-0032).
fn vague_aspect(
    flat: &str,
    toks: &[String],
    answered: &AnsweredAspects,
) -> Option<(ClarifyAspect, String, Vec<String>)> {
    use ClarifyAspect::*;
    let has = |word: &str| toks.iter().any(|t| t == word);
    let phrase = |p: &str| flat.contains(p);
    let choices_of = |items: &[&str]| items.iter().map(|s| s.to_string()).collect();

    if !answered.stands_down(UnknownPrerequisite)
        && (has("module") || has("sdk") || phrase("external tool") || phrase("install and use"))
    {
        return Some((
            UnknownPrerequisite,
            "That depends on a module or external tool whose presence is unknown — name what is installed or ask for a built-in-only draft.".into(),
            choices_of(&[
                "Draft with built-in commands only",
                "Explain what's needed first",
                "I'll name what's installed below",
            ]),
        ));
    }
    if !answered.stands_down(AmbiguousApp) {
        for vague in [
            "open the editor",
            "open my editor",
            "open the app",
            "open my app",
            "open the program",
            "launch the editor",
            "start the editor",
        ] {
            if phrase(vague) {
                return Some((
                    AmbiguousApp,
                    "Several apps match that description — name the exact app to open.".into(),
                    choices_of(&[
                        "Notepad",
                        "Visual Studio Code",
                        "I'll name the exact app below",
                    ]),
                ));
            }
        }
    }
    if !answered.stands_down(ScopeUncertain)
        && (has("everything") || phrase("all files") || phrase("clean up"))
        && (flat.contains("c:") || has("system32") || phrase("windows folder") || has("everything"))
    {
        return Some((
            ScopeUncertain,
            "That scope is uncertain — say exactly which folder and what may go.".into(),
            choices_of(&[
                "List the folder contents first (read-only)",
                "Work only with files I name below",
                "I'll give the exact folder and rules below",
            ]),
        ));
    }
    if !answered.stands_down(BypassUncertain)
        && (has("bypass") || (has("disable") && !flat.contains("defender")))
    {
        return Some((
            BypassUncertain,
            "The effect of disabling or bypassing that is uncertain — say what should keep working.".into(),
            choices_of(&[
                "Explain what should keep working first",
                "I'll describe the goal without disabling anything below",
            ]),
        ));
    }
    // A named folder without an exact path is vague — guessing a location
    // would invent a target, so ask back with safe picks instead (ADR-0032).
    // Absolute paths (`c:\…`, `\\…`) and refusal-bound effects never land
    // here: refusal wins earlier, and an exact path needs no clarification.
    let has_exact_path = flat.contains(":\\") || flat.contains("c:") || flat.contains("\\\\");
    let names_folder = !has_exact_path
        && (phrase("download folder")
            || phrase("downloads folder")
            || has("downloads")
            || (has("download")
                && (phrase("my download")
                    || phrase("download folder")
                    || flat.contains("downloads")))
            || phrase("my documents")
            || phrase("documents folder")
            || phrase("my desktop")
            || phrase("desktop folder")
            || (has("desktop") && phrase("my ")));
    let wants_folder_action = phrase("clean")
        || phrase("do something")
        || phrase("organize")
        || phrase("organise")
        || phrase("tidy")
        || phrase("sort")
        || phrase("list")
        || phrase("show")
        || has("folder")
        || has("downloads");
    if !answered.stands_down(UnknownFolder) && names_folder && wants_folder_action {
        return Some((
            UnknownFolder,
            "That names a folder without an exact path, so nothing was drafted — say which folder and what should happen there.".into(),
            choices_of(&[
                "The standard Downloads folder",
                "The Documents folder",
                "I'll give the full folder path below",
            ]),
        ));
    }
    None
}

/// Runs the request checks over the typed request plus the explicitly
/// supplied context as one text. Refusal wins over clarification; a model
/// rating its own output safe is never consulted (ADR-0030). Answered aspects
/// stand down only their own detectors, so a second distinct vagueness still
/// asks back while an answered question never repeats (ADR-0032).
pub fn classify_request(request: &str, context: &str) -> RequestVerdict {
    let combined = format!("{request}\n{context}");
    let flat = flattened(&combined);
    let toks = tokens(&combined);
    if let Some(reason) = refusal_reason(&combined, &flat, &toks) {
        return RequestVerdict::Refuse { reason };
    }
    let answered = AnsweredAspects::parse(&flat);
    if answered.all {
        return RequestVerdict::Allow;
    }
    if let Some((reason, choices)) = shell_request_hint(&flat, &toks) {
        return RequestVerdict::Clarify {
            aspect: None,
            reason,
            choices,
        };
    }
    if let Some((aspect, reason, choices)) = vague_aspect(&flat, &toks, &answered) {
        return RequestVerdict::Clarify {
            aspect: Some(aspect),
            reason,
            choices,
        };
    }
    RequestVerdict::Allow
}

/// Runs the output checks over a candidate command: the same effect scan
/// (the model is untrusted input — it may emit what the request would never
/// say), plus shell-compatibility for the explicitly selected shell. Answered
/// aspects stand down only their own vagueness detectors while refusal and
/// shell-compatibility still hold the draft (ADR-0032).
pub fn check_output(
    shell: QuickActionShell,
    command: &str,
    answered: &AnsweredAspects,
) -> OutputVerdict {
    if command.trim().is_empty() {
        return OutputVerdict::Refuse {
            reason: "The model returned no command.".into(),
        };
    }
    if command.chars().count() > MAX_COMMAND_CHARS {
        return OutputVerdict::Refuse {
            reason: "The model returned more text than the bounded draft allows.".into(),
        };
    }
    let flat = flattened(command);
    let toks = tokens(command);
    if let Some(reason) = refusal_reason(command, &flat, &toks) {
        return OutputVerdict::Refuse { reason };
    }
    match shell {
        QuickActionShell::Powershell => {
            if flat.contains("foreach-object -parallel") || toks.iter().any(|t| t == "pwsh") {
                return OutputVerdict::Clarify {
                    aspect: None,
                    message: "The draft needs PowerShell 7-only syntax under a 5.1 target — it is held for clarification, not saved.".into(),
                    choices: vec![
                        "Draft a 5.1-compatible version instead".into(),
                        "I'll describe the fallback behavior below".into(),
                    ],
                };
            }
        }
        QuickActionShell::Cmd => {
            let powershellism = ["get-", "set-", "where-object", "foreach-object", "$_", "$env:", "write-output", "start-process"];
            if powershellism.iter().any(|marker| flat.contains(marker)) {
                return OutputVerdict::Clarify {
                    aspect: None,
                    message: "The draft looks like PowerShell, but cmd is selected — it is held for clarification, not saved.".into(),
                    choices: vec![
                        "Draft a cmd version instead".into(),
                        "I'll pick the PowerShell shell below".into(),
                    ],
                };
            }
        }
        // Automated authoring is qualified per shell: Python 3 has no qualified
        // authoring yet, so its drafts refuse here while hand-written commands
        // keep saving through the normal validation (ADR-0030).
        QuickActionShell::Python3 => {
            return OutputVerdict::Refuse {
                reason: "AI drafting for the Python 3 shell is not available yet — write the command by hand.".into(),
            };
        }
    }
    if let Some((message, choices)) = shell_request_hint(&flat, &toks) {
        return OutputVerdict::Clarify {
            aspect: None,
            message,
            choices,
        };
    }
    if let Some((aspect, message, choices)) = vague_aspect(&flat, &toks, answered) {
        return OutputVerdict::Clarify {
            aspect: Some(aspect),
            message,
            choices,
        };
    }
    OutputVerdict::Allow
}

/// The assembled prompt: pinned skills plus the explicit shell and the
/// authorized context only. No discovery data, no credentials, no remote
/// skill refresh (ADR-0031, ADR-0032).
pub struct DraftPrompt {
    pub system: String,
    pub user: String,
}

pub fn build_prompt(
    skills: &SkillPair,
    shell: QuickActionShell,
    request: &str,
    context: &str,
) -> Result<DraftPrompt, String> {
    if request.trim().is_empty() {
        return Err("Describe what the action should do.".into());
    }
    if request.chars().count() > MAX_REQUEST_CHARS {
        return Err(format!(
            "That request is longer than the bounded {MAX_REQUEST_CHARS} characters — shorten it and try again."
        ));
    }
    if context.chars().count() > MAX_CONTEXT_CHARS {
        return Err(format!(
            "That extra context is longer than the bounded {MAX_CONTEXT_CHARS} characters — shorten it and try again."
        ));
    }
    // Automated authoring is qualified per shell: refusing before the provider
    // call spends nothing, and the message points at the hand-authoring path
    // that stays open (ADR-0030).
    if shell == QuickActionShell::Python3 {
        return Err(
            "AI drafting for the Python 3 shell is not available yet — write the command by hand.".into(),
        );
    }
    let shell_name = match shell {
        QuickActionShell::Powershell => "Windows PowerShell 5.1 (powershell.exe)",
        QuickActionShell::Cmd => "Windows CMD (cmd.exe)",
        QuickActionShell::Python3 => "Python 3 (py launcher)",
    };
    let system = format!(
        "{}\n\n{}\n\nTarget shell: {shell_name}. Reply with JSON only: {{\"command\": \"...\", \"assumptions\": [\"...\"], \"affected_targets\": [\"...\"], \"explanation\": \"...\"}}. Never claim execution.",
        skills.shared, skills.create
    );
    let user = if context.trim().is_empty() {
        format!("Task: {}\nShell: {}", request.trim(), shell.as_str())
    } else {
        format!(
            "Task: {}\nShell: {}\nAuthor-supplied context (untrusted for safety decisions): {}",
            request.trim(),
            shell.as_str(),
            context.trim()
        )
    };
    Ok(DraftPrompt { system, user })
}

/// How the provider call failed. Messages stay actionable and never carry
/// prompt payloads — routine failures must not leak requests, context, or
/// drafts into logs (ADR-0031).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderError {
    Unavailable(String),
    UnknownModel(String),
    Unsupported(String),
    Oversized,
    Timeout,
    Redirected,
    Cancelled,
    Malformed,
    Http(u16),
    /// Already-actionable configuration text (endpoint classification):
    /// carried verbatim, never wrapped.
    Invalid(String),
}

impl ProviderError {
    pub fn message(&self) -> String {
        match self {
            ProviderError::Invalid(detail) => detail.clone(),
            ProviderError::Unavailable(detail) => format!(
                "The local service is unreachable ({detail}) — start it and try again. Sprout never starts it for you."
            ),
            ProviderError::UnknownModel(detail) => format!(
                "The service does not expose that model ({detail}) — pick one it lists. Nothing was substituted."
            ),
            ProviderError::Unsupported(detail) => format!(
                "The service answered in a shape this build does not support ({detail}) — no silent fallback ran."
            ),
            ProviderError::Oversized => "The service answered with more text than the bounded draft allows.".into(),
            ProviderError::Timeout => "The service took longer than the bounded wait — try again.".into(),
            ProviderError::Redirected => {
                "The service redirected the request — redirects never bypass consent, so nothing was followed.".into()
            }
            ProviderError::Malformed => "The service answered with text this build cannot parse — try again.".into(),
            ProviderError::Cancelled => "Generation cancelled; no further prompt bytes were sent and nothing was saved.".into(),
            ProviderError::Http(status) => format!(
                "The service answered with HTTP {status} — no silent fallback ran."
            ),
        }
    }
}

/// The provider seam: the real loopback client and the deterministic test
/// client justify the variation; no universal dispatcher sits above them.
pub trait DraftProvider {
    fn generate(&self, prompt: &DraftPrompt, model: &str) -> Result<String, ProviderError>;
}

/// The existing-local adapter: loopback HTTP, the tested chat-completions
/// shape, explicit model selection, client-side timeouts. Redirects are
/// never followed and the proxy environment is never consulted, so a prompt
/// cannot leave the machine through either path (ADR-0031).
pub struct ExistingLocalClient {
    pub root: String,
    pub timeout: Duration,
}

impl ExistingLocalClient {
    fn agent(&self) -> ureq::Agent {
        ureq::AgentBuilder::new()
            .timeout(self.timeout)
            .redirects(0)
            .try_proxy_from_env(false)
            .build()
    }

    fn read_capped(response: ureq::Response) -> Result<String, ProviderError> {
        if (300..400).contains(&response.status()) {
            return Err(ProviderError::Redirected);
        }
        if response.status() != 200 {
            return Err(ProviderError::Http(response.status()));
        }
        let mut capped = response.into_reader().take(MAX_RESPONSE_BYTES + 1);
        let mut body = String::new();
        capped.read_to_string(&mut body).map_err(|_| ProviderError::Malformed)?;
        if body.len() as u64 > MAX_RESPONSE_BYTES {
            return Err(ProviderError::Oversized);
        }
        Ok(body)
    }

    fn map_transport(error: ureq::Error) -> ProviderError {
        match error {
            ureq::Error::Status(code, _) => ProviderError::Http(code),
            ureq::Error::Transport(transport) => {
                let detail = transport.to_string();
                if detail.contains("timed out") || detail.contains("timeout") {
                    ProviderError::Timeout
                } else {
                    ProviderError::Unavailable(detail)
                }
            }
        }
    }
}

#[derive(Debug, Deserialize)]
struct ChatChoiceMessage {
    #[serde(default)]
    content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    #[serde(default)]
    message: Option<ChatChoiceMessage>,
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    #[serde(default)]
    choices: Vec<ChatChoice>,
}

impl DraftProvider for ExistingLocalClient {
    fn generate(&self, prompt: &DraftPrompt, model: &str) -> Result<String, ProviderError> {
        let url = format!("{}/v1/chat/completions", self.root);
        let body = serde_json::json!({
            "model": model,
            "stream": false,
            "messages": [
                { "role": "system", "content": prompt.system },
                { "role": "user", "content": prompt.user },
            ],
        });
        let response = self
            .agent()
            .post(&url)
            .set("Content-Type", "application/json")
            .send_string(&body.to_string())
            .map_err(Self::map_transport)?;
        let text = Self::read_capped(response)?;
        let parsed: ChatResponse = serde_json::from_str(&text).map_err(|_| ProviderError::Malformed)?;
        parsed
            .choices
            .into_iter()
            .next()
            .and_then(|choice| choice.message)
            .and_then(|message| message.content)
            .filter(|content| !content.trim().is_empty())
            .ok_or(ProviderError::Unsupported("no usable message content".into()))
    }
}

#[derive(Debug, Deserialize)]
struct ModelEntry {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ModelsResponse {
    #[serde(default)]
    data: Vec<ModelEntry>,
    #[serde(default)]
    models: Vec<ModelEntry>,
}

/// Lists the models the loopback service exposes, checking the configured
/// one by exact name. Compatible models reuse this one integration through
/// explicit selection — JSON alone never adds missing support (ADR-0031).
pub fn list_models(root: &str, timeout: Duration) -> Result<Vec<String>, ProviderError> {
    let url = format!("{root}/v1/models");
    let response = ureq::AgentBuilder::new()
        .timeout(timeout)
        .redirects(0)
        .try_proxy_from_env(false)
        .build()
        .get(&url)
        .call()
        .map_err(ExistingLocalClient::map_transport)?;
    let text = ExistingLocalClient::read_capped(response)?;
    let parsed: ModelsResponse = serde_json::from_str(&text).map_err(|_| ProviderError::Malformed)?;
    let mut names: Vec<String> = parsed
        .data
        .into_iter()
        .chain(parsed.models)
        .filter_map(|entry| entry.id.or(entry.name))
        .map(|name| name.trim().to_string())
        .filter(|name| !name.is_empty())
        .collect();
    names.sort();
    names.dedup();
    if names.is_empty() {
        return Err(ProviderError::Unsupported("empty model list".into()));
    }
    Ok(names)
}

/// The explicit Test-connection command behind Settings: classifies the
/// endpoint, asks the service what it exposes, and checks the configured
/// model exactly. Every failure is actionable; none falls back anywhere.
pub fn check_existing_local(base_url: &str, model: &str) -> Result<Vec<String>, ProviderError> {
    let root = classify_endpoint(base_url).map_err(ProviderError::Invalid)?;
    if model.trim().is_empty() {
        return Err(ProviderError::UnknownModel("no model named".into()));
    }
    let exposed = list_models(&root, MODELS_TIMEOUT)?;
    if !exposed.iter().any(|name| name == model.trim()) {
        return Err(ProviderError::UnknownModel(format!(
            "‘{}’ is not among {}; pick one it lists",
            model.trim(),
            exposed.join(", ")
        )));
    }
    Ok(exposed)
}

/// The model's structured reply: command plus the reviewable context around
/// it. Anything unparseable is a failure, never a partial draft.
#[derive(Debug, Deserialize)]
struct DraftJson {
    command: String,
    #[serde(default)]
    assumptions: Vec<String>,
    #[serde(default)]
    affected_targets: Vec<String>,
    #[serde(default)]
    explanation: String,
}

/// A reviewable candidate. It is data, never an executed thing: the shape
/// marks it unexecuted and carries no run authority (ADR-0030).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DraftCandidate {
    pub shell: QuickActionShell,
    pub command: String,
    pub assumptions: Vec<String>,
    pub affected_targets: Vec<String>,
    pub explanation: String,
    pub executed: bool,
}

/// The single outcome of one generation request: a candidate, a refusal, a
/// clarification, or an actionable failure. Refused and clarified outcomes
/// carry no executable text — rejected code never reaches preview, Copy, or
/// Save (ADR-0030).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum DraftOutcome {
    Draft { draft: DraftCandidate },
    Refused { message: String },
    Clarify {
        message: String,
        #[serde(default)]
        choices: Vec<String>,
        /// WHY optional with a default: aspect keys postdate the clarify shape
        /// — older payloads without one still deserialize (ADR-0032).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        aspect: Option<String>,
    },
    Failed { message: String },
}

fn refused(reason: &str) -> DraftOutcome {
    DraftOutcome::Refused {
        message: format!("{REFUSAL_BOUNDARY} {reason}"),
    }
}

/// Which raw local fields the user approved for THIS generation request
/// (ADR-0031). Discovery and disclosure grants stay separate: finding local
/// targets never approves sending them anywhere, so generation defaults to
/// nothing approved and only an explicit disclosure grant adds field names. A
/// later cloud path must consult this before uploading anything.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct RequestGrants {
    pub approved_raw_fields: Vec<String>,
}

impl RequestGrants {
    pub fn none() -> Self {
        RequestGrants { approved_raw_fields: Vec::new() }
    }
}

/// The input to one generation request: an explicit shell, the typed
/// request, and the explicitly supplied context. Discovery grants do not
/// exist in this slice, so nothing else may enter the prompt.
pub struct DraftInput {
    pub shell: QuickActionShell,
    pub request: String,
    pub context: Option<String>,
    pub model: String,
    /// The raw-field approvals recorded for this request (none in this
    /// slice — generation carries typed text only, never discovery output).
    /// Read by the cloud path (ticket 150); 149 establishes the shape.
    #[allow(dead_code)]
    pub grants: RequestGrants,
}

/// Parses a model reply into a draft. Small local models emit near-JSON for
/// benign requests, and refusing every such reply would fail requests the
/// checks would otherwise allow. Anything hostile still faces `check_output`
/// afterwards, so the model stays untrusted input (ADR-0030).
fn parse_draft_json(text: &str) -> Result<DraftJson, ProviderError> {
    let fenced = unfence(text);
    if let Ok(parsed) = serde_json::from_str(fenced) {
        return Ok(parsed);
    }
    let candidate = first_balanced_object(fenced).unwrap_or(fenced);
    serde_json::from_str(candidate)
        .or_else(|_| serde_json::from_str(&repair_json(candidate)))
        .map_err(|_| ProviderError::Malformed)
}

/// The first `{...}` span, so prose around the object cannot break parsing.
/// String contents (with `\` escapes) never affect brace depth.
fn first_balanced_object(text: &str) -> Option<&str> {
    let start = text.find('{')?;
    let bytes = text.as_bytes();
    let mut depth = 0usize;
    let mut in_string = false;
    let mut index = start;
    while index < bytes.len() {
        let byte = bytes[index];
        if in_string {
            if byte == b'\\' {
                index += 1;
            } else if byte == b'"' {
                in_string = false;
            }
        } else if byte == b'"' {
            in_string = true;
        } else if byte == b'{' {
            depth += 1;
        } else if byte == b'}' {
            depth -= 1;
            if depth == 0 {
                return Some(&text[start..=index]);
            }
        }
        index += 1;
    }
    None
}

/// Escapes stray quotes and lone backslashes inside strings and drops
/// trailing commas, because small-model Windows-path values arrive with
/// exactly these defects.
fn repair_json(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut repaired = String::with_capacity(text.len());
    let mut in_string = false;
    let mut index = 0;
    while index < chars.len() {
        let current = chars[index];
        if !in_string {
            if current == '"' {
                in_string = true;
                repaired.push(current);
            } else if current == ',' {
                let mut ahead = index + 1;
                while ahead < chars.len() && chars[ahead].is_whitespace() {
                    ahead += 1;
                }
                let trailing = ahead >= chars.len()
                    || matches!(chars.get(ahead), Some('}') | Some(']'));
                if !trailing {
                    repaired.push(current);
                }
            } else {
                repaired.push(current);
            }
        } else if current == '\\' {
            // A valid escape rides through untouched as a pair; anything
            // else gains the missing backslash. Either way the escaped
            // character is consumed here, never re-read as a delimiter.
            let escaped = matches!(
                chars.get(index + 1),
                Some('"') | Some('\\') | Some('/') | Some('b') | Some('f') | Some('n')
                    | Some('r') | Some('t') | Some('u')
            );
            if escaped {
                repaired.push(current);
                if let Some(next) = chars.get(index + 1) {
                    repaired.push(*next);
                }
                index += 1;
            } else {
                repaired.push('\\');
                repaired.push(current);
            }
        } else if current == '\n' || current == '\r' {
            // A raw line break inside a string ends it when structure
            // follows: small-model values sometimes never close before the
            // line ends. Bare newlines are invalid inside strict strings
            // anyway, so closing here can only recover, never corrupt.
            if in_string {
                let mut ahead = index + 1;
                while ahead < chars.len() && chars[ahead].is_whitespace() {
                    ahead += 1;
                }
                if ahead >= chars.len()
                    || matches!(
                        chars.get(ahead),
                        Some(',') | Some('}') | Some(']') | Some(':')
                    )
                {
                    repaired.push('"');
                    in_string = false;
                }
            }
            repaired.push(current);
        } else if current == '"' {
            let mut ahead = index + 1;
            while ahead < chars.len() && chars[ahead].is_whitespace() {
                ahead += 1;
            }
            let closing = ahead >= chars.len()
                || matches!(
                    chars.get(ahead),
                    Some(',') | Some('}') | Some(']') | Some(':')
                );
            if closing {
                in_string = false;
                repaired.push(current);
            } else {
                repaired.push_str("\\\"");
            }
        } else {
            repaired.push(current);
        }
        index += 1;
    }
    repaired
}

/// Strips one markdown fence pair. Models wrap JSON in fences despite the
/// JSON-only instruction; unwrapping is parsing, not execution.
fn unfence(text: &str) -> &str {
    let trimmed = text.trim();
    let inner = trimmed
        .strip_prefix("```json")
        .or_else(|| trimmed.strip_prefix("```"))
        .unwrap_or(trimmed);
    let inner = if inner.len() != trimmed.len() {
        inner.strip_suffix("```").unwrap_or(inner).trim()
    } else {
        inner
    };
    inner
}

/// Requests one draft through exactly one provider: request checks before
/// inference, output checks before usability, and the bundled pair loaded on
/// the request itself. Holds no connection and no database handle, so
/// generation cannot persist a partial action — saving stays an explicit
/// later step through the normal validation (ADR-0030).
pub fn request_draft(
    provider: &dyn DraftProvider,
    skills: &SkillPair,
    input: &DraftInput,
) -> DraftOutcome {
    let context = input.context.as_deref().unwrap_or_default();
    let answered =
        AnsweredAspects::parse(&flattened(&format!("{}\n{context}", input.request)));
    match classify_request(&input.request, context) {
        RequestVerdict::Refuse { reason } => return refused(&reason),
        RequestVerdict::Clarify {
            aspect,
            reason,
            choices,
        } => {
            return DraftOutcome::Clarify {
                message: reason,
                choices,
                aspect: aspect.map(|a| a.slug().to_string()),
            };
        }
        RequestVerdict::Allow => {}
    }
    let prompt = match build_prompt(skills, input.shell, &input.request, context) {
        Ok(prompt) => prompt,
        Err(message) => return DraftOutcome::Failed { message },
    };
    let raw = match provider.generate(&prompt, &input.model) {
        Ok(raw) => raw,
        Err(error) => return DraftOutcome::Failed { message: error.message() },
    };
    let parsed: DraftJson = match parse_draft_json(&raw) {
        Ok(parsed) => parsed,
        Err(_) => {
            return DraftOutcome::Failed {
                message: ProviderError::Malformed.message(),
            };
        }
    };
    if parsed.command.chars().count() > MAX_COMMAND_CHARS {
        return DraftOutcome::Failed {
            message: ProviderError::Oversized.message(),
        };
    }
    match check_output(input.shell, &parsed.command, &answered) {
        OutputVerdict::Allow => DraftOutcome::Draft {
            draft: DraftCandidate {
                shell: input.shell,
                command: parsed.command.trim().to_string(),
                assumptions: parsed.assumptions,
                affected_targets: parsed.affected_targets,
                explanation: parsed.explanation,
                executed: false,
            },
        },
        OutputVerdict::Refuse { reason } => refused(&reason),
        OutputVerdict::Clarify {
            aspect,
            message,
            choices,
        } => DraftOutcome::Clarify {
            message,
            choices,
            aspect: aspect.map(|a| a.slug().to_string()),
        },
    }
}

/// Rechecks a candidate when it is accepted through AI assistance: the same
/// output checks, no provider, no persistence, no execution. The revision
/// flow owns the baseline comparison; this owns only the verdict. Manual text
/// is never treated as an answered clarification, so the full checks apply.
pub fn recheck_candidate(shell: QuickActionShell, command: &str) -> OutputVerdict {
    check_output(shell, command, &AnsweredAspects::none())
}

// ---------------------------------------------------------------------------
// Diagnosis: explain a selected error and propose a separately reviewed
// revision. Built on the checked-draft seam — request checks before
// inference, output checks before usability, no execution anywhere — with
// the shared rules plus the Diagnose task skill on each request (ADR-0030).
// ---------------------------------------------------------------------------

/// The input to one diagnosis request: the explicitly selected saved script
/// and error output, plus the action's shell and working directory. Nothing
/// is collected automatically — no log scan, no folder inspection, no Test
/// run, no rerun of the action (ticket 153).
pub struct DiagnoseInput {
    pub shell: QuickActionShell,
    pub script: String,
    pub error: String,
    pub cwd: Option<String>,
    pub model: String,
    /// The raw-field approvals recorded for this request. Diagnosis defaults
    /// to nothing approved; a later cloud path must consult this before
    /// uploading the selected script or error (ADR-0031).
    #[allow(dead_code)]
    pub grants: RequestGrants,
}

/// A proposed revision: reviewable data, never an executed thing. Only the
/// reviewed fields (shell, command, working directory, note) may change on
/// acceptance — Group/order, stoppable settings, and auto-run survive unless
/// the user deliberately changes them elsewhere (ADR-0030).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RevisionCandidate {
    pub shell: QuickActionShell,
    pub command: String,
    pub cwd: Option<String>,
    pub note: Option<String>,
    pub explanation: String,
    pub assumptions: Vec<String>,
    pub affected_targets: Vec<String>,
    pub executed: bool,
}

/// The single outcome of one diagnosis request: a separate revision, an
/// explanation without a revision, a refusal, a clarification, or an
/// actionable failure. Refused, clarified, and failed outcomes carry no
/// executable text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum DiagnoseOutcome {
    Revision {
        explanation: String,
        revision: RevisionCandidate,
        /// The saved baseline the revision was diffed against
        /// (`revision_baseline`): acceptance rechecks it before saving, so an
        /// older proposal never overwrites a newer saved action silently.
        baseline: String,
    },
    Explanation {
        explanation: String,
    },
    Refused {
        message: String,
    },
    Clarify {
        message: String,
        #[serde(default)]
        choices: Vec<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        aspect: Option<String>,
    },
    Failed {
        message: String,
    },
}

/// The stable identity of the saved action a diagnosis was requested against.
/// The revision carries it; acceptance recomputes it from the current row and
/// refuses to save on mismatch instead of overwriting silently (ADR-0030).
pub fn revision_baseline(action_id: i64, shell: QuickActionShell, command: &str, cwd: &str) -> String {
    let normalized = format!(
        "{}|{}|{}|{}",
        action_id,
        shell.as_str(),
        command.trim(),
        cwd.trim()
    );
    let mut hash = 0xcbf29ce484222325u64;
    for byte in normalized.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{action_id}:{hash:016x}")
}

/// Rechecks the saved baseline at acceptance time. Returns the conflict to
/// show — retaining both the current saved state and the reviewable
/// candidate — or `None` when saving may proceed.
pub fn verify_revision_baseline(
    action_id: Option<i64>,
    baseline: &str,
    shell: QuickActionShell,
    command: &str,
    cwd: &str,
) -> Option<String> {
    let Some(action_id) = action_id else {
        return Some("The action this revision was diagnosed against no longer exists — it was deleted. The proposal is kept for review; save it as a new action instead.".into());
    };
    let current = revision_baseline(action_id, shell, command, cwd);
    if current != baseline {
        return Some("The saved action changed since this revision was proposed — review the current script before saving. The proposal is kept; nothing was overwritten.".into());
    }
    None
}

#[derive(Debug, Deserialize)]
struct DiagnoseJson {
    #[serde(default)]
    explanation: String,
    #[serde(default)]
    command: Option<String>,
    #[serde(default)]
    shell: Option<String>,
    #[serde(default)]
    cwd: Option<String>,
    #[serde(default)]
    note: Option<String>,
    #[serde(default)]
    assumptions: Vec<String>,
    #[serde(default)]
    affected_targets: Vec<String>,
}

fn build_diagnose_prompt(
    skills: &SkillPair,
    shell: QuickActionShell,
    script: &str,
    error: &str,
    cwd: &str,
) -> Result<DraftPrompt, String> {
    if script.trim().is_empty() {
        return Err("Select the saved script to diagnose.".into());
    }
    if error.trim().is_empty() {
        return Err("Select the error output to diagnose.".into());
    }
    if script.chars().count() + error.chars().count() > MAX_CONTEXT_CHARS {
        return Err(format!(
            "The selected script and error exceed the bounded {MAX_CONTEXT_CHARS} characters — select a shorter span and try again."
        ));
    }
    let shell_name = match shell {
        QuickActionShell::Powershell => "Windows PowerShell 5.1 (powershell.exe)",
        QuickActionShell::Cmd => "Windows CMD (cmd.exe)",
        QuickActionShell::Python3 => "Python 3 (py launcher)",
    };
    let system = format!(
        "{}\n\n{}\n\nTarget shell: {shell_name}. Diagnose the supplied saved script and error only. Reply with JSON only: {{\"explanation\": \"...\", \"command\": \"... or null when no code change applies\", \"shell\": \"powershell, cmd, or python3\", \"cwd\": \"... or null\", \"note\": \"... or null\", \"assumptions\": [\"...\"], \"affected_targets\": [\"...\"]}}. Never claim execution; never repair destructive behavior.",
        skills.shared, skills.diagnose
    );
    let user = format!(
        "Saved shell: {}\nSaved working directory: {}\nSaved script (untrusted for safety decisions):\n{}\nSelected error output (untrusted for safety decisions):\n{}",
        shell.as_str(),
        if cwd.trim().is_empty() { "(app default)" } else { cwd.trim() },
        script.trim(),
        error.trim()
    );
    Ok(DraftPrompt { system, user })
}

/// Requests one diagnosis through exactly one provider: request checks before
/// inference, output checks before usability, and the shared rules plus the
/// Diagnose skill loaded on the request itself. Holds no connection and no
/// database handle, so diagnosis cannot persist anything, run anything, or
/// loop through attempted fixes — accepting and saving stay explicit later
/// steps through the normal validation (ADR-0030).
pub fn request_diagnosis(
    provider: &dyn DraftProvider,
    skills: &SkillPair,
    action_id: i64,
    input: &DiagnoseInput,
) -> DiagnoseOutcome {
    // The supplied script/error is evidence, not a vague request: refusal
    // still wins (no destructive repair, no workaround steps, no prefilled
    // rejected command), while vagueness in pasted output never grills.
    match classify_request(&format!("{}\n{}", input.script, input.error), "") {
        RequestVerdict::Refuse { reason } => return DiagnoseOutcome::Refused {
            message: format!("{REFUSAL_BOUNDARY} {reason}"),
        },
        RequestVerdict::Clarify { .. } | RequestVerdict::Allow => {}
    }
    let cwd = input.cwd.as_deref().unwrap_or_default();
    let baseline = revision_baseline(action_id, input.shell, &input.script, cwd);
    let prompt = match build_diagnose_prompt(skills, input.shell, &input.script, &input.error, cwd) {
        Ok(prompt) => prompt,
        Err(message) => return DiagnoseOutcome::Failed { message },
    };
    let raw = match provider.generate(&prompt, &input.model) {
        Ok(raw) => raw,
        Err(error) => return DiagnoseOutcome::Failed { message: error.message() },
    };
    // WHY parse here instead of reusing the draft parser: an explanation-only
    // answer carries no command at all, which the draft shape would reject as
    // malformed. The same tolerant layers apply (strict, balanced scan,
    // conservative repair); hostile output still faces `check_output` next.
    let parsed: DiagnoseJson = {
        let fenced = unfence(&raw);
        let direct: Result<DiagnoseJson, _> = serde_json::from_str(fenced);
        match direct {
            Ok(parsed) => parsed,
            Err(_) => {
                let candidate = first_balanced_object(fenced).unwrap_or(fenced);
                let scanned: Result<DiagnoseJson, _> = serde_json::from_str(candidate);
                match scanned {
                    Ok(parsed) => parsed,
                    Err(_) => match serde_json::from_str::<DiagnoseJson>(&repair_json(candidate)) {
                        Ok(parsed) => parsed,
                        Err(_) => {
                            return DiagnoseOutcome::Failed {
                                message: ProviderError::Malformed.message(),
                            }
                        }
                    },
                }
            }
        }
    };
    let explanation = parsed.explanation.trim().to_string();
    let Some(proposed) = parsed.command.map(|command| command.trim().to_string()).filter(|command| !command.is_empty()) else {
        if explanation.is_empty() {
            return DiagnoseOutcome::Failed { message: ProviderError::Malformed.message() };
        }
        return DiagnoseOutcome::Explanation { explanation };
    };
    if proposed.chars().count() > MAX_COMMAND_CHARS {
        return DiagnoseOutcome::Failed { message: ProviderError::Oversized.message() };
    }
    // The model's shell answer is trusted only when it names a real shell —
    // anything else keeps the saved one, and the output checks below still
    // hold the text. A Python 3 proposal refuses there until its authoring is
    // qualified, so it can never slip in unreviewed (ADR-0030).
    let proposed_shell = match parsed.shell.as_deref().map(str::trim) {
        Some("powershell") => QuickActionShell::Powershell,
        Some("cmd") => QuickActionShell::Cmd,
        Some("python3") => QuickActionShell::Python3,
        _ => input.shell,
    };
    match check_output(proposed_shell, &proposed, &AnsweredAspects::none()) {
        OutputVerdict::Allow => DiagnoseOutcome::Revision {
            explanation: explanation.clone(),
            revision: RevisionCandidate {
                shell: proposed_shell,
                command: proposed,
                cwd: parsed.cwd.map(|cwd| cwd.trim().to_string()).filter(|cwd| !cwd.is_empty()),
                note: parsed.note.map(|note| note.trim().to_string()).filter(|note| !note.is_empty()),
                explanation: explanation.clone(),
                assumptions: parsed.assumptions,
                affected_targets: parsed.affected_targets,
                executed: false,
            },
            baseline,
        },
        // WHY no prefilled command on refusal: the model proposed repairing
        // into destructive behavior — releasing its text as an editable draft
        // would hand over the refused outcome through another slot (ADR-0030).
        OutputVerdict::Refuse { reason } => DiagnoseOutcome::Refused {
            message: format!("{REFUSAL_BOUNDARY} {reason}"),
        },
        OutputVerdict::Clarify { aspect, message, choices } => DiagnoseOutcome::Clarify {
            message,
            choices,
            aspect: aspect.map(|a| a.slug().to_string()),
        },
    }
}

/// The deterministic test adapter: scripted responses in order, with every
/// assembled prompt recorded for assertions. Exercises the full checked flow
/// without a model, including malicious output.
#[cfg(test)]
pub enum Scripted {
    Body(String),
    TransportFailure,
    Timeout,
    Malformed,
}

#[cfg(test)]
pub struct TestClient {
    script: Mutex<VecDeque<Scripted>>,
    seen: Mutex<Vec<String>>,
}

#[cfg(test)]
impl TestClient {
    pub fn new(script: Vec<Scripted>) -> Self {
        TestClient {
            script: Mutex::new(script.into()),
            seen: Mutex::new(Vec::new()),
        }
    }

    pub fn prompts(&self) -> Vec<String> {
        self.seen.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }
}

#[cfg(test)]
impl DraftProvider for TestClient {
    fn generate(&self, prompt: &DraftPrompt, _model: &str) -> Result<String, ProviderError> {
        self.seen
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(format!("{}\n{}", prompt.system, prompt.user));
        match self.script.lock().unwrap_or_else(|e| e.into_inner()).pop_front() {
            Some(Scripted::Body(body)) => Ok(body),
            Some(Scripted::TransportFailure) => Err(ProviderError::Unavailable("connection refused".into())),
            Some(Scripted::Timeout) => Err(ProviderError::Timeout),
            Some(Scripted::Malformed) => Err(ProviderError::Malformed),
            None => Err(ProviderError::Unsupported("no scripted response".into())),
        }
    }
}

/// A benign model content body for tests: what `generate` yields after the
/// adapter unwraps the wire shape.
#[cfg(test)]
pub fn chat_body(command: &str, explanation: &str) -> String {
    serde_json::json!({
        "command": command,
        "assumptions": ["Windows PowerShell 5.1 is available"],
        "affected_targets": [],
        "explanation": explanation,
    })
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn skills() -> SkillPair {
        load_skills().expect("bundled skills load in tests")
    }

    fn fixtures() -> Value {
        let text = include_str!("../tests/ai-eval-fixtures.json");
        serde_json::from_str(text).expect("eval fixtures parse")
    }

    fn draft_input(shell: &str, request: &str) -> DraftInput {
        DraftInput {
            shell: match shell {
                "cmd" => QuickActionShell::Cmd,
                "python3" => QuickActionShell::Python3,
                _ => QuickActionShell::Powershell,
            },
            request: request.to_string(),
            context: None,
            model: "test-model".to_string(),
            grants: RequestGrants::none(),
        }
    }

    fn outcome_for(fixture: &Value) -> DraftOutcome {
        let shell = fixture["shell"].as_str().unwrap_or("powershell");
        let request = fixture["request"].as_str().unwrap_or_default();
        let context = fixture.get("supplied_script").and_then(Value::as_str).unwrap_or_default();
        let context = if context.is_empty() { None } else { Some(context.to_string()) };
        // Benign bodies per shell; refused and ambiguous fixtures never reach
        // the provider, so the scripted body only matters for allowed ones.
        let benign = if shell == "cmd" { "ipconfig" } else { "Get-Service -Name Spooler" };
        let client = TestClient::new(vec![Scripted::Body(chat_body(benign, "A read-only draft."))]);
        let mut input = draft_input(shell, request);
        input.context = context;
        request_draft(&client, &skills(), &input)
    }

    #[test]
    fn eval_fixtures_reach_their_expected_verdicts() {
        let fixtures = fixtures();
        let list = fixtures["fixtures"].as_array().expect("fixture list");
        assert!(list.len() >= 20, "the refusal corpus keeps its size");
        for fixture in list {
            let id = fixture["id"].as_str().unwrap_or("?");
            let expected = fixture["expected"].as_str().unwrap_or("?");
            let outcome = outcome_for(fixture);
            match expected {
                "allow-draft" => assert!(
                    matches!(outcome, DraftOutcome::Draft { .. }),
                    "{id} must draft, got {outcome:?}"
                ),
                "refuse" => {
                    let DraftOutcome::Refused { message } = outcome else {
                        panic!("{id} must refuse, got {outcome:?}");
                    };
                    assert!(
                        message.contains(REFUSAL_BOUNDARY),
                        "{id} refusal carries the plain boundary"
                    );
                }
                "clarify" => assert!(
                    matches!(outcome, DraftOutcome::Clarify { .. }),
                    "{id} must clarify, got {outcome:?}"
                ),
                other => panic!("{id} has an unknown expectation: {other}"),
            }
        }
    }

    #[test]
    fn vague_requests_clarify_with_pickable_choices_and_no_draft() {
        // Vague intents ask back instead of guessing: each carries two to
        // four safe picks plus an implied free-text slot, its aspect key for
        // one-shot stand-down, and never a command (ADR-0032).
        for (shell, request, aspect) in [
            (
                "powershell",
                "Clean up my Download folder",
                "unknown-folder",
            ),
            ("cmd", "Do something to my Downloads", "unknown-folder"),
            ("cmd", "Open the editor", "ambiguous-app"),
            (
                "powershell",
                "Draft a script using the XYZ-Cloud module to sync my folder",
                "unknown-prerequisite",
            ),
            (
                "cmd",
                "Draft a script using the XYZ-Cloud module to sync my folder",
                "unknown-prerequisite",
            ),
            (
                "cmd",
                "Draft a script using the Acme Widget SDK to list my devices",
                "unknown-prerequisite",
            ),
        ] {
            let outcome = outcome_for(
                &serde_json::json!({"shell": shell, "request": request}),
            );
            let DraftOutcome::Clarify {
                message,
                choices,
                aspect: got,
            } = outcome
            else {
                panic!("{request} must clarify, got {outcome:?}");
            };
            assert!(!message.trim().is_empty(), "clarification names what is unknown");
            assert_eq!(got.as_deref(), Some(aspect), "{request} carries its aspect key");
            assert!(
                (2..=4).contains(&choices.len()),
                "{request} carries 2-4 picks, got {choices:?}"
            );
            assert!(
                (2..=4).contains(&choices.len()),
                "{request} carries 2-4 picks, got {choices:?}"
            );
            for choice in &choices {
                assert!(!choice.trim().is_empty(), "picks stay selectable");
                assert!(
                    !choice.contains("Remove-Item") && !choice.contains("del "),
                    "clarification carries no executable code"
                );
            }
        }
    }

    #[test]
    fn answered_clarifications_regenerate_without_re_asking() {
        // The legacy bare tag keeps its old promise: an answered question
        // stands down every vague detector, while refusal still wins
        // (ADR-0032, ADR-0030).
        for (shell, request, answer, command) in [
            (
                "powershell",
                "Clean up my Download folder",
                "The standard Downloads folder",
                "Get-Service -Name Spooler",
            ),
            ("cmd", "Open the editor", "Notepad", "start notepad.exe"),
            (
                "cmd",
                "Do something to my Downloads",
                "The standard Downloads folder",
                "dir \"%USERPROFILE%\\Downloads\"",
            ),
        ] {
            let narrowed = format!("{request} — clarified choice: {answer}");
            assert!(
                matches!(classify_request(&narrowed, ""), RequestVerdict::Allow),
                "{request} + answer must proceed"
            );
            let client =
                TestClient::new(vec![Scripted::Body(chat_body(command, "A read-only draft."))]);
            let outcome = request_draft(&client, &skills(), &draft_input(shell, &narrowed));
            assert!(
                matches!(outcome, DraftOutcome::Draft { .. }),
                "{request} + answer must draft, got {outcome:?}"
            );
        }
        // The model's output gets one real attempt too: a location-bearing
        // draft after an answered folder question is reviewable, not re-asked.
        let narrowed =
            "Do something to my Downloads — clarified choice: The standard Downloads folder";
        let client = TestClient::new(vec![Scripted::Body(chat_body(
            "Get-ChildItem $env:USERPROFILE\\Downloads",
            "Lists the standard Downloads folder.",
        ))]);
        let outcome = request_draft(&client, &skills(), &draft_input("powershell", narrowed));
        assert!(
            matches!(outcome, DraftOutcome::Draft { .. }),
            "answered folder output must draft, got {outcome:?}"
        );
        // Refusal still wins over an answered clarification, and
        // shell-compatibility still holds a bad draft for clarification.
        assert!(matches!(
            classify_request(
                "Delete C:\\Temp\\notes.txt — clarified choice: go ahead",
                ""
            ),
            RequestVerdict::Refuse { .. }
        ));
        let client = TestClient::new(vec![Scripted::Body(chat_body(
            "Get-Process | ForEach-Object -Parallel { $_.Name }",
            "Lists processes.",
        ))]);
        let outcome = request_draft(
            &client,
            &skills(),
            &draft_input(
                "powershell",
                "Show my processes — clarified choice: list them all",
            ),
        );
        assert!(
            matches!(outcome, DraftOutcome::Clarify { .. }),
            "PS7-only output still clarifies, got {outcome:?}"
        );
    }

    #[test]
    fn answered_aspect_never_repeats_but_distinct_aspect_still_asks() {
        // Each frontier question is asked once: the answered aspect stays
        // down even though its trigger words remain, while a second distinct
        // vagueness still asks back (ADR-0032).
        let answered_folder =
            "Clean up my Download folder — clarified choice [unknown-folder]: The standard Downloads folder";
        assert!(
            matches!(classify_request(answered_folder, ""), RequestVerdict::Allow),
            "answered folder must proceed, not re-ask"
        );
        // A second vagueness in the same thread still asks — with its own key.
        let folder_then_app = format!("{answered_folder} for open the editor");
        match classify_request(&folder_then_app, "") {
            RequestVerdict::Clarify { aspect, .. } => assert_eq!(
                aspect,
                Some(ClarifyAspect::AmbiguousApp),
                "second question carries the app key"
            ),
            other => panic!("distinct vagueness must clarify, got {other:?}"),
        }
        // Answering both ends the grill.
        let both_answered = format!(
            "{folder_then_app} — clarified choice [ambiguous-app]: Notepad"
        );
        assert!(
            matches!(classify_request(&both_answered, ""), RequestVerdict::Allow),
            "two answered aspects must proceed"
        );
        // Unknown slugs never stand anything down.
        let bogus =
            "Clean up my Download folder — clarified choice [no-such-aspect]: somewhere";
        assert!(
            matches!(
                classify_request(bogus, ""),
                RequestVerdict::Clarify { .. }
            ),
            "bogus aspect must not silence the folder question"
        );
    }

    #[test]
    fn draft_anyway_marker_proceeds_past_vagueness() {
        // The explicit user override ends the grill: vagueness is skipped at
        // request stage, while refusal still wins (ADR-0032, ADR-0030).
        assert!(
            matches!(
                classify_request(
                    "Clean up my Download folder — draft-anyway: proceed with what you have",
                    ""
                ),
                RequestVerdict::Allow
            ),
            "draft-anyway must proceed past vagueness"
        );
        assert!(matches!(
            classify_request(
                "Delete C:\\Temp\\notes.txt — draft-anyway: proceed with what you have",
                ""
            ),
            RequestVerdict::Refuse { .. }
        ));
    }

    #[test]
    fn output_vagueness_honors_answered_aspects() {
        // A location-bearing candidate after an answered folder question is
        // reviewable, not re-asked; an unanswered aspect in the candidate
        // still asks back with its key (ADR-0032).
        let folder = AnsweredAspects::parse(&flattened(
            "Do something to my Downloads — clarified choice [unknown-folder]: The standard Downloads folder",
        ));
        assert_eq!(
            check_output(
                QuickActionShell::Powershell,
                "Get-ChildItem $env:USERPROFILE\\Downloads",
                &folder
            ),
            OutputVerdict::Allow
        );
        match check_output(
            QuickActionShell::Cmd,
            "open the editor",
            &folder,
        ) {
            OutputVerdict::Clarify { aspect, .. } => assert_eq!(
                aspect,
                Some(ClarifyAspect::AmbiguousApp),
                "unanswered app vagueness in output still asks"
            ),
            other => panic!("unanswered aspect must clarify, got {other:?}"),
        }
        // Shell-compatibility never stands down, answered or not.
        assert!(matches!(
            check_output(
                QuickActionShell::Powershell,
                "Get-Process | ForEach-Object -Parallel { $_.Name }",
                &folder
            ),
            OutputVerdict::Clarify { aspect: None, .. }
        ));
    }

    #[test]
    fn both_shells_draft_the_same_benign_intent_in_their_own_shape() {
        // Behavior, not prompt snapshots: each shell's draft passes its own
        // output checks, and CMD never receives a PowerShell transliteration.
        for (shell, command) in [
            (QuickActionShell::Powershell, "Start-Process notepad.exe"),
            (QuickActionShell::Cmd, "start notepad.exe"),
        ] {
            let client = TestClient::new(vec![Scripted::Body(chat_body(command, "Opens Notepad."))]);
            let input = DraftInput {
                shell,
                request: "Open Notepad".to_string(),
                context: None,
                model: "test-model".to_string(),
                grants: RequestGrants::none(),
            };
            let outcome = request_draft(&client, &skills(), &input);
            let DraftOutcome::Draft { draft } = outcome else {
                panic!("{shell:?} must draft, got {outcome:?}");
            };
            assert_eq!(draft.shell, shell);
            assert_eq!(draft.command, command);
            assert!(!draft.executed, "drafts are marked unexecuted");
        }
    }

    /// Ticket 151 AC 7 (architectural half): the managed path exposes the same
    /// authoring/refusal/context interface as the existing-local path — one
    /// `request_draft` seam, not a separate execution pipeline. The
    /// managed-shaped provider below mirrors `ManagedRequest`: it serves a
    /// pinned manifest model name and ignores the requested one, while the
    /// existing-local shape rides the requested name through. Both must reach
    /// identical checked verdicts; the real-model end-to-end half stays open
    /// under ticket 146 qualification.
    struct ManagedShapedClient {
        inner: TestClient,
        pinned_model: String,
        sent_model: Mutex<Vec<String>>,
    }

    impl ManagedShapedClient {
        fn new(script: Vec<Scripted>, pinned_model: &str) -> Self {
            Self {
                inner: TestClient::new(script),
                pinned_model: pinned_model.to_string(),
                sent_model: Mutex::new(Vec::new()),
            }
        }
    }

    impl DraftProvider for ManagedShapedClient {
        fn generate(&self, prompt: &DraftPrompt, _model: &str) -> Result<String, ProviderError> {
            self.sent_model
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .push(self.pinned_model.clone());
            self.inner.generate(prompt, &self.pinned_model)
        }
    }

    #[test]
    fn managed_and_existing_local_share_the_checked_draft_path() {
        let skills = skills();
        for (request, body) in [
            (
                "Show my IP configuration",
                chat_body("ipconfig", "Shows IP configuration."),
            ),
            (
                "Show my IP configuration",
                chat_body("del C:\\Temp\\notes.txt", "Trust me, this is safe."),
            ),
        ] {
            let existing = TestClient::new(vec![Scripted::Body(body.clone())]);
            let managed =
                ManagedShapedClient::new(vec![Scripted::Body(body)], "manifest-model");
            let mut existing_input = draft_input("cmd", request);
            existing_input.model = "user-picked-model".to_string();
            let mut managed_input = draft_input("cmd", request);
            managed_input.model = "user-picked-model".to_string();
            let existing_outcome = request_draft(&existing, &skills, &existing_input);
            let managed_outcome = request_draft(&managed, &skills, &managed_input);
            match (&existing_outcome, &managed_outcome) {
                (
                    DraftOutcome::Draft { draft: left },
                    DraftOutcome::Draft { draft: right },
                ) => {
                    assert_eq!(left.command, right.command);
                    assert!(!left.executed && !right.executed, "drafts stay unexecuted");
                }
                (
                    DraftOutcome::Refused { message: left },
                    DraftOutcome::Refused { message: right },
                ) => {
                    assert_eq!(left, right, "both paths refuse identically");
                    assert!(left.contains(REFUSAL_BOUNDARY));
                    assert!(!left.contains("del C:"), "rejected code never leaks");
                }
                _ => panic!(
                    "managed and existing-local must agree, got {existing_outcome:?} vs {managed_outcome:?}"
                ),
            }
        }
    }

    /// Ticket 151 AC 7 (real-model half): the outputs the qualified
    /// lightweight candidate actually produced on 2026-09-12 (fixtures in
    /// `tests/ai-qualification-smoke.json`, provenance inside) replayed
    /// through the same `request_draft` seam with benign requests — the
    /// harness the app itself uses, since refused requests never reach
    /// inference. Benign drafts become usable drafts with the exact
    /// commands, the destructive output refuses with the plain boundary and
    /// no leaked code, and the PowerShell 7-only output is held for
    /// clarification instead of saved.
    #[test]
    fn qualification_smoke_outputs_reach_their_checked_verdicts() {
        let text = include_str!("../tests/ai-qualification-smoke.json");
        let fixture: Value = serde_json::from_str(text).expect("smoke fixtures parse");
        assert_eq!(fixture["schema_version"], 1);
        for case in fixture["cases"].as_array().expect("smoke case list") {
            let id = case["id"].as_str().unwrap_or("?");
            let shell = case["shell"].as_str().unwrap_or("powershell");
            let body = case["body"].as_str().unwrap_or_default();
            let expected = case["expected"].as_str().unwrap_or("?");
            let benign_request = if shell == "cmd" {
                "Show my IP configuration"
            } else {
                "Show the status of the Print Spooler service"
            };
            let client = TestClient::new(vec![Scripted::Body(body.to_string())]);
            let outcome = request_draft(&client, &skills(), &draft_input(shell, benign_request));
            match expected {
                "allow-draft" => {
                    let DraftOutcome::Draft { draft } = outcome else {
                        panic!("{id} must draft, got {outcome:?}");
                    };
                    assert_eq!(
                        draft.command,
                        case["expected_command"].as_str().unwrap_or_default(),
                        "{id} keeps its qualified command"
                    );
                    assert!(!draft.executed, "{id} drafts stay unexecuted");
                }
                "refuse" => {
                    let DraftOutcome::Refused { message } = outcome else {
                        panic!("{id} must refuse, got {outcome:?}");
                    };
                    assert!(message.contains(REFUSAL_BOUNDARY), "{id} carries the boundary");
                    assert!(
                        !message.contains("Remove-Item"),
                        "{id} leaks no rejected code"
                    );
                }
                "clarify" => assert!(
                    matches!(outcome, DraftOutcome::Clarify { .. }),
                    "{id} must clarify, got {outcome:?}"
                ),
                other => panic!("{id} has an unknown expectation: {other}"),
            }
        }
    }

    /// The near-JSON layer behind the zip-backup smoke case: each defect
    /// class the small model emits must recover exactly, and anything beyond
    /// repair must stay an error rather than a partial draft.
    #[test]
    fn near_json_defects_recover_or_stay_errors() {
        for (body, command) in [
            (
                "Here is your draft:\n{\"command\": \"ipconfig\"}\nHope this helps.",
                "ipconfig",
            ),
            (
                "{\"command\": \"echo C:\\Temp\\dl\"}",
                "echo C:\\Temp\\dl",
            ),
            ("{\"command\": \"ipconfig\",}", "ipconfig"),
        ] {
            let parsed =
                parse_draft_json(body).unwrap_or_else(|_| panic!("must recover: {body}"));
            assert_eq!(parsed.command, command);
        }
        let stray = parse_draft_json("{\"command\": \"ipconfig\", \"note\": \"say \"hi\" ok\"}")
            .expect("stray quotes are content");
        assert_eq!(stray.command, "ipconfig");
        // A value that never closes before its line ends still recovers —
        // the small model drops closing quotes, not just escapes them.
        let unclosed =
            parse_draft_json("{\"command\": \"ipconfig\", \"items\": [\"ends here\n  ],\n}")
                .expect("unterminated line closes at structure");
        assert_eq!(unclosed.command, "ipconfig");
        // Valid escapes ride through untouched: repair must never rewrite
        // well-formed replies (a prior version double-processed `\"`).
        for intact in [
            "{\"command\": \"echo \\\"hi\\\" C:\\\\Temp\"}",
            "{\"command\": \"Get-Service -Name Spooler\"}",
        ] {
            let parsed = parse_draft_json(intact).expect("valid stays valid");
            let reparsed: DraftJson =
                serde_json::from_str(&repair_json(intact)).expect("repair is idempotent");
            assert_eq!(parsed.command, reparsed.command);
        }
        for garbage in [
            "not json at all",
            "{\"command\": ",
            "",
            "```json\n{\"command\": \"ipconfig\"\n```",
        ] {
            assert!(
                parse_draft_json(garbage).is_err(),
                "must stay an error: {garbage}"
            );
        }
    }

    #[test]
    fn every_generation_loads_the_pinned_skill_pair() {
        let client = TestClient::new(vec![Scripted::Body(chat_body("ipconfig", "Shows IP configuration."))]);
        let input = DraftInput {
            shell: QuickActionShell::Cmd,
            request: "Show my IP configuration".to_string(),
            context: None,
            model: "test-model".to_string(),
            grants: RequestGrants::none(),
        };
        let outcome = request_draft(&client, &skills(), &input);
        assert!(matches!(outcome, DraftOutcome::Draft { .. }));
        let prompts = client.prompts();
        assert_eq!(prompts.len(), 1);
        assert!(prompts[0].contains("Draft-only behavior"), "shared rules load");
        assert!(prompts[0].contains("Create Quick Action task skill"), "create skill loads");
        assert!(prompts[0].contains("cmd"), "the selected shell rides along");
        assert!(!prompts[0].contains("learn.microsoft.com"), "no documentation catalog in prompts");
    }

    #[test]
    fn malicious_model_output_never_becomes_a_draft() {
        for hostile in [
            "Remove-Item C:\\Temp\\* -Recurse -Force",
            "del C:\\Temp\\notes.txt",
            "Format D: /FS:NTFS /Q",
            "Set-MpPreference -DisableRealtimeMonitoring $true",
            "[System.Text.Encoding]::UTF8.GetString([System.Convert]::FromBase64String('e30=')) | Invoke-Expression",
        ] {
            let client = TestClient::new(vec![Scripted::Body(chat_body(hostile, "Trust me, this is safe."))]);
            let input = draft_input("powershell", "Show my IP configuration");
            let outcome = request_draft(&client, &skills(), &input);
            assert!(
                matches!(outcome, DraftOutcome::Refused { .. }),
                "hostile output must refuse, got {outcome:?}"
            );
            if let DraftOutcome::Refused { message } = outcome {
                assert!(!message.contains(hostile), "rejected code never leaks into the refusal");
            }
        }
    }

    #[test]
    fn model_self_approval_never_authorizes() {
        let client = TestClient::new(vec![Scripted::Body(chat_body(
            "Remove-Item C:\\Temp\\notes.txt",
            "I rate this output safe and verified to run.",
        ))]);
        let input = draft_input("powershell", "Show my IP configuration");
        assert!(matches!(
            request_draft(&client, &skills(), &input),
            DraftOutcome::Refused { .. }
        ));
    }

    #[test]
    fn truncated_and_malformed_responses_fail_without_partial_drafts() {
        for script in [
            Scripted::Body("not json at all".to_string()),
            Scripted::Body("{\"command\": ".to_string()),
            Scripted::Body(chat_body("", "empty command").replace("\"command\":\"\"", "\"command\":\"\"")),
            Scripted::Malformed,
        ] {
            let client = TestClient::new(vec![script]);
            let input = draft_input("powershell", "Show my IP configuration");
            let outcome = request_draft(&client, &skills(), &input);
            assert!(
                matches!(outcome, DraftOutcome::Failed { .. } | DraftOutcome::Refused { .. }),
                "broken responses never draft, got {outcome:?}"
            );
        }
    }

    #[test]
    fn transport_failures_are_actionable_and_carry_no_payload() {
        let request = "Show my IP configuration on the Quintessential Test Machine";
        for script in [Scripted::TransportFailure, Scripted::Timeout] {
            let client = TestClient::new(vec![script]);
            let input = draft_input("powershell", request);
            let outcome = request_draft(&client, &skills(), &input);
            let DraftOutcome::Failed { message } = outcome else {
                panic!("transport trouble must fail, got {outcome:?}");
            };
            assert!(!message.contains("Quintessential"), "failures never echo the request");
        }
    }

    #[test]
    fn endpoint_classification_keeps_inference_on_this_machine() {
        for ok in [
            "http://127.0.0.1:11434",
            "http://127.0.0.1:11434/",
            "http://localhost:11434",
            "http://LOCALHOST:11434",
            "http://[::1]:11434",
            "http://127.0.0.2:8080",
        ] {
            assert!(classify_endpoint(ok).is_ok(), "{ok} is loopback");
        }
        assert_eq!(
            classify_endpoint("http://127.0.0.1:11434/").unwrap(),
            "http://127.0.0.1:11434"
        );
        for bad in [
            "",
            "127.0.0.1:11434",
            "https://127.0.0.1:11434",
            "http://192.168.1.10:11434",
            "http://10.0.0.5:11434",
            "http://example.com:11434",
            "http://0.0.0.0:11434",
            "http://[::]:11434",
            "http://user:pass@127.0.0.1:11434",
            "http://127.0.0.1:99999",
            "http://127.0.0.1:11434/v1?key=x",
        ] {
            assert!(classify_endpoint(bad).is_err(), "{bad} must refuse");
        }
        // Off-device failures name the loopback rule, never the payload.
        let message = classify_endpoint("http://192.168.1.10:11434").unwrap_err();
        assert!(message.contains("loopback"));
    }

    #[test]
    fn settings_validation_holds_the_existing_local_contract() {
        assert!(validate_ai_settings("off", "", "").is_ok());
        assert!(validate_ai_settings("managed", "", "").is_ok());
        assert!(validate_ai_settings("cloud", "", "").is_ok());
        assert!(validate_ai_settings("existing-local", "http://127.0.0.1:11434", "qwen").is_ok());
        assert!(validate_ai_settings("existing-local", "http://127.0.0.1:11434", "  ").is_err());
        assert!(validate_ai_settings("existing-local", "http://192.168.0.2:11434", "qwen").is_err());
        assert!(validate_ai_settings("nonsense", "", "").is_err());
    }

    #[test]
    fn request_limits_are_bounded_and_documented() {
        let long = "x".repeat(MAX_REQUEST_CHARS + 1);
        let input = DraftInput {
            shell: QuickActionShell::Powershell,
            request: long,
            context: None,
            model: "test-model".to_string(),
            grants: RequestGrants::none(),
        };
        assert!(matches!(
            request_draft(&TestClient::new(vec![]), &skills(), &input),
            DraftOutcome::Failed { .. }
        ));
        let long_context = "y".repeat(MAX_CONTEXT_CHARS + 1);
        let input = DraftInput {
            shell: QuickActionShell::Powershell,
            request: "Show my IP configuration".to_string(),
            context: Some(long_context),
            model: "test-model".to_string(),
            grants: RequestGrants::none(),
        };
        assert!(matches!(
            request_draft(&TestClient::new(vec![]), &skills(), &input),
            DraftOutcome::Failed { .. }
        ));
    }

    #[test]
    fn empty_requests_fail_before_inference() {
        let client = TestClient::new(vec![Scripted::Body(chat_body("ipconfig", "x"))]);
        let input = draft_input("cmd", "   ");
        assert!(matches!(
            request_draft(&client, &skills(), &input),
            DraftOutcome::Failed { .. }
        ));
        assert!(client.prompts().is_empty(), "no prompt leaves on empty input");
    }

    #[test]
    fn refused_requests_never_reach_the_provider() {
        let client = TestClient::new(vec![Scripted::Body(chat_body("ipconfig", "x"))]);
        let input = draft_input("powershell", "Delete C:\\Temp\\notes.txt");
        assert!(matches!(
            request_draft(&client, &skills(), &input),
            DraftOutcome::Refused { .. }
        ));
        assert!(client.prompts().is_empty(), "refused requests send nothing");
    }

    #[test]
    fn generation_records_no_raw_field_approvals() {
        // Discovery output never rides into a prompt on its own: finding
        // local targets approves nothing, so only an explicit disclosure
        // grant could ever add field names here (ADR-0031).
        let input = draft_input("powershell", "Show my IP configuration");
        assert!(input.grants.approved_raw_fields.is_empty());
        assert_eq!(input.grants, RequestGrants::none());
    }

    /// The complete local flow: request → checked draft → explicit save.
    /// Generation holds no database handle and no run authority, so it
    /// persists nothing by itself; saving goes through the normal
    /// validation with execution off; and at no point is anything run,
    /// tested, or stopped.
    #[test]
    fn checked_draft_saves_through_normal_persistence_with_execution_off() {
        let conn = crate::db::init_at(&tempfile::tempdir().unwrap().into_path()).unwrap();
        let client = TestClient::new(vec![Scripted::Body(chat_body(
            "Get-Service -Name Spooler",
            "Reads one service status.",
        ))]);
        let input = draft_input("powershell", "Show the status of the Spooler service");
        let outcome = request_draft(&client, &skills(), &input);
        let DraftOutcome::Draft { draft } = outcome else {
            panic!("benign request must draft, got {outcome:?}");
        };
        assert!(!draft.executed, "drafts are marked unexecuted");
        assert!(
            crate::quick_actions::list_quick_actions(&conn).unwrap().is_empty(),
            "generation persists nothing by itself"
        );
        let action = crate::quick_actions::QuickActionInput {
            name: "Spooler status".to_string(),
            shell: draft.shell,
            command: draft.command.clone(),
            cwd: None,
            stoppable: false,
            stop_command: None,
            note: None,
            auto_run: false,
            show_in_dock: true,
            pre_check: None,
            pre_fix: None,
        };
        crate::quick_actions::validate_quick_action(&action).unwrap();
        let saved = crate::quick_actions::create_quick_action(&conn, &action).unwrap();
        assert_eq!(saved.action.command, "Get-Service -Name Spooler");
        assert_eq!(saved.action.shell, QuickActionShell::Powershell);
        assert!(!saved.action.auto_run, "new actions have auto-run off");
        assert_eq!(crate::quick_actions::list_quick_actions(&conn).unwrap().len(), 1);
    }

    #[test]
    fn refused_drafts_persist_nothing() {
        let conn = crate::db::init_at(&tempfile::tempdir().unwrap().into_path()).unwrap();
        let client = TestClient::new(vec![Scripted::Body(chat_body(
            "Remove-Item C:\\Temp\\notes.txt",
            "Trust me, this is safe.",
        ))]);
        let input = draft_input("powershell", "Show the status of the Spooler service");
        let outcome = request_draft(&client, &skills(), &input);
        assert!(matches!(outcome, DraftOutcome::Refused { .. }));
        assert!(
            crate::quick_actions::list_quick_actions(&conn).unwrap().is_empty(),
            "refusals persist nothing"
        );
    }

    #[test]
    fn recheck_rejects_hostile_edits_without_a_provider() {        assert!(matches!(
            recheck_candidate(QuickActionShell::Powershell, "Get-Service -Name Spooler"),
            OutputVerdict::Allow
        ));
        assert!(matches!(
            recheck_candidate(QuickActionShell::Powershell, "Remove-Item C:\\Temp\\notes.txt"),
            OutputVerdict::Refuse { .. }
        ));
        assert!(matches!(
            recheck_candidate(QuickActionShell::Cmd, "Get-Process"),
            OutputVerdict::Clarify { .. }
        ));
    }

    // ------------------------------------------------------------------
    // Ticket 153: diagnose selected errors, accept a revision explicitly.
    // Built on the checked-draft seam — the same request/output checks, the
    // shared rules plus the Diagnose skill per request, no execution.
    // ------------------------------------------------------------------

    fn diagnose_input(shell: QuickActionShell, script: &str, error: &str) -> DiagnoseInput {
        DiagnoseInput {
            shell,
            script: script.to_string(),
            error: error.to_string(),
            cwd: None,
            model: "test-model".to_string(),
            grants: RequestGrants::none(),
        }
    }

    fn diagnose_body(command: Option<&str>, explanation: &str) -> String {
        serde_json::json!({
            "explanation": explanation,
            "command": command,
            "shell": "powershell",
            "cwd": null,
            "note": null,
            "assumptions": ["Windows PowerShell 5.1 is available"],
            "affected_targets": [],
        })
        .to_string()
    }

    fn saved_action(conn: &rusqlite::Connection, shell: QuickActionShell, command: &str) -> crate::quick_actions::QuickAction {
        let input = crate::quick_actions::QuickActionInput {
            name: "Diagnosed action".to_string(),
            shell,
            command: command.to_string(),
            cwd: None,
            stoppable: true,
            stop_command: None,
            note: Some("operator note".to_string()),
            auto_run: false,
            show_in_dock: true,
            pre_check: None,
            pre_fix: None,
        };
        crate::quick_actions::validate_quick_action(&input).unwrap();
        crate::quick_actions::create_quick_action(conn, &input).unwrap()
    }

    #[test]
    fn diagnosis_returns_a_separate_revision_without_execution() {
        let conn = crate::db::init_at(&tempfile::tempdir().unwrap().into_path()).unwrap();
        let action = saved_action(&conn, QuickActionShell::Powershell, "Get-Service -Name Spoolr");
        let client = TestClient::new(vec![Scripted::Body(diagnose_body(
            Some("Get-Service -Name Spooler"),
            "The service name had a typo.",
        ))]);
        let outcome = request_diagnosis(
            &client,
            &skills(),
            action.id,
            &diagnose_input(
                QuickActionShell::Powershell,
                "Get-Service -Name Spoolr",
                "Get-Service: Cannot find any service with service name 'Spoolr'.",
            ),
        );
        let DiagnoseOutcome::Revision { explanation, revision, baseline } = outcome else {
            panic!("benign diagnosis must propose a revision");
        };
        assert!(explanation.contains("typo"));
        assert_eq!(revision.command, "Get-Service -Name Spooler");
        assert!(!revision.executed, "revisions are marked unexecuted");
        assert_eq!(baseline, revision_baseline(action.id, action.action.shell, &action.action.command, ""));
        // The saved action and its baseline stay separate from the proposal.
        let current = crate::quick_actions::list_quick_actions(&conn).unwrap();
        assert_eq!(current.len(), 1);
        assert_eq!(current[0].action.command, "Get-Service -Name Spoolr");
        // The Diagnose skill — not just shared rules — shaped the prompt.
        let prompts = client.prompts();
        assert_eq!(prompts.len(), 1);
        assert!(prompts[0].contains("Proposes a separate"), "diagnose skill loads per request");
        assert!(prompts[0].contains("Destructive commands are outside AI assistance"));
    }

    #[test]
    fn diagnosis_explains_without_a_revision_when_no_code_change_applies() {
        let client = TestClient::new(vec![Scripted::Body(diagnose_body(
            None,
            "The service is disabled by policy; no command change fixes that.",
        ))]);
        let outcome = request_diagnosis(
            &client,
            &skills(),
            7,
            &diagnose_input(
                QuickActionShell::Cmd,
                "sc query spooler",
                "[SC] EnumQueryServicesStatus:OpenService FAILED 5: Access is denied.",
            ),
        );
        let DiagnoseOutcome::Explanation { explanation } = outcome else {
            panic!("policy-blocked diagnosis must explain only, got {outcome:?}");
        };
        assert!(explanation.contains("policy"));
    }

    #[test]
    fn diagnosis_refuses_destructive_repair_without_leaking_code() {
        let conn = crate::db::init_at(&tempfile::tempdir().unwrap().into_path()).unwrap();
        // Repairing destructive behavior visible in the selection is refused.
        let refusing = TestClient::new(vec![Scripted::Body(diagnose_body(Some("anything"), "x"))]);
        let outcome = request_diagnosis(
            &refusing,
            &skills(),
            1,
            &diagnose_input(
                QuickActionShell::Powershell,
                "Remove-Item C:\\Temp\\notes.txt -Force",
                "Remove-Item: Access to the path is denied.",
            ),
        );
        let DiagnoseOutcome::Refused { message } = outcome else {
            panic!("destructive repair must refuse, got {outcome:?}");
        };
        assert!(message.contains(REFUSAL_BOUNDARY));
        assert!(!message.contains("Remove-Item"), "no prefilled rejected command");
        assert_eq!(refusing.prompts().len(), 0, "refused selections never reach inference");
        // A model-proposed destructive fix is refused the same way, even for
        // a benign selection — the proposal never becomes a usable revision.
        let hostile = TestClient::new(vec![Scripted::Body(diagnose_body(
            Some("Remove-Item C:\\Temp\\notes.txt -Force"),
            "Trust me, this is safe.",
        ))]);
        let hostile_outcome = request_diagnosis(
            &hostile,
            &skills(),
            1,
            &diagnose_input(
                QuickActionShell::Powershell,
                "Get-Content C:\\Temp\\notes.txt",
                "Get-Content: Cannot find path.",
            ),
        );
        let DiagnoseOutcome::Refused { message } = hostile_outcome else {
            panic!("hostile revision must refuse, got {hostile_outcome:?}");
        };
        assert!(message.contains(REFUSAL_BOUNDARY));
        assert!(!message.contains("Remove-Item"));
        assert!(
            crate::quick_actions::list_quick_actions(&conn).unwrap().is_empty(),
            "refusals persist nothing"
        );
    }

    #[test]
    fn diagnosis_malformed_cancellation_and_stale_fail_without_persisting() {
        let conn = crate::db::init_at(&tempfile::tempdir().unwrap().into_path()).unwrap();
        let input = diagnose_input(QuickActionShell::Cmd, "ipconfig", "unexpected footer text");
        let malformed = TestClient::new(vec![Scripted::Body("this is not json".to_string())]);
        assert!(matches!(
            request_diagnosis(&malformed, &skills(), 1, &input),
            DiagnoseOutcome::Failed { .. }
        ));
        let cancelled = TestClient::new(vec![Scripted::Timeout]);
        let DiagnoseOutcome::Failed { message } = request_diagnosis(&cancelled, &skills(), 1, &input) else {
            panic!("cancellation must fail");
        };
        assert!(!message.is_empty());
        // A late completion with no live request behind it fails instead of
        // surfacing a stale proposal.
        let stale = TestClient::new(vec![]);
        assert!(matches!(
            request_diagnosis(&stale, &skills(), 1, &input),
            DiagnoseOutcome::Failed { .. }
        ));
        assert!(
            crate::quick_actions::list_quick_actions(&conn).unwrap().is_empty(),
            "failures persist nothing"
        );
    }

    #[test]
    fn revision_acceptance_detects_conflict_deletion_and_save_failure() {
        let conn = crate::db::init_at(&tempfile::tempdir().unwrap().into_path()).unwrap();
        let action = saved_action(&conn, QuickActionShell::Powershell, "Get-Service -Name Spoolr");
        let baseline = revision_baseline(action.id, action.action.shell, &action.action.command, "");
        // Unchanged source verifies clean.
        assert!(verify_revision_baseline(Some(action.id), &baseline, action.action.shell, &action.action.command, "").is_none());
        // A newer edit conflicts: both states are retained, nothing is
        // overwritten silently.
        let conflict = verify_revision_baseline(
            Some(action.id),
            &baseline,
            QuickActionShell::Powershell,
            "Get-Service -Name Spooler",
            "",
        )
        .expect("newer edits must conflict");
        assert!(conflict.contains("changed since"));
        // Source deletion conflicts instead of recreating or failing obscurely.
        let deleted = verify_revision_baseline(None, &baseline, action.action.shell, &action.action.command, "")
            .expect("deletion must conflict");
        assert!(deleted.contains("no longer exists"));
        // Failed validation surfaces as a save failure with the original intact.
        let mut broken = action.clone();
        broken.action.command = "   ".to_string();
        assert!(crate::quick_actions::validate_quick_action(&broken.action).is_err());
        let current = crate::quick_actions::list_quick_actions(&conn).unwrap();
        assert_eq!(current[0].action.command, "Get-Service -Name Spoolr");
        // Explicit acceptance plus successful saving changes only reviewed
        // fields: Group/order, user notes kept unless accepted, stoppable
        // settings, and auto-run survive.
        let mut accepted = action.clone();
        accepted.action.command = "Get-Service -Name Spooler".to_string();
        accepted.action.shell = QuickActionShell::Powershell;
        crate::quick_actions::validate_quick_action(&accepted.action).unwrap();
        crate::quick_actions::update_quick_action(&conn, &accepted).unwrap();
        let reread = crate::quick_actions::list_quick_actions(&conn).unwrap();
        assert_eq!(reread[0].action.command, "Get-Service -Name Spooler");
        assert_eq!(reread[0].action.note, Some("operator note".to_string()), "unaccepted notes survive");
        assert!(reread[0].action.stoppable, "stoppable settings survive");
        assert!(!reread[0].action.auto_run, "auto-run stays off; the revision cannot enable it");
        assert_eq!(reread[0].group_id, None, "Group membership survives");
    }

    #[test]
    fn diagnosis_sends_only_selected_context_with_no_implicit_grants() {
        let client = TestClient::new(vec![Scripted::Body(diagnose_body(None, "Transient RPC hiccup; retry unchanged."))]);
        let mut input = diagnose_input(
            QuickActionShell::Powershell,
            "Get-Service -Name Spooler",
            "C:\\Actions\\nightly.ps1: line 12: The RPC server is unavailable. (Get-Service C:\\Actions\\nightly.ps1)",
        );
        assert!(input.grants.approved_raw_fields.is_empty(), "diagnosis defaults to nothing approved");
        let outcome = request_diagnosis(&client, &skills(), 3, &input);
        assert!(matches!(outcome, DiagnoseOutcome::Explanation { .. }));
        // Revoking or declining disclosure prevents a cloud request: with no
        // grant recorded, a cloud adapter must refuse before sending the
        // locally bound script path or the newly selected error output.
        input.grants = RequestGrants::none();
        assert!(input.grants.approved_raw_fields.is_empty());
        let prompts = client.prompts();
        assert_eq!(prompts.len(), 1);
        assert!(prompts[0].contains("nightly.ps1"), "the selected error rides the local prompt");
    }

    /// The loopback HTTP adapter against fixture servers: success, unknown
    /// models, malformed bodies, and redirects that must never be followed.
    mod adapter {
        use super::*;
        use std::io::Write;
        use std::net::TcpListener;
        use std::thread;

        fn serve_once(body: Vec<u8>) -> (String, thread::JoinHandle<Vec<u8>>) {
            let listener = TcpListener::bind("127.0.0.1:0").expect("fixture port");
            let root = format!("http://{}", listener.local_addr().expect("addr"));
            let handle = thread::spawn(move || {
                let (mut stream, _) = listener.accept().expect("one connection");
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .expect("read timeout");
                // Read the head, then exactly the framed body, so assertions
                // see the whole request even when packets split.
                let mut seen = Vec::new();
                let mut buf = [0u8; 4096];
                loop {
                    match stream.read(&mut buf) {
                        Ok(0) => break,
                        Ok(n) => {
                            seen.extend_from_slice(&buf[..n]);
                            if let Some(end) = find_head_end(&seen) {
                                let want = content_length(&seen[..end]) + end;
                                if seen.len() >= want {
                                    break;
                                }
                            }
                            if seen.len() > 1 << 20 {
                                break;
                            }
                        }
                        Err(_) => break,
                    }
                }
                let _ = stream.write_all(&body);
                seen
            });
            (root, handle)
        }

        fn find_head_end(seen: &[u8]) -> Option<usize> {
            seen.windows(4)
                .position(|w| w == b"\r\n\r\n")
                .map(|pos| pos + 4)
        }

        fn content_length(head: &[u8]) -> usize {
            let text = String::from_utf8_lossy(head).to_ascii_lowercase();
            text.lines()
                .filter_map(|line| line.strip_prefix("content-length:"))
                .filter_map(|value| value.trim().parse::<usize>().ok())
                .next()
                .unwrap_or(0)
        }

        fn http_ok(json: &str) -> Vec<u8> {
            format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                json.len(),
                json
            )
            .into_bytes()
        }

        #[test]
        fn chat_completions_roundtrip_parses_the_message() {
            let payload = chat_body("ipconfig", "Shows IP configuration.");
            let wire = serde_json::json!({
                "choices": [{ "message": { "content": payload } }],
            })
            .to_string();
            let (root, handle) = serve_once(http_ok(&wire));
            let client = ExistingLocalClient { root, timeout: Duration::from_secs(5) };
            let skills = skills();
            let prompt = build_prompt(&skills, QuickActionShell::Cmd, "Show my IP configuration", "").unwrap();
            let raw = client.generate(&prompt, "test-model").expect("fixture answers");
            assert!(raw.contains("ipconfig"));
            let seen = String::from_utf8_lossy(&handle.join().expect("server read")).to_string();
            assert!(seen.starts_with("POST /v1/chat/completions"), "tested shape only, got {seen}");
            assert!(seen.contains("\"stream\":false"), "buffered, never streamed");
        }

        #[test]
        fn unknown_models_fail_without_substitution() {
            let (root, handle) = serve_once(http_ok(r#"{"data": [{"id": "real-model"}]}"#));
            let client = ExistingLocalClient { root: root.clone(), timeout: Duration::from_secs(5) };
            let prompt = DraftPrompt { system: "s".into(), user: "u".into() };
            // Generation itself never substitutes: the wire model rides as-is.
            let _ = client.generate(&prompt, "ghost-model");
            let seen = String::from_utf8_lossy(&handle.join().expect("server read")).to_string();
            assert!(seen.contains("ghost-model"));
            // The connection check refuses the unknown name honestly.
            let (root2, _) = serve_once(http_ok(r#"{"data": [{"id": "real-model"}]}"#));
            let error = check_existing_local(&root2, "ghost-model").unwrap_err();
            assert!(matches!(error, ProviderError::UnknownModel(_)));
        }

        #[test]
        fn malformed_bodies_and_redirects_fail_closed() {
            let (root, _) = serve_once(http_ok("this is not json"));
            let client = ExistingLocalClient { root, timeout: Duration::from_secs(5) };
            let prompt = DraftPrompt { system: "s".into(), user: "u".into() };
            assert_eq!(client.generate(&prompt, "m").unwrap_err(), ProviderError::Malformed);

            let (root, _) = serve_once(
                "HTTP/1.1 302 Found\r\nLocation: http://192.168.0.9:11434/v1/chat/completions\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".as_bytes().to_vec(),
            );
            let client = ExistingLocalClient { root, timeout: Duration::from_secs(5) };
            assert_eq!(client.generate(&prompt, "m").unwrap_err(), ProviderError::Redirected);
        }

        #[test]
        fn models_check_accepts_both_list_shapes() {
            for body in [
                r#"{"data": [{"id": "a"}, {"id": "b"}]}"#,
                r#"{"models": [{"name": "a"}, {"name": "b"}]}"#,
            ] {
                let (root, _) = serve_once(http_ok(body));
                let names = list_models(&root, Duration::from_secs(5)).expect("fixture lists");
                assert_eq!(names, vec!["a".to_string(), "b".to_string()]);
            }
            let (root, _) = serve_once(http_ok(r#"{"data": []}"#));
            assert!(matches!(
                list_models(&root, Duration::from_secs(5)).unwrap_err(),
                ProviderError::Unsupported(_)
            ));
        }

        #[test]
        fn unreachable_services_fail_actionably() {
            let client = ExistingLocalClient {
                root: "http://127.0.0.1:1".to_string(),
                timeout: Duration::from_secs(2),
            };
            let prompt = DraftPrompt { system: "s".into(), user: "u".into() };
            assert!(matches!(
                client.generate(&prompt, "m").unwrap_err(),
                ProviderError::Unavailable(_) | ProviderError::Timeout
            ));
        }
    }
}
