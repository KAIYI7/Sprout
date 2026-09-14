//! Read-only prerequisite detection for AI-assisted authoring.
//!
//! The one deep seam between "does the machine have X?" and its callers: a
//! small interface (`detect_with` plus the verdict types below) hides the
//! closed v1 source list — PATH lookup, winget install state, runtime
//! `--version` probes, the Playwright probe, and the editor-extension probe.
//! Deleting this module would re-scatter probe argv, version parsing, and
//! timeout semantics across the skill and dialog callers, so it earns its
//! keep; callers supply names and consume verdicts, never subprocess calls.
//!
//! Separation of authority, enforced by construction (ADR-0030, ADR-0031):
//! detection takes prerequisite keys only, never draft text, and every probe
//! is a fixed app-owned argv run without a shell — there is no parameter
//! through which generated script could reach execution. Nothing here touches
//! the network, and nothing writes: probes read install state and report.
//!
//! Windows compatibility stays with its owners (ADR-0029): every probe runs
//! through the timed-process owner in `super::process`, so this submodule
//! introduces no second invocation site. Winget argv and table parsing stay
//! in `winget/` — the install-state map arrives as injected data from the
//! Tauri command, which reads the existing read-only snapshot.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use serde::Serialize;

use crate::windows_execution::{run_timed_process, ProcessRun};

/// Per-probe timebox: a probe outliving it is killed with its tree and reads
/// as `not-verifiable`, never as absent.
pub const DETECT_PER_PROBE_TIMEOUT: Duration = Duration::from_secs(15);

/// Whole-request budget across every probe: once exhausted, unrun keys read
/// as `not-verifiable` without spawning, so a large request still returns.
pub const DETECT_TOTAL_BUDGET: Duration = Duration::from_secs(60);

/// Upper bound on keys per request, so one call cannot queue unbounded work.
pub const DETECT_MAX_PREREQUISITES: usize = 25;

/// Upper bound on one key's length; keys ride as single argv elements, never
/// through a shell, so this bounds work rather than blocking injection.
pub const DETECT_MAX_KEY_CHARS: usize = 200;

/// The executables detection may spawn, fixed at compile time. A probe
/// outside this set is a code change, not a caller choice — which is what
/// keeps generated text from ever reaching a process.
///
/// WHY test-only: production probes name these executables at their own
/// call sites; this table exists so the no-execution test can oracle every
/// spawned argv against one fixed list.
#[cfg(test)]
const DETECT_PROBE_ALLOW_LIST: [&str; 6] = [
    "where.exe",
    "node",
    "python",
    "python3",
    "npx",
    "code",
];

/// One prerequisite's install state. `version` carries a probe-reported
/// version only where meaningful (runtimes, Playwright, winget rows,
/// extensions); PATH-only presence leaves it `None`, serialized as null so
/// the shape never shifts under the dialog.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PrerequisiteVerdict {
    pub name: String,
    pub status: PrereqStatus,
    pub version: Option<String>,
    pub detail: String,
}

/// The three honest outcomes: `present` (the check ran and found it),
/// `not-found` (the check ran and did not), `not-verifiable` (offline,
/// timeout, or otherwise uncertain — never a guess).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PrereqStatus {
    Present,
    NotFound,
    NotVerifiable,
}

/// The internal probe seam: production runs timed processes, tests supply a
/// scripted transcript. Two adapters ride this seam, so it is real rather
/// than hypothetical — and the test adapter is what makes the verdict matrix
/// deterministic.
pub trait ProbeRunner {
    fn timed(&self, exe: &str, args: &[String], timeout: Duration) -> ProcessRun;
}

/// The production adapter: every probe through the timed-process owner, which
/// kills the whole tree on timebox expiry.
pub struct NativeProbes;

impl ProbeRunner for NativeProbes {
    fn timed(&self, exe: &str, args: &[String], timeout: Duration) -> ProcessRun {
        run_timed_process(exe, args, timeout)
    }
}

