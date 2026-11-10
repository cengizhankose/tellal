//! Live tail: every `live_poll_interval`, one read-only `sqlite3` query (two `UNION ALL` parts)
//! for new complete assistant messages and waiting user requests of the listed threads.
//!
//! Per-thread cursor = newest `created_at` handled (initially `Core::start` time, so history is
//! never replayed) plus the message ids seen at exactly that timestamp.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use serde::Deserialize;
use serde_json::{Value, json};

use crate::extract::live_line;
use crate::speech::Speech;
use crate::t3::{self, sql_str};
use crate::telemetry::{Telemetry, ms, parse_rfc3339_ms, rfc3339_from_ms};
use crate::ThreadView;

/// Spoken on a listening channel when its thread starts waiting for the user.
const WAIT_TEXT: &str = "senden cevap bekliyor.";

/// One row of the tail query: `k` = `m` (assistant message: `ts` = created_at, `text`) or
/// `w` (waiting request: `ts` = updated_at, `text` = request type).
#[derive(Deserialize)]
struct Row {
    k: String,
    id: String,
    thread: String,
    ts: String,
    text: String,
}

struct Cursor {
    at: String,
    seen: HashSet<String>,
}

fn query(cursors: &HashMap<String, Cursor>, ids: &[String]) -> String {
    let msgs: Vec<String> = ids
        .iter()
        .map(|id| format!("(thread_id = {} and created_at >= {})", sql_str(id), sql_str(&cursors[id].at)))
        .collect();
    let list: Vec<String> = ids.iter().map(|id| sql_str(id)).collect();
    // `status in (...)` lets SQLite use the partial index on active turn items.
    format!(
        "select 'm' as k, message_id as id, thread_id as thread, created_at as ts, \
         coalesce(json_extract(payload_json,'$.text'),'') as text \
         from orchestration_v2_projection_messages \
         where role = 'assistant' and streaming = 0 and ({}) \
         union all \
         select 'w', turn_item_id, thread_id, updated_at, type \
         from orchestration_v2_projection_turn_items \
         where thread_id in ({}) and status in ('pending','running','waiting') and status = 'waiting' \
         and type in ('user_input_request','approval_request','secret_request') \
         order by 1, 4, 2;",
        msgs.join(" or "),
        list.join(",")
    )
}

/// Spawns the `t3p-live` thread.
pub(crate) fn spawn(
    tel: Arc<Telemetry>,
    interval: Duration,
    start_ms: i64,
    threads: Arc<RwLock<Vec<ThreadView>>>,
    speech: Speech,
) -> std::io::Result<()> {
    std::thread::Builder::new().name("t3p-live".into()).spawn(move || {
        let start = rfc3339_from_ms(start_ms);
        let mut cursors: HashMap<String, Cursor> = HashMap::new();
        // Waiting request ids already handled, per thread.
        let mut requests: HashMap<String, HashSet<String>> = HashMap::new();
        let mut prev_ids: Vec<String> = Vec::new();
        loop {
            let tick = Instant::now();
            let ids: Vec<String> =
                threads.read().unwrap_or_else(|p| p.into_inner()).iter().map(|t| t.id.clone()).collect();
            if !ids.is_empty() {
                // Threads listed for the first time: their already-waiting requests are not news.
                let fresh: HashSet<&str> =
                    ids.iter().filter(|id| !requests.contains_key(*id)).map(String::as_str).collect();
                for id in &ids {
                    cursors.entry(id.clone()).or_insert_with(|| Cursor { at: start.clone(), seen: HashSet::new() });
                }
                let sql = query(&cursors, &ids);
                let (mut input, res) = t3::sqlite_json::<Row>(&t3::state_db(), &sql);
                input["threads"] = json!(ids.len());
                // The full statement only when the thread set changed (it is long and runs every second).
                if ids == prev_ids
                    && let Value::Object(o) = &mut input
                {
                    o.remove("argv");
                }
                match res {
                    Ok((o, rows)) => {
                        let (mut new_msgs, mut waiting_n) = (0, 0);
                        let mut waiting: HashSet<&str> = HashSet::new();
                        for row in &rows {
                            if row.k == "w" {
                                waiting_n += 1;
                                waiting.insert(row.thread.as_str());
                                let seen = requests.entry(row.thread.clone()).or_default();
                                if seen.insert(row.id.clone()) {
                                    let initial = fresh.contains(row.thread.as_str());
                                    let spoken = !initial && speech.notice(&row.thread, "wait", WAIT_TEXT);
                                    tel.event(
                                        "wait.detect",
                                        None,
                                        true,
                                        json!({"thread": row.thread, "request": row.id, "type": row.text, "updated_at": row.ts}),
                                        json!({"initial": initial, "spoken": spoken}),
                                        None,
                                    );
                                }
                                continue;
                            }
                            let Some(cur) = cursors.get_mut(&row.thread) else { continue };
                            if row.ts < cur.at || (row.ts == cur.at && cur.seen.contains(&row.id)) {
                                continue;
                            }
                            if row.ts > cur.at {
                                cur.at.clone_from(&row.ts);
                                cur.seen.clear();
                            }
                            cur.seen.insert(row.id.clone());
                            new_msgs += 1;
                            let line = live_line(&row.text);
                            let created_ms = parse_rfc3339_ms(&row.ts).unwrap_or(start_ms);
                            let action = speech.live_message(&row.thread, created_ms, &line.text);
                            tel.event(
                                "live.message",
                                None,
                                true,
                                json!({"thread": row.thread, "message_id": row.id, "created_at": row.ts}),
                                json!({"chars": row.text.chars().count(), "rule": line.rule, "action": action, "text": line.text}),
                                None,
                            );
                        }
                        for id in &ids {
                            requests.entry(id.clone()).or_default();
                        }
                        // Answered requests leave the set so the map stays small.
                        for (thread, seen) in &mut requests {
                            if !waiting.contains(thread.as_str()) {
                                seen.clear();
                            }
                        }
                        speech.set_waiting(&ids, &waiting);
                        let mut out = o.telemetry();
                        out["threads"] = json!(ids.len());
                        out["rows"] = json!(rows.len());
                        out["new_msgs"] = json!(new_msgs);
                        out["waiting"] = json!(waiting_n);
                        tel.event("live.poll", Some(o.wall_ms), true, input, out, None);
                        tel.update_stats(|s| s.last_live_poll_ms = Some(o.wall_ms));
                    }
                    Err(e) => {
                        tel.event("live.poll", Some(ms(tick.elapsed())), false, input, Value::Null, Some(&format!("{e:#}")));
                    }
                }
                prev_ids = ids;
            }
            std::thread::sleep(interval.saturating_sub(tick.elapsed()).max(Duration::from_millis(100)));
        }
    })?;
    Ok(())
}
