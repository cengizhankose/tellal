//! T3 thread source: project discovery (read-only state DB), `t3 thread list/show` subprocesses
//! with per-call resource accounting, and the background poller that feeds `Core::threads`.

use std::collections::HashMap;
use std::ffi::OsStr;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use crossbeam_channel::Sender;
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

use crate::telemetry::{self, Telemetry, ms, round2};
use crate::{Config, CoreEvent, ThreadView};

/// Max concurrent `t3 thread list` processes per poll (each one boots the T3 runtime).
const LIST_CONCURRENCY: usize = 4;
/// Safety net: every project is re-listed this often even when the change query says idle.
const FULL_RELIST: Duration = Duration::from_secs(120);

/// Result of one subprocess run, reaped with `wait4` so CPU/RSS are exact for that child.
pub struct CmdOutput {
    pub stdout: Vec<u8>,
    pub stderr: String,
    /// `None` when the child was killed by a signal.
    pub exit_code: Option<i32>,
    pub wall_ms: f64,
    pub child_cpu_ms: f64,
    pub child_maxrss_mb: f64,
}

impl CmdOutput {
    pub fn success(&self) -> bool {
        self.exit_code == Some(0)
    }

    /// The `out` fields every subprocess event carries.
    pub fn telemetry(&self) -> Value {
        json!({
            "stdout_bytes": self.stdout.len(), "exit_code": self.exit_code,
            "child_cpu_ms": round2(self.child_cpu_ms), "child_maxrss_mb": round2(self.child_maxrss_mb),
        })
    }

    fn failure(&self) -> String {
        let code = self.exit_code.map_or_else(|| "signal".to_owned(), |c| c.to_string());
        format!("exit {code}: {}", self.stderr.trim())
    }
}

/// Runs `argv` to completion, capturing stdout/stderr, and reaps it with `wait4` for its rusage.
pub fn run_cmd<S: AsRef<OsStr>>(argv: &[S]) -> std::io::Result<CmdOutput> {
    let start = Instant::now();
    let mut child = Command::new(&argv[0])
        .args(&argv[1..])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let pid = child.id() as libc::pid_t;
    let mut child_err = child.stderr.take().expect("stderr piped");
    let err_reader = std::thread::spawn(move || {
        let mut s = String::new();
        let _ = child_err.read_to_string(&mut s);
        s
    });
    let mut stdout = Vec::new();
    child.stdout.take().expect("stdout piped").read_to_end(&mut stdout)?;
    let stderr = err_reader.join().unwrap_or_default();
    let mut status: libc::c_int = 0;
    // SAFETY: zeroed rusage is a valid out-parameter for wait4.
    let mut ru: libc::rusage = unsafe { std::mem::zeroed() };
    loop {
        // SAFETY: `pid` is our un-reaped child; std's `Child` is never waited on afterwards.
        let r = unsafe { libc::wait4(pid, &mut status, 0, &mut ru) };
        if r == pid {
            break;
        }
        let e = std::io::Error::last_os_error();
        if e.kind() != std::io::ErrorKind::Interrupted {
            return Err(e);
        }
    }
    let exit_code = libc::WIFEXITED(status).then(|| libc::WEXITSTATUS(status));
    Ok(CmdOutput {
        stdout,
        stderr,
        exit_code,
        wall_ms: ms(start.elapsed()),
        child_cpu_ms: telemetry::rusage_cpu_ms(&ru),
        child_maxrss_mb: telemetry::maxrss_to_mb(ru.ru_maxrss),
    })
}

fn argv_json<S: AsRef<OsStr>>(argv: &[S]) -> Value {
    argv.iter().map(|a| Value::String(a.as_ref().to_string_lossy().into_owned())).collect()
}

/// A watched T3 project.
#[derive(Clone, Debug)]
pub struct Project {
    pub id: String,
    pub title: String,
    pub root: String,
}