/// Whether any key needs winget install state, so the command can skip the
/// snapshot subprocess when no `winget:` key is present.
pub(crate) fn needs_winget_snapshot(names: &[String]) -> bool {
    names.iter().any(|name| {
        let key = name.trim().to_lowercase();
        key.starts_with("winget:") && key.len() > "winget:".len()
    })
}

/// The closed v1 catalog: each key routes to exactly one source. Anything
/// unrecognized is a PATH executable lookup — an unknown tool name still gets
/// an honest `not-found` from a real check, never a guess.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Catalog {
    RuntimeNode,
    RuntimePython,
    Playwright,
    Winget(String),
    Extension(String),
    PathExe(String),
}

fn route(raw: &str) -> Result<Catalog, String> {
    let key = raw.trim();
    let lowered = key.to_lowercase();
    match lowered.as_str() {
        "node" | "nodejs" => Ok(Catalog::RuntimeNode),
        "python" | "python3" => Ok(Catalog::RuntimePython),
        "playwright" => Ok(Catalog::Playwright),
        _ => {
            if let Some(id) = split_prefix(key, "winget:") {
                return if id.trim().is_empty() {
                    Err("A 'winget:' prerequisite must name a winget id.".to_string())
                } else {
                    Ok(Catalog::Winget(id.trim().to_string()))
                };
            }
            if let Some(id) = split_prefix(key, "extension:") {
                return if id.trim().is_empty() {
                    Err("An 'extension:' prerequisite must name an extension id.".to_string())
                } else {
                    Ok(Catalog::Extension(id.trim().to_string()))
                };
            }
            Ok(Catalog::PathExe(key.to_string()))
        }
    }
}

/// Splits a `prefix:value` key case-insensitively, preserving the value's own
/// casing (winget ids and extension ids compare case-insensitively later).
fn split_prefix<'a>(key: &'a str, prefix: &str) -> Option<&'a str> {
    if key.len() >= prefix.len() && key[..prefix.len()].eq_ignore_ascii_case(prefix) {
        Some(&key[prefix.len()..])
    } else {
        None
    }
}

fn validate(names: &[String]) -> Result<Vec<String>, String> {
    if names.len() > DETECT_MAX_PREREQUISITES {
        return Err(format!(
            "At most {} prerequisites per check — split the request.",
            DETECT_MAX_PREREQUISITES
        ));
    }
    names
        .iter()
        .map(|name| {
            let key = name.trim();
            if key.is_empty() {
                return Err("Prerequisite names must not be blank.".to_string());
            }
            if key.len() > DETECT_MAX_KEY_CHARS {
                return Err(format!("'{key}' is longer than a prerequisite name can be."));
            }
            Ok(key.to_string())
        })
        .collect()
}

/// Runs the closed-catalog check for every key, in order, under the shared
/// budget. `winget` is the injected install-state map (`None` when winget
/// state could not be read): the module never builds winget argv itself, so
/// winget compatibility knowledge stays in its owner.
pub fn detect_with(
    names: &[String],
    winget: Option<&HashMap<String, (String, Option<String>)>>,
    runner: &impl ProbeRunner,
) -> Result<Vec<PrerequisiteVerdict>, String> {
    detect_with_limits(
        names,
        winget,
        runner,
        DETECT_PER_PROBE_TIMEOUT,
        DETECT_TOTAL_BUDGET,
        &AtomicBool::new(false),
    )
}

