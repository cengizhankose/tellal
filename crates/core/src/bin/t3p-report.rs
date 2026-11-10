//! `t3p-report [run-dir|latest]` — summarizes a run's events.jsonl + metrics.jsonl, writes
//! `report.json` and a self-contained `report.html` (inline SVG) into the run dir.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use t3_panel_core::Config;
use t3_panel_core::telemetry::{parse_rfc3339_ms, round2};

fn main() -> anyhow::Result<()> {
    let arg = std::env::args().nth(1).unwrap_or_else(|| "latest".into());
    let dir = resolve(&arg);
    let events = read_jsonl(&dir.join("events.jsonl"))?;
    let metrics = read_jsonl(&dir.join("metrics.jsonl")).unwrap_or_default();
    let report = summarize(&events, &metrics);
    print_summary(&dir, &report);
    std::fs::write(dir.join("report.json"), serde_json::to_vec_pretty(&report)?)?;
    let html = dir.join("report.html");
    std::fs::write(&html, render_html(&dir, &report, &events, &metrics))?;
    println!("\nreport.json: {}", dir.join("report.json").display());
    println!("report.html: {}", html.display());
    Ok(())
}

fn resolve(arg: &str) -> PathBuf {
    let p = PathBuf::from(arg);
    if p.is_dir() {
        return p;
    }
    let root = Config::from_env().log_root;
    let p = root.join(arg);
    std::fs::canonicalize(&p).unwrap_or(p)
}

fn read_jsonl(path: &Path) -> anyhow::Result<Vec<Value>> {
    let text = std::fs::read_to_string(path).map_err(|e| anyhow::anyhow!("{}: {e}", path.display()))?;
    Ok(text.lines().filter(|l| !l.trim().is_empty()).filter_map(|l| serde_json::from_str(l).ok()).collect())
}

#[derive(Default)]
struct Dist(Vec<f64>);

impl Dist {
    fn push(&mut self, v: Option<f64>) {
        if let Some(v) = v {
            self.0.push(v);
        }
    }
    fn json(&mut self) -> Value {
        if self.0.is_empty() {
            return json!({"n": 0});
        }
        self.0.sort_by(f64::total_cmp);
        let q = |p: f64| self.0[((self.0.len() - 1) as f64 * p).round() as usize];
        let sum: f64 = self.0.iter().sum();
        json!({"n": self.0.len(), "min": round2(self.0[0]), "avg": round2(sum / self.0.len() as f64),
               "p50": round2(q(0.5)), "p95": round2(q(0.95)), "max": round2(*self.0.last().unwrap())})
    }
}