impl Project {
    /// Human label for the panel: title, else workspace dir name, else id.
    pub fn display_name(&self) -> &str {
        if !self.title.trim().is_empty() {
            return &self.title;
        }
        match self.root.trim_end_matches('/').rsplit('/').next() {
            Some(base) if !base.is_empty() => base,
            _ => &self.id,
        }
    }
}

/// One row of `t3 thread list --json` (and the `thread` object of `t3 thread show --json`).
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawThread {
    pub thread_id: String,
    #[serde(default)]
    pub project_id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub settled: bool,
    #[serde(default)]
    pub updated_at: String,
    #[serde(default)]
    pub link: Option<String>,
    #[serde(default)]
    pub provider_instance_id: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub latest_run_id: Option<String>,
}

impl RawThread {
    fn view(&self) -> ThreadView {
        ThreadView {
            id: self.thread_id.clone(),
            project: self.project_id.clone(),
            title: self.title.clone(),
            status: self.status.clone(),
            settled: self.settled,
            updated_at: self.updated_at.clone(),
            link: self.link.clone().unwrap_or_default(),
            provider: self.provider_instance_id.clone().unwrap_or_default(),
            model: self.model.clone().unwrap_or_default(),
        }
    }
}

/// One message of `t3 thread show --json`, oldest → newest.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    pub role: String,
    #[serde(default)]
    pub run_id: Option<String>,
    #[serde(default)]
    pub text: String,
}

#[derive(Debug, Deserialize)]
pub struct ShowResult {
    pub thread: RawThread,
    #[serde(default)]
    pub messages: Vec<Message>,
}

fn state_db() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".t3/userdata/statev2.sqlite")
}

/// Projects to watch: `Config.projects` (incl. `T3P_PROJECTS`) or the live projects in T3's
/// state DB, read through a `sqlite3 -readonly` subprocess (no SQLite linked into the panel).
pub fn discover(tel: &Telemetry, cfg: &Config) -> anyhow::Result<Vec<Project>> {
    if let Some(ids) = &cfg.projects {
        let projects: Vec<Project> = ids
            .iter()
            .map(|id| Project { id: id.clone(), title: String::new(), root: id.clone() })
            .collect();
        tel.event(
            "t3.discover",
            Some(0.0),
            true,
            json!({"source": "config", "projects": ids}),
            json!({"projects": projects.len()}),
            None,
        );
        return Ok(projects);
    }
    #[derive(Deserialize)]
    struct Row {
        project_id: String,
        title: Option<String>,
        workspace_root: Option<String>,
    }
    let sql = "SELECT project_id, title, workspace_root FROM projection_projects \
               WHERE deleted_at IS NULL AND archived_at IS NULL ORDER BY title";
    let (input, res) = sqlite_json::<Row>(sql);
    match res {
        Ok((o, rows)) => {
            let projects: Vec<Project> = rows
                .into_iter()
                .map(|r| Project {
                    id: r.project_id,
                    title: r.title.unwrap_or_default(),
                    root: r.workspace_root.unwrap_or_default(),
                })
                .collect();
            let mut out = o.telemetry();
            out["projects"] = projects
                .iter()
                .map(|p| json!({"id": p.id, "title": p.title, "root": p.root}))
                .collect();
            tel.event("t3.discover", Some(o.wall_ms), true, input, out, None);
            Ok(projects)
        }
        Err(e) => {
            tel.event("t3.discover", None, false, input, Value::Null, Some(&format!("{e:#}")));
            Err(e)
        }
    }
}

/// Runs one read-only query against T3's state DB through `/usr/bin/sqlite3 -readonly -json`.
/// Returns the event `in` (source, db, argv) and the parsed rows.
fn sqlite_json<T: DeserializeOwned>(sql: &str) -> (Value, anyhow::Result<(CmdOutput, Vec<T>)>) {
    let db = state_db();
    let argv: [&OsStr; 5] =
        ["/usr/bin/sqlite3".as_ref(), "-readonly".as_ref(), "-json".as_ref(), db.as_os_str(), sql.as_ref()];
    let input = json!({"source": "sqlite3 -readonly subprocess", "db": db, "argv": argv_json(&argv)});
    let res = run_cmd(&argv).map_err(anyhow::Error::from).and_then(|o| {
        if !o.success() {
            return Err(anyhow::anyhow!(o.failure()));
        }
        // sqlite3 -json prints nothing at all for an empty result set.
        let rows =
            if o.stdout.iter().all(u8::is_ascii_whitespace) { Vec::new() } else { serde_json::from_slice(&o.stdout)? };
        Ok((o, rows))
    });
    (input, res)
}

