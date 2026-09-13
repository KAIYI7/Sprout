//! Optional app-owned local inference behind the shared draft-provider seam.
//!
//! The public interface stays deliberately small: report the bundled catalog,
//! install one qualified selection after an explicit request, borrow one
//! request-scoped provider, cancel it, reap an idle runtime, or shut down.
//! Artifact, process, transport, and clock adapters are private seams used by
//! the production implementation and deterministic lifecycle tests.

use std::{
    collections::HashMap,
    fs::{self, File},
    io::{Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, Weak,
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};

use crate::{
    ai_assist::{DraftPrompt, DraftProvider, ProviderError},
    windows_execution,
};

const CATALOG_JSON: &str = include_str!("../resources/ai-model-recommendations.json");
const INSTALL_MANIFEST: &str = "managed-install.json";
const IDLE_INTERVAL: Duration = Duration::from_secs(5 * 60);
const STARTUP_TIMEOUT: Duration = Duration::from_secs(30);
const IO_POLL: Duration = Duration::from_millis(200);
const MAX_HTTP_RESPONSE: usize = 65_536;
/// Distribution hosts answer a pinned catalog URL with a short-lived
/// redirect (measured 2026-09-12: GitHub release assets and HuggingFace
/// resolve URLs both return 302 to presigned HTTPS). Following stays safe
/// here because the staged bytes are still size- and SHA-256-verified before
/// anything is activated, no credentials ride along, the hop count is
/// capped, and any non-HTTPS landing is refused by
/// `refuse_unless_https_landing`.
const MAX_REDIRECTS: u32 = 5;

#[derive(Clone, Deserialize)]
struct Catalog {
    schema_version: u32,
    note: String,
    managed_runtime: RuntimeCatalog,
    tiers: Vec<ModelCatalog>,
}

#[derive(Clone, Deserialize)]
struct RuntimeCatalog {
    name: String,
    candidate_version: String,
    windows_cpu_x64_artifact: String,
    source: String,
    license: String,
    status: String,
    verified: RuntimeVerification,
    blocker: String,
    #[serde(default)]
    download_url: Option<String>,
    #[serde(default)]
    sha256: Option<String>,
    #[serde(default)]
    size_bytes: Option<u64>,
    #[serde(default)]
    executable: Option<String>,
    #[serde(default)]
    launch_args: Option<Vec<String>>,
}

#[derive(Clone, Deserialize)]
struct RuntimeVerification {
    download: bool,
    per_user_launch: bool,
    health: bool,
    cancellation: bool,
    model_release: bool,
    process_ownership: bool,
    exit_behavior: bool,
}

impl RuntimeVerification {
    fn complete(&self) -> bool {
        self.download
            && self.per_user_launch
            && self.health
            && self.cancellation
            && self.model_release
            && self.process_ownership
            && self.exit_behavior
    }
}

#[derive(Clone, Deserialize)]
struct ModelCatalog {
    id: String,
    intent: String,
    candidate_artifact: String,
    artifact_source: String,
    revision: Option<String>,
    quantization: Option<String>,
    download_hash: Option<String>,
    download_size_bytes: Option<u64>,
    license: String,
    license_source: String,
    status: String,
    blocker: String,
    context_limit_tokens: Option<u64>,
    template_requirements: Option<String>,
    memory_needs_mb: Option<u64>,
    minimum_runtime_version: Option<String>,
    #[serde(default)]
    download_url: Option<String>,
    #[serde(default)]
    model_name: Option<String>,
}

#[derive(Clone)]
struct QualifiedSelection {
    runtime: RuntimeCatalog,
    model: ModelCatalog,
}

impl Catalog {
    fn parse(source: &str) -> Result<Self, String> {
        let catalog: Catalog = serde_json::from_str(source)
            .map_err(|error| format!("The bundled AI recommendation catalog is invalid: {error}"))?;
        if catalog.schema_version != 1 {
            return Err(format!(
                "This build cannot read AI recommendation catalog schema {}.",
                catalog.schema_version
            ));
        }
        // WHY reject here instead of at install time (ADR-0032): a duplicated
        // id would let two tiers claim one selection, and an executable recipe
        // would turn bundled data into code. Both fail the whole catalog
        // loudly rather than picking a winner silently.
        let mut seen = std::collections::HashSet::new();
        for tier in &catalog.tiers {
            if !seen.insert(tier.id.as_str()) {
                return Err(format!(
                    "The bundled AI recommendation catalog lists '{}' twice.",
                    tier.id
                ));
            }
        }
        for forbidden in [
            "install_recipe",
            "install_script",
            "setup_commands",
            "post_install",
            "executable_install",
        ] {
            if source.contains(forbidden) {
                return Err(
                    "The bundled AI recommendation catalog must not carry executable installation instructions.".into(),
                );
            }
        }
        // WHY validate qualified entries at parse time: a tier marked
        // qualified without its artifact, context, memory, or runtime evidence
        // is invalid metadata, not a pending qualification — installs through
        // `qualified()` would refuse it one request at a time instead of
        // failing the catalog once (ADR-0032).
        let runtime_complete = catalog.managed_runtime.download_url.is_some()
            && catalog.managed_runtime.sha256.is_some()
            && catalog.managed_runtime.size_bytes.is_some()
            && catalog.managed_runtime.executable.is_some()
            && catalog.managed_runtime.launch_args.is_some();
        if catalog.managed_runtime.status == "qualified"
            && catalog.managed_runtime.verified.complete()
            && !runtime_complete
        {
            return Err(
                "The bundled AI recommendation catalog marks its runtime qualified without download evidence.".into(),
            );
        }
        for tier in &catalog.tiers {
            if tier.status != "qualified" {
                continue;
            }
            let complete = tier.revision.is_some()
                && tier.download_hash.is_some()
                && tier.download_size_bytes.is_some()
                && tier.context_limit_tokens.is_some()
                && tier.template_requirements.is_some()
                && tier.memory_needs_mb.is_some()
                && tier.minimum_runtime_version.is_some()
                && tier.download_url.is_some()
                && tier.model_name.is_some();
            if !complete {
                return Err(format!(
                    "The bundled AI recommendation catalog marks '{}' qualified without artifact evidence.",
                    tier.id
                ));
            }
            // WHY reject here: JSON alone cannot supply a missing runtime
            // adapter — a qualified tier pinned to another runtime version
            // needs a tested app release, not a data edit (ADR-0031).
            if tier.minimum_runtime_version.as_deref()
                != Some(catalog.managed_runtime.candidate_version.as_str())
            {
                return Err(format!(
                    "The bundled AI recommendation catalog pins '{}' to an unsupported runtime version.",
                    tier.id
                ));
            }
        }
        Ok(catalog)
    }

    fn qualified(&self, id: &str) -> Result<QualifiedSelection, String> {
        let model = self
            .tiers
            .iter()
            .find(|entry| entry.id == id)
            .cloned()
            .ok_or_else(|| "That managed model is not in this build's bundled catalog.".to_string())?;
        if self.managed_runtime.status != "qualified" || !self.managed_runtime.verified.complete() {
            return Err(format!(
                "Managed installation is unavailable: {}",
                self.managed_runtime.blocker
            ));
        }
        if model.status != "qualified" {
            return Err(format!("This recommendation is unavailable: {}", model.blocker));
        }
        let missing_model = model.revision.is_none()
            || model.download_hash.is_none()
            || model.download_size_bytes.is_none()
            || model.context_limit_tokens.is_none()
            || model.template_requirements.is_none()
            || model.memory_needs_mb.is_none()
            || model.minimum_runtime_version.is_none()
            || model.download_url.is_none()
            || model.model_name.is_none();
        let missing_runtime = self.managed_runtime.download_url.is_none()
            || self.managed_runtime.sha256.is_none()
            || self.managed_runtime.size_bytes.is_none()
            || self.managed_runtime.executable.is_none()
            || self.managed_runtime.launch_args.is_none();
        if missing_model || missing_runtime {
            return Err(
                "This catalog entry is marked qualified but lacks required artifact, context, memory, or runtime evidence. Update Sprout before installing."
                    .into(),
            );
        }
        if model.minimum_runtime_version.as_deref()
            != Some(self.managed_runtime.candidate_version.as_str())
        {
            return Err(
                "The recommended model does not match this build's qualified managed runtime version. Update Sprout before installing."
                    .into(),
            );
        }
        Ok(QualifiedSelection {
            runtime: self.managed_runtime.clone(),
            model,
        })
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ManagedRuntimeView {
    pub name: String,
    pub version: String,
    pub artifact: String,
    pub source: String,
    pub license: String,
    pub status: String,
    pub blocker: String,
    pub qualified: bool,
    pub download_size_bytes: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ManagedModelView {
    pub id: String,
    pub intent: String,
    pub artifact: String,
    pub source: String,
    pub revision: Option<String>,
    pub quantization: Option<String>,
    pub sha256: Option<String>,
    pub download_size_bytes: Option<u64>,
    pub license: String,
    pub license_source: String,
    pub status: String,
    pub blocker: String,
    pub context_limit_tokens: Option<u64>,
    pub template_requirements: Option<String>,
    pub memory_needs_mb: Option<u64>,
    pub minimum_runtime_version: Option<String>,
    pub installable: bool,
    pub installed: bool,
    /// Absolute on-disk folder holding this model's weights, its runtime copy
    /// and its manifest (`installations/<id>/<revision>/` under the app-owned
    /// root), or `None` when not installed. Surfaced so the user can find —
    /// and, while the server is stopped, manually delete — exactly what Sprout
    /// owns for this model.
    pub installed_dir: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ManagedCatalogView {
    pub schema_version: u32,
    pub note: String,
    pub runtime: ManagedRuntimeView,
    pub models: Vec<ManagedModelView>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ManagedInstallResult {
    pub model_id: String,
    pub installed: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ManagedRemoveResult {
    pub model_id: String,
    pub removed: bool,
    pub remaining_installed: usize,
    pub message: String,
}

/// One install-progress event (ticket 193), emitted as the Tauri event
/// `MANAGED_INSTALL_PROGRESS_EVENT` while `install_with_progress` stages
/// artifacts. `phase` is `runtime` or `model`; byte counts are cumulative
/// within the phase. `stage` names the current step — `downloading` for byte
/// events, then `verifying-runtime`, `verifying-model`, `extracting`,
/// `activating` — so the frontend can name a long silent step instead of
/// sitting on a stuck-looking 100%. The frontend mirrors them in its
/// app-level store so progress survives tab switches; completion and failure
/// still arrive through the install call itself.
#[derive(Debug, Clone, Serialize)]
pub struct ManagedInstallProgress {
    pub model_id: String,
    pub phase: String,
    pub stage: String,
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
}

/// Keep in step with `MANAGED_INSTALL_PROGRESS_EVENT` in
/// `src/lib/managedInstall.svelte.ts`.
pub const MANAGED_INSTALL_PROGRESS_EVENT: &str = "managed-install-progress";

/// The cheap running indicator behind ticket 189's status line: whether the
/// owned runtime process is alive right now, which installed model it serves,
/// and how long it has been up. Process liveness — never config readiness.
#[derive(Debug, Clone, Serialize)]
pub struct ManagedRunState {
    pub running: bool,
    pub active_model_id: Option<String>,
    pub uptime_secs: Option<u64>,
}

/// Tooltip-grade live numbers behind ticket 190's lazy Details disclosure:
/// working set plus CPU % plus uptime for the owned runtime, or a stopped
/// marker when nothing runs. The frontend polls this only while the disclosure
/// is open and running; closed costs nothing. CPU % derives from successive
/// totals, so the first sample after start reports working set and uptime with
/// no percent yet.
#[derive(Debug, Clone, Serialize)]
pub struct ManagedResourceUsage {
    pub running: bool,
    pub active_model_id: Option<String>,
    pub uptime_secs: Option<u64>,
    pub working_set_bytes: Option<u64>,
    pub cpu_percent: Option<f64>,
}

#[derive(Clone, Deserialize, Serialize)]
struct InstalledManifest {
    schema_version: u32,
    model_id: String,
    revision: String,
    model_name: String,
    model_file: String,
    model_sha256: String,
    runtime_version: String,
    runtime_executable: String,
    runtime_args: Vec<String>,
    runtime_sha256: String,
    context_limit_tokens: u64,
    template_requirements: String,
    memory_needs_mb: u64,
    idle_seconds: u64,
}

trait Downloader: Send + Sync {
    fn fetch(
        &self,
        url: &str,
        target: &Path,
        expected_bytes: u64,
        cancelled: &AtomicBool,
        on_bytes: &dyn Fn(u64),
    ) -> Result<(), String>;
}

struct HttpDownloader;

impl Downloader for HttpDownloader {
    fn fetch(
        &self,
        url: &str,
        target: &Path,
        expected_bytes: u64,
        cancelled: &AtomicBool,
        on_bytes: &dyn Fn(u64),
    ) -> Result<(), String> {
        if !url.starts_with("https://") {
            return Err("Managed artifacts must use an HTTPS download URL.".into());
        }
        if cancelled.load(Ordering::SeqCst) {
            return Err("Managed installation cancelled; no staged artifact was activated.".into());
        }
        let response = ureq::AgentBuilder::new()
            .timeout_connect(Duration::from_secs(20))
            .timeout_read(Duration::from_secs(30))
            .redirects(MAX_REDIRECTS)
            .build()
            .get(url)
            .call()
            .map_err(|error| format!("Managed artifact download failed: {error}"))?;
        refuse_unless_https_landing(response.get_url())?;
        if response.status() != 200 {
            return Err(format!("Managed artifact download returned HTTP {}.", response.status()));
        }
        let mut reader = response.into_reader();
        let mut file = File::create(target)
            .map_err(|error| format!("Could not stage a managed artifact: {error}"))?;
        let mut total = 0u64;
        let mut buffer = [0u8; 64 * 1024];
        on_bytes(0);
        // WHY throttle here (not in the frontend): a gigabyte model is tens of
        // thousands of 64 KiB chunks, and every emit is a Tauri event plus a
        // Svelte state update. Unthrottled that floods the WebView2 renderer
        // until the window stops responding to Close/Cancel/taskbar input.
        // Emit at most every 200 ms (or 256 KiB), plus the final total, so the
        // bar stays live without jamming the event loop (ADR-0031 keeps the
        // download on the blocking worker; this only quiets its chatter).
        let mut last_emit = Instant::now();
        let mut last_emitted: u64 = 0;
        loop {
            if cancelled.load(Ordering::SeqCst) {
                return Err("Managed installation cancelled; no staged artifact was activated.".into());
            }
            let count = reader
                .read(&mut buffer)
                .map_err(|error| format!("Managed artifact download ended early: {error}"))?;
            if count == 0 {
                break;
            }
            total = total.saturating_add(count as u64);
            if total > expected_bytes {
                return Err("Managed artifact was larger than the qualified byte size.".into());
            }
            file.write_all(&buffer[..count])
                .map_err(|error| format!("Could not finish staging a managed artifact: {error}"))?;
            let since_emit = total.saturating_sub(last_emitted);
            if total == expected_bytes
                || since_emit >= 256 * 1024 && last_emit.elapsed() >= Duration::from_millis(200)
            {
                on_bytes(total);
                last_emitted = total;
                last_emit = Instant::now();
            }
        }
        if last_emitted != total {
            on_bytes(total);
        }
        file.sync_all()
            .map_err(|error| format!("Could not flush a staged managed artifact: {error}"))?;
        if total != expected_bytes {
            return Err(format!(
                "Managed artifact download was incomplete: expected {expected_bytes} bytes, received {total}."
            ));
        }
        Ok(())
    }
}

trait HostAdapter: Send + Sync {
    fn disk_bytes(&self, path: &Path) -> Result<u64, String>;
    fn memory_mb(&self) -> Result<u64, String>;
    fn extract_runtime(&self, archive: &Path, destination: &Path) -> Result<(), String>;
    fn reserve_port(&self) -> Result<u16, String>;
    fn spawn(&self, executable: &Path, args: &[String]) -> Result<Box<dyn RuntimeProcess>, String>;
    /// Tooltip-grade totals for one owned process id (ticket 190): working-set
    /// bytes plus total CPU milliseconds since the process started. The caller
    /// derives a live percent from successive totals.
    fn process_usage(&self, pid: u32) -> Result<(u64, u64), String>;
}

trait RuntimeProcess: Send {
    fn exited(&mut self) -> Result<Option<i32>, String>;
    fn pid(&self) -> u32;
    fn stop(&mut self);
}

struct WindowsHost;

struct WindowsRuntimeProcess(windows_execution::OwnedProcess);

impl RuntimeProcess for WindowsRuntimeProcess {
    fn exited(&mut self) -> Result<Option<i32>, String> {
        self.0.exited()
    }

    fn pid(&self) -> u32 {
        self.0.id()
    }

    fn stop(&mut self) {
        self.0.stop();
    }
}

impl HostAdapter for WindowsHost {
    fn disk_bytes(&self, path: &Path) -> Result<u64, String> {
        windows_execution::available_disk_bytes(path)
    }

    fn memory_mb(&self) -> Result<u64, String> {
        windows_execution::system_memory_mb()
    }

    fn extract_runtime(&self, archive: &Path, destination: &Path) -> Result<(), String> {
        windows_execution::extract_zip_hidden(archive, destination)
    }

    fn reserve_port(&self) -> Result<u16, String> {
        TcpListener::bind(("127.0.0.1", 0))
            .and_then(|listener| listener.local_addr())
            .map(|address| address.port())
            .map_err(|error| format!("Could not reserve a loopback endpoint for managed AI: {error}"))
    }

    fn spawn(&self, executable: &Path, args: &[String]) -> Result<Box<dyn RuntimeProcess>, String> {
        windows_execution::spawn_owned_hidden(executable, args)
            .map(|process| Box::new(WindowsRuntimeProcess(process)) as Box<dyn RuntimeProcess>)
    }

    fn process_usage(&self, pid: u32) -> Result<(u64, u64), String> {
        windows_execution::owned_process_usage(pid)
    }
}

trait Clock: Send + Sync {
    fn now_ms(&self) -> u64;
    fn sleep(&self, duration: Duration);
}

struct SystemClock {
    origin: Instant,
}

impl SystemClock {
    fn new() -> Self {
        Self { origin: Instant::now() }
    }
}

impl Clock for SystemClock {
    fn now_ms(&self) -> u64 {
        self.origin.elapsed().as_millis() as u64
    }

    fn sleep(&self, duration: Duration) {
        std::thread::sleep(duration);
    }
}

trait LocalTransport: Send + Sync {
    fn healthy(&self, endpoint: SocketAddr, cancelled: &AtomicBool) -> Result<bool, String>;
    fn generate(
        &self,
        endpoint: SocketAddr,
        prompt: &DraftPrompt,
        model: &str,
        cancelled: &AtomicBool,
    ) -> Result<String, ProviderError>;
}

struct LoopbackTransport;

impl LocalTransport for LoopbackTransport {
    fn healthy(&self, endpoint: SocketAddr, cancelled: &AtomicBool) -> Result<bool, String> {
        match http_exchange(endpoint, "GET", "/health", None, cancelled, Duration::from_secs(2)) {
            Ok((200, _)) => Ok(true),
            Ok(_) => Ok(false),
            Err(ProviderError::Cancelled) => Err("Managed generation cancelled before startup.".into()),
            Err(_) => Ok(false),
        }
    }

    fn generate(
        &self,
        endpoint: SocketAddr,
        prompt: &DraftPrompt,
        model: &str,
        cancelled: &AtomicBool,
    ) -> Result<String, ProviderError> {
        let body = serde_json::json!({
            "model": model,
            "stream": false,
            "messages": [
                { "role": "system", "content": prompt.system },
                { "role": "user", "content": prompt.user },
            ],
        })
        .to_string();
        let (status, body) = http_exchange(
            endpoint,
            "POST",
            "/v1/chat/completions",
            Some(&body),
            cancelled,
            crate::ai_assist::GENERATION_TIMEOUT,
        )?;
        if status != 200 {
            return Err(ProviderError::Http(status));
        }
        let parsed: serde_json::Value =
            serde_json::from_str(&body).map_err(|_| ProviderError::Malformed)?;
        parsed
            .get("choices")
            .and_then(|choices| choices.as_array())
            .and_then(|choices| choices.first())
            .and_then(|choice| choice.get("message"))
            .and_then(|message| message.get("content"))
            .and_then(|content| content.as_str())
            .filter(|content| !content.trim().is_empty())
            .map(str::to_string)
            .ok_or_else(|| ProviderError::Unsupported("no usable message content".into()))
    }
}

fn http_exchange(
    endpoint: SocketAddr,
    method: &str,
    path: &str,
    body: Option<&str>,
    cancelled: &AtomicBool,
    timeout: Duration,
) -> Result<(u16, String), ProviderError> {
    if cancelled.load(Ordering::SeqCst) {
        return Err(ProviderError::Cancelled);
    }
    let mut stream = TcpStream::connect_timeout(&endpoint, Duration::from_secs(2))
        .map_err(|error| ProviderError::Unavailable(error.to_string()))?;
    stream
        .set_read_timeout(Some(IO_POLL))
        .map_err(|error| ProviderError::Unavailable(error.to_string()))?;
    stream
        .set_write_timeout(Some(IO_POLL))
        .map_err(|error| ProviderError::Unavailable(error.to_string()))?;
    let payload = body.unwrap_or_default();
    let request = format!(
        "{method} {path} HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
        endpoint.port(),
        payload.len()
    );
    for chunk in request.as_bytes().chunks(4096) {
        if cancelled.load(Ordering::SeqCst) {
            return Err(ProviderError::Cancelled);
        }
        stream
            .write_all(chunk)
            .map_err(|error| ProviderError::Unavailable(error.to_string()))?;
    }
    let started = Instant::now();
    let mut response = Vec::new();
    let mut buffer = [0u8; 4096];
    loop {
        if cancelled.load(Ordering::SeqCst) {
            return Err(ProviderError::Cancelled);
        }
        if started.elapsed() >= timeout {
            return Err(ProviderError::Timeout);
        }
        match stream.read(&mut buffer) {
            Ok(0) => break,
            Ok(count) => {
                response.extend_from_slice(&buffer[..count]);
                if response.len() > MAX_HTTP_RESPONSE {
                    return Err(ProviderError::Oversized);
                }
            }
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) => {}
            Err(error) => return Err(ProviderError::Unavailable(error.to_string())),
        }
    }
    let split = response
        .windows(4)
        .position(|part| part == b"\r\n\r\n")
        .ok_or(ProviderError::Malformed)?;
    let head = std::str::from_utf8(&response[..split]).map_err(|_| ProviderError::Malformed)?;
    if head.to_ascii_lowercase().contains("transfer-encoding: chunked") {
        return Err(ProviderError::Unsupported(
            "chunked managed responses are outside the qualified contract".into(),
        ));
    }
    let status = head
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|value| value.parse::<u16>().ok())
        .ok_or(ProviderError::Malformed)?;
    if (300..400).contains(&status) {
        return Err(ProviderError::Redirected);
    }
    let body = String::from_utf8(response[(split + 4)..].to_vec())
        .map_err(|_| ProviderError::Malformed)?;
    Ok((status, body))
}

struct RunningRuntime {
    process: Box<dyn RuntimeProcess>,
    endpoint: SocketAddr,
    model_id: String,
    started_ms: u64,
}

#[derive(Default)]
struct Lifecycle {
    runtime: Option<RunningRuntime>,
    active_requests: usize,
    last_activity_ms: u64,
    /// Last CPU totals behind ticket 190's live percent: (serving model,
    /// total CPU ms, sample wall ms). Reset when the runtime stops or switches
    /// so a new process never inherits the old one's delta.
    last_usage: Option<(String, u64, u64)>,
}

pub struct ManagedAi {
    root: PathBuf,
    catalog_json: &'static str,
    downloader: Arc<dyn Downloader>,
    host: Arc<dyn HostAdapter>,
    transport: Arc<dyn LocalTransport>,
    clock: Arc<dyn Clock>,
    lifecycle: Mutex<Lifecycle>,
    cancellations: Mutex<HashMap<String, Arc<AtomicBool>>>,
    installing: AtomicBool,
    install_cancelled: AtomicBool,
    exiting: AtomicBool,
}

impl ManagedAi {
    pub fn new(root: PathBuf) -> Self {
        Self::with_adapters(
            root,
            CATALOG_JSON,
            Arc::new(HttpDownloader),
            Arc::new(WindowsHost),
            Arc::new(LoopbackTransport),
            Arc::new(SystemClock::new()),
        )
    }

    fn with_adapters(
        root: PathBuf,
        catalog_json: &'static str,
        downloader: Arc<dyn Downloader>,
        host: Arc<dyn HostAdapter>,
        transport: Arc<dyn LocalTransport>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            root,
            catalog_json,
            downloader,
            host,
            transport,
            clock,
            lifecycle: Mutex::new(Lifecycle::default()),
            cancellations: Mutex::new(HashMap::new()),
            installing: AtomicBool::new(false),
            install_cancelled: AtomicBool::new(false),
            exiting: AtomicBool::new(false),
        }
    }

    pub fn start_idle_guard(this: &Arc<Self>) {
        let weak = Arc::downgrade(this);
        std::thread::spawn(move || idle_guard(weak));
    }

    pub fn catalog_status(&self) -> Result<ManagedCatalogView, String> {
        let catalog = Catalog::parse(self.catalog_json)?;
        let runtime_qualified = catalog.managed_runtime.status == "qualified"
            && catalog.managed_runtime.verified.complete();
        let mut models: Vec<ManagedModelView> = catalog
            .tiers
            .iter()
            .map(|model| {
                let installable = runtime_qualified && catalog.qualified(&model.id).is_ok();
                let installed_dir = self
                    .read_installed(&model.id)
                    .ok()
                    .map(|(_, dir)| dir.to_string_lossy().into_owned());
                ManagedModelView {
                    id: model.id.clone(),
                    intent: model.intent.clone(),
                    artifact: model.candidate_artifact.clone(),
                    source: model.artifact_source.clone(),
                    revision: model.revision.clone(),
                    quantization: model.quantization.clone(),
                    sha256: model.download_hash.clone(),
                    download_size_bytes: model.download_size_bytes,
                    license: model.license.clone(),
                    license_source: model.license_source.clone(),
                    status: model.status.clone(),
                    blocker: model.blocker.clone(),
                    context_limit_tokens: model.context_limit_tokens,
                    template_requirements: model.template_requirements.clone(),
                    memory_needs_mb: model.memory_needs_mb,
                    minimum_runtime_version: model.minimum_runtime_version.clone(),
                    installable,
                    installed: installed_dir.is_some(),
                    installed_dir,
                }
            })
            .collect();
        // WHY surface orphans instead of hiding them (ADR-0032): a new app
        // release may recommend a different model without downloading,
        // switching, or deleting anything installed. An installed model
        // missing from the new list stays identifiable with a deliberate
        // recovery path — it is never silently remapped or dropped.
        for orphan_id in self.installed_ids() {
            if catalog.tiers.iter().any(|entry| entry.id == orphan_id) {
                continue;
            }
            let (manifest, dir) = match self.read_installed(&orphan_id) {
                Ok(found) => found,
                Err(_) => continue,
            };
            models.push(ManagedModelView {
                id: orphan_id,
                intent: "Previously installed managed model; this release no longer recommends it.".into(),
                artifact: manifest.model_name.clone(),
                source: String::new(),
                revision: Some(manifest.revision.clone()),
                quantization: None,
                sha256: Some(manifest.model_sha256.clone()),
                download_size_bytes: None,
                license: String::new(),
                license_source: String::new(),
                status: "removed-from-recommendations".into(),
                blocker: "This release no longer recommends the installed model. Keep using it, or remove it explicitly to free its files. Sprout never switches or deletes automatically.".into(),
                context_limit_tokens: Some(manifest.context_limit_tokens),
                template_requirements: Some(manifest.template_requirements.clone()),
                memory_needs_mb: Some(manifest.memory_needs_mb),
                minimum_runtime_version: Some(manifest.runtime_version.clone()),
                installable: false,
                installed: true,
                installed_dir: Some(dir.to_string_lossy().into_owned()),
            });
        }
        Ok(ManagedCatalogView {
            schema_version: catalog.schema_version,
            note: catalog.note,
            runtime: ManagedRuntimeView {
                name: catalog.managed_runtime.name.clone(),
                version: catalog.managed_runtime.candidate_version.clone(),
                artifact: catalog.managed_runtime.windows_cpu_x64_artifact.clone(),
                source: catalog.managed_runtime.source.clone(),
                license: catalog.managed_runtime.license.clone(),
                status: catalog.managed_runtime.status.clone(),
                blocker: catalog.managed_runtime.blocker.clone(),
                qualified: runtime_qualified,
                download_size_bytes: catalog.managed_runtime.size_bytes,
            },
            models,
        })
    }

    pub fn install(&self, id: &str) -> Result<ManagedInstallResult, String> {
        self.install_with_progress(id, &|_| {})
    }

    /// Stages and activates one qualified selection, reporting per-phase byte
    /// progress through `on_progress` (ticket 193). The callback fires on the
    /// installing thread; completion and failure arrive through the return
    /// value, never the callback. Single-flight stays in-memory: a second
    /// concurrent install is refused and staged files are never activated on
    /// failure or cancellation.
    pub fn install_with_progress(
        &self,
        id: &str,
        on_progress: &dyn Fn(ManagedInstallProgress),
    ) -> Result<ManagedInstallResult, String> {
        if self
            .installing
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return Err("A managed installation is already in progress.".into());
        }
        self.install_cancelled.store(false, Ordering::SeqCst);
        let result = self.install_inner(id, on_progress);
        self.installing.store(false, Ordering::SeqCst);
        result
    }

    /// Whether an install is in flight right now (ticket 193): the in-memory
    /// single-flight flag the frontend reconciles its app-level store
    /// against after a (re)mount. Lost on app restart by design — a fresh
    /// process has no flight to report.
    pub fn is_installing(&self) -> bool {
        self.installing.load(Ordering::SeqCst)
    }

    /// Names a silent post-download step before it starts, carrying full phase
    /// totals so the bar holds at 100% while the copy moves on. Checks
    /// cancellation first, so a Cancel racing the download's last byte lands
    /// without paying for a gigabyte hash.
    fn emit_stage(
        &self,
        id: &str,
        on_progress: &dyn Fn(ManagedInstallProgress),
        stage: &str,
        phase: &str,
        total_bytes: u64,
    ) -> Result<(), String> {
        if self.install_cancelled.load(Ordering::SeqCst) {
            return Err("Managed installation cancelled; no staged artifact was activated.".into());
        }
        on_progress(ManagedInstallProgress {
            model_id: id.into(),
            phase: phase.into(),
            stage: stage.into(),
            downloaded_bytes: total_bytes,
            total_bytes,
        });
        Ok(())
    }

    /// The cheap running indicator behind ticket 189's status line: process
    /// liveness plus the serving model and its uptime. A runtime that exited
    /// on its own reads as stopped (and is dropped so the next Start or
    /// Generate launches fresh); an unqueryable process reads as running
    /// rather than claiming a stop that may not have happened.
    pub fn runtime_status(&self) -> ManagedRunState {
        let mut state = match self.lifecycle.lock() {
            Ok(state) => state,
            Err(_) => {
                return ManagedRunState {
                    running: false,
                    active_model_id: None,
                    uptime_secs: None,
                }
            }
        };
        let Some(runtime) = state.runtime.as_mut() else {
            return ManagedRunState {
                running: false,
                active_model_id: None,
                uptime_secs: None,
            };
        };
        match runtime.process.exited() {
            Ok(None) => ManagedRunState {
                running: true,
                active_model_id: Some(runtime.model_id.clone()),
                uptime_secs: Some(
                    self.clock
                        .now_ms()
                        .saturating_sub(runtime.started_ms)
                        .saturating_div(1000),
                ),
            },
            Ok(Some(_)) => {
                state.runtime = None;
                state.last_usage = None;
                ManagedRunState {
                    running: false,
                    active_model_id: None,
                    uptime_secs: None,
                }
            }
            Err(_) => ManagedRunState {
                running: true,
                active_model_id: Some(runtime.model_id.clone()),
                uptime_secs: None,
            },
        }
    }

    /// Tooltip-grade live numbers behind ticket 190's lazy Details disclosure:
    /// working set plus CPU % plus uptime for the owned runtime. Closed costs
    /// nothing (the frontend never calls this); open + running polls ~2s; open
    /// + stopped reads as stopped with no process query; close cancels on the
    /// frontend by dropping its interval. A runtime that exited on its own
    /// reads as stopped (and is dropped); an unqueryable process reads as
    /// running with no numbers rather than claiming a stop.
    pub fn resource_usage(&self) -> ManagedResourceUsage {
        // WHY snapshot-then-query: process_usage shells to PowerShell
        // (seconds on a slow machine), and holding the mutex across it wedged
        // Stop, status and every other command behind each open Details poll.
        // The slot is re-validated before last_usage is touched, so a Stop or
        // a newer server landing mid-query never corrupts the CPU delta.
        let (model_id, pid, started_ms, uptime_secs) = {
            let mut state = match self.lifecycle.lock() {
                Ok(state) => state,
                Err(_) => return stopped_usage(),
            };
            let Some(runtime) = state.runtime.as_mut() else {
                state.last_usage = None;
                return stopped_usage();
            };
            match runtime.process.exited() {
                Ok(None) => {}
                Ok(Some(_)) => {
                    state.runtime = None;
                    state.last_usage = None;
                    return stopped_usage();
                }
                Err(_) => {
                    return ManagedResourceUsage {
                        running: true,
                        active_model_id: Some(runtime.model_id.clone()),
                        uptime_secs: None,
                        working_set_bytes: None,
                        cpu_percent: None,
                    };
                }
            }
            (
                runtime.model_id.clone(),
                runtime.process.pid(),
                runtime.started_ms,
                self.clock
                    .now_ms()
                    .saturating_sub(runtime.started_ms)
                    .saturating_div(1000),
            )
        };
        let now_ms = self.clock.now_ms();
        let (working_set_bytes, total_cpu_ms) = match self.host.process_usage(pid) {
            Ok(values) => values,
            Err(_) => {
                return ManagedResourceUsage {
                    running: true,
                    active_model_id: Some(model_id),
                    uptime_secs: Some(uptime_secs),
                    working_set_bytes: None,
                    cpu_percent: None,
                };
            }
        };
        let cpu_percent = match self.lifecycle.lock() {
            Ok(mut state) => {
                let still_ours = state.runtime.as_ref().is_some_and(|runtime| {
                    runtime.model_id == model_id && runtime.started_ms == started_ms
                });
                if !still_ours {
                    return stopped_usage();
                }
                match state.last_usage.clone() {
                    Some((last_model, last_cpu_ms, last_sample_ms))
                        if last_model == model_id && now_ms > last_sample_ms =>
                    {
                        let wall_ms = now_ms.saturating_sub(last_sample_ms);
                        let cpu_delta_ms = total_cpu_ms.saturating_sub(last_cpu_ms);
                        state.last_usage =
                            Some((model_id.clone(), total_cpu_ms, now_ms));
                        if wall_ms == 0 {
                            None
                        } else {
                            Some((cpu_delta_ms as f64 / wall_ms as f64) * 100.0)
                        }
                    }
                    _ => {
                        state.last_usage =
                            Some((model_id.clone(), total_cpu_ms, now_ms));
                        None
                    }
                }
            }
            Err(_) => None,
        };
        ManagedResourceUsage {
            running: true,
            active_model_id: Some(model_id),
            uptime_secs: Some(uptime_secs),
            working_set_bytes: Some(working_set_bytes),
            cpu_percent,
        }
    }

    /// Pre-warms the owned runtime for one installed model without touching
    /// provider selection (ticket 189): the same health path Generate uses,
    /// so metrics read live immediately after Start. The request is released
    /// at once — the runtime stays up under the idle reap, ready for the
    /// next Generate. A different model serving an active request honestly
    /// refuses instead of switching underneath inference.
    pub fn start_model(self: &Arc<Self>, id: &str) -> Result<String, String> {
        let id = id.trim();
        if id.is_empty() {
            return Err("Pick an installed managed model to start.".into());
        }
        self.read_installed(id)?;
        let request_id = format!("start-prewarm-{}-{}", safe_segment(id), self.clock.now_ms());
        let request = self.begin_request(&request_id, id)?;
        drop(request);
        Ok(id.into())
    }

    pub fn cancel_install(&self) -> bool {
        if self.installing.load(Ordering::SeqCst) {
            self.install_cancelled.store(true, Ordering::SeqCst);
            true
        } else {
            false
        }
    }

    fn install_inner(
        &self,
        id: &str,
        on_progress: &dyn Fn(ManagedInstallProgress),
    ) -> Result<ManagedInstallResult, String> {
        let selection = Catalog::parse(self.catalog_json)?.qualified(id)?;
        if self.read_installed(id).is_ok() {
            return Ok(ManagedInstallResult {
                model_id: id.into(),
                installed: true,
                message: "The verified managed model is already installed for this user.".into(),
            });
        }
        fs::create_dir_all(&self.root)
            .map_err(|error| format!("Could not create the per-user managed AI folder: {error}"))?;
        let runtime_bytes = selection.runtime.size_bytes.unwrap_or_default();
        let model_bytes = selection.model.download_size_bytes.unwrap_or_default();
        let required_disk = runtime_bytes
            .saturating_add(model_bytes)
            .saturating_mul(2);
        let available = self.host.disk_bytes(&self.root)?;
        if available < required_disk {
            return Err(format!(
                "Managed setup needs {} bytes free to stage and activate verified artifacts; only {} bytes are available.",
                required_disk, available
            ));
        }
        let required_memory = selection.model.memory_needs_mb.unwrap_or_default();
        let available_memory = self.host.memory_mb()?;
        if available_memory < required_memory {
            return Err(format!(
                "This qualified model needs {} MB of working RAM; this PC reports {} MB. Download size is not a RAM requirement.",
                required_memory, available_memory
            ));
        }
        // WHY fail fast here (not after gigabytes download): a previous
        // attempt that died before activation leaves its revision directory
        // behind, and retrying without an explicit remove would otherwise
        // re-download everything only to hit the same guard. The same check
        // runs again after staging as a race backstop; both keep the
        // `already occupies this model revision` predicate the frontend's
        // guard-hit recovery matches on, and neither deletes.
        let revision = selection.model.revision.clone().unwrap_or_default();
        let early_final_dir = self
            .root
            .join("installations")
            .join(safe_segment(id))
            .join(safe_segment(&revision));
        if early_final_dir.exists() {
            let incompatible = fs::read(early_final_dir.join(INSTALL_MANIFEST))
                .ok()
                .and_then(|bytes| serde_json::from_slice::<InstalledManifest>(&bytes).ok())
                .is_some_and(|manifest| {
                    manifest.schema_version != 1 || manifest.model_id != id
                });
            if incompatible {
                return Err(
                    "An incompatible installation already occupies this model revision. Nothing was replaced; update Sprout, or remove that app-owned selection explicitly and retry."
                        .into(),
                );
            }
            return Err(
                "An incomplete installation already occupies this model revision. Nothing was replaced; remove that app-owned revision explicitly and retry, or keep the files."
                    .into(),
            );
        }
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default();
        let staging = self.root.join("staging").join(format!("{}-{nonce}", safe_segment(id)));
        fs::create_dir_all(&staging)
            .map_err(|error| format!("Could not create a managed staging folder: {error}"))?;
        let mut cleanup = StagingCleanup(Some(staging.clone()));
        let runtime_archive = staging.join(&selection.runtime.windows_cpu_x64_artifact);
        let model_file_name = file_name_from_url(
            selection.model.download_url.as_deref().unwrap_or_default(),
            "model.gguf",
        );
        let staged_model = staging.join(&model_file_name);
        self.downloader.fetch(
            selection.runtime.download_url.as_deref().unwrap_or_default(),
            &runtime_archive,
            runtime_bytes,
            &self.install_cancelled,
            &|downloaded| {
                on_progress(ManagedInstallProgress {
                    model_id: id.into(),
                    phase: "runtime".into(),
                    stage: "downloading".into(),
                    downloaded_bytes: downloaded,
                    total_bytes: runtime_bytes,
                });
            },
        )?;
        self.emit_stage(id, on_progress, "verifying-runtime", "runtime", runtime_bytes)?;
        verify_file(
            &runtime_archive,
            runtime_bytes,
            selection.runtime.sha256.as_deref().unwrap_or_default(),
            &self.install_cancelled,
        )
        .map_err(|error| format!("Verifying runtime failed: {error}"))?;
        self.downloader.fetch(
            selection.model.download_url.as_deref().unwrap_or_default(),
            &staged_model,
            model_bytes,
            &self.install_cancelled,
            &|downloaded| {
                on_progress(ManagedInstallProgress {
                    model_id: id.into(),
                    phase: "model".into(),
                    stage: "downloading".into(),
                    downloaded_bytes: downloaded,
                    total_bytes: model_bytes,
                });
            },
        )?;
        self.emit_stage(id, on_progress, "verifying-model", "model", model_bytes)?;
        verify_file(
            &staged_model,
            model_bytes,
            selection.model.download_hash.as_deref().unwrap_or_default(),
            &self.install_cancelled,
        )
        .map_err(|error| format!("Verifying model failed: {error}"))?;
        self.emit_stage(id, on_progress, "extracting", "model", model_bytes)?;
        let runtime_dir = staging.join("runtime");
        fs::create_dir_all(&runtime_dir)
            .map_err(|error| format!("Could not stage the managed runtime: {error}"))?;
        self.host
            .extract_runtime(&runtime_archive, &runtime_dir)
            .map_err(|error| format!("Extracting runtime failed: {error}"))?;
        let executable = selection.runtime.executable.as_deref().unwrap_or_default();
        if !safe_relative(executable) || !runtime_dir.join(executable).is_file() {
            return Err("The verified runtime archive does not contain its qualified executable.".into());
        }
        let revision = selection.model.revision.clone().unwrap_or_default();
        let final_dir = self
            .root
            .join("installations")
            .join(safe_segment(id))
            .join(safe_segment(&revision));
        if final_dir.exists() {
            // WHY split the copy (ticket 193): a previous attempt that died
            // before activation leaves a revision directory with no readable
            // manifest — retry (after explicit remove) can finish it. A
            // directory whose manifest parses under another schema came from
            // an incompatible Sprout and needs an app update, not a retry.
            // Both keep the `already occupies this model revision` predicate
            // the frontend's guard-hit recovery matches on; neither deletes.
            let incompatible = fs::read(final_dir.join(INSTALL_MANIFEST))
                .ok()
                .and_then(|bytes| serde_json::from_slice::<InstalledManifest>(&bytes).ok())
                .is_some_and(|manifest| {
                    manifest.schema_version != 1 || manifest.model_id != id
                });
            if incompatible {
                return Err(
                    "An incompatible installation already occupies this model revision. Nothing was replaced; update Sprout, or remove that app-owned selection explicitly and retry."
                        .into(),
                );
            }
            return Err(
                "An incomplete installation already occupies this model revision. Nothing was replaced; remove that app-owned revision explicitly and retry, or keep the files."
                    .into(),
            );
        }
        let manifest = InstalledManifest {
            schema_version: 1,
            model_id: id.into(),
            revision,
            model_name: selection.model.model_name.unwrap_or_default(),
            model_file: model_file_name,
            model_sha256: selection.model.download_hash.unwrap_or_default(),
            runtime_version: selection.runtime.candidate_version,
            runtime_executable: format!("runtime/{executable}"),
            runtime_args: selection.runtime.launch_args.unwrap_or_default(),
            runtime_sha256: selection.runtime.sha256.unwrap_or_default(),
            context_limit_tokens: selection.model.context_limit_tokens.unwrap_or_default(),
            template_requirements: selection.model.template_requirements.unwrap_or_default(),
            memory_needs_mb: required_memory,
            idle_seconds: IDLE_INTERVAL.as_secs(),
        };
        fs::write(
            staging.join(INSTALL_MANIFEST),
            serde_json::to_vec_pretty(&manifest)
                .map_err(|error| format!("Could not record the managed installation: {error}"))?,
        )
        .map_err(|error| format!("Could not record the managed installation: {error}"))?;
        fs::create_dir_all(final_dir.parent().unwrap_or(&self.root))
            .map_err(|error| format!("Could not activate the managed installation: {error}"))?;
        self.emit_stage(id, on_progress, "activating", "model", model_bytes)?;
        if let Err(first) = fs::rename(&staging, &final_dir) {
            // WHY one retry (ADR-0031 owns this lifetime end to end): a fresh
            // gigabyte file can be briefly locked by antivirus or the indexer,
            // and remove() already treats that lag as transient rather than a
            // failure. Staging cleanup on drop still reclaims on a real error.
            std::thread::sleep(Duration::from_millis(500));
            fs::rename(&staging, &final_dir).map_err(|error| {
                format!("Could not activate the verified managed installation: {error} First attempt: {first}")
            })?;
        }
        cleanup.0 = None;
        Ok(ManagedInstallResult {
            model_id: id.into(),
            installed: true,
            message: "Installed verified runtime and model artifacts for this user. The runtime stays stopped until Generate is used.".into(),
        })
    }

    /// Removes one explicitly selected app-owned managed model installation.
    ///
    /// Only the directory under the app-owned root for a catalog-listed id is
    /// ever deleted, so a user-managed service or model elsewhere on disk is
    /// unreachable here (ADR-0031 keeps owned and foreign installations apart).
    /// The owned runtime serving this model is stopped before deletion, since
    /// Windows holds its executable open while it runs; an in-flight request
    /// refuses instead of deleting files from underneath inference.
    pub fn remove(&self, id: &str) -> Result<ManagedRemoveResult, String> {
        let id = id.trim();
        if id.is_empty() {
            return Err("Pick an installed managed model to remove.".into());
        }
        if self.installing.load(Ordering::SeqCst) {
            return Err(
                "A managed installation is already in progress — wait for it to finish before removing anything.".into(),
            );
        }
        let catalog = Catalog::parse(self.catalog_json)?;
        let listed = catalog.tiers.iter().any(|entry| entry.id == id);
        // WHY allow unlisted-but-installed ids (ADR-0032): a release that
        // drops a recommendation must not strand its files — the owner can
        // still remove them explicitly. Unknown, never-installed ids refuse
        // without touching owned files.
        if !listed && self.read_installed(id).is_err() {
            return Err(
                "That managed model is not in this build's bundled catalog.".to_string(),
            );
        }
        let model_root = self
            .root
            .join("installations")
            .join(safe_segment(id));
        if !model_root.starts_with(&self.root) {
            return Err("That managed model path is outside the app-owned folder.".into());
        }
        {
            let mut state = self
                .lifecycle
                .lock()
                .map_err(|_| "Managed AI state is unavailable.".to_string())?;
            let serves_this = state
                .runtime
                .as_ref()
                .is_some_and(|runtime| runtime.model_id == id);
            if serves_this && state.active_requests > 0 {
                return Err("That managed model is serving a generation request — cancel it and try again. Nothing was removed.".into());
            }
            if serves_this {
                stop_runtime(&mut state);
            }
        }
        if !model_root.exists() {
            return Ok(ManagedRemoveResult {
                model_id: id.into(),
                removed: false,
                remaining_installed: self.installed_count(&catalog),
                message: "That managed model is not installed.".into(),
            });
        }
        if let Err(first) = fs::remove_dir_all(&model_root) {
            // Stopping the process releases its file locks a beat later, so a
            // single retry avoids surfacing that lag as a removal failure.
            std::thread::sleep(Duration::from_millis(500));
            fs::remove_dir_all(&model_root).map_err(|error| {
                format!(
                    "Could not remove the managed model files ({error}); the owned runtime was stopped. Close any program holding the folder and try again. First attempt: {first}"
                )
            })?;
        }
        let installations = self.root.join("installations");
        if installations
            .read_dir()
            .map(|mut entries| entries.next().is_none())
            .unwrap_or(false)
        {
            let _ = fs::remove_dir(&installations);
        }
        // WHY sweep our own staging prefix here (ticket 186): a cancelled or
        // crashed install leaves `staging/<id>-<nonce>/` partial downloads
        // behind, and removal is the deliberate moment the user asked to
        // reclaim this model's app-owned space. Only this id's prefix is
        // swept — never another retained model's staging, never anything
        // outside the app-owned root — and an install in flight already
        // refused above, so no live staging is touched.
        let staging = self.root.join("staging");
        if staging.starts_with(&self.root) {
            let prefix = format!("{}-", safe_segment(id));
            if let Ok(entries) = fs::read_dir(&staging) {
                for entry in entries.flatten() {
                    let name = entry.file_name().to_string_lossy().into_owned();
                    if name.starts_with(&prefix) {
                        let _ = fs::remove_dir_all(entry.path());
                    }
                }
            }
            if staging
                .read_dir()
                .map(|mut entries| entries.next().is_none())
                .unwrap_or(false)
            {
                let _ = fs::remove_dir(&staging);
            }
        }
        let remaining = self.installed_count(&catalog);
        Ok(ManagedRemoveResult {
            model_id: id.into(),
            removed: true,
            remaining_installed: remaining,
            message: if remaining == 0 {
                "Removed the managed model and its owned runtime files. No managed model remains installed."
                    .into()
            } else {
                "Removed the managed model and its exclusive runtime files. Models you keep are untouched."
                    .into()
            },
        })
    }

    fn installed_count(&self, catalog: &Catalog) -> usize {
        let mut count = catalog
            .tiers
            .iter()
            .filter(|model| self.read_installed(&model.id).is_ok())
            .count();
        // Orphaned installs (removed from a newer recommendation list) still
        // count — they occupy the same app-owned inventory.
        for orphan_id in self.installed_ids() {
            if !catalog.tiers.iter().any(|entry| entry.id == orphan_id) {
                count += 1;
            }
        }
        count
    }

    /// Every app-owned installed model id, whether or not the bundled
    /// recommendation list still names it. The inventory lives on disk apart
    /// from the release's recommendation list, so a new release can recommend
    /// a different model without downloading, switching, or deleting anything
    /// installed (ADR-0032).
    fn installed_ids(&self) -> Vec<String> {
        let installations = self.root.join("installations");
        let entries = match fs::read_dir(&installations) {
            Ok(entries) => entries,
            Err(_) => return Vec::new(),
        };
        let mut ids = Vec::new();
        for entry in entries.flatten() {
            let id = entry.file_name().to_string_lossy().into_owned();
            if self.read_installed(&id).is_ok() {
                ids.push(id);
            }
        }
        ids.sort();
        ids
    }

    /// Stops the owned runtime and cancels in-flight owned requests without
    /// marking the manager exiting, so leaving the managed route releases
    /// model memory promptly instead of waiting out the idle reap (ADR-0031:
    /// the app owns this server's lifetime end to end).
    pub fn stop_for_disable(&self) {
        if let Ok(cancellations) = self.cancellations.lock() {
            for cancellation in cancellations.values() {
                cancellation.store(true, Ordering::SeqCst);
            }
        }
        if let Ok(mut state) = self.lifecycle.lock() {
            stop_runtime(&mut state);
        }
    }

    fn read_installed(&self, id: &str) -> Result<(InstalledManifest, PathBuf), String> {
        let model_root = self.root.join("installations").join(safe_segment(id));
        let entries = fs::read_dir(&model_root)
            .map_err(|_| "Install this qualified managed model before generating.".to_string())?;
        for entry in entries.flatten() {
            let path = entry.path();
            let manifest_path = path.join(INSTALL_MANIFEST);
            let Ok(bytes) = fs::read(&manifest_path) else {
                continue;
            };
            let Ok(manifest) = serde_json::from_slice::<InstalledManifest>(&bytes) else {
                continue;
            };
            if manifest.schema_version == 1
                && manifest.model_id == id
                && safe_relative(&manifest.model_file)
                && safe_relative(&manifest.runtime_executable)
                && path.join(&manifest.model_file).is_file()
                && path.join(&manifest.runtime_executable).is_file()
            {
                return Ok((manifest, path));
            }
        }
        Err("Install this qualified managed model before generating.".into())
    }

    pub fn begin_request(self: &Arc<Self>, request_id: &str, model_id: &str) -> Result<ManagedRequest, String> {
        if request_id.trim().is_empty() {
            return Err("Managed generation needs a request id for cancellation.".into());
        }
        if self.exiting.load(Ordering::SeqCst) {
            return Err("Sprout is exiting; managed generation did not start.".into());
        }
        let (manifest, install_dir) = self.read_installed(model_id)?;
        let cancelled = Arc::new(AtomicBool::new(false));
        {
            let mut cancellations = self
                .cancellations
                .lock()
                .map_err(|_| "Managed AI cancellation state is unavailable.".to_string())?;
            if cancellations.contains_key(request_id) {
                return Err("That managed generation request is already active.".into());
            }
            cancellations.insert(request_id.into(), Arc::clone(&cancelled));
        }
        let endpoint = {
            let mut state = match self.lifecycle.lock() {
                Ok(state) => state,
                Err(_) => {
                    self.remove_cancellation(request_id);
                    return Err("Managed AI state is unavailable.".into());
                }
            };
            // WHY reserve under one brief lock and poll without it: the health
            // poll below waits up to 30 seconds, and holding the mutex across
            // it wedged Stop, status and usage behind every startup.
            let slot = match prepare_runtime_slot(
                &mut state,
                model_id,
                &manifest,
                &install_dir,
                self.host.as_ref(),
                self.clock.as_ref(),
            ) {
                Ok(slot) => slot,
                Err(error) => {
                    drop(state);
                    self.remove_cancellation(request_id);
                    return Err(error);
                }
            };
            match slot {
                RuntimeSlot::Ready(endpoint) => endpoint,
                RuntimeSlot::Starting {
                    endpoint,
                    started_ms,
                } => {
                    drop(state);
                    if let Err(error) =
                        self.await_healthy(endpoint, started_ms, &cancelled)
                    {
                        self.remove_cancellation(request_id);
                        return Err(error);
                    }
                    endpoint
                }
            }
        };
        Ok(ManagedRequest {
            owner: Arc::clone(self),
            request_id: request_id.into(),
            endpoint,
            model_name: manifest.model_name,
            cancelled,
        })
    }

    pub fn cancel_request(&self, request_id: &str) -> bool {
        let Ok(cancellations) = self.cancellations.lock() else {
            return false;
        };
        let Some(cancelled) = cancellations.get(request_id) else {
            return false;
        };
        cancelled.store(true, Ordering::SeqCst);
        true
    }

    fn remove_cancellation(&self, request_id: &str) -> bool {
        self.cancellations
            .lock()
            .map(|mut cancellations| cancellations.remove(request_id).is_some())
            .unwrap_or(false)
    }

    /// Polls a freshly spawned server until it serves, WITHOUT holding the
    /// lifecycle lock: each iteration takes the lock only to inspect the slot
    /// (microseconds — exit check plus field reads), so Stop, status, usage
    /// and a second request stay live through a 30-second startup. A Stop or
    /// a newer starter that reclaims the slot ends the wait at once instead of
    /// after the timeout.
    fn await_healthy(
        &self,
        endpoint: SocketAddr,
        started_ms: u64,
        cancelled: &AtomicBool,
    ) -> Result<(), String> {
        let started = self.clock.now_ms();
        let mut checks = 0u32;
        loop {
            if cancelled.load(Ordering::SeqCst) {
                self.abandon_startup(endpoint, started_ms);
                return Err("Managed generation cancelled during runtime startup.".into());
            }
            {
                let mut state = self
                    .lifecycle
                    .lock()
                    .map_err(|_| "Managed AI state is unavailable.".to_string())?;
                match state.runtime.as_mut() {
                    Some(runtime)
                        if runtime.endpoint == endpoint
                            && runtime.started_ms == started_ms =>
                    {
                        match runtime.process.exited() {
                            Err(error) => {
                                state.active_requests =
                                    state.active_requests.saturating_sub(1);
                                return Err(error);
                            }
                            Ok(Some(code)) => {
                                state.runtime = None;
                                state.last_usage = None;
                                state.active_requests =
                                    state.active_requests.saturating_sub(1);
                                return Err(format!("Managed runtime exited during startup with code {code}. Try Generate again; no foreign process was touched."));
                            }
                            Ok(None) => {}
                        }
                    }
                    _ => {
                        state.active_requests =
                            state.active_requests.saturating_sub(1);
                        return Err(
                            "Managed runtime disappeared during startup.".to_string()
                        );
                    }
                }
            }
            match self.transport.healthy(endpoint, cancelled) {
                Ok(true) => {
                    if let Ok(mut state) = self.lifecycle.lock() {
                        state.last_activity_ms = self.clock.now_ms();
                    }
                    return Ok(());
                }
                Ok(false) => {}
                Err(error) => {
                    self.abandon_startup(endpoint, started_ms);
                    return Err(error);
                }
            }
            checks += 1;
            if self
                .clock
                .now_ms()
                .saturating_sub(started)
                >= STARTUP_TIMEOUT.as_millis() as u64
            {
                self.abandon_startup(endpoint, started_ms);
                return Err(format!(
                    "Managed runtime did not become healthy in 30 seconds ({checks} checks, process still running). The server started but never served — try Generate again; no foreign process was touched."
                ));
            }
            self.clock.sleep(IO_POLL);
        }
    }

    /// Drops a startup this request no longer waits on: releases its request
    /// hold, and stops the server only if the slot still holds exactly the
    /// spawn it waited on — never a newer server another request started.
    fn abandon_startup(&self, endpoint: SocketAddr, started_ms: u64) {
        if let Ok(mut state) = self.lifecycle.lock() {
            if state.runtime.as_ref().is_some_and(|runtime| {
                runtime.endpoint == endpoint && runtime.started_ms == started_ms
            }) {
                stop_runtime(&mut state);
            }
            state.active_requests = state.active_requests.saturating_sub(1);
        }
    }

    fn finish_request(&self, request_id: &str) {
        if self.remove_cancellation(request_id) {
            if let Ok(mut state) = self.lifecycle.lock() {
                state.active_requests = state.active_requests.saturating_sub(1);
                state.last_activity_ms = self.clock.now_ms();
            }
        }
    }

    pub fn reap_idle(&self) -> bool {
        let Ok(mut state) = self.lifecycle.lock() else {
            return false;
        };
        if state.active_requests != 0
            || state.runtime.is_none()
            || self.clock.now_ms().saturating_sub(state.last_activity_ms)
                < IDLE_INTERVAL.as_millis() as u64
        {
            return false;
        }
        stop_runtime(&mut state);
        true
    }

    pub fn shutdown(&self) {
        self.exiting.store(true, Ordering::SeqCst);
        self.install_cancelled.store(true, Ordering::SeqCst);
        if let Ok(cancellations) = self.cancellations.lock() {
            for cancellation in cancellations.values() {
                cancellation.store(true, Ordering::SeqCst);
            }
        }
        if let Ok(mut state) = self.lifecycle.lock() {
            stop_runtime(&mut state);
        }
    }
}

fn stopped_usage() -> ManagedResourceUsage {
    ManagedResourceUsage {
        running: false,
        active_model_id: None,
        uptime_secs: None,
        working_set_bytes: None,
        cpu_percent: None,
    }
}

fn idle_guard(owner: Weak<ManagedAi>) {
    loop {
        std::thread::sleep(Duration::from_secs(1));
        let Some(owner) = owner.upgrade() else {
            return;
        };
        if owner.exiting.load(Ordering::SeqCst) {
            return;
        }
        owner.reap_idle();
    }
}

/// What the brief slot reservation decided: join a live server, or wait out a
/// fresh spawn's health poll without holding the lock.
enum RuntimeSlot {
    Ready(SocketAddr),
    Starting { endpoint: SocketAddr, started_ms: u64 },
}

fn prepare_runtime_slot(
    state: &mut Lifecycle,
    model_id: &str,
    manifest: &InstalledManifest,
    install_dir: &Path,
    host: &dyn HostAdapter,
    clock: &dyn Clock,
) -> Result<RuntimeSlot, String> {
    if let Some(runtime) = state.runtime.as_mut() {
        match runtime.process.exited()? {
            None if runtime.model_id == model_id => {
                state.active_requests += 1;
                return Ok(RuntimeSlot::Ready(runtime.endpoint));
            }
            None if state.active_requests > 0 => {
                return Err("Another managed model is serving an active request; try again when it finishes.".into())
            }
            None => stop_runtime(state),
            Some(_) => {
                state.runtime = None;
                state.last_usage = None;
            }
        }
    }
    let port = host.reserve_port()?;
    let endpoint = SocketAddr::from(([127, 0, 0, 1], port));
    let executable = install_dir.join(&manifest.runtime_executable);
    let model = install_dir.join(&manifest.model_file);
    let args = manifest
        .runtime_args
        .iter()
        .map(|arg| {
            arg.replace("{port}", &port.to_string())
                .replace("{model}", &model.to_string_lossy())
        })
        .collect::<Vec<_>>();
    let process = host.spawn(&executable, &args)?;
    let started_ms = clock.now_ms();
    state.runtime = Some(RunningRuntime {
        process,
        endpoint,
        model_id: model_id.into(),
        started_ms,
    });
    state.active_requests += 1;
    Ok(RuntimeSlot::Starting {
        endpoint,
        started_ms,
    })
}

fn stop_runtime(state: &mut Lifecycle) {
    if let Some(mut runtime) = state.runtime.take() {
        runtime.process.stop();
    }
    state.last_usage = None;
}

pub struct ManagedRequest {
    owner: Arc<ManagedAi>,
    request_id: String,
    endpoint: SocketAddr,
    model_name: String,
    cancelled: Arc<AtomicBool>,
}

impl DraftProvider for ManagedRequest {
    fn generate(&self, prompt: &DraftPrompt, _model: &str) -> Result<String, ProviderError> {
        self.owner.transport.generate(
            self.endpoint,
            prompt,
            &self.model_name,
            &self.cancelled,
        )
    }
}

impl Drop for ManagedRequest {
    fn drop(&mut self) {
        self.owner.finish_request(&self.request_id);
    }
}

struct StagingCleanup(Option<PathBuf>);

impl Drop for StagingCleanup {
    fn drop(&mut self) {
        if let Some(path) = self.0.take() {
            let _ = fs::remove_dir_all(path);
        }
    }
}

fn safe_segment(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.') {
                character
            } else {
                '_'
            }
        })
        .collect()
}