fn summarize(events: &[Value], metrics: &[Value]) -> Value {
    let mut kinds: BTreeMap<&str, (usize, usize, Dist)> = BTreeMap::new();
    let mut c2a = Dist::default();
    let mut m2a = Dist::default();
    // live.poll: count, errors, dur, child CPU, rows, new messages
    let (mut live_n, mut live_err, mut live_dur, mut live_cpu, mut live_rows, mut live_new) =
        (0usize, 0usize, Dist::default(), 0.0f64, 0u64, 0u64);
    // audio.play: count, skipped, paused ms, label spoken
    let (mut audio_n, mut audio_skipped, mut audio_paused, mut audio_label) = (0usize, 0usize, 0.0f64, 0usize);
    // wispr.history: count, ok, latency from stop / from start, Wispr's e2eLatency
    let (mut dict_n, mut dict_ok, mut dict_stop, mut dict_start, mut dict_e2e) =
        (0usize, 0usize, Dist::default(), Dist::default(), Dist::default());
    // reply.send: count, ok, queued, dur
    let (mut reply_n, mut reply_ok, mut reply_queued, mut reply_dur) = (0usize, 0usize, 0usize, Dist::default());
    let mut channels: BTreeMap<&str, usize> = BTreeMap::new();
    let mut t3_cost: BTreeMap<&str, (usize, f64, f64)> = BTreeMap::new();
    let mut db_cost: BTreeMap<&str, (usize, f64, f64)> = BTreeMap::new();
    for e in events {
        let kind = e["kind"].as_str().unwrap_or("?");
        let entry = kinds.entry(kind).or_default();
        entry.0 += 1;
        if e["ok"] == Value::Bool(false) {
            entry.1 += 1;
        }
        entry.2.push(e["dur_ms"].as_f64());
        let (ok, inp, out) = (e["ok"] == true, &e["in"], &e["out"]);
        match kind {
            "live.poll" => {
                live_n += 1;
                live_err += usize::from(e["ok"] == false);
                live_dur.push(e["dur_ms"].as_f64());
                live_cpu += out["child_cpu_ms"].as_f64().unwrap_or(0.0);
                live_rows += out["rows"].as_u64().unwrap_or(0);
                live_new += out["new_msgs"].as_u64().unwrap_or(0);
            }
            "audio.play" => {
                c2a.push(out["click_to_audio_ms"].as_f64());
                m2a.push(out["msg_to_audio_ms"].as_f64());
                audio_n += 1;
                audio_skipped += usize::from(out["skipped"] == true);
                audio_paused += out["paused_ms"].as_f64().unwrap_or(0.0);
                audio_label += usize::from(out["label_spoken"] == true);
            }
            "wispr.history" => {
                dict_n += 1;
                dict_ok += usize::from(ok);
                dict_stop.push(out["latency_from_stop_ms"].as_f64());
                dict_start.push(out["latency_from_start_ms"].as_f64());
                dict_e2e.push(out["e2eLatency"].as_f64());
            }
            "reply.send" => {
                reply_n += 1;
                reply_ok += usize::from(ok);
                reply_queued += usize::from(inp["queued"] == true);
                reply_dur.push(e["dur_ms"].as_f64());
            }
            k if k.starts_with("channel.") => *channels.entry(k).or_default() += 1,
            _ => {}
        }
        // T3 CLI processes vs. the cheap read-only sqlite3 queries on T3's state DB.
        if let Some(cpu) = out["child_cpu_ms"].as_f64() {
            let bucket = match kind {
                "t3.list" | "reply.send" => &mut t3_cost,
                "t3.discover" | "t3.changes" | "t3.messages" | "live.poll" => &mut db_cost,
                _ => continue,
            };
            let c = bucket.entry(kind).or_default();
            c.0 += 1;
            c.1 += e["dur_ms"].as_f64().unwrap_or(0.0);
            c.2 += cpu;
        }
    }
    let per_kind: BTreeMap<&str, Value> = kinds
        .iter_mut()
        .map(|(k, (n, errs, d))| (*k, json!({"count": n, "errors": errs, "dur_ms": d.json()})))
        .collect();
    let mut rss = Dist::default();
    let mut cpu = Dist::default();
    for m in metrics {
        rss.push(m["rss_mb"].as_f64());
        cpu.push(m["cpu_pct"].as_f64());
    }
    let span_s = match (metrics.first(), metrics.last()) {
        (Some(a), Some(b)) => ts_ms(b).zip(ts_ms(a)).map_or(0.0, |(b, a)| (b - a) as f64 / 1000.0),
        _ => 0.0,
    };
    let totals = |m: &BTreeMap<&str, (usize, f64, f64)>| {
        let (calls, wall, cpu) = m.values().fold((0, 0.0, 0.0), |a, c| (a.0 + c.0, a.1 + c.1, a.2 + c.2));
        let by_kind: BTreeMap<String, Value> = m
            .iter()
            .map(|(k, c)| (k.to_string(), json!({"calls": c.0, "wall_ms": round2(c.1), "child_cpu_ms": round2(c.2)})))
            .collect();
        json!({"calls": calls, "wall_ms": round2(wall), "child_cpu_ms": round2(cpu), "by_kind": by_kind})
    };
    json!({
        "events": events.len(),
        "kinds": per_kind,
        "metrics": {"rows": metrics.len(), "span_s": round2(span_s),
                    "rows_per_s": if span_s > 0.0 { round2((metrics.len() - 1) as f64 / span_s) } else { 0.0 },
                    "rss_mb": rss.json(), "cpu_pct": cpu.json(),
                    "children_cpu_ms_total": metrics.last().and_then(|m| m["children_cpu_ms_total"].as_f64()).map(round2)},
        "click_to_audio_ms": c2a.json(),
        "msg_to_audio_ms": m2a.json(),
        "live_poll": {"count": live_n, "errors": live_err, "dur_ms": live_dur.json(),
                      "child_cpu_ms_total": round2(live_cpu), "rows_total": live_rows, "new_msgs_total": live_new},
        "audio_play": {"count": audio_n, "skipped": audio_skipped, "paused_ms_total": round2(audio_paused),
                       "label_spoken": audio_label},
        "dictation": {"count": dict_n, "ok": dict_ok, "latency_from_stop_ms": dict_stop.json(),
                      "latency_from_start_ms": dict_start.json(), "e2eLatency": dict_e2e.json()},
        "reply_send": {"count": reply_n, "ok": reply_ok, "queued": reply_queued, "dur_ms": reply_dur.json()},
        "channel_actions": {"total": channels.values().sum::<usize>(), "by_kind": channels},
        "t3_cli": totals(&t3_cost),
        "state_db": totals(&db_cost),
    })
}