fn detect_with_limits(
    names: &[String],
    winget: Option<&HashMap<String, (String, Option<String>)>>,
    runner: &impl ProbeRunner,
    per_probe: Duration,
    total: Duration,
    cancelled: &AtomicBool,
) -> Result<Vec<PrerequisiteVerdict>, String> {
    let keys = validate(names)?;
    let deadline = Instant::now() + total;
    let mut verdicts = Vec::with_capacity(keys.len());
    for key in &keys {
        if cancelled.load(Ordering::SeqCst) || Instant::now() >= deadline {
            verdicts.push(not_verifiable(
                key,
                "Not verifiable — the detection budget was exhausted before this check ran.",
            ));
            continue;
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        let timeout = per_probe.min(remaining.max(Duration::from_secs(1)));
        let catalog = route(key)?;
        verdicts.push(match catalog {
            Catalog::RuntimeNode => probe_runtime(key, "node", &["node"], runner, timeout),
            Catalog::RuntimePython => {
                probe_runtime(key, "python", &["python", "python3"], runner, timeout)
            }
            Catalog::Playwright => probe_playwright(key, runner, timeout),
            Catalog::Winget(id) => probe_winget(key, &id, winget),
            Catalog::Extension(id) => probe_extension(key, &id, runner, timeout),
            Catalog::PathExe(exe) => probe_path(key, &exe, runner, timeout),
        });
    }
    Ok(verdicts)
}

fn present(name: &str, version: Option<String>, detail: String) -> PrerequisiteVerdict {
    PrerequisiteVerdict {
        name: name.to_string(),
        status: PrereqStatus::Present,
        version,
        detail,
    }
}

fn not_found(name: &str) -> PrerequisiteVerdict {
    PrerequisiteVerdict {
        name: name.to_string(),
        status: PrereqStatus::NotFound,
        version: None,
        detail: format!("Not found for {name}"),
    }
}

fn not_verifiable(name: &str, detail: &str) -> PrerequisiteVerdict {
    PrerequisiteVerdict {
        name: name.to_string(),
        status: PrereqStatus::NotVerifiable,
        version: None,
        detail: detail.to_string(),
    }
}

fn timeout_detail(timeout: Duration) -> String {
    format!(
        "Not verifiable — the check timed out after {}s.",
        timeout.as_secs().max(1)
    )
}

/// Interprets one timed run: a timeout or a failure to spawn means the check
/// did not run, so the verdict is `not-verifiable` — never absent.
fn ran(run: &ProcessRun, name: &str, timeout: Duration) -> Result<(), PrerequisiteVerdict> {
    if run.timed_out {
        return Err(not_verifiable(name, &timeout_detail(timeout)));
    }
    if run.exit_code.is_none() {
        return Err(not_verifiable(
            name,
            "Not verifiable — the check itself could not start.",
        ));
    }
    Ok(())
}

/// PATH lookup through the fixed `where.exe` probe: exit 0 names a path,
/// anything else means the check ran and found nothing.
fn probe_path(
    name: &str,
    exe: &str,
    runner: &impl ProbeRunner,
    timeout: Duration,
) -> PrerequisiteVerdict {
    let run = runner.timed("where.exe", &[exe.to_string()], timeout);
    if let Err(verdict) = ran(&run, name, timeout) {
        return verdict;
    }
    if run.exit_code == Some(0) {
        let path = run.output.lines().map(str::trim).find(|line| !line.is_empty());
        match path {
            Some(path) => present(name, None, format!("Found on PATH ({path})")),
            None => not_found(name),
        }
    } else {
        not_found(name)
    }
}

/// Runtime probe: PATH lookup first, then the fixed `--version` probe for the
/// version. Any timeout in the chain reads as `not-verifiable`.
fn probe_runtime(
    name: &str,
    label: &str,
    candidates: &[&str],
    runner: &impl ProbeRunner,
    timeout: Duration,
) -> PrerequisiteVerdict {
    let mut found: Option<String> = None;
    for exe in candidates {
        let run = runner.timed("where.exe", &[exe.to_string()], timeout);
        if let Err(verdict) = ran(&run, name, timeout) {
            return verdict;
        }
        if run.exit_code == Some(0) {
            found = Some(exe.to_string());
            break;
        }
    }
    let Some(exe) = found else {
        return not_found(name);
    };
    let run = runner.timed(&exe, &["--version".to_string()], timeout);
    if let Err(verdict) = ran(&run, name, timeout) {
        return verdict;
    }
    if run.exit_code == Some(0) {
        match first_version_token(&run.output) {
            Some(version) => present(
                name,
                Some(version.clone()),
                format!("{label} {version} found"),
            ),
            None => not_found(name),
        }
    } else {
        not_found(name)
    }
}

/// Playwright probe: the fixed local query that never installs. A missing
/// local Playwright exits non-zero (`not-found`); an unreachable runner or a
/// timeout is `not-verifiable`.
fn probe_playwright(
    name: &str,
    runner: &impl ProbeRunner,
    timeout: Duration,
) -> PrerequisiteVerdict {
    let run = runner.timed(
        "npx",
        &[
            "--no-install".to_string(),
            "playwright".to_string(),
            "--version".to_string(),
        ],
        timeout,
    );
    if let Err(verdict) = ran(&run, name, timeout) {
        if run.timed_out {
            return verdict;
        }
        return not_verifiable(
            name,
            "Not verifiable — the Node.js tooling needed for the check is not available.",
        );
    }
    if run.exit_code == Some(0) {
        match first_version_token(&run.output) {
            Some(version) => present(
                name,
                Some(version.clone()),
                format!("playwright {version} found"),
            ),
            None => not_found(name),
        }
    } else {
        not_found(name)
    }
}

/// Winget install-state lookup against the injected map: no subprocess here,
/// so no second winget invocation site. A missing map means the state could
/// not be read — `not-verifiable`, never absent.
fn probe_winget(
    name: &str,
    id: &str,
    winget: Option<&HashMap<String, (String, Option<String>)>>,
) -> PrerequisiteVerdict {
    let Some(map) = winget else {
        return not_verifiable(
            name,
            "Not verifiable — winget install state could not be read.",
        );
    };
    match map.get(&id.to_lowercase()) {
        Some((version, _)) => present(
            name,
            Some(version.clone()),
            format!("{id} {version} installed (winget)"),
        ),
        None => not_found(name),
    }
}

/// Editor-extension probe: the fixed local extension listing, matched
/// case-insensitively against `id@version` rows. An unavailable `code` CLI
/// means the check did not run — `not-verifiable`.
fn probe_extension(
    name: &str,
    id: &str,
    runner: &impl ProbeRunner,
    timeout: Duration,
) -> PrerequisiteVerdict {
    let run = runner.timed(
        "code",
        &[
            "--list-extensions".to_string(),
            "--show-versions".to_string(),
        ],
        timeout,
    );
    if let Err(verdict) = ran(&run, name, timeout) {
        if run.timed_out {
            return verdict;
        }
        return not_verifiable(
            name,
            "Not verifiable — the editor CLI needed for the check is not available.",
        );
    }
    if run.exit_code != Some(0) {
        return not_verifiable(
            name,
            "Not verifiable — the editor extension listing did not succeed.",
        );
    }
    match parse_extension_version(&run.output, id) {
        Some(version) => present(
            name,
            Some(version.clone()),
            format!("{id} {version} found (editor extension)"),
        ),
        None => not_found(name),
    }
}

/// First version-shaped token in merged probe output: the leading `v` of
/// `v22.14.0` is stripped, and the `ERR ` prefixes of merged stderr lines are
/// skipped naturally since `ERR` is not version-shaped.
fn first_version_token(output: &str) -> Option<String> {
    output.split_whitespace().find_map(|token| {
        let bare = token
            .strip_prefix(|c| c == 'v' || c == 'V')
            .unwrap_or(token);
        let version: String = bare
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '.' || *c == '-' || *c == '+')
            .collect();
        let shaped = version.starts_with(|c: char| c.is_ascii_digit())
            && version.chars().any(|c| c.is_ascii_digit());
        shaped.then(|| version.trim_matches(|c| c == '.' || c == '-' || c == '+').to_string())
    })
}