fn safe_relative(value: &str) -> bool {
    let path = Path::new(value);
    !value.trim().is_empty()
        && !path.is_absolute()
        && path
            .components()
            .all(|part| matches!(part, std::path::Component::Normal(_)))
}

fn file_name_from_url(url: &str, fallback: &str) -> String {
    url.rsplit('/')
        .next()
        .and_then(|part| part.split(['?', '#']).next())
        .filter(|part| !part.is_empty() && safe_relative(part))
        .unwrap_or(fallback)
        .to_string()
}

/// Refuses any download that did not land on HTTPS — including a redirect
/// downgrade — before a single byte is staged. Pure so the boundary is
/// unit-tested without network; the staged bytes are additionally size- and
/// SHA-256-verified by `verify_file`, so a redirect target cannot substitute
/// content undetected either.
fn refuse_unless_https_landing(final_url: &str) -> Result<(), String> {
    if final_url.len() > "https://".len()
        && final_url[..8].eq_ignore_ascii_case("https://")
    {
        return Ok(());
    }
    Err("Managed artifact download left HTTPS; nothing was staged.".into())
}

fn verify_file(
    path: &Path,
    expected_bytes: u64,
    expected_sha256: &str,
    cancelled: &AtomicBool,
) -> Result<(), String> {
    // WHY stream instead of `fs::read` (no ticket, memory safety): a managed
    // model is gigabytes, and reading it whole into a `Vec<u8>` spikes working
    // set until the window stalls or the read fails. Metadata gives the size
    // instantly; the hash streams in 1 MiB chunks with periodic cancellation so
    // Close/Cancel stay live through verification (ADR-0032 verified-before-use
    // holds without the freeze).
    let actual_len = fs::metadata(path)
        .map(|meta| meta.len())
        .map_err(|error| format!("Could not read a staged managed artifact: {error}"))?;
    if actual_len != expected_bytes {
        return Err("A staged managed artifact does not match its qualified byte size.".into());
    }
    let mut file =
        File::open(path).map_err(|error| format!("Could not read a staged managed artifact: {error}"))?;
    let mut hasher = StreamingSha256::new();
    let mut buffer = vec![0u8; 1024 * 1024];
    loop {
        if cancelled.load(Ordering::SeqCst) {
            return Err("Managed installation cancelled; no staged artifact was activated.".into());
        }
        let count = file
            .read(&mut buffer)
            .map_err(|error| format!("Could not read a staged managed artifact: {error}"))?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    let actual = hasher.hex();
    if actual != expected_sha256.to_ascii_lowercase() {
        return Err(format!(
            "Managed artifact checksum mismatch; expected {}, received {}. Nothing was activated.",
            expected_sha256, actual
        ));
    }
    Ok(())
}

/// Incremental SHA-256 over the same compression function as `sha256_hex`,
/// so gigabyte artifacts hash without a whole-file allocation. `update` feeds
/// arbitrary chunks; `hex` pads and finalizes exactly once.
struct StreamingSha256 {
    state: [u32; 8],
    buffer: [u8; 64],
    buffered: usize,
    total_len: u64,
}

impl StreamingSha256 {
    fn new() -> Self {
        Self {
            state: [
                0x6a09e667u32,
                0xbb67ae85,
                0x3c6ef372,
                0xa54ff53a,
                0x510e527f,
                0x9b05688c,
                0x1f83d9ab,
                0x5be0cd19,
            ],
            buffer: [0u8; 64],
            buffered: 0,
            total_len: 0,
        }
    }

    fn update(&mut self, mut data: &[u8]) {
        self.total_len = self.total_len.wrapping_add(data.len() as u64);
        if self.buffered > 0 {
            let take = (64 - self.buffered).min(data.len());
            self.buffer[self.buffered..self.buffered + take].copy_from_slice(&data[..take]);
            self.buffered += take;
            data = &data[take..];
            if self.buffered == 64 {
                let block = self.buffer;
                compress_block(&mut self.state, &block);
                self.buffered = 0;
            }
        }
        while data.len() >= 64 {
            let mut block = [0u8; 64];
            block.copy_from_slice(&data[..64]);
            compress_block(&mut self.state, &block);
            data = &data[64..];
        }
        if !data.is_empty() {
            self.buffer[..data.len()].copy_from_slice(data);
            self.buffered = data.len();
        }
    }

    fn hex(mut self) -> String {
        let bit_len = self.total_len.wrapping_mul(8);
        self.update(&[0x80]);
        while self.buffered != 56 {
            if self.buffered == 64 {
                let block = self.buffer;
                compress_block(&mut self.state, &block);
                self.buffered = 0;
                self.buffer = [0u8; 64];
            } else {
                let pad_len = if self.buffered < 56 {
                    56 - self.buffered
                } else {
                    64 - self.buffered + 56
                };
                // Feed zeros without recursing through `update`'s length
                // accounting (padding is not message bytes).
                let zeros = [0u8; 64];
                let mut remaining = pad_len;
                while remaining > 0 {
                    let take = remaining.min(64 - self.buffered);
                    self.buffer[self.buffered..self.buffered + take]
                        .copy_from_slice(&zeros[..take]);
                    self.buffered += take;
                    remaining -= take;
                    if self.buffered == 64 {
                        let block = self.buffer;
                        compress_block(&mut self.state, &block);
                        self.buffered = 0;
                    }
                }
            }
        }
        let mut tail = [0u8; 64];
        tail[..56].copy_from_slice(&self.buffer[..56]);
        tail[56..].copy_from_slice(&bit_len.to_be_bytes());
        compress_block(&mut self.state, &tail);
        self.state.iter().map(|word| format!("{word:08x}")).collect()
    }
}

fn compress_block(state: &mut [u32; 8], chunk: &[u8; 64]) {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1,
        0x923f82a4, 0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3,
        0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786,
        0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
        0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147,
        0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13,
        0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
        0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a,
        0x5b9cca4f, 0x682e6ff3, 0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208,
        0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
    ];
    let mut words = [0u32; 64];
    for (index, word) in words[..16].iter_mut().enumerate() {
        *word = u32::from_be_bytes(chunk[index * 4..index * 4 + 4].try_into().unwrap());
    }
    for index in 16..64 {
        let s0 = words[index - 15].rotate_right(7)
            ^ words[index - 15].rotate_right(18)
            ^ (words[index - 15] >> 3);
        let s1 = words[index - 2].rotate_right(17)
            ^ words[index - 2].rotate_right(19)
            ^ (words[index - 2] >> 10);
        words[index] = words[index - 16]
            .wrapping_add(s0)
            .wrapping_add(words[index - 7])
            .wrapping_add(s1);
    }
    let mut work = *state;
    for index in 0..64 {
        let sum1 = work[4].rotate_right(6)
            ^ work[4].rotate_right(11)
            ^ work[4].rotate_right(25);
        let choose = (work[4] & work[5]) ^ (!work[4] & work[6]);
        let temp1 = work[7]
            .wrapping_add(sum1)
            .wrapping_add(choose)
            .wrapping_add(K[index])
            .wrapping_add(words[index]);
        let sum0 = work[0].rotate_right(2)
            ^ work[0].rotate_right(13)
            ^ work[0].rotate_right(22);
        let majority = (work[0] & work[1]) ^ (work[0] & work[2]) ^ (work[1] & work[2]);
        let temp2 = sum0.wrapping_add(majority);
        work = [
            temp1.wrapping_add(temp2),
            work[0],
            work[1],
            work[2],
            work[3].wrapping_add(temp1),
            work[4],
            work[5],
            work[6],
        ];
    }
    for index in 0..8 {
        state[index] = state[index].wrapping_add(work[index]);
    }
}

