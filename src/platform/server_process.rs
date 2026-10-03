//! Local backend: in-process `br-server` router (default), or a spawned Bun server as a dev escape
//! hatch (discover its port, health-check it, kill it on exit). Mirrors `electron/main.ts` (MIGRATION.md §7).

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::oneshot;

use crate::api::ApiClient;

const PORT_TIMEOUT: Duration = Duration::from_secs(30);
const HEALTH_ATTEMPTS: u32 = 60;
const HEALTH_INTERVAL: Duration = Duration::from_millis(500);
const STDERR_TAIL_LINES: usize = 40;
const LISTENING_MARKER: &str = "BR_SERVER_LISTENING";

#[derive(Debug, thiserror::Error)]
pub enum ServerError {
    #[error("could not start the server ({0}). Is `bun` installed (needed for `BETTERRACK_SERVER_ROOT`)?")]
    Spawn(std::io::Error),
    #[error("port already in use. Close the other BetterRack/server instance and try again.\n{0}")]
    PortInUse(String),
    #[error("the server exited before it was ready.\n{0}")]
    Exited(String),
    #[error("the server did not report a port within 30 s.\n{0}")]
    PortTimeout(String),
    #[error("the server did not become healthy.\n{0}")]
    Unhealthy(String),
}

/// How to launch the Bun server, the dev escape hatch (`BETTERRACK_SERVER_ROOT`). Local mode is
/// otherwise in-process, see [`start_in_process`].
#[derive(Debug, Clone)]
pub struct Launch {
    /// A BetterRack checkout: runs `bun run ./src/run.ts` with cwd = `root`, so the dev SQLite DBs
    /// in `src/database/` are shared with the Electron dev app.
    root: PathBuf,
}

impl Launch {
    /// `Some` when `BETTERRACK_SERVER_ROOT` points at a Bun checkout.
    pub fn detect() -> Option<Self> {
        std::env::var_os("BETTERRACK_SERVER_ROOT").map(|root| Self { root: root.into() })
    }

    fn command(&self) -> Command {
        let mut c = Command::new("bun");
        c.args(["run", "./src/run.ts"]).current_dir(&self.root);
        c
    }
}

/// True for an installed build: the staged layout ships `bin/7z(.exe)` next to the binary.
pub fn is_packaged() -> bool {
    app_dir().is_some_and(|dir| seven_zip_exe(&dir).exists())
}

fn app_dir() -> Option<PathBuf> {
    std::env::current_exe().ok().and_then(|e| e.parent().map(Path::to_path_buf))
}

fn seven_zip_exe(app_dir: &Path) -> PathBuf {
    app_dir.join("bin").join(if cfg!(windows) { "7z.exe" } else { "7zz" })
}

/// Local mode without a sidecar: open `br-core` in this process and return a client that calls the
/// `br-server` router directly. Same data dir and 7-Zip as the packaged sidecar, so existing data is
/// reused. Must run inside the Tokio runtime (it spawns the background scan and the wiki bridge).
pub fn start_in_process() -> Result<ApiClient, String> {
    let app_dir = app_dir();
    let data_dir = dirs::data_dir()
        .or_else(|| app_dir.clone())
        .ok_or("no data directory available")?
        .join("BetterRack");
    std::fs::create_dir_all(&data_dir).map_err(|e| format!("{}: {e}", data_dir.display()))?;

    let seven_zip_path = std::env::var_os("SEVEN_ZIP_PATH").map(PathBuf::from).or_else(|| {
        let path = seven_zip_exe(app_dir.as_deref()?);
        if path.exists() {
            Some(path)
        } else {
            tracing::warn!("SEVEN_ZIP_PATH not set, {} is missing; falling back to `7z` on PATH", path.display());
            None
        }
    });
    let config = br_core::config::Config {
        port: 0,
        api_key: None,
        log_level: "info".into(),
        seven_zip_path,
        data_dir,
    };
    let wiki = Arc::new(br_server::wiki::LiveWiki::new(br_wiki::WikiService::default()));
    let state = br_server::state::AppState::open_with_wiki(config, wiki).map_err(|e| e.to_string())?;
    state.spawn_rescan();
    Ok(ApiClient::in_process(br_server::app(state)))
}

/// A running (or externally provided) server. Dropping it kills the child.
pub struct ServerProcess {
    // Held for `kill_on_drop` and the Windows job object.
    _child: Option<Child>,
    #[cfg(windows)]
    _job: Option<win_job::JobObject>,
    pub base_url: String,
}

impl ServerProcess {
    /// Use an already-running server (e.g. `BETTERRACK_SERVER_URL`); nothing is spawned.
    pub fn external(base_url: String) -> Self {
        Self {
            _child: None,
            #[cfg(windows)]
            _job: None,
            base_url,
        }
    }

