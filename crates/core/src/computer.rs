//! Computer use: watches the local cua-driver daemon (`Config.cua_driver`) for
//! [`Core::computer`](crate::Core::computer) and stops / grants on request.
//!
//! Sources, cheapest first (measured on cua-driver 0.38.0):
//! - Installed: `stat` of the binary every tick; `cua-driver --version` only at start and when the
//!   binary's mtime changes.
//! - Daemon + live sessions: one `{"method":"sessions_list"}` request on the daemon's Unix socket
//!   (`~/Library/Caches/cua-driver/cua-driver.sock`, line-delimited JSON; the wire protocol the
//!   cua-driver CLI, MCP proxy and SDKs share, `cua_driver_core::daemon`). It is exactly what
//!   `cua-driver sessions --json` prints, without spawning a process (that costs ~10 ms CPU per
//!   call). No socket = no daemon. If a daemon ever rejects the method, the watcher falls back to
//!   the CLI for good (`method: "cli"`).
//! - Permission mode: `{"method":"authorization_status"}` on the socket (`permission_mode`).
//! - Permissions: `cua-driver permissions status --json` (answered by the daemon under its own TCC
//!   identity), only when the daemon (re)appears, every [`PERMISSIONS_EVERY`] and after a grant.
//!
//! cua-driver reports sessions content-free: label, client kind, transport, age and idle time.
//! It never says which app an agent acts on or what it did (Computer History needs
//! `serve --experimental-history` plus an opt-in and is encrypted), so `app` / `last_action` stay
//! `None`; `last_action_at` is derived from `idle_seconds` (the daemon resets it on every tool
//! call), and each move of it is logged as `computer.action`.
//!
//! Cadence: every 1 s while the daemon runs, every 5 s while it does not, every 10 s while the
//! binary is missing. Every tick logs `computer.poll`; `ComputerUpdated` fires only on change.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant, SystemTime};

use crossbeam_channel::{Receiver, RecvTimeoutError, Sender};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::t3::{CmdOutput, argv_json, run_cmd};
use crate::telemetry::{Telemetry, ms, now_unix_ms, parse_rfc3339_ms, rfc3339_from_ms, round2};
use crate::{ComputerSession, ComputerView, CoreEvent};

const POLL_RUNNING: Duration = Duration::from_secs(1);
const POLL_STOPPED: Duration = Duration::from_secs(5);
const POLL_MISSING: Duration = Duration::from_secs(10);
/// Permission re-check period while the daemon runs (grants rarely change on their own).
const PERMISSIONS_EVERY: Duration = Duration::from_secs(60);
/// One socket request (connect + write + reply); the daemon answers these in < 5 ms.
const SOCKET_TIMEOUT: Duration = Duration::from_secs(2);
/// `idle_seconds` is whole seconds, so a derived action time jitters by up to 1 s between polls;
/// only a move larger than this is a new action.
const ACTION_SLACK_MS: i64 = 1500;

/// What the watcher should do on the next tick besides polling.
#[derive(Clone, Copy, Default)]
struct Wake {
    /// Re-read permissions now (after a grant).
    permissions: bool,
    /// Emit `ComputerUpdated` even when nothing changed (the caller promised one).
    announce: bool,
}

impl Wake {
    fn merge(self, o: Wake) -> Wake {
        Wake { permissions: self.permissions || o.permissions, announce: self.announce || o.announce }
    }
}

/// Handle owned by `Core` outside demo mode.
pub(crate) struct Computer {
    tel: Arc<Telemetry>,
    bin: PathBuf,
    view: Arc<RwLock<ComputerView>>,
    wake: Sender<Wake>,
    events: Sender<CoreEvent>,
    granting: Arc<AtomicBool>,
}