fn ts_ms(v: &Value) -> Option<i64> {
    parse_rfc3339_ms(v["ts"].as_str()?)
}

fn f(v: &Value) -> String {
    v.as_f64().map_or_else(|| "-".into(), |x| format!("{x:.1}"))
}

/// `n=.. p50=.. p95=.. max=..` of a [`Dist`] JSON.
fn dist(d: &Value) -> String {
    format!("n={} p50={} p95={} max={}", d["n"], f(&d["p50"]), f(&d["p95"]), f(&d["max"]))
}

fn print_summary(dir: &Path, r: &Value) {
    println!("run: {}", dir.display());
    println!("events: {}", r["events"]);
    println!("\n{:<24} {:>6} {:>6} {:>10} {:>10} {:>10}", "kind", "count", "errors", "p50_ms", "p95_ms", "max_ms");
    for (k, v) in r["kinds"].as_object().into_iter().flatten() {
        let d = &v["dur_ms"];
        let n = |x: &Value| x.as_u64().unwrap_or(0);
        println!("{k:<24} {:>6} {:>6} {:>10} {:>10} {:>10}", n(&v["count"]), n(&v["errors"]), f(&d["p50"]), f(&d["p95"]), f(&d["max"]));
    }
    let m = &r["metrics"];
    println!(
        "\nmetrics: rows={} span_s={} rows/s={}",
        m["rows"], m["span_s"], m["rows_per_s"]
    );
    println!("rss_mb: min={} avg={} max={}", f(&m["rss_mb"]["min"]), f(&m["rss_mb"]["avg"]), f(&m["rss_mb"]["max"]));
    println!("cpu_pct: avg={} max={}", f(&m["cpu_pct"]["avg"]), f(&m["cpu_pct"]["max"]));
    println!("click_to_audio_ms: {}", dist(&r["click_to_audio_ms"]));
    println!("msg_to_audio_ms: {}", dist(&r["msg_to_audio_ms"]));
    let a = &r["audio_play"];
    println!(
        "audio.play: count={} skipped={} paused_ms_total={} label_spoken={}",
        a["count"], a["skipped"], f(&a["paused_ms_total"]), a["label_spoken"]
    );
    let l = &r["live_poll"];
    println!(
        "live.poll: count={} errors={} child_cpu_ms_total={} rows_total={} new_msgs_total={} dur_ms {}",
        l["count"], l["errors"], f(&l["child_cpu_ms_total"]), l["rows_total"], l["new_msgs_total"], dist(&l["dur_ms"])
    );
    let w = &r["dictation"];
    println!("dictation: count={} ok={}", w["count"], w["ok"]);
    println!("  latency_from_stop_ms: {}", dist(&w["latency_from_stop_ms"]));
    println!("  latency_from_start_ms: {}", dist(&w["latency_from_start_ms"]));
    println!("  e2eLatency: {}", dist(&w["e2eLatency"]));
    let s = &r["reply_send"];
    println!("reply.send: count={} ok={} queued={} dur_ms {}", s["count"], s["ok"], s["queued"], dist(&s["dur_ms"]));
    let ch = &r["channel_actions"];
    let by_kind: Vec<String> =
        ch["by_kind"].as_object().into_iter().flatten().map(|(k, v)| format!("{k}={v}")).collect();
    println!("channel actions: total={} {}", ch["total"], by_kind.join(" "));
    let t = &r["t3_cli"];
    println!("t3 cli: calls={} wall_ms={} child_cpu_ms={}", t["calls"], f(&t["wall_ms"]), f(&t["child_cpu_ms"]));
    for (k, v) in t["by_kind"].as_object().into_iter().flatten() {
        println!("  {k}: calls={} wall_ms={} child_cpu_ms={}", v["calls"], f(&v["wall_ms"]), f(&v["child_cpu_ms"]));
    }
    let d = &r["state_db"];
    println!("state db (sqlite3): calls={} wall_ms={} child_cpu_ms={}", d["calls"], f(&d["wall_ms"]), f(&d["child_cpu_ms"]));
}