    /// Spawn, wait for `BR_SERVER_LISTENING <port>` (30 s), then poll `/healthz` (60 × 500 ms)
    /// until `app == "betterrack"`. Must run on the Tokio runtime (see [`crate::runtime::run`]).
    pub async fn spawn(launch: &Launch) -> Result<Self, ServerError> {
        let mut cmd = launch.command();
        cmd.env("PORT", "0")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        #[cfg(windows)]
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW

        let mut child = cmd.spawn().map_err(ServerError::Spawn)?;

        // A job object kills the server even if we crash (kill_on_drop only covers clean drops),
        // so a stale server never keeps holding its port (gotcha #11).
        #[cfg(windows)]
        let job = win_job::JobObject::adopt(&child);

        let tail: Arc<Mutex<VecDeque<String>>> = Arc::default();
        let (port_tx, port_rx) = oneshot::channel::<u16>();

        let stdout = child.stdout.take().expect("piped stdout");
        let t = tail.clone();
        tokio::spawn(async move {
            let mut port_tx = Some(port_tx);
            let mut lines = BufReader::new(stdout).lines();
            // Keep draining after the port is found so the pipe never fills and blocks the server.
            while let Ok(Some(line)) = lines.next_line().await {
                if let (Some(port), true) = (parse_listening(&line), port_tx.is_some()) {
                    let _ = port_tx.take().unwrap().send(port);
                }
                tracing::info!(target: "server", "{line}");
                push_tail(&t, line);
            }
        });
        let stderr = child.stderr.take().expect("piped stderr");
        let t = tail.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                tracing::warn!(target: "server", "{line}");
                push_tail(&t, line);
            }
        });

        let tail_text = || tail.lock().map(|t| t.iter().cloned().collect::<Vec<_>>().join("\n")).unwrap_or_default();

        let port = tokio::select! {
            r = port_rx => match r {
                Ok(p) => p,
                // stdout closed without the marker: the process died.
                Err(_) => return Err(classify_exit(tail_text())),
            },
            _ = tokio::time::sleep(PORT_TIMEOUT) => return Err(ServerError::PortTimeout(tail_text())),
        };

        let base_url = format!("http://127.0.0.1:{port}");
        let client = ApiClient::new(&base_url, None).expect("valid local url");
        for _ in 0..HEALTH_ATTEMPTS {
            if let Ok(Some(status)) = child.try_wait() {
                return Err(classify_exit(format!("exit status: {status}\n{}", tail_text())));
            }
            if matches!(client.healthz().await, Ok(h) if h.app == "betterrack") {
                return Ok(Self {
                    _child: Some(child),
                    #[cfg(windows)]
                    _job: job,
                    base_url,
                });
            }
            tokio::time::sleep(HEALTH_INTERVAL).await;
        }
        Err(ServerError::Unhealthy(tail_text()))
    }
}

/// `BR_SERVER_LISTENING 3000` → `Some(3000)`.
pub fn parse_listening(line: &str) -> Option<u16> {
    line.trim().strip_prefix(LISTENING_MARKER)?.trim().parse().ok()
}

fn push_tail(tail: &Mutex<VecDeque<String>>, line: String) {
    if let Ok(mut t) = tail.lock() {
        if t.len() == STDERR_TAIL_LINES {
            t.pop_front();
        }
        t.push_back(line);
    }
}

fn classify_exit(output: String) -> ServerError {
    if output.contains("EADDRINUSE") || output.to_lowercase().contains("address already in use") {
        ServerError::PortInUse(output)
    } else {
        ServerError::Exited(output)
    }
}

#[cfg(windows)]
mod win_job {

    use tokio::process::Child;
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
        SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
        JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    };

    /// Kill-on-close job: when our process dies the OS closes the handle and kills the child.
    pub struct JobObject(HANDLE);

    // SAFETY: a job handle is just a kernel object id; it is only closed on drop.
    unsafe impl Send for JobObject {}
    unsafe impl Sync for JobObject {}

    impl JobObject {
        pub fn adopt(child: &Child) -> Option<Self> {
            let process = child.raw_handle()? as HANDLE;
            // SAFETY: plain Win32 calls with valid, locally owned arguments.
            unsafe {
                let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
                if job.is_null() {
                    return None;
                }
                let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
                info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
                let ok = SetInformationJobObject(
                    job,
                    JobObjectExtendedLimitInformation,
                    &info as *const _ as *const _,
                    std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                );
                if ok == 0 || AssignProcessToJobObject(job, process) == 0 {
                    CloseHandle(job);
                    return None;
                }
                Some(Self(job))
            }
        }
    }

    impl Drop for JobObject {
        fn drop(&mut self) {
            // SAFETY: we own this handle.
            unsafe { CloseHandle(self.0) };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_listening_marker() {
        assert_eq!(parse_listening("BR_SERVER_LISTENING 3000"), Some(3000));
        assert_eq!(parse_listening("  BR_SERVER_LISTENING 54321\r"), Some(54321));
        assert_eq!(parse_listening("Server running"), None);
        assert_eq!(parse_listening("BR_SERVER_LISTENING nope"), None);
    }

    #[test]
    fn detects_port_in_use() {
        assert!(matches!(classify_exit("error: EADDRINUSE".into()), ServerError::PortInUse(_)));
        assert!(matches!(classify_exit("boom".into()), ServerError::Exited(_)));
    }
}