impl Computer {
    /// Spawns the `t3p-computer` watcher, which publishes into `view`.
    pub(crate) fn spawn(
        tel: Arc<Telemetry>,
        bin: PathBuf,
        view: Arc<RwLock<ComputerView>>,
        events: Sender<CoreEvent>,
    ) -> std::io::Result<Computer> {
        let (wake, wakes) = crossbeam_channel::unbounded();
        let watch = Watch {
            tel: tel.clone(),
            bin: bin.clone(),
            socket: socket_path(),
            view: view.clone(),
            events: events.clone(),
            bin_mtime: None,
            version: None,
            permissions_at: None,
            permissions_ok: None,
            permission_mode: None,
            cli_fallback: false,
            sessions: Vec::new(),
        };
        std::thread::Builder::new().name("t3p-computer".into()).spawn(move || watch.run(&wakes))?;
        Ok(Computer { tel, bin, view, wake, events, granting: Arc::new(AtomicBool::new(false)) })
    }

    /// `cua-driver revoke --all` on a short-lived thread, logged as `computer.stop`; then an
    /// immediate re-poll that always announces `ComputerUpdated`.
    pub(crate) fn stop_all(&self) {
        let before: Vec<String> = read(&self.view).sessions.iter().map(|s| s.id.clone()).collect();
        let (tel, bin, wake) = (self.tel.clone(), self.bin.clone(), self.wake.clone());
        let spawned = std::thread::Builder::new().name("t3p-computer-stop".into()).spawn(move || {
            let argv: [&std::ffi::OsStr; 3] = [bin.as_os_str(), "revoke".as_ref(), "--all".as_ref()];
            let input = json!({"argv": argv_json(&argv), "sessions_before": before});
            match run_cmd(&argv, None) {
                Ok(o) => {
                    let stdout = String::from_utf8_lossy(&o.stdout).trim().to_owned();
                    let mut out = o.telemetry();
                    out["revoked"] = json!(revoked_count(&stdout));
                    out["stdout"] = json!(stdout);
                    out["stderr"] = json!(o.stderr.trim());
                    let err = (!o.success()).then(|| o.failure());
                    tel.event("computer.stop", Some(o.wall_ms), err.is_none(), input, out, err.as_deref());
                }
                Err(e) => {
                    tel.event("computer.stop", None, false, input, Value::Null, Some(&format!("spawn: {e}")));
                }
            }
            let _ = wake.send(Wake { permissions: false, announce: true });
        });
        if let Err(e) = spawned {
            let _ = self.events.send(CoreEvent::Error(format!("computer stop thread: {e}")));
        }
    }

    /// `cua-driver permissions grant` on its own thread (it launches CuaDriver through
    /// LaunchServices and waits for the user's answers), logged as `computer.permissions_grant`
    /// when it exits; then permissions are re-read and `ComputerUpdated` announced. One at a time.
    pub(crate) fn grant_permissions(&self) {
        let argv = [self.bin.as_os_str().to_owned(), "permissions".into(), "grant".into()];
        if self.granting.swap(true, Ordering::AcqRel) {
            let input = json!({"argv": argv_json(&argv)});
            let out = json!({"ignored": "a grant is already running"});
            self.tel.event("computer.permissions_grant", None, true, input, out, None);
            return;
        }
        let (tel, wake, granting) = (self.tel.clone(), self.wake.clone(), self.granting.clone());
        let spawned = std::thread::Builder::new().name("t3p-computer-grant".into()).spawn(move || {
            let input = json!({"argv": argv_json(&argv)});
            match run_cmd(&argv, None) {
                Ok(o) => {
                    let mut out = o.telemetry();
                    out["stdout"] = json!(String::from_utf8_lossy(&o.stdout).trim());
                    out["stderr"] = json!(o.stderr.trim());
                    let err = (!o.success()).then(|| o.failure());
                    tel.event("computer.permissions_grant", Some(o.wall_ms), err.is_none(), input, out, err.as_deref());
                }
                Err(e) => {
                    let err = format!("spawn: {e}");
                    tel.event("computer.permissions_grant", None, false, input, Value::Null, Some(&err));
                }
            }
            granting.store(false, Ordering::Release);
            let _ = wake.send(Wake { permissions: true, announce: true });
        });
        if let Err(e) = spawned {
            self.granting.store(false, Ordering::Release);
            let _ = self.events.send(CoreEvent::Error(format!("computer grant thread: {e}")));
        }
    }
}

fn read(view: &RwLock<ComputerView>) -> std::sync::RwLockReadGuard<'_, ComputerView> {
    view.read().unwrap_or_else(|p| p.into_inner())
}