// ---------- HTML ----------

const W: f64 = 760.0;
const H: f64 = 220.0;
const PAD: f64 = 44.0;

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn axes(svg: &mut String, title: &str, x_label: &str, y_label: &str, x_max: f64, y_max: f64) {
    let _ = write!(
        svg,
        r##"<svg viewBox="0 0 {W} {H}" width="{W}" height="{H}" xmlns="http://www.w3.org/2000/svg"><text x="{PAD}" y="16" class="t">{t}</text><line x1="{PAD}" y1="{yb}" x2="{xr}" y2="{yb}" class="ax"/><line x1="{PAD}" y1="24" x2="{PAD}" y2="{yb}" class="ax"/><text x="{xr}" y="{yl}" class="l" text-anchor="end">{xl} (max {xm:.1})</text><text x="4" y="34" class="l">{yl2}</text><text x="4" y="48" class="l">{ym:.1}</text><text x="4" y="{yb}" class="l">0</text>"##,
        t = esc(title),
        yb = H - 24.0,
        xr = W - 8.0,
        yl = H - 6.0,
        xl = esc(x_label),
        xm = x_max,
        yl2 = esc(y_label),
        ym = y_max,
    );
}

fn sx(x: f64, max: f64) -> f64 {
    PAD + x / max.max(1e-9) * (W - PAD - 8.0)
}
fn sy(y: f64, max: f64) -> f64 {
    (H - 24.0) - y / max.max(1e-9) * (H - 24.0 - 40.0)
}