/// Matches one `id@version` row of `code --list-extensions --show-versions`,
/// case-insensitively on the id.
fn parse_extension_version(output: &str, id: &str) -> Option<String> {
    output.lines().find_map(|line| {
        let (row_id, version) = line.trim().split_once('@')?;
        row_id
            .eq_ignore_ascii_case(id.trim())
            .then(|| version.trim().to_string())
            .filter(|version| !version.is_empty())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::{HashMap, VecDeque};

    /// Scripted probe adapter: each `timed` call records its argv and replays
    /// the next queued run, so the verdict matrix is fully deterministic.
    struct Transcript {
        calls: RefCell<Vec<(String, Vec<String>)>>,
        runs: RefCell<VecDeque<ProcessRun>>,
    }

    impl Transcript {
        fn replay(runs: Vec<ProcessRun>) -> Self {
            Self {
                calls: RefCell::new(Vec::new()),
                runs: RefCell::new(runs.into()),
            }
        }

        fn ok(output: &str) -> ProcessRun {
            ProcessRun {
                timed_out: false,
                exit_code: Some(0),
                output: output.to_string(),
            }
        }

        fn fail(output: &str) -> ProcessRun {
            ProcessRun {
                timed_out: false,
                exit_code: Some(1),
                output: output.to_string(),
            }
        }

        fn spawn_failed() -> ProcessRun {
            ProcessRun {
                timed_out: false,
                exit_code: None,
                output: "failed to start: not found".to_string(),
            }
        }

        fn timed_out() -> ProcessRun {
            ProcessRun {
                timed_out: true,
                exit_code: None,
                output: "partial\n[TIMED OUT after 15s - killed]\n".to_string(),
            }
        }

        fn exes(&self) -> Vec<String> {
            self.calls.borrow().iter().map(|(exe, _)| exe.clone()).collect()
        }
    }

    impl ProbeRunner for Transcript {
        fn timed(&self, exe: &str, args: &[String], _timeout: Duration) -> ProcessRun {
            self.calls
                .borrow_mut()
                .push((exe.to_string(), args.to_vec()));
            self.runs.borrow_mut().pop_front().expect("unexpected probe call")
        }
    }

    fn winget_map() -> HashMap<String, (String, Option<String>)> {
        HashMap::from([(
            "git.git".to_string(),
            ("2.47.0".to_string(), None),
        )])
    }

    #[test]
    fn verdict_matrix_per_source() {
        let probes = Transcript::replay(vec![
            Transcript::ok("C:\\nodejs\\node.exe"), // where node
            Transcript::ok("v22.14.0"),             // node --version
            Transcript::fail("INFO: Could not find files"), // where python
            Transcript::fail("INFO: Could not find files"), // where python3
            Transcript::ok("Version 1.49.1"),       // npx playwright --version
            Transcript::ok("ms-python.python@2024.10.0\ngitlens@1.0.0"), // code --list-extensions
            Transcript::ok("C:\\Tools\\ffmpeg.exe"), // where ffmpeg
        ]);
        let names = [
            "node",
            "python",
            "playwright",
            "winget:Git.Git",
            "extension:ms-python.python",
            "ffmpeg",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect::<Vec<_>>();
        let verdicts = detect_with(&names, Some(&winget_map()), &probes).unwrap();
        assert_eq!(verdicts.len(), 6);

        assert_eq!(verdicts[0].name, "node");
        assert_eq!(verdicts[0].status, PrereqStatus::Present);
        assert_eq!(verdicts[0].version.as_deref(), Some("22.14.0"));

        assert_eq!(verdicts[1].status, PrereqStatus::NotFound);
        assert_eq!(verdicts[1].version, None);
        assert!(verdicts[1].detail.contains("Not found for python"));

        assert_eq!(verdicts[2].status, PrereqStatus::Present);
        assert_eq!(verdicts[2].version.as_deref(), Some("1.49.1"));

        assert_eq!(verdicts[3].status, PrereqStatus::Present);
        assert_eq!(verdicts[3].version.as_deref(), Some("2.47.0"));

        assert_eq!(verdicts[4].status, PrereqStatus::Present);
        assert_eq!(verdicts[4].version.as_deref(), Some("2024.10.0"));

        assert_eq!(verdicts[5].status, PrereqStatus::Present);
        assert_eq!(verdicts[5].version, None);
    }

    #[test]
    fn not_found_matrix_per_source() {
        let probes = Transcript::replay(vec![
            Transcript::fail("no match"),                       // where node
            Transcript::ok("C:\\py\\python.exe"),               // where python
            Transcript::fail("not recognized"),                 // python --version nonzero
            Transcript::fail("npm ERR! could not determine executable"), // npx playwright missing
            Transcript::ok("gitlens@1.0.0"),                    // extensions without the id
            Transcript::fail("INFO: Could not find files"),     // where ffmpeg
        ]);
        let names = [
            "node",
            "python",
            "playwright",
            "winget:Missing.Product",
            "extension:ms-python.python",
            "ffmpeg",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect::<Vec<_>>();
        let verdicts = detect_with(&names, Some(&winget_map()), &probes).unwrap();
        assert!(verdicts.iter().all(|v| v.status == PrereqStatus::NotFound));
        assert!(verdicts.iter().all(|v| v.version.is_none()));
        for verdict in &verdicts {
            assert!(
                verdict.detail.starts_with("Not found for"),
                "{}",
                verdict.detail
            );
        }
    }

    #[test]
    fn not_verifiable_matrix_for_unrunnable_checks() {
        let probes = Transcript::replay(vec![
            Transcript::spawn_failed(), // where node cannot start
            Transcript::spawn_failed(), // npx cannot start
            Transcript::spawn_failed(), // code cannot start
        ]);
        let names = ["node", "playwright", "extension:ms-python.python"]
            .iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>();
        let verdicts = detect_with(&names, None, &probes).unwrap();
        assert!(verdicts.iter().all(|v| v.status == PrereqStatus::NotVerifiable));
        // The winget check never spawned (no map) yet still reads honestly.
        let winget = detect_with(&["winget:Git.Git".to_string()], None, &probes).unwrap();
        assert_eq!(winget[0].status, PrereqStatus::NotVerifiable);
        assert!(probes
            .calls
            .borrow()
            .iter()
            .all(|(exe, _)| exe != "winget"));
    }

    #[test]
    fn timeout_reads_as_not_verifiable_never_absent() {
        let probes = Transcript::replay(vec![
            Transcript::timed_out(), // where node hangs
            Transcript::ok("C:\\py\\python.exe"),
            Transcript::timed_out(), // python --version hangs
            Transcript::timed_out(), // npx hangs
        ]);
        let names = ["node", "python", "playwright"]
            .iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>();
        let verdicts = detect_with(&names, Some(&winget_map()), &probes).unwrap();
        assert_eq!(verdicts.len(), 3);
        for verdict in &verdicts {
            assert_eq!(verdict.status, PrereqStatus::NotVerifiable);
            assert_eq!(verdict.version, None);
            assert!(
                verdict.detail.starts_with("Not verifiable"),
                "{}",
                verdict.detail
            );
        }
    }

    #[test]
    fn exhausted_budget_marks_remaining_without_spawning() {
        let probes = Transcript::replay(vec![Transcript::ok("C:\\nodejs\\node.exe"), Transcript::ok("v1.0.0")]);
        let names = ["node".to_string(), "python".to_string()];
        let verdicts = detect_with_limits(
            &names,
            None,
            &probes,
            Duration::from_secs(15),
            Duration::ZERO,
            &AtomicBool::new(false),
        )
        .unwrap();
        assert_eq!(verdicts.len(), 2);
        assert!(verdicts.iter().all(|v| v.status == PrereqStatus::NotVerifiable));
        assert!(probes.calls.borrow().is_empty());
    }

    #[test]
    fn zero_script_execution_calls_across_detect() {
        let probes = Transcript::replay(vec![
            Transcript::ok("C:\\nodejs\\node.exe"),
            Transcript::ok("v22.14.0"),
            Transcript::ok("C:\\py\\python.exe"),
            Transcript::ok("Python 3.14.7"),
            Transcript::ok("Version 1.49.1"),
            Transcript::ok("ms-python.python@2024.10.0"),
            Transcript::ok("C:\\Tools\\ffmpeg.exe"),
        ]);
        let names = [
            "node",
            "python",
            "playwright",
            "winget:Git.Git",
            "extension:ms-python.python",
            "ffmpeg",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect::<Vec<_>>();
        detect_with(&names, Some(&winget_map()), &probes).unwrap();
        for (exe, args) in probes.calls.borrow().iter() {
            assert!(
                DETECT_PROBE_ALLOW_LIST.contains(&exe.as_str()),
                "probe outside the fixed allow-list: {exe}"
            );
            assert!(!["powershell", "cmd", "winget"].contains(&exe.as_str()));
            for arg in args {
                assert!(!arg.contains("Invoke-"), "script text in probe args: {arg}");
            }
        }
    }

    #[test]
    fn routing_is_case_insensitive_and_trims() {
        assert_eq!(route("Node").unwrap(), Catalog::RuntimeNode);
        assert_eq!(route("  PYTHON3 ").unwrap(), Catalog::RuntimePython);
        assert_eq!(
            route("WINGET:Git.Git").unwrap(),
            Catalog::Winget("Git.Git".to_string())
        );
        assert_eq!(
            route("Extension:MS-Python.Python").unwrap(),
            Catalog::Extension("MS-Python.Python".to_string())
        );
        assert_eq!(
            route("  ffmpeg ").unwrap(),
            Catalog::PathExe("ffmpeg".to_string())
        );
        assert!(route("winget:").is_err());
        assert!(route("extension:").is_err());
    }

    #[test]
    fn validation_rejects_blank_oversize_and_overcount() {
        assert!(detect_with(&["  ".to_string()], None, &Transcript::replay(vec![])).is_err());
        let long = "x".repeat(DETECT_MAX_KEY_CHARS + 1);
        assert!(detect_with(&[long], None, &Transcript::replay(vec![])).is_err());
        let many = (0..DETECT_MAX_PREREQUISITES + 1)
            .map(|i| format!("tool{i}"))
            .collect::<Vec<_>>();
        assert!(detect_with(&many, None, &Transcript::replay(vec![])).is_err());
        assert!(detect_with(&[], None, &Transcript::replay(vec![])).unwrap().is_empty());
    }

    #[test]
    fn version_tokens_parse_across_probe_shapes() {
        assert_eq!(first_version_token("v22.14.0\r\n").as_deref(), Some("22.14.0"));
        assert_eq!(
            first_version_token("Python 3.14.7").as_deref(),
            Some("3.14.7")
        );
        assert_eq!(
            first_version_token("Version 1.49.1").as_deref(),
            Some("1.49.1")
        );
        assert_eq!(
            first_version_token("ERR Python 2.7.18").as_deref(),
            Some("2.7.18")
        );
        assert_eq!(first_version_token("not found"), None);
        assert_eq!(first_version_token(""), None);
        assert_eq!(
            parse_extension_version("ms-python.python@2024.10.0\nother@1.0", "MS-Python.Python").as_deref(),
            Some("2024.10.0")
        );
        assert_eq!(parse_extension_version("other@1.0", "ms-python.python"), None);
    }

    #[test]
    fn winget_lookup_matches_case_insensitively() {
        let probes = Transcript::replay(vec![]);
        let verdicts = detect_with(
            &["winget:GIT.git".to_string()],
            Some(&winget_map()),
            &probes,
        )
        .unwrap();
        assert_eq!(verdicts[0].status, PrereqStatus::Present);
        assert!(probes.calls.borrow().is_empty());
    }
}