/// The release daemon's socket (`cua_driver_core::daemon::socket_path_for_namespace("cua-driver")`).
fn socket_path() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_default()).join("Library/Caches/cua-driver/cua-driver.sock")
}

/// `Revoked 2 authorization scope(s).` → 2.
fn revoked_count(stdout: &str) -> Option<u64> {
    stdout.strip_prefix("Revoked ")?.split_whitespace().next()?.parse().ok()
}

enum SocketError {
    /// No socket file or nobody listening: the daemon is not running.
    NotRunning,
    /// The daemon answered `ok: false` (e.g. an unknown method after a protocol change).
    Rejected(String),
    /// I/O or framing failure on a listening socket.
    Failed(String),
}

/// One request/response on the daemon socket.
fn socket_request(path: &Path, method: &str) -> Result<Value, SocketError> {
    let mut stream = UnixStream::connect(path).map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound | std::io::ErrorKind::ConnectionRefused => SocketError::NotRunning,
        _ => SocketError::Failed(format!("connect: {e}")),
    })?;
    let io = |e: std::io::Error| SocketError::Failed(e.to_string());
    stream.set_read_timeout(Some(SOCKET_TIMEOUT)).map_err(io)?;
    stream.set_write_timeout(Some(SOCKET_TIMEOUT)).map_err(io)?;
    let line = json!({"method": method, "observation_origin": "direct"}).to_string() + "\n";
    stream.write_all(line.as_bytes()).map_err(io)?;
    let mut reply = String::new();
    BufReader::new(stream).read_line(&mut reply).map_err(io)?;
    #[derive(Deserialize)]
    struct Reply {
        ok: bool,
        #[serde(default)]
        result: Value,
        #[serde(default)]
        error: Option<String>,
    }
    let r: Reply = serde_json::from_str(&reply).map_err(|e| SocketError::Failed(format!("reply: {e}")))?;
    if r.ok { Ok(r.result) } else { Err(SocketError::Rejected(r.error.unwrap_or_else(|| "ok: false".into()))) }
}

/// One entry of `sessions_list` / `sessions --json`.
#[derive(Debug, Deserialize)]
struct RawSession {
    /// Caller-declared public label; `None` for implicit sessions.
    #[serde(default)]
    session: Option<String>,
    #[serde(default)]
    client_kind: Option<String>,
    #[serde(default)]
    transport: Option<String>,
    #[serde(default)]
    owner_short_id: Option<String>,
    #[serde(default)]
    state: Option<String>,
    #[serde(default)]
    started_seconds_ago: u64,
    #[serde(default)]
    idle_seconds: u64,
}

#[derive(Deserialize)]
struct SessionList {
    #[serde(default)]
    sessions: Vec<RawSession>,
}

/// One tick's subprocess accounting.
#[derive(Default)]
struct Cost {
    cli_calls: u32,
    child_cpu_ms: f64,
}

impl Cost {
    fn run(&mut self, argv: &[&std::ffi::OsStr]) -> std::io::Result<CmdOutput> {
        let o = run_cmd(argv, None)?;
        self.cli_calls += 1;
        self.child_cpu_ms += o.child_cpu_ms;
        Ok(o)
    }
}

/// How one tick saw the daemon.
enum Daemon {
    Missing,
    Stopped,
    /// Listening; `None` = the session list could not be read this tick (keep the last one).
    Running(Option<Vec<RawSession>>),
}

struct Watch {
    tel: Arc<Telemetry>,
    bin: PathBuf,
    socket: PathBuf,
    view: Arc<RwLock<ComputerView>>,
    events: Sender<CoreEvent>,
    /// Binary mtime the version was read for (`Some(None)`: the filesystem gives no mtime).
    bin_mtime: Option<Option<SystemTime>>,
    version: Option<String>,
    /// Last permission check while the daemon runs; `None` = due.
    permissions_at: Option<Instant>,
    permissions_ok: Option<bool>,
    permission_mode: Option<String>,
    /// The daemon rejected a socket method once: use the CLI from now on.
    cli_fallback: bool,
    sessions: Vec<ComputerSession>,
}

