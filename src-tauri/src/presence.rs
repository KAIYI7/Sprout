//! Discord Rich Presence with a continuous session clock (ADR-0033): while
//! Sprout runs, Discord users see "Using Sprout" with a section state and one
//! elapsed timer counting since app launch. The crate speaks only to Discord's
//! local IPC pipe — no network, no OAuth scopes, no token storage, no user-ID
//! read — so the offline posture holds: the seam below can carry only fixed
//! text plus the session start, never user content.
//!
//! WHY a background loop instead of set-once: Discord may be closed at boot or
//! restarted mid-session, so connect→set retries with capped backoff and the
//! live connection is re-asserted on a heartbeat; a dropped pipe returns to
//! the retry loop instead of dying silently. Every failure stays local
//! (`eprintln` only) — presence never blocks startup, opens no window, and
//! shows no toast or dialog.
//!
//! WHY the session start is captured once: every set reuses it, so the elapsed
//! clock survives text updates, heartbeat re-asserts, and reconnects instead
//! of resetting to 0:00 (ADR-0033).
//!
//! WHY the single-flight guard: setup runs once per process, but a second
//! `start` must never spawn a second loop fighting over the same IPC pipe.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use discord_rich_presence::{
    activity::{Activity, Assets, Timestamps},
    DiscordIpc, DiscordIpcClient,
};

/// Discord Application (= Client) ID identifying Sprout to the desktop
/// client. WHY hardcoded rather than Settings-editable, backed up, or
/// exported: it is one app-wide value, not per-user configuration — and it is
/// not a secret (it ships in the binary and travels to the local Discord
/// client only), so there is nothing to protect with a secret store
/// (ADR-0033).
pub const APPLICATION_ID: &str = "1548219927264235580";

/// The fixed detail line presence always sends (ADR-0033): the section state
/// varies, the detail never does.
pub const DETAILS: &str = "Using Sprout";
/// The default section state: shown until the UI reports a section, and the
/// fallback for anything without a mapping (ADR-0033).
pub const STATE: &str = "Composing presets";

/// WHY a cap: Discord truncates overlong activity text, so oversize input is
/// refused at the boundary instead of shipping clipped.
const MAX_TEXT_LEN: usize = 128;

/// How often a live connection re-asserts the activity: frequent enough to
/// notice a Discord restart, rare enough to stay quiet on a local pipe.
const HEARTBEAT: Duration = Duration::from_secs(30);

/// Whether the background loop has been claimed; WHY a static: setup and exit
/// are process-global events with no handle to thread through.
static STARTED: AtomicBool = AtomicBool::new(false);
/// WHY a static beside STARTED: the loop is detached, so shutdown signals
/// through shared state and confirms with its own synchronous clear below.
static STOP: AtomicBool = AtomicBool::new(false);

/// The session clock, captured once per process. WHY an `OnceLock`: every set
/// reuses the same value, so text updates, heartbeats, and reconnects never
/// reset the elapsed timer (ADR-0033).
static SESSION_START_MS: OnceLock<i64> = OnceLock::new();

/// The current section text. WHY a lock beside the clock: text changes while
/// the clock stands still — the loop snapshots both on every set (ADR-0033).
static CURRENT: OnceLock<Mutex<(String, String)>> = OnceLock::new();

fn current_lock() -> &'static Mutex<(String, String)> {
    CURRENT.get_or_init(|| Mutex::new((DETAILS.to_string(), STATE.to_string())))
}

/// Unix-millis clock for the session start. WHY wall-clock: Discord renders
/// the elapsed timer from `timestamps.start`, which is defined in unix time.
fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis().min(i64::MAX as u128) as i64)
        .unwrap_or(0)
}

/// The process-wide session start, captured on first read. `start` reads it at
/// launch, so the clock measures the whole run even when Discord connects
/// later (ADR-0033).
pub fn session_started_at_ms() -> i64 {
    *SESSION_START_MS.get_or_init(now_ms)
}

/// The current section text for the next set.
pub fn current_text() -> (String, String) {
    let guard = current_lock()
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    (guard.0.clone(), guard.1.clone())
}

/// The exact activity for one set: caller text plus the session start plus the
/// static brand art. Pure in `start_ms` so tests pin the clock instead of
/// racing wall time.
pub fn activity_for(details: &str, state: &str, start_ms: i64) -> Activity<'static> {
    // WHY fixed literals: the art keys resolve to portal-uploaded brand art,
    // so the payload carries only static strings — never user content (ADR-0033).
    Activity::new()
        .details(details.to_owned())
        .state(state.to_owned())
        .timestamps(Timestamps::new().start(start_ms))
        .assets(
            Assets::new()
                .large_image("rp_large")
                .large_text("Sprout")
                .small_image("rp_small")
                .small_text("Ready"),
        )
}