/// `max(updated_at)` per project from T3's thread projection (indexed on
/// `(project_id, updated_at)`). A thread's `updated_at` moves on every run start/finish, so a
/// changed value means that project's `t3 thread list` output may have changed.
pub fn project_changes() -> (Value, anyhow::Result<(CmdOutput, HashMap<String, String>)>) {
    #[derive(Deserialize)]
    struct Row {
        project_id: String,
        u: Option<String>,
    }
    let sql = "select project_id, max(updated_at) as u from orchestration_v2_projection_threads \
               where deleted_at is null group by project_id;";
    let (input, res) = sqlite_json::<Row>(sql);
    let res = res.map(|(o, rows)| (o, rows.into_iter().filter_map(|r| Some((r.project_id, r.u?))).collect()));
    (input, res)
}

/// `t3 thread list --project <id> --json`, logged as `t3.list`.
pub fn list_threads(tel: &Telemetry, t3_bin: &Path, project: &str) -> anyhow::Result<Vec<RawThread>> {
    let argv: [&OsStr; 6] = [
        t3_bin.as_os_str(),
        "thread".as_ref(),
        "list".as_ref(),
        "--project".as_ref(),
        project.as_ref(),
        "--json".as_ref(),
    ];
    let input = json!({"project": project, "argv": argv_json(&argv)});
    run_logged(tel, "t3.list", &argv, input, |stdout| {
        let threads: Vec<RawThread> = serde_json::from_slice(stdout)?;
        let n = threads.len();
        Ok((threads, json!({"threads": n})))
    })
}

/// `t3 thread show <id> --json --messages <n>`, logged as `t3.show`.
pub fn show_thread(tel: &Telemetry, t3_bin: &Path, thread: &str, messages: u32) -> anyhow::Result<ShowResult> {
    let n = messages.to_string();
    let argv: [&OsStr; 7] = [
        t3_bin.as_os_str(),
        "thread".as_ref(),
        "show".as_ref(),
        thread.as_ref(),
        "--json".as_ref(),
        "--messages".as_ref(),
        n.as_ref(),
    ];
    let input = json!({"thread": thread, "argv": argv_json(&argv)});
    run_logged(tel, "t3.show", &argv, input, |stdout| {
        let show: ShowResult = serde_json::from_slice(stdout)?;
        let out = json!({"status": show.thread.status, "messages": show.messages.len()});
        Ok((show, out))
    })
}

/// Runs a subprocess and writes one `kind` event with argv, wall time, stdout bytes, exit code,
/// child CPU and max RSS plus whatever `parse` adds.
pub fn run_logged<S: AsRef<OsStr>, T>(
    tel: &Telemetry,
    kind: &str,
    argv: &[S],
    input: Value,
    parse: impl FnOnce(&[u8]) -> anyhow::Result<(T, Value)>,
) -> anyhow::Result<T> {
    let o = match run_cmd(argv) {
        Ok(o) => o,
        Err(e) => {
            tel.event(kind, None, false, input, Value::Null, Some(&format!("spawn: {e}")));
            return Err(e.into());
        }
    };
    let mut out = o.telemetry();
    let res = if o.success() { parse(&o.stdout) } else { Err(anyhow::anyhow!(o.failure())) };
    match res {
        Ok((value, extra)) => {
            if let (Value::Object(dst), Value::Object(src)) = (&mut out, extra) {
                dst.extend(src);
            }
            tel.event(kind, Some(o.wall_ms), true, input, out, None);
            Ok(value)
        }
        Err(e) => {
            let err = format!("{e:#}");
            tel.event(kind, Some(o.wall_ms), false, input, out, Some(&err));
            Err(e)
        }
    }
}

