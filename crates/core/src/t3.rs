//! T3 thread source: project discovery and message/request reads (read-only state DB through
//! `sqlite3`), `t3 thread list|send` subprocesses with per-call resource accounting, and the
//! background poller that feeds `Core::threads`.

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
use crate::{Config, CoreEvent, ProjectIcon, ThreadView};

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

    pub(crate) fn failure(&self) -> String {
        let code = self.exit_code.map_or_else(|| "signal".to_owned(), |c| c.to_string());
        format!("exit {code}: {}", self.stderr.trim())
    }
}

/// Runs `argv` to completion (feeding `stdin`, if any), capturing stdout/stderr, and reaps it
/// with `wait4` for its rusage.
pub fn run_cmd<S: AsRef<OsStr>>(argv: &[S], stdin: Option<&[u8]>) -> std::io::Result<CmdOutput> {
    let start = Instant::now();
    let mut child = Command::new(&argv[0])
        .args(&argv[1..])
        .stdin(if stdin.is_some() { Stdio::piped() } else { Stdio::null() })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    if let (Some(data), Some(mut pipe)) = (stdin, child.stdin.take()) {
        use std::io::Write;
        // Small payloads (replies); dropping the pipe sends EOF. A write error means the child
        // already exited, which its exit code reports.
        let _ = pipe.write_all(data);
    }
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

pub(crate) fn argv_json<S: AsRef<OsStr>>(argv: &[S]) -> Value {
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

/// One row of `t3 thread list --json`.
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
    pub created_at: String,
    #[serde(default)]
    pub snoozed: bool,
    #[serde(default)]
    pub snoozed_until: Option<String>,
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
    /// The panel row: list fields plus the project, pin and snooze state from `meta`.
    fn into_view(self, meta: &Meta, now_ms: i64) -> ThreadView {
        let project = meta.projects.get(&self.project_id);
        let until = meta.snoozed_until.get(&self.thread_id).or(self.snoozed_until.as_ref());
        let snoozed = match until.and_then(|u| telemetry::parse_rfc3339_ms(u)) {
            Some(until_ms) => until_ms > now_ms,
            None => self.snoozed,
        };
        ThreadView {
            project: project
                .filter(|p| !p.name.trim().is_empty())
                .map_or_else(|| self.project_id.clone(), |p| p.name.clone()),
            project_icon: project.and_then(|p| p.icon.clone()),
            project_pinned: project.is_some_and(|p| p.pinned),
            pinned: meta.pins.contains_key(&self.thread_id),
            snoozed,
            running_since: meta.running_since.get(&self.thread_id).cloned(),
            id: self.thread_id,
            project_id: self.project_id,
            title: self.title,
            status: self.status,
            settled: self.settled,
            created_at: self.created_at,
            updated_at: self.updated_at,
            link: self.link.unwrap_or_default(),
            provider: self.provider_instance_id.unwrap_or_default(),
            model: self.model.unwrap_or_default(),
        }
    }
}

/// Sidebar state of a T3 project (`projection_projects`).
#[derive(Clone, Debug, Default)]
pub struct ProjectMeta {
    /// Display name: the T3 title; the poller fills an empty one from the discovered project
    /// ([`Project::display_name`]).
    pub name: String,
    pub icon: Option<ProjectIcon>,
    pub pinned: bool,
}

/// Sidebar state that `t3 thread list` does not print, read from T3's state DB every tick.
#[derive(Clone, Debug, Default)]
pub struct Meta {
    /// T3 project id → title, icon, pinned.
    pub projects: HashMap<String, ProjectMeta>,
    /// Pinned thread id → its `pinOrderKey` (T3 orders pins by it, ascending string compare;
    /// `None` when T3 stored no key).
    pub pins: HashMap<String, Option<String>>,
    /// Thread id → `snoozedUntil` (RFC3339) for threads that have one.
    pub snoozed_until: HashMap<String, String>,
    /// Thread id → start (RFC3339 `startedAt`, else `requestedAt`) of its earliest active run
    /// (queued/preparing/starting/running/waiting): what T3's "Working 16m" counts from.
    pub running_since: HashMap<String, String>,
}

/// One `t3.changes` query result.
pub struct Changes {
    /// T3 project id → newest thread `updated_at`.
    pub updated: HashMap<String, String>,
    pub meta: Meta,
    /// Non-null `project_icon_json` values that are not a valid icon (those projects get none).
    pub invalid_icons: usize,
}

/// A `project_icon_json` value; `Ok(None)` for JSON null, `Err` for anything unparsable.
fn parse_icon(json: &str) -> Result<Option<ProjectIcon>, serde_json::Error> {
    #[derive(Deserialize)]
    struct Raw {
        kind: String,
        name: String,
        #[serde(default)]
        color: Option<String>,
    }
    let raw: Option<Raw> = serde_json::from_str(json)?;
    Ok(raw.map(|r| ProjectIcon { kind: r.kind, name: r.name, color: r.color.filter(|c| !c.is_empty()) }))
}

/// One thread message, oldest → newest (read from T3's state DB).
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    pub role: String,
    #[serde(default)]
    pub run_id: Option<String>,
    #[serde(default)]
    pub text: String,
}