impl Watch {
    fn run(mut self, wakes: &Receiver<Wake>) {
        let mut wake = Wake::default();
        loop {
            let next = self.tick(wake);
            wake = Wake::default();
            match wakes.recv_timeout(next) {
                Ok(w) => wake = wake.merge(w),
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return,
            }
            while let Ok(w) = wakes.try_recv() {
                wake = wake.merge(w);
            }
        }
    }

    /// One poll; returns the delay until the next.
    fn tick(&mut self, wake: Wake) -> Duration {
        let start = Instant::now();
        let now = now_unix_ms();
        let mut cost = Cost::default();
        let mut errors: Vec<String> = Vec::new();
        let mtime = std::fs::metadata(&self.bin).ok().map(|m| m.modified().ok());
        // `Some(mtime)` the version was read for; a failed read is not retried until it changes.
        let version_checked = match mtime {
            None => {
                self.bin_mtime = None;
                self.version = None;
                false
            }
            Some(m) if self.bin_mtime != Some(m) => {
                self.bin_mtime = Some(m);
                self.version = self.read_version(&mut cost, &mut errors);
                true
            }
            Some(_) => false,
        };
        let method = if self.cli_fallback { "cli" } else { "socket" };
        let daemon = if mtime.is_none() { Daemon::Missing } else { self.daemon(&mut cost, &mut errors) };
        let running = matches!(daemon, Daemon::Running(_));
        let mut permissions_checked = false;
        if running {
            if wake.permissions || self.permissions_at.is_none_or(|t| t.elapsed() >= PERMISSIONS_EVERY) {
                self.read_permissions(&mut cost, &mut errors);
                self.permissions_at = Some(Instant::now());
                permissions_checked = true;
            }
        } else {
            self.permissions_at = None;
            self.permissions_ok = None;
            self.permission_mode = None;
        }
        match daemon {
            Daemon::Running(None) => {}
            Daemon::Running(Some(raw)) => self.merge_sessions(raw, now),
            Daemon::Missing | Daemon::Stopped => self.merge_sessions(Vec::new(), now),
        }
        let view = ComputerView {
            installed: mtime.is_some(),
            version: self.version.clone(),
            daemon_running: running,
            permissions_ok: self.permissions_ok,
            permission_mode: self.permission_mode.clone(),
            sessions: self.sessions.clone(),
        };
        let changed = {
            let mut cur = self.view.write().unwrap_or_else(|p| p.into_inner());
            let changed = *cur != view;
            if changed {
                *cur = view;
            }
            changed
        };
        if changed || wake.announce {
            let _ = self.events.send(CoreEvent::ComputerUpdated);
        }
        let err = (!errors.is_empty()).then(|| errors.join("; "));
        self.tel.event(
            "computer.poll",
            Some(ms(start.elapsed())),
            err.is_none(),
            json!({"method": method, "socket": self.socket, "bin": self.bin, "wake_permissions": wake.permissions,
                   "wake_announce": wake.announce}),
            json!({"installed": mtime.is_some(), "daemon_running": running, "sessions": self.sessions.len(),
                   "cli_calls": cost.cli_calls, "child_cpu_ms": round2(cost.child_cpu_ms),
                   "version_checked": version_checked.then(|| self.version.clone()),
                   "permissions_checked": permissions_checked, "changed": changed}),
            err.as_deref(),
        );
        match (mtime.is_some(), running) {
            (false, _) => POLL_MISSING,
            (true, false) => POLL_STOPPED,
            (true, true) => POLL_RUNNING,
        }
    }

    /// `cua-driver --version` → `0.38.0`.
    fn read_version(&self, cost: &mut Cost, errors: &mut Vec<String>) -> Option<String> {
        let argv: [&std::ffi::OsStr; 2] = [self.bin.as_os_str(), "--version".as_ref()];
        match cost.run(&argv) {
            Ok(o) if o.success() => {
                let out = String::from_utf8_lossy(&o.stdout);
                let v = out.trim();
                Some(v.strip_prefix("cua-driver ").unwrap_or(v).to_owned()).filter(|v| !v.is_empty())
            }
            Ok(o) => {
                errors.push(format!("--version: {}", o.failure()));
                None
            }
            Err(e) => {
                errors.push(format!("--version: spawn: {e}"));
                None
            }
        }
    }