fn line_chart(title: &str, y_label: &str, pts: &[(f64, f64)], color: &str) -> String {
    let x_max = pts.iter().map(|p| p.0).fold(1.0, f64::max);
    let y_max = pts.iter().map(|p| p.1).fold(1.0, f64::max) * 1.1;
    let mut s = String::new();
    axes(&mut s, title, "seconds", y_label, x_max, y_max);
    let path: Vec<String> = pts.iter().map(|&(x, y)| format!("{:.1},{:.1}", sx(x, x_max), sy(y, y_max))).collect();
    let _ = write!(s, r#"<polyline fill="none" stroke="{color}" stroke-width="1.5" points="{}"/></svg>"#, path.join(" "));
    s
}

fn scatter(title: &str, pts: &[(f64, f64, String)]) -> String {
    let x_max = pts.iter().map(|p| p.0).fold(1.0, f64::max) * 1.05;
    let y_max = pts.iter().map(|p| p.1).fold(1.0, f64::max) * 1.1;
    let mut s = String::new();
    axes(&mut s, title, "chars", "synth_ms", x_max, y_max);
    for (x, y, text) in pts {
        let _ = write!(
            s,
            r##"<circle cx="{:.1}" cy="{:.1}" r="4" fill="#d9480f" fill-opacity="0.7"><title>{} chars, {:.1} ms: {}</title></circle>"##,
            sx(*x, x_max),
            sy(*y, y_max),
            x,
            y,
            esc(text)
        );
    }
    s.push_str("</svg>");
    s
}

fn kind_bars(kinds: &Value) -> String {
    let rows: Vec<(&String, f64, f64, f64)> = kinds
        .as_object()
        .into_iter()
        .flatten()
        .filter_map(|(k, v)| {
            let d = &v["dur_ms"];
            Some((k, d["p50"].as_f64()?, d["p95"].as_f64()?, d["max"].as_f64()?))
        })
        .collect();
    let max = rows.iter().map(|r| r.3).fold(1.0, f64::max);
    let row_h = 22.0;
    let h = 30.0 + row_h * rows.len() as f64;
    let label_w = 170.0;
    let scale = |v: f64| v / max * (W - label_w - 170.0);
    let mut s = format!(r#"<svg viewBox="0 0 {W} {h}" width="{W}" height="{h}" xmlns="http://www.w3.org/2000/svg"><text x="4" y="16" class="t">dur_ms per kind (light = max, mid = p95, dark = p50)</text>"#);
    for (i, (k, p50, p95, mx)) in rows.iter().enumerate() {
        let y = 26.0 + i as f64 * row_h;
        let _ = write!(
            s,
            r##"<text x="4" y="{ty:.1}" class="l">{k}</text><rect x="{label_w}" y="{y:.1}" width="{w3:.1}" height="16" fill="#c5d7f2"/><rect x="{label_w}" y="{y:.1}" width="{w2:.1}" height="16" fill="#6f9fe0"/><rect x="{label_w}" y="{y:.1}" width="{w1:.1}" height="16" fill="#1c4f9c"/><text x="{tx:.1}" y="{ty:.1}" class="l">{p50:.1} / {p95:.1} / {mx:.1}</text>"##,
            ty = y + 12.0,
            k = esc(k),
            w3 = scale(*mx).max(1.0),
            w2 = scale(*p95).max(1.0),
            w1 = scale(*p50).max(1.0),
            tx = label_w + scale(*mx) + 6.0,
        );
    }
    s.push_str("</svg>");
    s
}

fn render_html(dir: &Path, report: &Value, events: &[Value], metrics: &[Value]) -> String {
    let t0 = metrics.first().and_then(ts_ms).unwrap_or(0);
    let secs = |m: &Value| ts_ms(m).map_or(0.0, |t| (t - t0) as f64 / 1000.0);
    let rss: Vec<(f64, f64)> = metrics.iter().filter_map(|m| Some((secs(m), m["rss_mb"].as_f64()?))).collect();
    let cpu: Vec<(f64, f64)> = metrics.iter().filter_map(|m| Some((secs(m), m["cpu_pct"].as_f64()?))).collect();
    let synth: Vec<(f64, f64, String)> = events
        .iter()
        .filter(|e| e["kind"] == "tts.synth" && e["ok"] == true)
        .filter_map(|e| {
            Some((e["in"]["chars"].as_f64()?, e["dur_ms"].as_f64()?, e["in"]["text"].as_str().unwrap_or_default().to_owned()))
        })
        .collect();
    let utterances: String = events
        .iter()
        .filter(|e| e["kind"] == "speak.enqueue")
        .map(|e| {
            format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td></tr>",
                esc(e["in"]["thread"].as_str().unwrap_or_default()),
                esc(e["in"]["source"].as_str().unwrap_or_default()),
                esc(e["in"]["text"].as_str().unwrap_or_default())
            )
        })
        .collect();
    let m = &report["metrics"];
    let c = &report["click_to_audio_ms"];
    let t = &report["t3_cli"];
    let db = &report["state_db"];
    let (lp, ap, dc, rs, ch) = (
        &report["live_poll"],
        &report["audio_play"],
        &report["dictation"],
        &report["reply_send"],
        &report["channel_actions"],
    );
    let channel_kinds: Vec<String> =
        ch["by_kind"].as_object().into_iter().flatten().map(|(k, v)| format!("{k}={v}")).collect();
    let sections = [
        ("click→audio ms", dist(c)),
        ("msg→audio ms", dist(&report["msg_to_audio_ms"])),
        (
            "audio.play",
            format!(
                "count={} skipped={} paused_ms_total={} label_spoken={}",
                ap["count"], ap["skipped"], f(&ap["paused_ms_total"]), ap["label_spoken"]
            ),
        ),
        (
            "live.poll",
            format!(
                "count={} errors={} child_cpu_ms_total={} rows_total={} new_msgs_total={}",
                lp["count"], lp["errors"], f(&lp["child_cpu_ms_total"]), lp["rows_total"], lp["new_msgs_total"]
            ),
        ),
        ("live.poll dur ms", dist(&lp["dur_ms"])),
        ("dictation", format!("count={} ok={}", dc["count"], dc["ok"])),
        ("dictation from stop ms", dist(&dc["latency_from_stop_ms"])),
        ("dictation from start ms", dist(&dc["latency_from_start_ms"])),
        ("Wispr e2eLatency ms", dist(&dc["e2eLatency"])),
        ("reply.send", format!("count={} ok={} queued={}", rs["count"], rs["ok"], rs["queued"])),
        ("reply.send dur ms", dist(&rs["dur_ms"])),
        ("channel actions", format!("total={} {}", ch["total"], channel_kinds.join(" "))),
        (
            "T3 CLI",
            format!("calls={} wall_ms={} child_cpu_ms={}", t["calls"], f(&t["wall_ms"]), f(&t["child_cpu_ms"])),
        ),
        (
            "state DB (sqlite3)",
            format!("calls={} wall_ms={} child_cpu_ms={}", db["calls"], f(&db["wall_ms"]), f(&db["child_cpu_ms"])),
        ),
    ];
    let stats: String =
        sections.iter().map(|(k, v)| format!("<tr><th>{}</th><td>{}</td></tr>", esc(k), esc(v))).collect();
    format!(
        r#"<!doctype html><html><head><meta charset="utf-8"><title>t3-panel report {run}</title><style>
body{{font:13px -apple-system,system-ui,sans-serif;margin:24px;color:#222;max-width:820px}}
h1{{font-size:18px}} h2{{font-size:14px;margin-top:28px}} table{{border-collapse:collapse}} td,th{{padding:2px 8px;border-bottom:1px solid #eee;text-align:left}}
.t{{font:600 12px sans-serif}} .l{{font:11px sans-serif;fill:#555}} .ax{{stroke:#999}} pre{{background:#f6f6f6;padding:8px;overflow:auto}}
</style></head><body>
<h1>t3-panel run {run}</h1>
<p>{events} events · {rows} metric rows over {span}s ({rps} rows/s) · RSS min/avg/max {rmin}/{ravg}/{rmax} MB · CPU avg/max {cavg}/{cmax}%<br>
click→audio p50/p95/max {c50}/{c95}/{cmax2} ms (n={cn}) · T3 CLI {calls} calls, {twall} ms wall, {tcpu} ms child CPU<br>
msg→audio p50 {m50} ms · dictation {dn} ({dok} ok), from stop p50 {ds50} ms · replies {rn} ({rq} queued) · state DB {dbcalls} calls, {dbcpu} ms child CPU</p>
<h2>Live, audio, dictation, replies</h2><table>{stats}</table>
<h2>Memory &amp; CPU</h2>{rss_svg}<br>{cpu_svg}
<h2>TTS synth_ms vs chars</h2>{scatter_svg}
<h2>Duration by kind</h2>{bars_svg}
<h2>Spoken texts (speak.enqueue)</h2><table><tr><th>thread</th><th>source</th><th>text</th></tr>{utterances}</table>
<h2>report.json</h2><pre>{json}</pre>
</body></html>"#,
        run = esc(&dir.file_name().unwrap_or_default().to_string_lossy()),
        events = report["events"],
        rows = m["rows"],
        span = m["span_s"],
        rps = m["rows_per_s"],
        rmin = f(&m["rss_mb"]["min"]),
        ravg = f(&m["rss_mb"]["avg"]),
        rmax = f(&m["rss_mb"]["max"]),
        cavg = f(&m["cpu_pct"]["avg"]),
        cmax = f(&m["cpu_pct"]["max"]),
        c50 = f(&c["p50"]),
        c95 = f(&c["p95"]),
        cmax2 = f(&c["max"]),
        cn = c["n"],
        calls = t["calls"],
        twall = f(&t["wall_ms"]),
        tcpu = f(&t["child_cpu_ms"]),
        m50 = f(&report["msg_to_audio_ms"]["p50"]),
        dn = dc["count"],
        dok = dc["ok"],
        ds50 = f(&dc["latency_from_stop_ms"]["p50"]),
        rn = rs["count"],
        rq = rs["queued"],
        dbcalls = db["calls"],
        dbcpu = f(&db["child_cpu_ms"]),
        rss_svg = line_chart("RSS over time", "MB", &rss, "#1c4f9c"),
        cpu_svg = line_chart("CPU over time", "%", &cpu, "#2b8a3e"),
        scatter_svg = scatter("tts.synth: synth_ms vs chars", &synth),
        bars_svg = kind_bars(&report["kinds"]),
        json = esc(&serde_json::to_string_pretty(report).unwrap_or_default()),
    )
}
