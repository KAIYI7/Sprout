# Model recommendations and curated AI skills ship with Sprout releases

> Status: amended 2026-09-13 — scoped clarification + bounded grill delivered in 184 under spec 181 (see dated amendments); original text preserved. Broader qualification (146/150–152/155) stays open.

Sprout maintains its model recommendations as JSON in the repository and bundles that data with the application. Curated script-authoring and diagnosis skills are likewise versioned in the repository and packaged as runtime resources. Both change through reviewed Sprout application releases. This accepts slower recommendation delivery to keep distribution and configuration simple, without another backend or independently updated policy channel.

## Decisions

- There is no separate recommendation backend, remote catalog refresh, periodic catalog request, independently signed catalog publication, remote skill refresh, or user-editable/custom skill facility in the first release.
- Changing shipped recommendations or skills requires a normal application version bump and release tag. A repository edit alone does not change an installed application. The established signed application-update verification in ADR-0012 is reused; this does not create an additional signing-key system or change first-install trust.
- Recommendation data identifies the exact model artifact/revision, download location, checksum, size, license, runtime compatibility, and measured suitability information. It contains no executable installation recipes. Model/runtime downloads occur only when requested, from their distribution sources, and are verified before use.
- Two tested tiers are intended: lightweight (approximately the proposed 3B class) and stronger (approximately 7–8B class). Exact models, quantization, licenses, and hardware requirements remain blocked on ticket 145. Qwen2.5-Coder-3B-Instruct is a candidate, not an accepted shipping default; that family's larger candidate is 7B, not an 8B artifact.
- A new recommendation does not download, replace, remove, or switch an installed selection. Removing an entry from a later recommendation list does not reinterpret the user's installed selection as the new default. Incompatible installed models produce an honest choice/recovery path, not silent replacement or cloud fallback.
- Skills are a pinned, attributed, adapted subset of Matt Pocock's workflows for script authoring and diagnosis. Sprout loads the relevant content; merely listing names cannot load it. Developer-only instructions to run tests, execute repairs, commit, or require unavailable agent tools are removed/adapted to respect ADR-0030. Upstream notices and license obligations are retained.
- Fixed skills improve predictability but are not a security boundary. Application-enforced permissions and checks remain independent of model and skill text. Safety policy and skill updates travel through reviewed app releases, not model output or arbitrary files found during discovery.

## Considered options and consequences

A separate signed, remotely refreshed model catalog was considered and rejected in favor of bundled JSON. Static hosting could have avoided a backend server, but still required another publication, refresh, and verification mechanism. Users on older Sprout versions knowingly see older recommendations. Custom skills were rejected to avoid configuration drift and exposing unsupported capabilities; advanced model choice remains available through the supported existing-local and cloud paths.

## Amendment — 2026-09-06 (tracker numbering)

Another session published issue 144 before this planning package was completed. The authoritative parent is [spec 145](../../.scratch/sprout-app/issues/145-ai-assisted-quick-action-authoring-spec.md), implemented by tickets 146–155. The original ticket-145 qualification references above are now [ticket 146](../../.scratch/sprout-app/issues/146-ai-model-runtime-provider-qualification.md). Only tracker numbering changed; the decision is unchanged.

## Amendment — 2026-09-06 (Sprout task skills and command knowledge)

Maintain Sprout-owned shared rules plus Create Quick Action and Diagnose Quick Action task skills in the repository, bundled into the installed app. Shared rules cover draft-only behavior, destructive refusal, selected-shell compatibility, discovery/disclosure grants and explicit review/save. Creation covers authorized targets, shell-specific quoting and prerequisites; diagnosis uses selected scripts/errors and proposes a separate revision without execution. Load actual shared and relevant task content on each applicable request. Adapt useful pinned Matt Pocock principles with required notices; no unmodified developer execution/test/commit workflows or custom skills. Skills cannot grant authority. Tickets 146, 148, 153 and 155 own qualification, delivery and verification.

The user declined a copied/downloaded Microsoft command-reference dataset to avoid documentation redistribution obligations. Do not download, copy, bundle or retrieve Microsoft documentation as a runtime command catalog. Use model knowledge constrained by Sprout-authored instructions, independently authored benign examples and non-executing compatibility checks. Target Windows PowerShell 5.1 and Windows CMD explicitly; do not assume PowerShell 7 parameters or optional modules/external programs exist. Disclose or clarify unknown prerequisites. Maintainers may consult and link primary documentation for research. Command knowledge is not a safety boundary. This choice does not remove separate model/runtime or adapted-skill licensing qualification.

## Amendment — 2026-09-12 (scoped clarification template; spec 181, ticket 184)

The pinned `create-quick-action` skill gains one appended Clarification template section: vague requests (unknown path, ambiguous app, unknown prerequisite) return `Clarify` with two to four pickable choices plus a free-text slot — the grill-with-docs frontier question scoped to command generation. Tone, language, rules, and section structure of the skill are otherwise unchanged; NOTICES and attribution are retained. App-side enforcement is unchanged (ADR-0030 request/output checks; ADR-0031 bounded discovery plus separate disclosure grants): clarification carries no executable code and answering never widens consent. No custom, editable, or remote skills. Implementation pending in 184.

## Amendment — 2026-09-13 (bounded grill consensus; ticket 184 follow-up)

The single round-trip rule is replaced by aspect-keyed one-shot questions: each
vagueness aspect (unknown folder, ambiguous app, unknown prerequisite, scope,
bypass) fires at most once per generation thread, answers chain onto the
request, and an answered question never repeats while a distinct one still asks
back — the exchange ends when no question remains open (detector consensus) or
when the user drafts anyway with what they have (user consensus). Shell
compatibility stays outside the grill and always re-fires; refusal still beats
everything. Enforcement stays app-side and rule-based (ADR-0030/ADR-0031
unchanged); the skill template gains only the one-shot wording above.

## Amendment — 2026-09-13 (round delivery; spec 181, ticket 185)

Delivered in 184 with the 183↔184 dialog handoff: the Clarification template
append ships as described (append-only to `create-quick-action.md`, existing
sections byte-identical, NOTICES retained, `include_str!` packaging unchanged);
vague fixtures clarify with 2–4 choices plus free text and no usable draft,
answering regenerates via aspect-keyed tags (`clarified choice [aspect]: …`,
legacy bare tag honored, `draft-anyway:` override), refusal and
shell-compatibility guard every draft, cloud/disclosure/binding boundaries
unchanged (ADR-0030/0031). Frontend renders choices as radios reusing the
find-pick row pattern (183 owns the view). Fixture battery green
(`ai-eval-fixtures.json` verdicts asserted in `eval_fixtures_reach_their_expected_verdicts`);
packaging check holds (installed app serves pinned skills without checkout).
Original plus 2026-09-06/12/13 text untouched; broader qualification
(146/150–152/155) stays open.

## Amendment — 2026-09-15 (prerequisite surfacing; ticket 200)

The pinned `create-quick-action` skill gains one appended Prerequisites
section: unknown prerequisites are disclosed or detected, never invented —
`present` / `not-found for X` / `not-verifiable` results surface where
available, offline or uncertain checks read `Not verifiable`, every
dependency is recorded in `assumptions`, and the dialog warns with install
guidance without ever blocking saving or auto-installing. Existing sections
are byte-identical, NOTICES are retained, and `include_str!` packaging is
unchanged. Detection stays read-only under its existing owner (ticket 199);
no custom, editable, or remote skills (ADR-0030/0031 unchanged).