    fn daemon(&mut self, cost: &mut Cost, errors: &mut Vec<String>) -> Daemon {
        if !self.cli_fallback {
            match socket_request(&self.socket, "sessions_list") {
                Ok(v) => {
                    return match serde_json::from_value::<SessionList>(v) {
                        Ok(l) => Daemon::Running(Some(l.sessions)),
                        Err(e) => {
                            errors.push(format!("sessions_list: {e}"));
                            Daemon::Running(None)
                        }
                    };
                }
                Err(SocketError::NotRunning) => return Daemon::Stopped,
                Err(SocketError::Failed(e)) => {
                    errors.push(format!("sessions_list: {e}"));
                    return Daemon::Running(None);
                }
                Err(SocketError::Rejected(e)) => {
                    errors.push(format!("sessions_list rejected ({e}); using the CLI from now on"));
                    self.cli_fallback = true;
                }
            }
        }
        let argv: [&std::ffi::OsStr; 3] = [self.bin.as_os_str(), "sessions".as_ref(), "--json".as_ref()];
        match cost.run(&argv) {
            Ok(o) if o.success() => match serde_json::from_slice::<SessionList>(&o.stdout) {
                Ok(l) => Daemon::Running(Some(l.sessions)),
                Err(e) => {
                    errors.push(format!("sessions --json: {e}"));
                    Daemon::Running(None)
                }
            },
            // `session listing failed: connect to …: No such file or directory`.
            Ok(o) if o.stderr.contains("connect to") => Daemon::Stopped,
            Ok(o) => {
                errors.push(format!("sessions --json: {}", o.failure()));
                Daemon::Running(None)
            }
            Err(e) => {
                errors.push(format!("sessions --json: spawn: {e}"));
                Daemon::Stopped
            }
        }
    }

    /// Permission mode (socket `authorization_status`, or the CLI `status` text) and the TCC
    /// grants (`permissions status --json`).
    fn read_permissions(&mut self, cost: &mut Cost, errors: &mut Vec<String>) {
        let mode = if self.cli_fallback {
            let argv: [&std::ffi::OsStr; 2] = [self.bin.as_os_str(), "status".as_ref()];
            match cost.run(&argv) {
                Ok(o) => Ok(status_permission_mode(&String::from_utf8_lossy(&o.stdout))),
                Err(e) => Err(format!("status: spawn: {e}")),
            }
        } else {
            match socket_request(&self.socket, "authorization_status") {
                Ok(v) => Ok(v["permission_mode"].as_str().map(str::to_owned)),
                Err(SocketError::NotRunning) => Ok(None),
                Err(SocketError::Rejected(e) | SocketError::Failed(e)) => Err(format!("authorization_status: {e}")),
            }
        };
        match mode {
            Ok(m) => self.permission_mode = m,
            Err(e) => errors.push(e),
        }
        let argv: [&std::ffi::OsStr; 4] =
            [self.bin.as_os_str(), "permissions".as_ref(), "status".as_ref(), "--json".as_ref()];
        match cost.run(&argv) {
            Ok(o) if o.success() => match serde_json::from_slice::<Value>(&o.stdout) {
                Ok(v) => self.permissions_ok = permissions_ok(&v),
                Err(e) => errors.push(format!("permissions status: {e}")),
            },
            Ok(o) => errors.push(format!("permissions status: {}", o.failure())),
            Err(e) => errors.push(format!("permissions status: spawn: {e}")),
        }
    }