/// Lists all projects with at most [`LIST_CONCURRENCY`] concurrent `t3` processes. Results are
/// in `projects` order.
pub fn list_all(tel: &Telemetry, cfg: &Config, projects: &[Project]) -> Vec<anyhow::Result<Vec<RawThread>>> {
    let mut results: Vec<Option<anyhow::Result<Vec<RawThread>>>> = (0..projects.len()).map(|_| None).collect();
    for (chunk_idx, chunk) in projects.chunks(LIST_CONCURRENCY).enumerate() {
        std::thread::scope(|s| {
            let handles: Vec<_> = chunk
                .iter()
                .map(|p| s.spawn(|| list_threads(tel, &cfg.t3_bin, &p.id)))
                .collect();
            for (i, h) in handles.into_iter().enumerate() {
                let r = h.join().unwrap_or_else(|_| Err(anyhow::anyhow!("list thread panicked")));
                results[chunk_idx * LIST_CONCURRENCY + i] = Some(r);
            }
        });
    }
    results.into_iter().map(|r| r.expect("every project listed")).collect()
}

/// Filters (unsettled or updated within `recent_window`), sorts (unsettled first, newest first)
/// and caps to `max_threads`.
pub fn select(cfg: &Config, threads: impl IntoIterator<Item = RawThread>, now_ms: i64) -> Vec<ThreadView> {
    let window_ms = cfg.recent_window.as_millis() as i64;
    let mut rows: Vec<(i64, RawThread)> = threads
        .into_iter()
        .map(|t| (telemetry::parse_rfc3339_ms(&t.updated_at).unwrap_or(0), t))
        .filter(|(upd, t)| !t.settled || now_ms - upd <= window_ms)
        .collect();
    rows.sort_by(|(ua, a), (ub, b)| a.settled.cmp(&b.settled).then(ub.cmp(ua)));
    rows.truncate(cfg.max_threads);
    rows.iter().map(|(_, t)| t.view()).collect()
}

/// Newest `updated_at` of a watched project in a `t3.changes` result.
fn newest(changes: &HashMap<String, String>, canonical: &HashMap<String, String>, p: &Project) -> Option<String> {
    changes.get(canonical.get(&p.id).unwrap_or(&p.id)).cloned()
}