/// Path of T3's state DB (opened read-only only).
pub(crate) fn state_db() -> PathBuf {
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
    let (input, res) = sqlite_json::<Row>(&state_db(), sql);
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

/// Runs one read-only query through `/usr/bin/sqlite3 -readonly -json <db>`.
/// Returns the event `in` (source, db, argv) and the parsed rows.
pub(crate) fn sqlite_json<T: DeserializeOwned>(db: &Path, sql: &str) -> (Value, anyhow::Result<(CmdOutput, Vec<T>)>) {
    let argv: [&OsStr; 5] =
        ["/usr/bin/sqlite3".as_ref(), "-readonly".as_ref(), "-json".as_ref(), db.as_os_str(), sql.as_ref()];
    let input = json!({"source": "sqlite3 -readonly subprocess", "db": db, "argv": argv_json(&argv)});
    let res = run_cmd(&argv, None).map_err(anyhow::Error::from).and_then(|o| {
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

/// SQL string literal (`'` doubled).
pub(crate) fn sql_str(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

/// The latest `n` complete messages of a thread (oldest → newest) from T3's state DB, logged as
/// `t3.messages`.
pub fn latest_messages(tel: &Telemetry, thread: &str, n: u32) -> anyhow::Result<Vec<Message>> {
    let sql = format!(
        "select role, run_id as runId, coalesce(json_extract(payload_json,'$.text'),'') as text \
         from orchestration_v2_projection_messages where thread_id = {} and streaming = 0 \
         order by created_at desc, message_id desc limit {n};",
        sql_str(thread)
    );
    let (mut input, res) = sqlite_json::<Message>(&state_db(), &sql);
    input["thread"] = json!(thread);
    match res {
        Ok((o, mut rows)) => {
            rows.reverse();
            let mut out = o.telemetry();
            out["messages"] = json!(rows.len());
            tel.event("t3.messages", Some(o.wall_ms), true, input, out, None);
            Ok(rows)
        }
        Err(e) => {
            tel.event("t3.messages", None, false, input, Value::Null, Some(&format!("{e:#}")));
            Err(e)
        }
    }
}

/// One read-only query per poll tick, three row kinds:
/// - `u`: `max(updated_at)` per project from T3's thread projection (indexed on
///   `(project_id, updated_at)`). A thread's `updated_at` moves on every run start/finish, so a
///   changed value means that project's `t3 thread list` output may have changed.
/// - `p`: every live project's title, icon and pin.
/// - `t`: threads whose payload has `pinnedAt` or `snoozedUntil` (pin order key, snooze end).
/// - `r`: active runs (partial index on `status`) with their start time.
pub fn project_changes() -> (Value, anyhow::Result<(CmdOutput, Changes)>) {
    #[derive(Deserialize)]
    struct Row {
        kind: char,
        id: String,
        a: Option<String>,
        b: Option<String>,
        c: Option<String>,
    }
    let sql = "select 'u' as kind, project_id as id, max(updated_at) as a, null as b, null as c \
               from orchestration_v2_projection_threads where deleted_at is null group by project_id \
               union all \
               select 'p', project_id, title, project_icon_json, pinned_at \
               from projection_projects where deleted_at is null \
               union all \
               select 't', thread_id, cast(json_extract(p, '$.pinnedAt') as text), \
               cast(json_extract(p, '$.pinOrderKey') as text), cast(json_extract(p, '$.snoozedUntil') as text) \
               from (select thread_id, case when json_valid(payload_json) then payload_json else '{}' end as p \
               from orchestration_v2_projection_threads where deleted_at is null) \
               where json_extract(p, '$.pinnedAt') is not null or json_extract(p, '$.snoozedUntil') is not null \
               union all \
               select 'r', thread_id, coalesce(case when json_valid(payload_json) \
               then json_extract(payload_json, '$.startedAt') end, requested_at), null, null \
               from orchestration_v2_projection_runs \
               where status in ('queued', 'preparing', 'starting', 'running', 'waiting');";
    let (input, res) = sqlite_json::<Row>(&state_db(), sql);
    let res = res.map(|(o, rows)| {
        let mut ch = Changes { updated: HashMap::new(), meta: Meta::default(), invalid_icons: 0 };
        for r in rows {
            match r.kind {
                'u' => {
                    if let Some(u) = r.a {
                        ch.updated.insert(r.id, u);
                    }
                }
                'p' => {
                    let icon = r.b.as_deref().map_or(Ok(None), parse_icon).unwrap_or_else(|_| {
                        ch.invalid_icons += 1;
                        None
                    });
                    let meta = ProjectMeta { name: r.a.unwrap_or_default(), icon, pinned: r.c.is_some() };
                    ch.meta.projects.insert(r.id, meta);
                }
                't' => match (r.a.is_some(), r.c) {
                    (true, Some(until)) => {
                        ch.meta.snoozed_until.insert(r.id.clone(), until);
                        ch.meta.pins.insert(r.id, r.b);
                    }
                    (true, None) => {
                        ch.meta.pins.insert(r.id, r.b);
                    }
                    (false, Some(until)) => {
                        ch.meta.snoozed_until.insert(r.id, until);
                    }
                    (false, None) => {}
                },
                'r' => {
                    if let Some(since) = r.a {
                        let slot = ch.meta.running_since.entry(r.id).or_insert_with(|| since.clone());
                        if since < *slot {
                            *slot = since;
                        }
                    }
                }
                _ => {}
            }
        }
        (o, ch)
    });
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

/// Runs a subprocess and writes one `kind` event with argv, wall time, stdout bytes, exit code,
/// child CPU and max RSS plus whatever `parse` adds.
pub fn run_logged<S: AsRef<OsStr>, T>(
    tel: &Telemetry,
    kind: &str,
    argv: &[S],
    input: Value,
    parse: impl FnOnce(&[u8]) -> anyhow::Result<(T, Value)>,
) -> anyhow::Result<T> {
    let o = match run_cmd(argv, None) {
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

/// Filters (unsettled, pinned, or updated within `recent_window`), fills project/pin/snooze
/// state from `meta`, sorts in T3's sidebar order and caps to `max_threads`:
/// 1. pinned threads by `pinOrderKey` ascending (missing keys last), then newest `created_at`;
/// 2. active threads (not settled, not snoozed), newest `created_at` first;
/// 3. settled or snoozed threads, newest `created_at` first.
///
/// Ties break on thread id. `updated_at` never affects the order.
pub fn select(cfg: &Config, threads: impl IntoIterator<Item = RawThread>, now_ms: i64, meta: &Meta) -> Vec<ThreadView> {
    let window_ms = cfg.recent_window.as_millis() as i64;
    let mut rows: Vec<(u8, Option<&str>, i64, ThreadView)> = threads
        .into_iter()
        .filter_map(|t| {
            let pin = meta.pins.get(&t.thread_id);
            if pin.is_none() && t.settled && now_ms - telemetry::parse_rfc3339_ms(&t.updated_at).unwrap_or(0) > window_ms {
                return None;
            }
            let created_ms = telemetry::parse_rfc3339_ms(&t.created_at).unwrap_or(0);
            let v = t.into_view(meta, now_ms);
            let group = if v.pinned {
                0
            } else if v.settled || v.snoozed {
                2
            } else {
                1
            };
            Some((group, pin.and_then(Option::as_deref), created_ms, v))
        })
        .collect();
    rows.sort_by(|(ga, ka, ca, a), (gb, kb, cb, b)| {
        ga.cmp(gb)
            .then_with(|| ka.is_none().cmp(&kb.is_none()))
            .then_with(|| ka.cmp(kb))
            .then_with(|| cb.cmp(ca))
            .then_with(|| a.id.cmp(&b.id))
    });
    rows.truncate(cfg.max_threads);
    rows.into_iter().map(|(.., v)| v).collect()
}

/// Newest `updated_at` of a watched project in a `t3.changes` result.
fn newest(changes: &HashMap<String, String>, canonical: &HashMap<String, String>, p: &Project) -> Option<String> {
    changes.get(canonical.get(&p.id).unwrap_or(&p.id)).cloned()
}

/// Spawns the poller thread: discover once, then every `poll_interval` run the cheap
/// `t3.changes` query and `t3 thread list` only the projects whose newest `updated_at` moved
/// (all projects on the first tick, every [`FULL_RELIST`], or when the query fails).
/// `on_status(thread, to)` runs for every logged `state.change`.
pub(crate) fn spawn_poller(
    tel: Arc<Telemetry>,
    cfg: Config,
    snapshot: Arc<RwLock<Vec<ThreadView>>>,
    tx: Sender<CoreEvent>,
    on_status: impl Fn(&str, &str) + Send + 'static,
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
        // Last good `t3.changes` sidebar state (project titles/icons/pins, thread pins/snoozes).
        let mut meta = Meta::default();
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
                Ok((o, ch)) => {
                    let changed: Vec<usize> = (0..projects.len())
                        .filter(|&i| listed_at.get(&projects[i].id) != Some(&newest(&ch.updated, &canonical, &projects[i])))
                        .collect();
                    let now = telemetry::now_unix_ms();
                    let snoozed = ch
                        .meta
                        .snoozed_until
                        .values()
                        .filter(|u| telemetry::parse_rfc3339_ms(u).is_some_and(|u| u > now))
                        .count();
                    let mut out = o.telemetry();
                    out["projects"] = json!(ch.meta.projects.len());
                    out["thread_projects"] = json!(ch.updated.len());
                    out["pinned_threads"] = json!(ch.meta.pins.len());
                    out["active_runs"] = json!(ch.meta.running_since.len());
                    out["snoozed_threads"] = json!(snoozed);
                    out["invalid_icons"] = json!(ch.invalid_icons);
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
            // A failed query keeps the last known project meta, pins and snoozes.
            let changes = changes.map(|(_, ch)| {
                meta = ch.meta;
                ch.updated
            });
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
                        let seen = changes.as_ref().ok().and_then(|map| newest(map, &canonical, p));
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
            for p in &projects {
                let id = canonical.get(&p.id).unwrap_or(&p.id);
                match meta.projects.get_mut(id) {
                    Some(m) if !m.name.trim().is_empty() => {}
                    Some(m) => m.name = p.display_name().to_owned(),
                    None => {
                        let m = ProjectMeta { name: p.display_name().to_owned(), ..ProjectMeta::default() };
                        meta.projects.insert(id.clone(), m);
                    }
                }
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
                    on_status(&t.thread_id, &t.status);
                }
            }
            first = false;
            let total = by_project.values().map(Vec::len).sum::<usize>();
            let views = select(&cfg, by_project.values().flatten().cloned(), telemetry::now_unix_ms(), &meta);
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
            let connected = failed.is_empty();
            tel.update_stats(|s| {
                s.last_t3_poll_ms = Some(poll_ms);
                s.t3_connected = connected;
            });
            if !failed.is_empty() {
                let _ = tx.send(CoreEvent::Error(format!("t3 list failed: {}", failed.join("; "))));
            }
            let _ = tx.send(CoreEvent::ThreadsUpdated);
            std::thread::sleep(cfg.poll_interval.saturating_sub(tick.elapsed()).max(Duration::from_millis(200)));
        }
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(id: &str, created: &str, updated: &str, settled: bool) -> RawThread {
        RawThread {
            thread_id: id.into(),
            project_id: "p".into(),
            title: String::new(),
            status: String::new(),
            settled,
            updated_at: updated.into(),
            created_at: created.into(),
            snoozed: false,
            snoozed_until: None,
            link: None,
            provider_instance_id: None,
            model: None,
            latest_run_id: None,
        }
    }

    fn ids(views: &[ThreadView]) -> Vec<&str> {
        views.iter().map(|v| v.id.as_str()).collect()
    }

    #[test]
    fn select_orders_like_t3_sidebar() {
        let mut cfg = Config::from_env();
        cfg.max_threads = 100;
        cfg.recent_window = Duration::from_secs(6 * 3600);
        let now = telemetry::parse_rfc3339_ms("2026-11-11T12:00:00.000Z").expect("valid");
        let mut meta = Meta::default();
        meta.pins.insert("pin-b".into(), Some("a1".into()));
        meta.pins.insert("pin-a".into(), Some("a0".into()));
        meta.pins.insert("pin-nokey".into(), None);
        meta.snoozed_until.insert("snoozed".into(), "2026-11-12T06:00:00.000Z".into());
        meta.snoozed_until.insert("woke".into(), "2026-11-10T06:00:00.000Z".into());
        meta.projects.insert("p".into(), ProjectMeta { name: "Proj".into(), icon: None, pinned: true });
        let old = "2026-11-01T00:00:00.000Z";
        let threads = |busy: &[&str]| {
            let upd = |id: &str, base: &'static str| if busy.contains(&id) { "2026-11-11T11:59:59.000Z" } else { base };
            let mut list_snoozed = raw("list-snoozed", "2026-11-11T11:50:00.000Z", upd("list-snoozed", old), false);
            list_snoozed.snoozed = true;
            vec![
                raw("old-active", "2026-11-11T08:00:00.000Z", upd("old-active", old), false),
                raw("settled-recent", "2026-11-11T10:30:00.000Z", upd("settled-recent", "2026-11-11T11:00:00.000Z"), true),
                raw("pin-nokey", "2026-11-11T10:00:00.000Z", upd("pin-nokey", old), true),
                raw("snoozed", "2026-11-11T11:45:00.000Z", upd("snoozed", old), false),
                raw("settled-old", "2026-11-11T11:55:00.000Z", old, true),
                raw("pin-b", "2026-11-11T11:00:00.000Z", upd("pin-b", old), false),
                list_snoozed,
                raw("woke", "2026-11-11T07:00:00.000Z", upd("woke", old), false),
                raw("new-active", "2026-11-11T11:30:00.000Z", upd("new-active", old), false),
                raw("pin-a", "2026-11-11T09:00:00.000Z", upd("pin-a", old), false),
            ]
        };
        let expected = [
            "pin-a",
            "pin-b",
            "pin-nokey",
            "new-active",
            "old-active",
            "woke",
            "list-snoozed",
            "snoozed",
            "settled-recent",
        ];
        let views = select(&cfg, threads(&[]), now, &meta);
        assert_eq!(ids(&views), expected);
        // Activity never reorders.
        let busy = select(&cfg, threads(&["old-active", "woke", "settled-recent", "pin-b", "snoozed"]), now, &meta);
        assert_eq!(ids(&busy), expected);

        let v = |id: &str| views.iter().find(|v| v.id == id).expect("listed");
        assert!(v("pin-nokey").pinned && v("pin-nokey").settled);
        assert!(!v("woke").snoozed && v("snoozed").snoozed && v("list-snoozed").snoozed);
        assert!(v("new-active").project == "Proj" && v("new-active").project_id == "p" && v("new-active").project_pinned);

        cfg.max_threads = 2;
        assert_eq!(ids(&select(&cfg, threads(&[]), now, &meta)), ["pin-a", "pin-b"]);
    }
}