    /// Replaces the session list, keeping each known session's first-seen `started_at` and
    /// deriving `last_action_at` from `idle_seconds`; logs starts, actions and ends.
    fn merge_sessions(&mut self, raw: Vec<RawSession>, now: i64) {
        let mut prev = std::mem::take(&mut self.sessions);
        let mut next: Vec<ComputerSession> = Vec::with_capacity(raw.len());
        for r in raw {
            let base = r.session.clone().or_else(|| r.owner_short_id.clone()).unwrap_or_else(|| "implicit".to_owned());
            let mut id = base.clone();
            let mut n = 1;
            while next.iter().any(|s| s.id == id) {
                n += 1;
                id = format!("{base}#{n}");
            }
            let action_ms = now - r.idle_seconds as i64 * 1000;
            let old = prev.iter().position(|s| s.id == id).map(|i| prev.swap_remove(i));
            let started_at = match &old {
                Some(o) => o.started_at.clone(),
                None => Some(rfc3339_from_ms(now - r.started_seconds_ago as i64 * 1000)),
            };
            let started_ms = started_at.as_deref().and_then(parse_rfc3339_ms).unwrap_or(now);
            let last_ms = old.as_ref().and_then(|o| o.last_action_at.as_deref()).and_then(parse_rfc3339_ms);
            let acted = action_ms > last_ms.unwrap_or(started_ms) + ACTION_SLACK_MS;
            let last_action_at = if acted {
                Some(rfc3339_from_ms(action_ms))
            } else {
                old.as_ref().and_then(|o| o.last_action_at.clone())
            };
            let session = ComputerSession {
                id: id.clone(),
                client: r.client_kind.clone(),
                started_at,
                app: None,
                last_action: None,
                last_action_at,
            };
            let fields = json!({"session": id, "label": r.session, "client_kind": r.client_kind, "transport": r.transport,
                                "owner_short_id": r.owner_short_id, "state": r.state,
                                "started_seconds_ago": r.started_seconds_ago, "idle_seconds": r.idle_seconds});
            if old.is_none() {
                let out = json!({"started_at": session.started_at, "last_action_at": session.last_action_at});
                self.tel.event("computer.session_start", None, true, fields, out, None);
            } else if acted {
                let out = json!({"last_action_at": session.last_action_at, "source": "idle_seconds"});
                self.tel.event("computer.action", None, true, fields, out, None);
            }
            next.push(session);
        }
        for gone in prev {
            let lived_s = gone.started_at.as_deref().and_then(parse_rfc3339_ms).map(|s| (now - s) / 1000);
            let input = json!({"session": gone.id, "client_kind": gone.client});
            let out = json!({"started_at": gone.started_at, "last_action_at": gone.last_action_at, "lived_s": lived_s});
            self.tel.event("computer.session_end", None, true, input, out, None);
        }
        self.sessions = next;
    }
}

/// `permission mode: standard (built_in_default)` line of `cua-driver status`.
fn status_permission_mode(stdout: &str) -> Option<String> {
    stdout
        .lines()
        .find_map(|l| l.trim().strip_prefix("permission mode:"))
        .and_then(|rest| rest.split_whitespace().next())
        .map(str::to_owned)
}

/// `permissions status --json`: both grants as booleans → `Some(both)`; `"status": "unknown"`
/// (no daemon to ask) → `None`.
fn permissions_ok(v: &Value) -> Option<bool> {
    match (v["accessibility"].as_bool(), v["screen_recording"].as_bool()) {
        (Some(a), Some(s)) => Some(a && s),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_cli_outputs() {
        assert_eq!(revoked_count("Revoked 2 authorization scope(s)."), Some(2));
        assert_eq!(revoked_count("nope"), None);
        let status = "Cua Driver daemon is running\n  socket: /x.sock\n  pid: 1\n  permission mode: standard (built_in_default)\n";
        assert_eq!(status_permission_mode(status).as_deref(), Some("standard"));
        assert_eq!(permissions_ok(&json!({"accessibility": true, "screen_recording": false})), Some(false));
        assert_eq!(permissions_ok(&json!({"accessibility": true, "screen_recording": true})), Some(true));
        assert_eq!(permissions_ok(&json!({"daemon_running": false, "status": "unknown"})), None);
        let l: SessionList = serde_json::from_str(
            r#"{"count":1,"sessions":[{"session":null,"implicit":true,"state":"active","client_kind":"mcp",
                "transport":"mcp_stdio","owner_short_id":"ab12cd34","cursor_visible":false,"recording_active":false,
                "started_seconds_ago":5,"idle_seconds":2,"expires_in_seconds":600}]}"#,
        )
        .expect("parses");
        assert_eq!(l.sessions.len(), 1);
        assert_eq!(l.sessions[0].owner_short_id.as_deref(), Some("ab12cd34"));
    }
}