#[cfg(test)]
fn sha256_hex(input: &[u8]) -> String {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1,
        0x923f82a4, 0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3,
        0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786,
        0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
        0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147,
        0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13,
        0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
        0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a,
        0x5b9cca4f, 0x682e6ff3, 0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208,
        0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
    ];
    let mut state = [
        0x6a09e667u32,
        0xbb67ae85,
        0x3c6ef372,
        0xa54ff53a,
        0x510e527f,
        0x9b05688c,
        0x1f83d9ab,
        0x5be0cd19,
    ];
    let bit_len = (input.len() as u64).wrapping_mul(8);
    let mut padded = input.to_vec();
    padded.push(0x80);
    while padded.len() % 64 != 56 {
        padded.push(0);
    }
    padded.extend_from_slice(&bit_len.to_be_bytes());
    for chunk in padded.chunks_exact(64) {
        let mut words = [0u32; 64];
        for (index, word) in words[..16].iter_mut().enumerate() {
            *word = u32::from_be_bytes(chunk[index * 4..index * 4 + 4].try_into().unwrap());
        }
        for index in 16..64 {
            let s0 = words[index - 15].rotate_right(7)
                ^ words[index - 15].rotate_right(18)
                ^ (words[index - 15] >> 3);
            let s1 = words[index - 2].rotate_right(17)
                ^ words[index - 2].rotate_right(19)
                ^ (words[index - 2] >> 10);
            words[index] = words[index - 16]
                .wrapping_add(s0)
                .wrapping_add(words[index - 7])
                .wrapping_add(s1);
        }
        let mut work = state;
        for index in 0..64 {
            let sum1 = work[4].rotate_right(6)
                ^ work[4].rotate_right(11)
                ^ work[4].rotate_right(25);
            let choose = (work[4] & work[5]) ^ (!work[4] & work[6]);
            let temp1 = work[7]
                .wrapping_add(sum1)
                .wrapping_add(choose)
                .wrapping_add(K[index])
                .wrapping_add(words[index]);
            let sum0 = work[0].rotate_right(2)
                ^ work[0].rotate_right(13)
                ^ work[0].rotate_right(22);
            let majority = (work[0] & work[1]) ^ (work[0] & work[2]) ^ (work[1] & work[2]);
            let temp2 = sum0.wrapping_add(majority);
            work = [
                temp1.wrapping_add(temp2),
                work[0],
                work[1],
                work[2],
                work[3].wrapping_add(temp1),
                work[4],
                work[5],
                work[6],
            ];
        }
        for index in 0..8 {
            state[index] = state[index].wrapping_add(work[index]);
        }
    }
    state.iter().map(|word| format!("{word:08x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, AtomicUsize};

    const RUNTIME_BYTES: &[u8] = b"runtime archive";
    const MODEL_BYTES: &[u8] = b"model weights";

    struct FakeDownloader {
        corrupt_model: bool,
        cancelled: bool,
    }

    impl Downloader for FakeDownloader {
        fn fetch(&self, url: &str, target: &Path, _: u64, cancelled: &AtomicBool, on_bytes: &dyn Fn(u64)) -> Result<(), String> {
            if self.cancelled {
                cancelled.store(true, Ordering::SeqCst);
                return Err("Managed installation cancelled; no staged artifact was activated.".into());
            }
            let bytes = if url.ends_with("runtime.zip") {
                RUNTIME_BYTES
            } else if self.corrupt_model {
                b"corrupt"
            } else {
                MODEL_BYTES
            };
            fs::write(target, bytes).map_err(|error| error.to_string())?;
            on_bytes(bytes.len() as u64);
            Ok(())
        }
    }

    struct FakeProcess {
        stops: Arc<AtomicUsize>,
        crashed: bool,
    }

    impl RuntimeProcess for FakeProcess {
        fn exited(&mut self) -> Result<Option<i32>, String> {
            Ok(self.crashed.then_some(9))
        }
        fn pid(&self) -> u32 {
            4242
        }
        fn stop(&mut self) { self.stops.fetch_add(1, Ordering::SeqCst); }
    }

    struct FakeHost {
        spawns: Arc<AtomicUsize>,
        stops: Arc<AtomicUsize>,
        fail_port: bool,
        crash: bool,
    }

    impl HostAdapter for FakeHost {
        fn disk_bytes(&self, _: &Path) -> Result<u64, String> { Ok(u64::MAX) }
        fn memory_mb(&self) -> Result<u64, String> { Ok(u64::MAX) }
        fn extract_runtime(&self, _: &Path, destination: &Path) -> Result<(), String> {
            fs::write(destination.join("llama-server.exe"), b"exe").map_err(|error| error.to_string())
        }
        fn reserve_port(&self) -> Result<u16, String> {
            if self.fail_port { Err("occupied endpoint".into()) } else { Ok(32123) }
        }
        fn spawn(&self, _: &Path, _: &[String]) -> Result<Box<dyn RuntimeProcess>, String> {
            self.spawns.fetch_add(1, Ordering::SeqCst);
            Ok(Box::new(FakeProcess { stops: Arc::clone(&self.stops), crashed: self.crash }))
        }
        fn process_usage(&self, _: u32) -> Result<(u64, u64), String> {
            Ok((256 * 1024 * 1024, 1000))
        }
    }

    /// A usage query that blocks until released, standing in for a slow
    /// PowerShell on a struggling machine.
    struct SlowUsageHost {
        inner: Arc<FakeHost>,
        entered: Arc<AtomicBool>,
        release: Arc<AtomicBool>,
    }

    impl HostAdapter for SlowUsageHost {
        fn disk_bytes(&self, path: &Path) -> Result<u64, String> { self.inner.disk_bytes(path) }
        fn memory_mb(&self) -> Result<u64, String> { self.inner.memory_mb() }
        fn extract_runtime(&self, archive: &Path, destination: &Path) -> Result<(), String> {
            self.inner.extract_runtime(archive, destination)
        }
        fn reserve_port(&self) -> Result<u16, String> { self.inner.reserve_port() }
        fn spawn(&self, executable: &Path, args: &[String]) -> Result<Box<dyn RuntimeProcess>, String> {
            self.inner.spawn(executable, args)
        }
        fn process_usage(&self, _: u32) -> Result<(u64, u64), String> {
            self.entered.store(true, Ordering::SeqCst);
            while !self.release.load(Ordering::SeqCst) {
                std::thread::yield_now();
            }
            Ok((256 * 1024 * 1024, 1000))
        }
    }

    struct FakeClock(AtomicU64);

    impl Clock for FakeClock {
        fn now_ms(&self) -> u64 { self.0.load(Ordering::SeqCst) }
        fn sleep(&self, duration: Duration) { self.0.fetch_add(duration.as_millis() as u64, Ordering::SeqCst); }
    }

    struct FakeTransport {
        healthy: bool,
    }

    impl LocalTransport for FakeTransport {
        fn healthy(&self, _: SocketAddr, cancelled: &AtomicBool) -> Result<bool, String> {
            if cancelled.load(Ordering::SeqCst) { return Err("cancelled".into()); }
            Ok(self.healthy)
        }
        fn generate(&self, _: SocketAddr, _: &DraftPrompt, _: &str, cancelled: &AtomicBool) -> Result<String, ProviderError> {
            if cancelled.load(Ordering::SeqCst) { Err(ProviderError::Cancelled) } else { Ok("{}".into()) }
        }
    }

    struct WaitForCancellationTransport {
        entered: Arc<AtomicBool>,
    }

    impl LocalTransport for WaitForCancellationTransport {
        fn healthy(&self, _: SocketAddr, cancelled: &AtomicBool) -> Result<bool, String> {
            self.entered.store(true, Ordering::SeqCst);
            while !cancelled.load(Ordering::SeqCst) {
                std::thread::yield_now();
            }
            Err("Managed generation cancelled during runtime startup.".into())
        }

        fn generate(&self, _: SocketAddr, _: &DraftPrompt, _: &str, _: &AtomicBool) -> Result<String, ProviderError> {
            unreachable!("startup cancellation never reaches generation")
        }
    }

    fn qualified_catalog() -> &'static str {
        Box::leak(format!(r#"{{
          "schema_version":1,"note":"qualified fixture",
          "managed_runtime":{{"name":"llama.cpp","candidate_version":"v1","windows_cpu_x64_artifact":"runtime.zip","source":"https://example/runtime","license":"MIT","status":"qualified","verified":{{"download":true,"per_user_launch":true,"health":true,"cancellation":true,"model_release":true,"process_ownership":true,"exit_behavior":true}},"blocker":"","download_url":"https://example/runtime.zip","sha256":"{}","size_bytes":{},"executable":"llama-server.exe","launch_args":["--host","127.0.0.1","--port","{{port}}","--model","{{model}}"]}},
          "tiers":[{{"id":"lite","intent":"fixture","candidate_artifact":"lite.gguf","artifact_source":"https://example/model","revision":"rev1","quantization":"Q4","download_hash":"{}","download_size_bytes":{},"license":"Apache-2.0","license_source":"https://example/license","status":"qualified","blocker":"","context_limit_tokens":4096,"template_requirements":"chatml","memory_needs_mb":2048,"minimum_runtime_version":"v1","download_url":"https://example/model.gguf","model_name":"lite"}}]
        }}"#, sha256_hex(RUNTIME_BYTES), RUNTIME_BYTES.len(), sha256_hex(MODEL_BYTES), MODEL_BYTES.len()).into_boxed_str())
    }

    fn fixture(root: PathBuf, downloader: Arc<dyn Downloader>, host: Arc<FakeHost>, clock: Arc<FakeClock>, transport: Arc<dyn LocalTransport>) -> Arc<ManagedAi> {
        Arc::new(ManagedAi::with_adapters(root, qualified_catalog(), downloader, host, transport, clock))
    }

    #[test]
    fn shipped_catalog_qualifies_only_the_measured_lightweight_tier() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("managed");
        let manager = ManagedAi::new(root.clone());
        let status = manager.catalog_status().unwrap();
        assert!(status.runtime.qualified, "the measured runtime ships qualified");
        let lite = status
            .models
            .iter()
            .find(|model| model.id == "lightweight-candidate")
            .expect("the lightweight tier ships");
        assert!(lite.installable, "the measured tier is installable");
        assert!(!lite.installed, "status never installs anything");
        assert_eq!(
            lite.revision.as_deref(),
            Some("f86cb2c1fa58255f8052cc32aeede1b7482d4361"),
            "the pinned revision rides along"
        );
        assert!(lite.sha256.is_some() && lite.download_size_bytes.is_some());
        for model in &status.models {
            if model.id != "lightweight-candidate" {
                assert!(!model.installable, "{} stays non-installable", model.id);
            }
        }
        assert!(!root.exists(), "status creates no install state");
    }

    #[test]
    fn sha256_matches_the_standard_vector() {
        assert_eq!(sha256_hex(b"abc"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    }

    #[test]
    fn https_landing_check_blocks_downgrades() {
        for landing in [
            "https://cdn.example/model.gguf",
            "https://127.0.0.1:9/file?sig=abc",
            "HTTPS://EXAMPLE/UPPER",
        ] {
            assert!(refuse_unless_https_landing(landing).is_ok(), "{landing} stays");
        }
        for landing in [
            "http://cdn.example/model.gguf",
            "https://",
            "https:/typo.example/file",
            "",
            "file:///etc/model.gguf",
        ] {
            assert!(
                refuse_unless_https_landing(landing).is_err(),
                "{landing} must not stage"
            );
        }
    }

    #[test]
    fn non_https_initial_url_is_refused_before_touching_disk() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("never-created.bin");
        let downloader = HttpDownloader;
        let cancelled = AtomicBool::new(false);
        let error = downloader
            .fetch("http://127.0.0.1:9/model.gguf", &target, 3, &cancelled, &|_| {})
            .expect_err("plain HTTP must fail before any request");
        assert!(error.contains("HTTPS"), "actionable, got {error}");
        assert!(!target.exists(), "refused downloads stage nothing");
    }

    #[test]
    fn install_activates_only_after_both_artifacts_verify() {
        let dir = tempfile::tempdir().unwrap();
        let host = Arc::new(FakeHost { spawns: Arc::new(AtomicUsize::new(0)), stops: Arc::new(AtomicUsize::new(0)), fail_port: false, crash: false });
        let manager = fixture(dir.path().join("managed"), Arc::new(FakeDownloader { corrupt_model: false, cancelled: false }), host, Arc::new(FakeClock(AtomicU64::new(0))), Arc::new(FakeTransport { healthy: true }));
        assert!(manager.install("lite").unwrap().installed);
        let (manifest, path) = manager.read_installed("lite").unwrap();
        assert_eq!(manifest.runtime_version, "v1");
        assert!(path.join(INSTALL_MANIFEST).is_file());
        assert!(!manager.root.join("staging").read_dir().map(|mut entries| entries.next().is_some()).unwrap_or(false));
    }

    #[test]
    fn catalog_names_the_installed_folder_once_it_lands() {
        let dir = tempfile::tempdir().unwrap();
        let host = Arc::new(FakeHost { spawns: Arc::new(AtomicUsize::new(0)), stops: Arc::new(AtomicUsize::new(0)), fail_port: false, crash: false });
        let manager = fixture(dir.path().join("managed"), Arc::new(FakeDownloader { corrupt_model: false, cancelled: false }), host, Arc::new(FakeClock(AtomicU64::new(0))), Arc::new(FakeTransport { healthy: true }));
        let before = manager.catalog_status().unwrap();
        assert!(before.models.iter().all(|model| model.installed_dir.is_none()));
        manager.install("lite").unwrap();
        let after = manager.catalog_status().unwrap();
        let lite = after.models.iter().find(|model| model.id == "lite").expect("the tier stays listed");
        let (_, path) = manager.read_installed("lite").unwrap();
        assert_eq!(lite.installed_dir.as_deref(), Some(path.to_string_lossy().as_ref()));
        assert!(Path::new(lite.installed_dir.as_deref().unwrap_or_default()).is_dir());
    }

    #[test]
    fn corrupt_or_cancelled_download_never_activates() {
        for downloader in [
            FakeDownloader { corrupt_model: true, cancelled: false },
            FakeDownloader { corrupt_model: false, cancelled: true },
        ] {
            let dir = tempfile::tempdir().unwrap();
            let host = Arc::new(FakeHost { spawns: Arc::new(AtomicUsize::new(0)), stops: Arc::new(AtomicUsize::new(0)), fail_port: false, crash: false });
            let manager = fixture(dir.path().join("managed"), Arc::new(downloader), host, Arc::new(FakeClock(AtomicU64::new(0))), Arc::new(FakeTransport { healthy: true }));
            assert!(manager.install("lite").is_err());
            assert!(manager.read_installed("lite").is_err());
        }
    }

    #[test]
    fn overlapping_requests_share_one_owned_runtime_and_idle_reaps_it() {
        let dir = tempfile::tempdir().unwrap();
        let spawns = Arc::new(AtomicUsize::new(0));
        let stops = Arc::new(AtomicUsize::new(0));
        let host = Arc::new(FakeHost { spawns: Arc::clone(&spawns), stops: Arc::clone(&stops), fail_port: false, crash: false });
        let clock = Arc::new(FakeClock(AtomicU64::new(0)));
        let manager = fixture(dir.path().join("managed"), Arc::new(FakeDownloader { corrupt_model: false, cancelled: false }), host, Arc::clone(&clock), Arc::new(FakeTransport { healthy: true }));
        manager.install("lite").unwrap();
        let first = manager.begin_request("first", "lite").unwrap();
        let second = manager.begin_request("second", "lite").unwrap();
        assert_eq!(spawns.load(Ordering::SeqCst), 1);
        assert!(manager.cancel_request("first"));
        let prompt = DraftPrompt { system: "system".into(), user: "user".into() };
        assert!(matches!(first.generate(&prompt, "lite"), Err(ProviderError::Cancelled)));
        assert!(!manager.reap_idle());
        drop(first);
        drop(second);
        clock.0.store(IDLE_INTERVAL.as_millis() as u64 + 1, Ordering::SeqCst);
        assert!(manager.reap_idle());
        assert_eq!(stops.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn crashes_and_occupied_endpoints_fail_without_foreign_kills() {
        let dir = tempfile::tempdir().unwrap();
        let stops = Arc::new(AtomicUsize::new(0));
        let host = Arc::new(FakeHost { spawns: Arc::new(AtomicUsize::new(0)), stops: Arc::clone(&stops), fail_port: true, crash: false });
        let manager = fixture(dir.path().join("managed"), Arc::new(FakeDownloader { corrupt_model: false, cancelled: false }), host, Arc::new(FakeClock(AtomicU64::new(0))), Arc::new(FakeTransport { healthy: true }));
        manager.install("lite").unwrap();
        let error = manager
            .begin_request("occupied", "lite")
            .err()
            .expect("the occupied endpoint must fail before returning a request");
        assert!(error.contains("occupied endpoint"));
        assert_eq!(stops.load(Ordering::SeqCst), 0);

        let crash_dir = tempfile::tempdir().unwrap();
        let crash_stops = Arc::new(AtomicUsize::new(0));
        let crash_host = Arc::new(FakeHost { spawns: Arc::new(AtomicUsize::new(0)), stops: Arc::clone(&crash_stops), fail_port: false, crash: true });
        let crash_manager = fixture(crash_dir.path().join("managed"), Arc::new(FakeDownloader { corrupt_model: false, cancelled: false }), crash_host, Arc::new(FakeClock(AtomicU64::new(0))), Arc::new(FakeTransport { healthy: true }));
        crash_manager.install("lite").unwrap();
        let crash_error = crash_manager
            .begin_request("crashed", "lite")
            .err()
            .expect("an exited runtime must fail before returning a request");
        assert!(crash_error.contains("exited during startup"));
        assert_eq!(crash_stops.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn startup_timeout_and_cancellation_stop_the_owned_process() {
        let timeout_dir = tempfile::tempdir().unwrap();
        let timeout_stops = Arc::new(AtomicUsize::new(0));
        let timeout_host = Arc::new(FakeHost { spawns: Arc::new(AtomicUsize::new(0)), stops: Arc::clone(&timeout_stops), fail_port: false, crash: false });
        let timeout_manager = fixture(timeout_dir.path().join("managed"), Arc::new(FakeDownloader { corrupt_model: false, cancelled: false }), timeout_host, Arc::new(FakeClock(AtomicU64::new(0))), Arc::new(FakeTransport { healthy: false }));
        timeout_manager.install("lite").unwrap();
        let timeout_error = timeout_manager
            .begin_request("timeout", "lite")
            .err()
            .expect("an unhealthy runtime must time out");
        assert!(timeout_error.contains("did not become healthy"));
        assert_eq!(timeout_stops.load(Ordering::SeqCst), 1);

        let cancel_dir = tempfile::tempdir().unwrap();
        let cancel_stops = Arc::new(AtomicUsize::new(0));
        let cancel_host = Arc::new(FakeHost { spawns: Arc::new(AtomicUsize::new(0)), stops: Arc::clone(&cancel_stops), fail_port: false, crash: false });
        let entered = Arc::new(AtomicBool::new(false));
        let cancel_manager = fixture(
            cancel_dir.path().join("managed"),
            Arc::new(FakeDownloader { corrupt_model: false, cancelled: false }),
            cancel_host,
            Arc::new(FakeClock(AtomicU64::new(0))),
            Arc::new(WaitForCancellationTransport { entered: Arc::clone(&entered) }),
        );
        cancel_manager.install("lite").unwrap();
        let worker_manager = Arc::clone(&cancel_manager);
        let worker = std::thread::spawn(move || match worker_manager.begin_request("starting", "lite") {
            Ok(_) => panic!("startup cancellation unexpectedly returned a request"),
            Err(error) => error,
        });
        while !entered.load(Ordering::SeqCst) {
            std::thread::yield_now();
        }
        assert!(cancel_manager.cancel_request("starting"));
        let cancel_error = worker.join().unwrap();
        assert!(cancel_error.contains("cancelled during runtime startup"));
        assert_eq!(cancel_stops.load(Ordering::SeqCst), 1);
        assert!(!cancel_manager.cancel_request("starting"));
    }

    #[test]
    fn stop_during_startup_answers_at_once_and_status_stays_live() {
        let dir = tempfile::tempdir().unwrap();
        let stops = Arc::new(AtomicUsize::new(0));
        let host = Arc::new(FakeHost { spawns: Arc::new(AtomicUsize::new(0)), stops: Arc::clone(&stops), fail_port: false, crash: false });
        let entered = Arc::new(AtomicBool::new(false));
        let manager = fixture(
            dir.path().join("managed"),
            Arc::new(FakeDownloader { corrupt_model: false, cancelled: false }),
            host,
            Arc::new(FakeClock(AtomicU64::new(0))),
            Arc::new(WaitForCancellationTransport { entered: Arc::clone(&entered) }),
        );
        manager.install("lite").unwrap();
        let worker_manager = Arc::clone(&manager);
        let worker = std::thread::spawn(move || worker_manager.begin_request("stop-race", "lite"));
        while !entered.load(Ordering::SeqCst) {
            std::thread::yield_now();
        }
        // Status answers while the startup is still in flight — holding the
        // lock across the health poll wedged this behind the 30-second wait.
        let status = manager.runtime_status();
        assert!(status.running);
        assert_eq!(status.active_model_id.as_deref(), Some("lite"));
        manager.stop_for_disable();
        let error = match worker.join().unwrap() {
            Ok(_) => panic!("stop must end the startup"),
            Err(error) => error,
        };
        assert!(error.contains("cancelled during runtime startup"), "stop cancels the wait, got {error}");
        assert!(!manager.runtime_status().running);
        assert_eq!(stops.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn second_model_refuses_promptly_while_a_startup_is_in_flight() {
        let dir = tempfile::tempdir().unwrap();
        let host = Arc::new(FakeHost { spawns: Arc::new(AtomicUsize::new(0)), stops: Arc::new(AtomicUsize::new(0)), fail_port: false, crash: false });
        let entered = Arc::new(AtomicBool::new(false));
        let manager = Arc::new(ManagedAi::with_adapters(
            dir.path().join("managed"),
            two_tier_catalog(),
            Arc::new(FakeDownloader { corrupt_model: false, cancelled: false }),
            host,
            Arc::new(WaitForCancellationTransport { entered: Arc::clone(&entered) }),
            Arc::new(FakeClock(AtomicU64::new(0))),
        ));
        manager.install("lite").unwrap();
        manager.install("heavy").unwrap();
        let worker_manager = Arc::clone(&manager);
        let worker = std::thread::spawn(move || worker_manager.begin_request("first", "lite"));
        while !entered.load(Ordering::SeqCst) {
            std::thread::yield_now();
        }
        let refused = match manager.begin_request("second", "heavy") {
            Ok(_) => panic!("busy switch must refuse"),
            Err(error) => error,
        };
        assert!(refused.contains("active request"), "actionable, got {refused}");
        assert!(manager.cancel_request("first"));
        let error = match worker.join().unwrap() {
            Ok(_) => panic!("cancelled startup must fail"),
            Err(error) => error,
        };
        assert!(error.contains("cancelled during runtime startup"), "got {error}");
    }

    #[test]
    fn actual_shutdown_stops_only_the_owned_process() {
        let dir = tempfile::tempdir().unwrap();
        let stops = Arc::new(AtomicUsize::new(0));
        let host = Arc::new(FakeHost { spawns: Arc::new(AtomicUsize::new(0)), stops: Arc::clone(&stops), fail_port: false, crash: false });
        let manager = fixture(dir.path().join("managed"), Arc::new(FakeDownloader { corrupt_model: false, cancelled: false }), host, Arc::new(FakeClock(AtomicU64::new(0))), Arc::new(FakeTransport { healthy: true }));
        manager.install("lite").unwrap();
        let request = manager.begin_request("active", "lite").unwrap();
        manager.shutdown();
        assert_eq!(stops.load(Ordering::SeqCst), 1);
        drop(request);
    }

    fn two_tier_catalog() -> &'static str {
        Box::leak(format!(r#"{{
          "schema_version":1,"note":"two-tier fixture",
          "managed_runtime":{{"name":"llama.cpp","candidate_version":"v1","windows_cpu_x64_artifact":"runtime.zip","source":"https://example/runtime","license":"MIT","status":"qualified","verified":{{"download":true,"per_user_launch":true,"health":true,"cancellation":true,"model_release":true,"process_ownership":true,"exit_behavior":true}},"blocker":"","download_url":"https://example/runtime.zip","sha256":"{}","size_bytes":{},"executable":"llama-server.exe","launch_args":["--host","127.0.0.1","--port","{{port}}","--model","{{model}}"]}},
          "tiers":[
            {{"id":"lite","intent":"fixture","candidate_artifact":"lite.gguf","artifact_source":"https://example/lite","revision":"rev1","quantization":"Q4","download_hash":"{}","download_size_bytes":{},"license":"Apache-2.0","license_source":"https://example/license","status":"qualified","blocker":"","context_limit_tokens":4096,"template_requirements":"chatml","memory_needs_mb":2048,"minimum_runtime_version":"v1","download_url":"https://example/lite.gguf","model_name":"lite"}},
            {{"id":"heavy","intent":"fixture","candidate_artifact":"heavy.gguf","artifact_source":"https://example/heavy","revision":"rev2","quantization":"Q5","download_hash":"{}","download_size_bytes":{},"license":"Apache-2.0","license_source":"https://example/license","status":"qualified","blocker":"","context_limit_tokens":8192,"template_requirements":"chatml","memory_needs_mb":4096,"minimum_runtime_version":"v1","download_url":"https://example/heavy.gguf","model_name":"heavy"}}
          ]
        }}"#, sha256_hex(RUNTIME_BYTES), RUNTIME_BYTES.len(), sha256_hex(MODEL_BYTES), MODEL_BYTES.len(), sha256_hex(MODEL_BYTES), MODEL_BYTES.len()).into_boxed_str())
    }

    fn two_tier_fixture(root: PathBuf) -> (Arc<ManagedAi>, Arc<AtomicUsize>) {
        let stops = Arc::new(AtomicUsize::new(0));
        let host = Arc::new(FakeHost { spawns: Arc::new(AtomicUsize::new(0)), stops: Arc::clone(&stops), fail_port: false, crash: false });
        let manager = Arc::new(ManagedAi::with_adapters(
            root,
            two_tier_catalog(),
            Arc::new(FakeDownloader { corrupt_model: false, cancelled: false }),
            host,
            Arc::new(FakeTransport { healthy: true }),
            Arc::new(FakeClock(AtomicU64::new(0))),
        ));
        (manager, stops)
    }

    #[test]
    fn remove_deletes_only_the_selected_model_and_reports_survivors() {
        let dir = tempfile::tempdir().unwrap();
        let (manager, _) = two_tier_fixture(dir.path().join("managed"));
        manager.install("lite").unwrap();
        manager.install("heavy").unwrap();
        let removed = manager.remove("lite").unwrap();
        assert!(removed.removed);
        assert_eq!(removed.model_id, "lite");
        assert_eq!(removed.remaining_installed, 1);
        assert!(manager.read_installed("lite").is_err());
        assert!(manager.read_installed("heavy").is_ok());
    }

    #[test]
    fn remove_reports_zero_remaining_when_the_last_model_goes() {
        let dir = tempfile::tempdir().unwrap();
        let (manager, _) = two_tier_fixture(dir.path().join("managed"));
        manager.install("lite").unwrap();
        let removed = manager.remove("lite").unwrap();
        assert!(removed.removed);
        assert_eq!(removed.remaining_installed, 0);
        let again = manager.remove("lite").unwrap();
        assert!(!again.removed);
        assert_eq!(again.remaining_installed, 0);
    }

    #[test]
    fn remove_rejects_unknown_ids_without_touching_owned_files() {
        let dir = tempfile::tempdir().unwrap();
        let (manager, _) = two_tier_fixture(dir.path().join("managed"));
        manager.install("lite").unwrap();
        let error = manager.remove("nope").expect_err("foreign ids must not delete");
        assert!(error.contains("bundled catalog"));
        assert!(manager.read_installed("lite").is_ok());
    }

    #[test]
    fn remove_stops_the_idle_owned_runtime_before_deleting() {
        let dir = tempfile::tempdir().unwrap();
        let (manager, stops) = two_tier_fixture(dir.path().join("managed"));
        manager.install("lite").unwrap();
        let request = manager.begin_request("idle", "lite").unwrap();
        drop(request);
        assert_eq!(stops.load(Ordering::SeqCst), 0);
        assert!(manager.remove("lite").unwrap().removed);
        assert_eq!(stops.load(Ordering::SeqCst), 1);
        assert!(manager.read_installed("lite").is_err());
    }

    #[test]
    fn remove_refuses_while_its_model_serves_a_request() {
        let dir = tempfile::tempdir().unwrap();
        let (manager, _) = two_tier_fixture(dir.path().join("managed"));
        manager.install("lite").unwrap();
        manager.install("heavy").unwrap();
        let request = manager.begin_request("busy", "lite").unwrap();
        let error = manager.remove("lite").expect_err("in-flight inference must block removal");
        assert!(error.contains("serving a generation request"));
        assert!(manager.read_installed("lite").is_ok());
        drop(request);
        assert!(manager.remove("lite").unwrap().removed);
        assert!(manager.read_installed("heavy").is_ok());
    }

    #[test]
    fn stop_for_disable_releases_the_runtime_and_stays_reusable() {
        let dir = tempfile::tempdir().unwrap();
        let (manager, stops) = two_tier_fixture(dir.path().join("managed"));
        manager.install("lite").unwrap();
        let request = manager.begin_request("first", "lite").unwrap();
        drop(request);
        manager.stop_for_disable();
        assert_eq!(stops.load(Ordering::SeqCst), 1);
        let second = manager.begin_request("second", "lite");
        assert!(second.is_ok(), "disabling must not poison later use");
    }

    // ------------------------------------------------------------------
    // Ticket 152: stronger-tier selection, inventory split, and migration.
    // The shipped stronger candidate stays non-installable (no invented
    // pins); these fixtures prove the logic that a future qualified
    // stronger tier will ride on (ADR-0032).
    // ------------------------------------------------------------------

    fn heavy_only_catalog() -> &'static str {
        Box::leak(format!(r#"{{
          "schema_version":1,"note":"heavy-only fixture",
          "managed_runtime":{{"name":"llama.cpp","candidate_version":"v1","windows_cpu_x64_artifact":"runtime.zip","source":"https://example/runtime","license":"MIT","status":"qualified","verified":{{"download":true,"per_user_launch":true,"health":true,"cancellation":true,"model_release":true,"process_ownership":true,"exit_behavior":true}},"blocker":"","download_url":"https://example/runtime.zip","sha256":"{}","size_bytes":{},"executable":"llama-server.exe","launch_args":["--host","127.0.0.1","--port","{{port}}","--model","{{model}}"]}},
          "tiers":[{{"id":"heavy","intent":"fixture","candidate_artifact":"heavy.gguf","artifact_source":"https://example/heavy","revision":"rev2","quantization":"Q5","download_hash":"{}","download_size_bytes":{},"license":"Apache-2.0","license_source":"https://example/license","status":"qualified","blocker":"","context_limit_tokens":8192,"template_requirements":"chatml","memory_needs_mb":4096,"minimum_runtime_version":"v1","download_url":"https://example/heavy.gguf","model_name":"heavy"}}]
        }}"#, sha256_hex(RUNTIME_BYTES), RUNTIME_BYTES.len(), sha256_hex(MODEL_BYTES), MODEL_BYTES.len()).into_boxed_str())
    }

    struct LowMemoryHost {
        inner: Arc<FakeHost>,
    }

    impl HostAdapter for LowMemoryHost {
        fn disk_bytes(&self, path: &Path) -> Result<u64, String> { self.inner.disk_bytes(path) }
        fn memory_mb(&self) -> Result<u64, String> { Ok(512) }
        fn extract_runtime(&self, archive: &Path, destination: &Path) -> Result<(), String> {
            self.inner.extract_runtime(archive, destination)
        }
        fn reserve_port(&self) -> Result<u16, String> { self.inner.reserve_port() }
        fn spawn(&self, executable: &Path, args: &[String]) -> Result<Box<dyn RuntimeProcess>, String> {
            self.inner.spawn(executable, args)
        }
        fn process_usage(&self, pid: u32) -> Result<(u64, u64), String> {
            self.inner.process_usage(pid)
        }
    }

    struct HeavyCorruptDownloader;

    impl Downloader for HeavyCorruptDownloader {
        fn fetch(&self, url: &str, target: &Path, _: u64, _cancelled: &AtomicBool, on_bytes: &dyn Fn(u64)) -> Result<(), String> {
            let bytes = if url.ends_with("runtime.zip") {
                RUNTIME_BYTES
            } else if url.contains("heavy") {
                b"corrupt" as &[u8]
            } else {
                MODEL_BYTES
            };
            fs::write(target, bytes).map_err(|error| error.to_string())?;
            on_bytes(bytes.len() as u64);
            Ok(())
        }
    }

    #[test]
    fn catalog_rejects_duplicate_ids_executable_recipes_and_bad_pins() {
        let base = qualified_catalog();
        let duplicate = base.replacen("\"id\":\"lite\"", "\"id\":\"lite\"", 1).replacen(
            "\"tiers\":[",
            "\"tiers\":[{\"id\":\"lite\",\"intent\":\"clone\",\"candidate_artifact\":\"x\",\"artifact_source\":\"https://example/x\",\"revision\":\"r\",\"quantization\":\"Q\",\"download_hash\":\"h\",\"download_size_bytes\":1,\"license\":\"L\",\"license_source\":\"https://example/l\",\"status\":\"blocked-x\",\"blocker\":\"x\",\"context_limit_tokens\":1,\"template_requirements\":\"t\",\"memory_needs_mb\":1,\"minimum_runtime_version\":\"v1\"},",
            1,
        );
        assert!(Catalog::parse(&duplicate).is_err(), "duplicate ids must fail the catalog");
        let recipe = base.replacen(
            "\"tiers\":[",
            "\"tiers\":[{\"id\":\"evil\",\"intent\":\"x\",\"candidate_artifact\":\"x\",\"artifact_source\":\"https://example/x\",\"revision\":\"r\",\"quantization\":\"Q\",\"download_hash\":\"h\",\"download_size_bytes\":1,\"license\":\"L\",\"license_source\":\"https://example/l\",\"status\":\"blocked-x\",\"blocker\":\"x\",\"install_recipe\":\"rm -rf /\"},",
            1,
        );
        assert!(
            Catalog::parse(&recipe).is_err(),
            "executable installation instructions must fail the catalog"
        );
        let unpinned_qualified = base.replacen("\"revision\":\"rev1\"", "\"revision\":null", 1);
        assert!(
            Catalog::parse(&unpinned_qualified).is_err(),
            "a qualified tier without artifact evidence is invalid metadata"
        );
        let wrong_runtime = base.replacen("\"minimum_runtime_version\":\"v1\"", "\"minimum_runtime_version\":\"v9\"", 1);
        assert!(
            Catalog::parse(&wrong_runtime).is_err(),
            "JSON alone cannot supply a missing runtime adapter"
        );
    }

    #[test]
    fn old_catalog_install_survives_a_new_recommendation_list() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("managed");
        let host = Arc::new(FakeHost { spawns: Arc::new(AtomicUsize::new(0)), stops: Arc::new(AtomicUsize::new(0)), fail_port: false, crash: false });
        let old = Arc::new(ManagedAi::with_adapters(
            root.clone(),
            qualified_catalog(),
            Arc::new(FakeDownloader { corrupt_model: false, cancelled: false }),
            host,
            Arc::new(FakeTransport { healthy: true }),
            Arc::new(FakeClock(AtomicU64::new(0))),
        ));
        old.install("lite").unwrap();
        // A new release recommends an additional stronger tier: nothing is
        // downloaded, switched, or deleted automatically.
        let host2 = Arc::new(FakeHost { spawns: Arc::new(AtomicUsize::new(0)), stops: Arc::new(AtomicUsize::new(0)), fail_port: false, crash: false });
        let migrated = Arc::new(ManagedAi::with_adapters(
            root.clone(),
            two_tier_catalog(),
            Arc::new(FakeDownloader { corrupt_model: false, cancelled: false }),
            host2,
            Arc::new(FakeTransport { healthy: true }),
            Arc::new(FakeClock(AtomicU64::new(0))),
        ));
        let status = migrated.catalog_status().unwrap();
        assert!(status.models.iter().any(|model| model.id == "lite" && model.installed));
        let heavy = status.models.iter().find(|model| model.id == "heavy").expect("new tier listed");
        assert!(!heavy.installed, "a new recommendation never installs itself");
        assert!(migrated.read_installed("lite").is_ok(), "the explicit selection survives migration");
        assert!(migrated.begin_request("after-migration", "lite").is_ok());
    }

    #[test]
    fn explicit_switching_serves_the_chosen_tier() {
        let dir = tempfile::tempdir().unwrap();
        let (manager, _) = two_tier_fixture(dir.path().join("managed"));
        manager.install("lite").unwrap();
        manager.install("heavy").unwrap();
        // Switching is explicit: the request names its tier, and each tier's
        // qualified context/template configuration resolves for generation.
        assert!(manager.begin_request("use-lite", "lite").is_ok());
        assert!(manager.begin_request("use-heavy", "heavy").is_ok());
        let status = manager.catalog_status().unwrap();
        assert!(status.models.iter().all(|model| model.installed));
    }

    #[test]
    fn failed_stronger_install_keeps_the_previous_selection_usable() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("managed");
        let host = Arc::new(FakeHost { spawns: Arc::new(AtomicUsize::new(0)), stops: Arc::new(AtomicUsize::new(0)), fail_port: false, crash: false });
        let manager = Arc::new(ManagedAi::with_adapters(
            root,
            two_tier_catalog(),
            Arc::new(HeavyCorruptDownloader),
            host,
            Arc::new(FakeTransport { healthy: true }),
            Arc::new(FakeClock(AtomicU64::new(0))),
        ));
        manager.install("lite").unwrap();
        let error = manager.install("heavy").expect_err("corrupt stronger bytes must fail");
        assert!(error.contains("checksum") || error.contains("byte size"), "actionable, got {error}");
        assert!(manager.read_installed("lite").is_ok(), "the previous selection survives");
        assert!(manager.begin_request("still-lite", "lite").is_ok(), "generation keeps serving it");
    }

    #[test]
    fn insufficient_memory_refuses_before_staging() {
        let dir = tempfile::tempdir().unwrap();
        let host = Arc::new(FakeHost { spawns: Arc::new(AtomicUsize::new(0)), stops: Arc::new(AtomicUsize::new(0)), fail_port: false, crash: false });
        let manager = Arc::new(ManagedAi::with_adapters(
            dir.path().join("managed"),
            qualified_catalog(),
            Arc::new(FakeDownloader { corrupt_model: false, cancelled: false }),
            Arc::new(LowMemoryHost { inner: host }),
            Arc::new(FakeTransport { healthy: true }),
            Arc::new(FakeClock(AtomicU64::new(0))),
        ));
        let error = manager.install("lite").expect_err("512 MB cannot serve a 2048 MB tier");
        assert!(error.contains("working RAM"), "download size is not the RAM requirement, got {error}");
        assert!(manager.read_installed("lite").is_err(), "refused installs stage nothing");
    }

    #[test]
    fn removed_recommendations_stay_identifiable_and_removable() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("managed");
        let host = Arc::new(FakeHost { spawns: Arc::new(AtomicUsize::new(0)), stops: Arc::new(AtomicUsize::new(0)), fail_port: false, crash: false });
        let old = Arc::new(ManagedAi::with_adapters(
            root.clone(),
            qualified_catalog(),
            Arc::new(FakeDownloader { corrupt_model: false, cancelled: false }),
            host,
            Arc::new(FakeTransport { healthy: true }),
            Arc::new(FakeClock(AtomicU64::new(0))),
        ));
        old.install("lite").unwrap();
        // The new release drops "lite": the install stays usable and listed
        // as removed-from-recommendations with a deliberate recovery path.
        let host2 = Arc::new(FakeHost { spawns: Arc::new(AtomicUsize::new(0)), stops: Arc::new(AtomicUsize::new(0)), fail_port: false, crash: false });
        let updated = Arc::new(ManagedAi::with_adapters(
            root.clone(),
            heavy_only_catalog(),
            Arc::new(FakeDownloader { corrupt_model: false, cancelled: false }),
            host2,
            Arc::new(FakeTransport { healthy: true }),
            Arc::new(FakeClock(AtomicU64::new(0))),
        ));
        let status = updated.catalog_status().unwrap();
        let orphan = status
            .models
            .iter()
            .find(|model| model.id == "lite")
            .expect("removed installs stay identifiable");
        assert!(orphan.installed && !orphan.installable);
        assert_eq!(orphan.status, "removed-from-recommendations");
        assert!(updated.begin_request("use-orphan", "lite").is_ok(), "no silent remap");
        let removed = updated.remove("lite").unwrap();
        assert!(removed.removed, "explicit removal still frees orphaned files");
        assert!(updated.read_installed("lite").is_err());
    }

    #[test]
    fn broken_catalog_never_touches_installed_inventory() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("managed");
        let host = Arc::new(FakeHost { spawns: Arc::new(AtomicUsize::new(0)), stops: Arc::new(AtomicUsize::new(0)), fail_port: false, crash: false });
        let good = Arc::new(ManagedAi::with_adapters(
            root.clone(),
            qualified_catalog(),
            Arc::new(FakeDownloader { corrupt_model: false, cancelled: false }),
            host,
            Arc::new(FakeTransport { healthy: true }),
            Arc::new(FakeClock(AtomicU64::new(0))),
        ));
        good.install("lite").unwrap();
        // Offline or broken-catalog startup (unparseable schema): status
        // fails closed, while the on-disk inventory — and the Quick Actions
        // in SQLite, which no managed path writes — survive untouched.
        let broken_json: &'static str = Box::leak(r#"{"schema_version":99,"note":"x","managed_runtime":{},"tiers":[]}"#.to_string().into_boxed_str());
        let host2 = Arc::new(FakeHost { spawns: Arc::new(AtomicUsize::new(0)), stops: Arc::new(AtomicUsize::new(0)), fail_port: false, crash: false });
        let broken = Arc::new(ManagedAi::with_adapters(
            root.clone(),
            broken_json,
            Arc::new(FakeDownloader { corrupt_model: false, cancelled: false }),
            host2,
            Arc::new(FakeTransport { healthy: true }),
            Arc::new(FakeClock(AtomicU64::new(0))),
        ));
        assert!(broken.catalog_status().is_err());
        assert!(broken.read_installed("lite").is_ok(), "inventory survives a broken catalog");
        assert!(broken.begin_request("offline-startup", "lite").is_ok());
    }

    // ------------------------------------------------------------------
    // Tickets 189/193: running status, Start pre-warm, install progress,
    // install-active flag, and the occupying-revision guard split.
    // ------------------------------------------------------------------

    #[test]
    fn runtime_status_tracks_process_liveness_with_uptime() {
        let dir = tempfile::tempdir().unwrap();
        let host = Arc::new(FakeHost { spawns: Arc::new(AtomicUsize::new(0)), stops: Arc::new(AtomicUsize::new(0)), fail_port: false, crash: false });
        let clock = Arc::new(FakeClock(AtomicU64::new(0)));
        let manager = fixture(dir.path().join("managed"), Arc::new(FakeDownloader { corrupt_model: false, cancelled: false }), host, Arc::clone(&clock), Arc::new(FakeTransport { healthy: true }));
        let idle = manager.runtime_status();
        assert!(!idle.running && idle.active_model_id.is_none() && idle.uptime_secs.is_none());
        manager.install("lite").unwrap();
        // Installed but never started still reads as stopped.
        assert!(!manager.runtime_status().running);
        let request = manager.begin_request("watch", "lite").unwrap();
        clock.0.store(65_000, Ordering::SeqCst);
        let running = manager.runtime_status();
        assert!(running.running);
        assert_eq!(running.active_model_id.as_deref(), Some("lite"));
        assert_eq!(running.uptime_secs, Some(65));
        drop(request);
        // Dropping the last request leaves the runtime up under the idle reap.
        assert!(manager.runtime_status().running);
        manager.stop_for_disable();
        let stopped = manager.runtime_status();
        assert!(!stopped.running && stopped.active_model_id.is_none());
    }

    #[test]
    fn failed_startup_leaves_status_stopped() {
        let dir = tempfile::tempdir().unwrap();
        let host = Arc::new(FakeHost { spawns: Arc::new(AtomicUsize::new(0)), stops: Arc::new(AtomicUsize::new(0)), fail_port: false, crash: true });
        let manager = fixture(dir.path().join("managed"), Arc::new(FakeDownloader { corrupt_model: false, cancelled: false }), host, Arc::new(FakeClock(AtomicU64::new(0))), Arc::new(FakeTransport { healthy: true }));
        manager.install("lite").unwrap();
        assert!(manager.begin_request("crashed-status", "lite").is_err());
        assert!(!manager.runtime_status().running, "an exited runtime never reads as running");
    }

    #[test]
    fn start_model_prewarm_shares_the_runtime_and_refuses_a_busy_switch() {
        let dir = tempfile::tempdir().unwrap();
        let (manager, _) = two_tier_fixture(dir.path().join("managed"));
        manager.install("lite").unwrap();
        manager.install("heavy").unwrap();
        assert_eq!(manager.start_model("lite").as_deref(), Ok("lite"));
        let status = manager.runtime_status();
        assert!(status.running);
        assert_eq!(status.active_model_id.as_deref(), Some("lite"));
        // Pre-warming the same model reuses the owned process (no second spawn).
        assert!(manager.start_model("lite").is_ok());
        // A different model serving an active request refuses honestly.
        let busy = manager.begin_request("busy-switch", "lite").unwrap();
        let refused = manager.start_model("heavy").expect_err("busy switch must refuse");
        assert!(refused.contains("active request"), "actionable, got {refused}");
        drop(busy);
        // Idle, the switch stops the old runtime and starts the new one.
        assert!(manager.start_model("heavy").is_ok());
        assert_eq!(manager.runtime_status().active_model_id.as_deref(), Some("heavy"));
    }

    #[test]
    fn start_model_refuses_without_touching_provider_state() {
        let dir = tempfile::tempdir().unwrap();
        let (manager, _) = two_tier_fixture(dir.path().join("managed"));
        let error = manager.start_model("lite").expect_err("nothing installed yet");
        assert!(error.contains("Install this qualified managed model"), "actionable, got {error}");
        assert!(!manager.runtime_status().running);
        assert!(manager.start_model("  ").is_err(), "blank ids refuse");
    }

    #[test]
    fn install_reports_runtime_then_model_progress() {
        use std::sync::Mutex;
        let dir = tempfile::tempdir().unwrap();
        let host = Arc::new(FakeHost { spawns: Arc::new(AtomicUsize::new(0)), stops: Arc::new(AtomicUsize::new(0)), fail_port: false, crash: false });
        let manager = fixture(dir.path().join("managed"), Arc::new(FakeDownloader { corrupt_model: false, cancelled: false }), host, Arc::new(FakeClock(AtomicU64::new(0))), Arc::new(FakeTransport { healthy: true }));
        let events = Mutex::new(Vec::new());
        manager
            .install_with_progress("lite", &|event: ManagedInstallProgress| {
                events.lock().unwrap().push((event.phase.clone(), event.downloaded_bytes, event.total_bytes, event.model_id.clone()));
            })
            .unwrap();
        let events = events.lock().unwrap();
        assert!(events.len() >= 2, "both phases report, got {events:?}");
        assert_eq!(events[0].0, "runtime");
        assert_eq!(events[0].2 as usize, RUNTIME_BYTES.len());
        assert_eq!(events[0].1 as usize, RUNTIME_BYTES.len());
        let model = events.iter().find(|event| event.0 == "model").expect("model phase reports");
        assert_eq!(model.2 as usize, MODEL_BYTES.len());
        assert_eq!(model.1 as usize, MODEL_BYTES.len());
        assert!(events.iter().all(|event| event.3 == "lite"));
    }

    #[test]
    fn install_reports_stage_transitions_after_downloads() {
        use std::sync::Mutex;
        let dir = tempfile::tempdir().unwrap();
        let host = Arc::new(FakeHost { spawns: Arc::new(AtomicUsize::new(0)), stops: Arc::new(AtomicUsize::new(0)), fail_port: false, crash: false });
        let manager = fixture(dir.path().join("managed"), Arc::new(FakeDownloader { corrupt_model: false, cancelled: false }), host, Arc::new(FakeClock(AtomicU64::new(0))), Arc::new(FakeTransport { healthy: true }));
        let events = Mutex::new(Vec::new());
        manager
            .install_with_progress("lite", &|event: ManagedInstallProgress| {
                events.lock().unwrap().push((event.phase.clone(), event.stage.clone()));
            })
            .unwrap();
        let events = events.lock().unwrap();
        let known = [
            "downloading",
            "verifying-runtime",
            "verifying-model",
            "extracting",
            "activating",
        ];
        assert!(
            events.iter().all(|(_, stage)| known.contains(&stage.as_str())),
            "every event names a known step, got {events:?}"
        );
        // The silent steps arrive in pipeline order after their downloads, so
        // the frontend can narrate a long hash instead of sitting on 100%.
        let mut cursor = 0;
        for stage in ["verifying-runtime", "verifying-model", "extracting", "activating"] {
            let pos = events
                .iter()
                .position(|(_, event_stage)| event_stage == stage)
                .unwrap_or_else(|| panic!("{stage} reports, got {events:?}"));
            assert!(pos >= cursor, "{stage} keeps pipeline order, got {events:?}");
            cursor = pos;
        }
        assert_eq!(events.last().map(|(_, stage)| stage.as_str()), Some("activating"));
    }

    #[test]
    fn install_flag_and_cancel_stay_honest_when_idle() {
        let dir = tempfile::tempdir().unwrap();
        let host = Arc::new(FakeHost { spawns: Arc::new(AtomicUsize::new(0)), stops: Arc::new(AtomicUsize::new(0)), fail_port: false, crash: false });
        let manager = fixture(dir.path().join("managed"), Arc::new(FakeDownloader { corrupt_model: false, cancelled: false }), host, Arc::new(FakeClock(AtomicU64::new(0))), Arc::new(FakeTransport { healthy: true }));
        assert!(!manager.is_installing());
        assert!(!manager.cancel_install(), "cancelling nothing reports false");
    }

    #[test]
    fn occupying_revision_splits_incomplete_from_incompatible() {
        // Incomplete: a revision directory with no readable manifest — the
        // previous attempt died before activation, so remove-and-retry applies.
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("managed");
        let host = Arc::new(FakeHost { spawns: Arc::new(AtomicUsize::new(0)), stops: Arc::new(AtomicUsize::new(0)), fail_port: false, crash: false });
        let incomplete = Arc::new(ManagedAi::with_adapters(
            root.clone(),
            qualified_catalog(),
            Arc::new(FakeDownloader { corrupt_model: false, cancelled: false }),
            host,
            Arc::new(FakeTransport { healthy: true }),
            Arc::new(FakeClock(AtomicU64::new(0))),
        ));
        fs::create_dir_all(root.join("installations").join("lite").join("rev1")).unwrap();
        fs::write(root.join("installations").join("lite").join("rev1").join("orphan.gguf"), b"partial").unwrap();
        let error = incomplete.install("lite").expect_err("the occupying revision must block");
        assert!(error.contains("already occupies this model revision"), "guard predicate, got {error}");
        assert!(error.contains("incomplete"), "retry copy, got {error}");
        assert!(incomplete.read_installed("lite").is_err(), "partial never reads as installed");

        // Incompatible: a manifest from another schema — retry cannot finish
        // it, so the copy points at a Sprout update instead.
        let dir2 = tempfile::tempdir().unwrap();
        let root2 = dir2.path().join("managed");
        let host2 = Arc::new(FakeHost { spawns: Arc::new(AtomicUsize::new(0)), stops: Arc::new(AtomicUsize::new(0)), fail_port: false, crash: false });
        let incompatible = Arc::new(ManagedAi::with_adapters(
            root2.clone(),
            qualified_catalog(),
            Arc::new(FakeDownloader { corrupt_model: false, cancelled: false }),
            host2,
            Arc::new(FakeTransport { healthy: true }),
            Arc::new(FakeClock(AtomicU64::new(0))),
        ));
        let occupying = root2.join("installations").join("lite").join("rev1");
        fs::create_dir_all(&occupying).unwrap();
        fs::write(
            occupying.join(INSTALL_MANIFEST),
            serde_json::json!({
                "schema_version": 2,
                "model_id": "lite",
                "revision": "rev1",
                "model_name": "lite",
                "model_file": "model.gguf",
                "model_sha256": sha256_hex(MODEL_BYTES),
                "runtime_version": "v1",
                "runtime_executable": "runtime/llama-server.exe",
                "runtime_args": [],
                "runtime_sha256": sha256_hex(RUNTIME_BYTES),
                "context_limit_tokens": 4096,
                "template_requirements": "chatml",
                "memory_needs_mb": 2048,
                "idle_seconds": 300
            })
            .to_string(),
        )
        .unwrap();
        let error2 = incompatible.install("lite").expect_err("the occupying revision must block");
        assert!(error2.contains("already occupies this model revision"), "guard predicate, got {error2}");
        assert!(error2.contains("incompatible"), "update copy, got {error2}");
    }

    // ------------------------------------------------------------------
    // Ticket 190: lazy resource usage — stopped reads as stopped with no
    // process query, running reports working set + uptime immediately with CPU
    // on the second sample, and a crash reads as stopped.
    // ------------------------------------------------------------------

    #[test]
    fn resource_usage_reports_stopped_without_a_process() {
        let dir = tempfile::tempdir().unwrap();
        let host = Arc::new(FakeHost { spawns: Arc::new(AtomicUsize::new(0)), stops: Arc::new(AtomicUsize::new(0)), fail_port: false, crash: false });
        let manager = fixture(dir.path().join("managed"), Arc::new(FakeDownloader { corrupt_model: false, cancelled: false }), host, Arc::new(FakeClock(AtomicU64::new(0))), Arc::new(FakeTransport { healthy: true }));
        let stopped = manager.resource_usage();
        assert!(!stopped.running);
        assert!(stopped.active_model_id.is_none());
        assert!(stopped.working_set_bytes.is_none());
        assert!(stopped.cpu_percent.is_none());
        manager.install("lite").unwrap();
        let idle = manager.resource_usage();
        assert!(!idle.running, "installed but never started still reads as stopped");
    }

    #[test]
    fn resource_usage_reports_working_set_then_cpu_percent() {
        let dir = tempfile::tempdir().unwrap();
        let host = Arc::new(FakeHost { spawns: Arc::new(AtomicUsize::new(0)), stops: Arc::new(AtomicUsize::new(0)), fail_port: false, crash: false });
        let clock = Arc::new(FakeClock(AtomicU64::new(0)));
        let manager = fixture(dir.path().join("managed"), Arc::new(FakeDownloader { corrupt_model: false, cancelled: false }), host, Arc::clone(&clock), Arc::new(FakeTransport { healthy: true }));
        manager.install("lite").unwrap();
        let request = manager.begin_request("usage", "lite").unwrap();
        clock.0.store(65_000, Ordering::SeqCst);
        let first = manager.resource_usage();
        assert!(first.running);
        assert_eq!(first.active_model_id.as_deref(), Some("lite"));
        assert_eq!(first.uptime_secs, Some(65));
        assert_eq!(first.working_set_bytes, Some(256 * 1024 * 1024));
        assert!(first.cpu_percent.is_none(), "the first sample has no delta yet");
        clock.0.store(67_000, Ordering::SeqCst);
        let second = manager.resource_usage();
        assert!(second.running);
        assert_eq!(second.working_set_bytes, Some(256 * 1024 * 1024));
        assert_eq!(second.cpu_percent, Some(0.0), "identical totals over 2s read as idle");
        drop(request);
        manager.stop_for_disable();
        let stopped = manager.resource_usage();
        assert!(!stopped.running);
        assert!(stopped.working_set_bytes.is_none());
    }

    #[test]
    fn resource_usage_reads_a_crash_as_stopped() {
        let dir = tempfile::tempdir().unwrap();
        let host = Arc::new(FakeHost { spawns: Arc::new(AtomicUsize::new(0)), stops: Arc::new(AtomicUsize::new(0)), fail_port: false, crash: true });
        let manager = fixture(dir.path().join("managed"), Arc::new(FakeDownloader { corrupt_model: false, cancelled: false }), host, Arc::new(FakeClock(AtomicU64::new(0))), Arc::new(FakeTransport { healthy: true }));
        manager.install("lite").unwrap();
        assert!(manager.begin_request("crashed-usage", "lite").is_err());
        assert!(!manager.resource_usage().running);
    }

    #[test]
    fn usage_poll_never_blocks_stop_or_status() {
        let dir = tempfile::tempdir().unwrap();
        let stops = Arc::new(AtomicUsize::new(0));
        let inner = Arc::new(FakeHost { spawns: Arc::new(AtomicUsize::new(0)), stops: Arc::clone(&stops), fail_port: false, crash: false });
        let entered = Arc::new(AtomicBool::new(false));
        let release = Arc::new(AtomicBool::new(false));
        let host = Arc::new(SlowUsageHost { inner, entered: Arc::clone(&entered), release: Arc::clone(&release) });
        let manager = Arc::new(ManagedAi::with_adapters(
            dir.path().join("managed"),
            qualified_catalog(),
            Arc::new(FakeDownloader { corrupt_model: false, cancelled: false }),
            host,
            Arc::new(FakeTransport { healthy: true }),
            Arc::new(FakeClock(AtomicU64::new(0))),
        ));
        manager.install("lite").unwrap();
        let _live = manager.begin_request("live", "lite").unwrap();
        let worker_manager = Arc::clone(&manager);
        let worker = std::thread::spawn(move || worker_manager.resource_usage());
        while !entered.load(Ordering::SeqCst) {
            std::thread::yield_now();
        }
        // The usage query is mid-PowerShell holding no lock: Stop and status
        // must answer at once instead of queueing behind the open Details poll.
        manager.stop_for_disable();
        assert!(!manager.runtime_status().running);
        release.store(true, Ordering::SeqCst);
        assert!(!worker.join().unwrap().running, "the orphaned poll reads stopped, not stale numbers");
        assert_eq!(stops.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn streaming_sha256_matches_one_shot_across_block_boundaries() {
        for len in [0usize, 1, 55, 56, 57, 63, 64, 65, 119, 128, 1000] {
            let input: Vec<u8> = (0..len).map(|i| (i % 251) as u8).collect();
            let expected = sha256_hex(&input);
            // Feed in odd chunks to cross 64-byte block edges mid-stream.
            let mut stream = StreamingSha256::new();
            let mut at = 0;
            while at < input.len() {
                let take = (at % 17 + 1).min(input.len() - at);
                stream.update(&input[at..at + take]);
                at += take;
            }
            assert_eq!(stream.hex(), expected, "len {len}");
        }
    }

    #[test]
    fn verify_file_streams_without_loading_gigabytes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("artifact.bin");
        let bytes = vec![7u8; 3 * 1024 * 1024 + 13];
        fs::write(&path, &bytes).unwrap();
        let cancelled = AtomicBool::new(false);
        assert!(verify_file(&path, bytes.len() as u64, &sha256_hex(&bytes), &cancelled).is_ok());
        assert!(verify_file(&path, bytes.len() as u64 + 1, &sha256_hex(&bytes), &cancelled).is_err());
        assert!(verify_file(&path, bytes.len() as u64, &sha256_hex(b"other"), &cancelled).is_err());
        cancelled.store(true, Ordering::SeqCst);
        assert!(verify_file(&path, bytes.len() as u64, &sha256_hex(&bytes), &cancelled)
            .unwrap_err()
            .contains("cancelled"));
    }

    #[test]
    fn occupying_revision_fails_before_any_staging_download() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("managed");
        let host = Arc::new(FakeHost { spawns: Arc::new(AtomicUsize::new(0)), stops: Arc::new(AtomicUsize::new(0)), fail_port: false, crash: false });
        let manager = fixture(root.clone(), Arc::new(FakeDownloader { corrupt_model: false, cancelled: false }), host, Arc::new(FakeClock(AtomicU64::new(0))), Arc::new(FakeTransport { healthy: true }));
        fs::create_dir_all(root.join("installations").join("lite").join("rev1")).unwrap();
        let error = manager.install("lite").expect_err("the occupying revision must block");
        assert!(error.contains("already occupies this model revision"), "guard predicate, got {error}");
        assert!(!root.join("staging").exists(), "no gigabytes download before the guard");
    }

    #[test]
    fn remove_sweeps_only_its_own_staging_partials() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("managed");
        let host = Arc::new(FakeHost { spawns: Arc::new(AtomicUsize::new(0)), stops: Arc::new(AtomicUsize::new(0)), fail_port: false, crash: false });
        let manager = fixture(root.clone(), Arc::new(FakeDownloader { corrupt_model: false, cancelled: false }), host, Arc::new(FakeClock(AtomicU64::new(0))), Arc::new(FakeTransport { healthy: true }));
        manager.install("lite").unwrap();
        // Crashed-attempt leftovers: ours plus another model's.
        fs::create_dir_all(root.join("staging").join("lite-123")).unwrap();
        fs::write(root.join("staging").join("lite-123").join("partial.gguf"), b"partial").unwrap();
        fs::create_dir_all(root.join("staging").join("other-456")).unwrap();
        fs::write(root.join("staging").join("other-456").join("partial.gguf"), b"partial").unwrap();
        manager.remove("lite").unwrap();
        assert!(!root.join("staging").join("lite-123").exists(), "own partials are reclaimed");
        assert!(root.join("staging").join("other-456").exists(), "another model's staging survives");
        assert!(manager.read_installed("lite").is_err());
    }
}