/// WHY a boundary: overlong text would arrive clipped and blank text would
/// blank the status, so both are refused before they reach the pipe (ADR-0033).
fn validated(text: &str, what: &str) -> Result<String, String> {
    let trimmed = text.trim().to_string();
    if trimmed.is_empty() {
        return Err(format!("Presence {what} must not be blank"));
    }
    if trimmed.chars().count() > MAX_TEXT_LEN {
        return Err(format!(
            "Presence {what} is longer than {MAX_TEXT_LEN} characters"
        ));
    }
    Ok(trimmed)
}

/// Stores validated section text for the loop's next set. Split from
/// `set_status` so tests cover validation and storage without touching the
/// real IPC pipe.
fn store_status(details: &str, state: &str) -> Result<(String, String), String> {
    let details = validated(details, "details")?;
    let state = validated(state, "state")?;
    *current_lock()
        .lock()
        .unwrap_or_else(|e| e.into_inner()) = (details.clone(), state.clone());
    Ok((details, state))
}

/// Reports a section change: stores validated text for the loop's next set and
/// pushes one immediate set so the message follows promptly instead of waiting
/// out the heartbeat. WHY `Ok` past validation: Discord absent is a silent
/// no-op — presence never surfaces errors (ADR-0033).
pub fn set_status(details: &str, state: &str) -> Result<(), String> {
    let (details, state) = store_status(details, state)?;
    let start_ms = session_started_at_ms();
    let mut ipc = RealIpc::new();
    match ipc.connect() {
        Ok(()) => {
            if let Err(e) = ipc.set_current(&details, &state, start_ms) {
                eprintln!("{e} — continuing without presence");
            }
        }
        Err(e) => eprintln!("{e} — continuing without presence"),
    }
    Ok(())
}

/// The module's internal seam: everything presence can do to the outside
/// world. WHY a seam with two adapters: Discord present vs absent is one
/// variation, deterministic tests the other — a real seam, not a hypothetical
/// one. The interface admits only fixed text plus the session start, which is
/// what makes the no-account-access promise structural instead of verbal.
trait Ipc {
    fn connect(&mut self) -> Result<(), String>;
    fn set_current(&mut self, details: &str, state: &str, start_ms: i64) -> Result<(), String>;
    fn clear_and_close(&mut self);
}

/// The live adapter: the handwired crate over the local IPC pipe, and the
/// only Discord IPC site in the app (ADR-0029).
struct RealIpc {
    client: DiscordIpcClient,
}

impl RealIpc {
    fn new() -> Self {
        Self {
            client: DiscordIpcClient::new(APPLICATION_ID),
        }
    }
}

impl Ipc for RealIpc {
    fn connect(&mut self) -> Result<(), String> {
        // WHY map_err to String: the loop logs uniformly without learning the
        // crate's error type — callers never match on failure kinds.
        self.client
            .connect()
            .map_err(|e| format!("Discord IPC connect failed: {e}"))
    }

    fn set_current(&mut self, details: &str, state: &str, start_ms: i64) -> Result<(), String> {
        self.client
            .set_activity(activity_for(details, state, start_ms))
            .map_err(|e| format!("Discord IPC set failed: {e}"))
    }

    fn clear_and_close(&mut self) {
        // WHY best-effort with no return: shutdown paths must not fail — a
        // dead pipe and an already-cleared activity are both fine outcomes.
        let _ = self.client.clear_activity();
        let _ = self.client.close();
    }
}

/// Capped backoff between reconnect attempts: quick first retries for a
/// client still starting up, then a steady 10 s cadence that stays quiet.
fn backoff_for_attempt(failures: u32) -> Duration {
    match failures {
        0 => Duration::from_secs(1),
        1 => Duration::from_secs(2),
        2 => Duration::from_secs(5),
        _ => Duration::from_secs(10),
    }
}

/// One connect→set attempt: both halves must succeed before the activity is
/// considered live, so a half-open pipe never reads as presence. Text and
/// clock are snapshotted together for this set, so a section change never
/// ships without the original start.
fn connect_and_set(
    ipc: &mut impl Ipc,
    start_ms: i64,
    current: impl Fn() -> (String, String),
) -> Result<(), String> {
    ipc.connect()?;
    let (details, state) = current();
    ipc.set_current(&details, &state, start_ms)
}