/// Spawns the poller thread: discover once, then every `poll_interval` run the cheap
/// `t3.changes` query and `t3 thread list` only the projects whose newest `updated_at` moved
/// (all projects on the first tick, every [`FULL_RELIST`], or when the query fails).
pub(crate) fn spawn_poller(
    tel: Arc<Telemetry>,
    cfg: Config,
    snapshot: Arc<RwLock<Vec<ThreadView>>>,
    tx: Sender<CoreEvent>,
) -> std::io::Result<()> {
    std::thread::Builder::new().name("t3p-poller".into()).spawn(move || {
        let mut projects: Vec<Project> = Vec::new();
        // Last good list per project, so unchanged or failing projects keep their rows.
        let mut by_project: HashMap<String, Vec<RawThread>> = HashMap::new();
        // Watched id (may be a path from config) → T3 project id, learned from list output.
        let mut canonical: HashMap<String, String> = HashMap::new();
        // Watched id → max(updated_at) observed right before its last successful list.
        let mut listed_at: HashMap<String, Option<String>> = HashMap::new();
        let mut statuses: HashMap<String, String> = HashMap::new();
        let mut last_full: Option<Instant> = None;
        let mut first = true;
        loop {
            let tick = Instant::now();
            if projects.is_empty() {
                match discover(&tel, &cfg) {
                    Ok(p) => projects = p,
                    Err(e) => {
                        let _ = tx.send(CoreEvent::Error(format!("project discovery failed: {e:#}")));
                    }
                }
            }
            let (input, changes) = project_changes();
            let (reason, full_cause, to_list): (&str, Option<&str>, Vec<usize>) = match &changes {
                Ok((o, map)) => {
                    let changed: Vec<usize> = (0..projects.len())
                        .filter(|&i| listed_at.get(&projects[i].id) != Some(&newest(map, &canonical, &projects[i])))
                        .collect();
                    let mut out = o.telemetry();
                    out["projects"] = json!(map.len());
                    out["changed"] = changed.iter().map(|&i| Value::String(projects[i].id.clone())).collect();
                    tel.event("t3.changes", Some(o.wall_ms), true, input, out, None);
                    if listed_at.is_empty() {
                        ("initial", None, (0..projects.len()).collect())
                    } else if last_full.is_none_or(|t| t.elapsed() >= FULL_RELIST) {
                        ("full", Some("interval"), (0..projects.len()).collect())
                    } else if changed.is_empty() {
                        ("idle", None, changed)
                    } else {
                        ("changed", None, changed)
                    }
                }
                Err(e) => {
                    tel.event("t3.changes", None, false, input, Value::Null, Some(&format!("{e:#}")));
                    let reason = if listed_at.is_empty() { "initial" } else { "full" };
                    (reason, Some("changes_query_failed"), (0..projects.len()).collect())
                }
            };
            let listing: Vec<Project> = to_list.iter().map(|&i| projects[i].clone()).collect();
            let results = list_all(&tel, &cfg, &listing);
            let mut failed = Vec::new();
            for (p, r) in listing.iter().zip(results) {
                match r {
                    Ok(list) => {
                        if let Some(t) = list.first() {
                            canonical.insert(p.id.clone(), t.project_id.clone());
                        }
                        // Value read *before* listing: a change racing the list is caught next tick.
                        let seen = changes.as_ref().ok().and_then(|(_, map)| newest(map, &canonical, p));
                        listed_at.insert(p.id.clone(), seen);
                        by_project.insert(p.id.clone(), list);
                    }
                    Err(e) => {
                        failed.push(format!("{}: {e:#}", p.display_name()));
                    }
                }
            }
            if listing.len() == projects.len() {
                last_full = Some(Instant::now());
            }
            for t in by_project.values().flatten() {
                let prev = statuses.insert(t.thread_id.clone(), t.status.clone());
                if !first && prev.as_deref() != Some(t.status.as_str()) {
                    tel.event(
                        "state.change",
                        None,
                        true,
                        json!({"thread": t.thread_id, "title": t.title, "from": prev, "to": t.status}),
                        Value::Null,
                        None,
                    );
                }
            }
            first = false;
            let total = by_project.values().map(Vec::len).sum::<usize>();
            let mut views = select(&cfg, by_project.values().flatten().cloned(), telemetry::now_unix_ms());
            for v in &mut views {
                if let Some(p) = projects.iter().find(|p| canonical.get(&p.id).unwrap_or(&p.id) == &v.project) {
                    v.project = p.display_name().to_owned();
                }
            }
            let poll_ms = ms(tick.elapsed());
            tel.event(
                "t3.poll",
                Some(poll_ms),
                failed.is_empty(),
                json!({"projects": projects.len()}),
                json!({"reason": reason, "full_cause": full_cause, "projects_listed": listing.len(),
                       "projects_skipped": projects.len() - listing.len(),
                       "threads_total": total, "threads_shown": views.len()}),
                (!failed.is_empty()).then(|| failed.join("; ")).as_deref(),
            );
            *snapshot.write().unwrap_or_else(|p| p.into_inner()) = views;
            tel.update_stats(|s| s.last_t3_poll_ms = Some(poll_ms));
            if !failed.is_empty() {
                let _ = tx.send(CoreEvent::Error(format!("t3 list failed: {}", failed.join("; "))));
            }
            let _ = tx.send(CoreEvent::ThreadsUpdated);
            std::thread::sleep(cfg.poll_interval.saturating_sub(tick.elapsed()).max(Duration::from_millis(200)));
        }
    })?;
    Ok(())
}