/// Starts the presence loop on a background thread: connect→set, then
/// heartbeat re-asserts until shutdown. The session start is captured here —
/// at launch — so the elapsed clock measures the whole run even when Discord
/// connects later. Never blocks the caller; a second call is a no-op so
/// overlapping starts never duplicate the loop.
pub fn start() {
    if STARTED.swap(true, Ordering::SeqCst) {
        return;
    }
    STOP.store(false, Ordering::SeqCst);
    let start_ms = session_started_at_ms();
    std::thread::spawn(move || {
        let mut ipc = RealIpc::new();
        serve_with(
            &mut ipc,
            HEARTBEAT,
            std::thread::sleep,
            &STOP,
            start_ms,
            current_text,
        );
    });
}

/// Stops the loop and clears the activity. WHY a synchronous clear beside the
/// flag: the process may exit before the background thread wakes, so actual
/// exit clears through its own one-shot client while the flag stops the loop
/// from reconnecting behind it. Main-window close-to-tray never calls this —
/// only real exit does.
pub fn shutdown() {
    STOP.store(true, Ordering::SeqCst);
    RealIpc::new().clear_and_close();
}

/// The loop body with its sleeps injected: connect→set with backoff, then a
/// heartbeat that returns to retry when the pipe drops. WHY the heartbeat
/// re-sets instead of idling: idling would never notice a Discord restart.
/// WHY `start_ms` is a parameter rather than the `SESSION_START_MS` static:
/// deterministic tests pin the clock instead of racing wall time, and parallel
/// tests never share it. WHY `stop` is a parameter rather than the `STOP`
/// static: deterministic tests drive this exact function with a local flag
/// instead of process-global state parallel tests would flake on.
/// WHY the text is a provider rather than a value: each set snapshots the
/// latest section, so message changes ride the same unbroken clock.
fn serve_with(
    ipc: &mut impl Ipc,
    heartbeat: Duration,
    sleep: impl Fn(Duration),
    stop: &AtomicBool,
    start_ms: i64,
    current: impl Fn() -> (String, String),
) {
    let mut failures: u32 = 0;
    loop {
        if stop.load(Ordering::SeqCst) {
            ipc.clear_and_close();
            return;
        }
        match connect_and_set(ipc, start_ms, &current) {
            Ok(()) => {
                failures = 0;
                loop {
                    sleep(heartbeat);
                    if stop.load(Ordering::SeqCst) {
                        ipc.clear_and_close();
                        return;
                    }
                    let (details, state) = current();
                    if let Err(e) = ipc.set_current(&details, &state, start_ms) {
                        eprintln!("Discord presence lost ({e}) — retrying quietly");
                        break;
                    }
                }
            }
            Err(e) => {
                eprintln!("{e} — continuing without presence");
                sleep(backoff_for_attempt(failures));
                failures = failures.saturating_add(1);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    /// Calls the seam can observe: the test surface mirrors the trait, so a
    /// test asserting "only Connect/Set/ClearClose with text plus the session
    /// start" asserts everything the module can ever emit.
    #[derive(Debug, Clone, PartialEq, Eq)]
    enum Call {
        Connect,
        Set {
            details: String,
            state: String,
            start_ms: i64,
        },
        ClearClose,
    }

    /// WHY a pinned clock: wall time never enters a test — every assertion
    /// compares sets against this constant, so runs are deterministic.
    const PINNED_START: i64 = 1_786_000_000_000;

    fn fixed_text() -> (String, String) {
        (DETAILS.to_string(), STATE.to_string())
    }

    /// Deterministic stand-in for Discord: scripted connect/set failures plus
    /// a full call log. The payload it records can only be text plus a start —
    /// the seam gives it no other shape to carry. It trips the shared stop
    /// flag itself after a scripted number of connects or sets, so each test
    /// runs the real `serve_with` loop to a deterministic halt.
    struct FakeIpc {
        calls: Vec<Call>,
        stop: Arc<AtomicBool>,
        connects_done: u32,
        sets_done: u32,
        connect_failures_left: u32,
        set_failures_left: u32,
        stop_after_connects: Option<u32>,
        stop_after_sets: Option<u32>,
    }

    impl FakeIpc {
        fn new(stop: &Arc<AtomicBool>) -> Self {
            Self {
                calls: Vec::new(),
                stop: Arc::clone(stop),
                connects_done: 0,
                sets_done: 0,
                connect_failures_left: 0,
                set_failures_left: 0,
                stop_after_connects: None,
                stop_after_sets: None,
            }
        }

        fn failing_connects(mut self, n: u32) -> Self {
            self.connect_failures_left = n;
            self
        }

        fn stop_after_connects(mut self, n: u32) -> Self {
            self.stop_after_connects = Some(n);
            self
        }

        fn stop_after_sets(mut self, n: u32) -> Self {
            self.stop_after_sets = Some(n);
            self
        }
    }

    impl Ipc for FakeIpc {
        fn connect(&mut self) -> Result<(), String> {
            self.calls.push(Call::Connect);
            self.connects_done += 1;
            if let Some(n) = self.stop_after_connects {
                if self.connects_done >= n {
                    self.stop.store(true, Ordering::SeqCst);
                }
            }
            if self.connect_failures_left > 0 {
                self.connect_failures_left -= 1;
                return Err("Discord IPC connect failed: no Discord".into());
            }
            Ok(())
        }

        fn set_current(
            &mut self,
            details: &str,
            state: &str,
            start_ms: i64,
        ) -> Result<(), String> {
            self.calls.push(Call::Set {
                details: details.to_string(),
                state: state.to_string(),
                start_ms,
            });
            self.sets_done += 1;
            if let Some(n) = self.stop_after_sets {
                if self.sets_done >= n {
                    self.stop.store(true, Ordering::SeqCst);
                }
            }
            if self.set_failures_left > 0 {
                self.set_failures_left -= 1;
                return Err("Discord IPC set failed: pipe dropped".into());
            }
            Ok(())
        }

        fn clear_and_close(&mut self) {
            self.calls.push(Call::ClearClose);
        }
    }

    /// Runs the real `serve_with` loop against a fake: sleeps are recorded
    /// for backoff assertions and never taken, the clock is pinned, and the
    /// fake halts the loop itself — deterministic, no threads, no timing flakes.
    fn drive(
        mut ipc: FakeIpc,
        stop: &AtomicBool,
        heartbeat: Duration,
        start_ms: i64,
        current: impl Fn() -> (String, String),
    ) -> (Vec<Call>, Vec<Duration>) {
        let sleeps: Vec<Duration> = Vec::new();
        let sleeps_cell = Mutex::new(sleeps);
        serve_with(
            &mut ipc,
            heartbeat,
            |d| sleeps_cell.lock().unwrap().push(d),
            stop,
            start_ms,
            current,
        );
        (ipc.calls, sleeps_cell.into_inner().unwrap())
    }

    fn sets_in(calls: &[Call]) -> Vec<(String, String, i64)> {
        calls
            .iter()
            .filter_map(|c| match c {
                Call::Set {
                    details,
                    state,
                    start_ms,
                } => Some((details.clone(), state.clone(), *start_ms)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn static_payload_carries_exactly_the_v1_text() {
        // WHY via the session clock: the default set reuses the process-wide
        // start, so the text assertion pins the start to the same read.
        let start_ms = session_started_at_ms();
        let value = serde_json::to_value(activity_for(DETAILS, STATE, start_ms)).unwrap();
        assert_eq!(value["details"], DETAILS);
        assert_eq!(value["state"], STATE);
        assert_eq!(value["timestamps"]["start"], start_ms);
        assert_eq!(DETAILS, "Using Sprout");
        assert_eq!(STATE, "Composing presets");
    }

    #[test]
    fn payload_sets_text_start_and_brand_assets_but_no_other_optional_sections() {
        // WHY shape-assert instead of field-assert: the crate's fields are
        // private, so the serialized form is the observable contract — text,
        // `timestamps.start`, and the static brand assets must be present,
        // while buttons/party/secrets must arrive as null or absent, never
        // populated, or this fails loudly (ADR-0033).
        let value =
            serde_json::to_value(activity_for(DETAILS, STATE, PINNED_START)).unwrap();
        assert_eq!(value["details"], DETAILS);
        assert_eq!(value["state"], STATE);
        assert_eq!(value["timestamps"]["start"], PINNED_START);
        assert!(value["timestamps"]["end"].is_null());
        assert_eq!(value["assets"]["large_image"], "rp_large");
        assert_eq!(value["assets"]["large_text"], "Sprout");
        assert_eq!(value["assets"]["small_image"], "rp_small");
        assert_eq!(value["assets"]["small_text"], "Ready");
        for key in ["buttons", "party", "secrets"] {
            assert!(
                value.get(key).is_none_or(|v| v.is_null()),
                "presence must not set {key}: {value}"
            );
        }
        let flat = serde_json::to_string(&value).unwrap().to_lowercase();
        assert!(
            !flat.contains("token") && !flat.contains("oauth"),
            "presence payload must carry no account material: {flat}"
        );
    }

    #[test]
    fn session_start_is_captured_once_and_nonzero() {
        let first = session_started_at_ms();
        assert!(first > 0);
        assert_eq!(session_started_at_ms(), first);
    }

    #[test]
    fn status_storage_validates_and_stores_without_touching_ipc() {
        assert!(store_status("", STATE).is_err());
        assert!(store_status("   ", STATE).is_err());
        assert!(store_status(DETAILS, "").is_err());
        assert!(store_status(&"x".repeat(MAX_TEXT_LEN + 1), STATE).is_err());
        assert!(store_status(DETAILS, &"y".repeat(MAX_TEXT_LEN + 1)).is_err());
        // WHY the restore: `CURRENT` is process-global, so this test leaves
        // the default behind for every other test.
        assert_eq!(
            store_status("  Using Sprout  ", "Reading logs").unwrap(),
            ("Using Sprout".to_string(), "Reading logs".to_string())
        );
        assert_eq!(
            current_text(),
            ("Using Sprout".to_string(), "Reading logs".to_string())
        );
        assert!(store_status(DETAILS, STATE).is_ok());
        assert_eq!(current_text(), fixed_text());
    }

    #[test]
    fn application_id_is_a_plain_numeric_id_not_a_secret_shape() {
        // WHY numeric-only: a Discord Application ID is a snowflake of digits
        // — anything shaped like a token, URI, or key refuses to ship here.
        // WHY the placeholder refusal: an all-zeros ID can never show
        // presence, so shipping one again must fail loudly, not silently.
        assert!(!APPLICATION_ID.is_empty());
        assert!(APPLICATION_ID.chars().all(|c| c.is_ascii_digit()));
        assert!(APPLICATION_ID.chars().any(|c| c != '0'));
    }

    #[test]
    fn backoff_starts_quick_then_settles_at_ten_seconds() {
        assert_eq!(
            (0..6).map(backoff_for_attempt).collect::<Vec<_>>(),
            vec![
                Duration::from_secs(1),
                Duration::from_secs(2),
                Duration::from_secs(5),
                Duration::from_secs(10),
                Duration::from_secs(10),
                Duration::from_secs(10),
            ]
        );
    }

    #[test]
    fn discord_running_sets_the_static_pair_then_clears_on_stop() {
        let stop = Arc::new(AtomicBool::new(false));
        let ipc = FakeIpc::new(&stop).stop_after_sets(1);
        // One live set parks the loop on its heartbeat; the stop tripped by
        // that set then clears on the way out.
        let heartbeat = Duration::from_millis(1);
        let (calls, sleeps) = drive(ipc, &stop, heartbeat, PINNED_START, fixed_text);
        assert!(calls.contains(&Call::Connect));
        assert!(sets_in(&calls).contains(&(
            DETAILS.to_string(),
            STATE.to_string(),
            PINNED_START
        )));
        assert_eq!(calls.last(), Some(&Call::ClearClose));
        assert_eq!(sleeps, vec![heartbeat]);
    }

    #[test]
    fn discord_absent_retries_with_backoff_and_never_sets() {
        let stop = Arc::new(AtomicBool::new(false));
        let ipc = FakeIpc::new(&stop)
            .failing_connects(u32::MAX)
            .stop_after_connects(3);
        // Silent failure keeps retrying — never sets, never panics — with the
        // capped backoff between attempts, then clears on the way out.
        let (calls, sleeps) = drive(ipc, &stop, Duration::from_millis(1), PINNED_START, fixed_text);
        assert!(sets_in(&calls).is_empty());
        assert_eq!(
            calls,
            vec![Call::Connect, Call::Connect, Call::Connect, Call::ClearClose]
        );
        assert_eq!(
            sleeps,
            vec![
                Duration::from_secs(1),
                Duration::from_secs(2),
                Duration::from_secs(5),
            ]
        );
    }

    #[test]
    fn reconnect_after_startup_flaps_sets_eventually() {
        let stop = Arc::new(AtomicBool::new(false));
        let ipc = FakeIpc::new(&stop).failing_connects(2).stop_after_sets(1);
        let heartbeat = Duration::from_millis(1);
        let (calls, sleeps) = drive(ipc, &stop, heartbeat, PINNED_START, fixed_text);
        assert_eq!(
            calls,
            vec![
                Call::Connect,
                Call::Connect,
                Call::Connect,
                Call::Set {
                    details: DETAILS.to_string(),
                    state: STATE.to_string(),
                    start_ms: PINNED_START,
                },
                Call::ClearClose,
            ]
        );
        assert_eq!(
            sleeps,
            vec![
                Duration::from_secs(1),
                Duration::from_secs(2),
                heartbeat,
            ]
        );
    }

    #[test]
    fn consecutive_sets_reuse_the_original_start_when_text_changes() {
        let stop = Arc::new(AtomicBool::new(false));
        let ipc = FakeIpc::new(&stop).stop_after_sets(3);
        // The section changes under the loop while the clock stands still —
        // the text provider, not the loop, owns the message.
        let texts = Mutex::new(vec![
            fixed_text(),
            (
                DETAILS.to_string(),
                "Reviewing a plan".to_string(),
            ),
            (DETAILS.to_string(), "Reading logs".to_string()),
        ]);
        let (calls, _) = drive(ipc, &stop, Duration::from_millis(1), PINNED_START, || {
            texts.lock().unwrap().pop().unwrap_or_else(fixed_text)
        });
        let sets = sets_in(&calls);
        assert_eq!(sets.len(), 3);
        // WHY the clock assertion first: it is the whole point — three
        // messages, one start, no reset to 0:00 (ADR-0033).
        for (_, _, start_ms) in &sets {
            assert_eq!(*start_ms, PINNED_START);
        }
        let messages: Vec<(String, String)> = sets
            .iter()
            .map(|(details, state, _)| (details.clone(), state.clone()))
            .collect();
        assert!(
            messages.windows(2).any(|w| w[0] != w[1]),
            "the text must actually change across the sets: {messages:?}"
        );
    }

    #[test]
    fn dropped_pipe_returns_to_retry_instead_of_dying() {
        let stop = Arc::new(AtomicBool::new(false));
        let mut ipc = FakeIpc::new(&stop);
        assert!(connect_and_set(&mut ipc, PINNED_START, fixed_text).is_ok());
        // The heartbeat re-set fails: the loop must break back to
        // connect→set (a fresh Connect follows) rather than park silently.
        ipc.set_failures_left = 1;
        assert!(ipc.set_current(DETAILS, STATE, PINNED_START).is_err());
        assert!(connect_and_set(&mut ipc, PINNED_START, fixed_text).is_ok());
        let connects = ipc.calls.iter().filter(|c| **c == Call::Connect).count();
        assert_eq!(connects, 2);
        // The reconnect re-sends the original start, never a fresh one.
        for (_, _, start_ms) in sets_in(&ipc.calls) {
            assert_eq!(start_ms, PINNED_START);
        }
    }

    #[test]
    fn shutdown_path_clears_and_closes() {
        let stop = Arc::new(AtomicBool::new(false));
        let mut ipc = FakeIpc::new(&stop);
        assert!(connect_and_set(&mut ipc, PINNED_START, fixed_text).is_ok());
        ipc.clear_and_close();
        assert_eq!(ipc.calls.last(), Some(&Call::ClearClose));
    }

    #[test]
    fn only_text_and_start_ever_cross_the_seam() {
        let stop = Arc::new(AtomicBool::new(false));
        let mut ipc = FakeIpc::new(&stop).failing_connects(1);
        let _ = connect_and_set(&mut ipc, PINNED_START, fixed_text);
        let _ = connect_and_set(&mut ipc, PINNED_START, || {
            (DETAILS.to_string(), "Reviewing history".to_string())
        });
        ipc.clear_and_close();
        // WHY exhaustive: the fake's `Call` enum is the whole vocabulary of
        // the seam — matching every variant here means a new leaking call
        // cannot compile without updating this proof.
        for call in &ipc.calls {
            match call {
                Call::Connect | Call::ClearClose => {}
                Call::Set {
                    details,
                    state,
                    start_ms,
                } => {
                    assert!(!details.trim().is_empty());
                    assert!(!state.trim().is_empty());
                    assert!(details.chars().count() <= MAX_TEXT_LEN);
                    assert!(state.chars().count() <= MAX_TEXT_LEN);
                    assert_eq!(*start_ms, PINNED_START);
                }
            }
        }
        assert!(!sets_in(&ipc.calls).is_empty());
    }
}
