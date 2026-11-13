#!/usr/bin/env python3
"""results.json → index.html (self-contained, relative audio paths, works from file://). stdlib only.

  python3 harness/render.py
"""
import html, json
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
R = json.loads((ROOT / "results.json").read_text())
esc = html.escape


def num(v, fmt="{:.0f}"):
    return "–" if v is None else fmt.format(v)


def cls_wer(w):
    return "good" if w <= 0.03 else "ok" if w <= 0.08 else "bad"


def cls_ms(v, good, ok):
    return "good" if v <= good else "ok" if v <= ok else "bad"


def seedtxt(e):
    s = e.get("seeds")
    return "" if not s else "<br>seeds 1/2/3: " + "/".join(f"{s[k]['wer']*100:.0f}" for k in ("1", "2", "3") if k in s) + f" (mean {s['wer_mean']*100:.1f}%)"


engines = sorted(R["engines"], key=lambda e: (not e["baseline"], e["wer"] + (0 if e["numbers_ok"] else 0.05), e["ttfa_p50_ms"]))
rows = []
for e in engines:
    p2 = e.get("pass2") or {}
    rows.append(f"""<tr class="{'baseline' if e['baseline'] else ''}" data-id="{esc(e['id'])}">
<td class="name"><b>{esc(e['label'])}</b>{' <span class="tag">BASELINE</span>' if e['baseline'] else ''}<br><code>{esc(e['id'])}</code></td>
<td>{esc(e['device'])}<br><span class="dim">{e['sample_rate']} Hz</span></td>
<td data-v="{e['load_ms']}">{num(e['load_ms'])}{f'<br><span class="dim">2nd {num(p2.get("load_ms"))}</span>' if p2 else ''}</td>
<td data-v="{e['ttfa_first_ms']}">{num(e['ttfa_first_ms'])}</td>
<td data-v="{e['ttfa_p50_ms']}" class="{cls_ms(e['ttfa_p50_ms'], 150, 400)}">{num(e['ttfa_p50_ms'])}{' <span class="dim">str</span>' if e['streams'] else ''}{f'<br><span class="dim">2nd {num(p2.get("ttfa_p50_ms"))}</span>' if p2 else ''}</td>
<td data-v="{e['perceived_ttfa_p50_ms']}">{num(e['perceived_ttfa_p50_ms'])}<br><span class="dim">lead {e['lead_p50_s']:.2f}s</span></td>
<td data-v="{e['rtf']}" class="{cls_ms(e['rtf'], 0.1, 0.5)}">{e['rtf']:.3f}</td>
<td data-v="{e['max_rss_mb'] or 0}">{num(e['max_rss_mb'])}<br><span class="dim">fp {num(e['peak_footprint_mb'])}</span></td>
<td data-v="{e['wer']}" class="{cls_wer(e['wer'])}">{e['wer']*100:.1f}%<br><span class="dim">CER {e['cer']*100:.1f}%{seedtxt(e)}</span></td>
<td data-v="{int(e['numbers_ok'])}" class="{'good' if e['numbers_ok'] else 'bad'}">{'yes' if e['numbers_ok'] else 'no'}{'<br><span class="dim">seeds ' + e['seeds']['numbers_ok_count'] + '</span>' if e.get('seeds') else ''}</td>
<td data-v="{e['english_ok'].split('/')[0]}">{e['english_ok']}</td>
<td data-v="{e['flags']}">{e['flags']}</td>
<td class="small">{esc(e['license'])}</td>
<td class="small">{esc(e['rust'])}</td>
<td class="small">{esc(e['note'])}</td></tr>""")

corpus = R["corpus"]
mrows = []
for e in engines:
    cells = []
    for s in e["sentences"]:
        fl = "; ".join(s["flags"])
        nb = (' <span class="good">#ok</span>' if s.get("numbers_ok") else ' <span class="bad">#bad</span>') if "numbers_ok" in s else ""
        cells.append(f"""<td><audio controls preload="none" src="{esc(s['audio'])}"></audio>
<div class="heard {cls_wer(s['wer'])}" title="{esc(fl)}">{esc(s['heard'])}</div>
<div class="dim">WER {s['wer']*100:.0f}% · {s['ttfa_ms']:.0f} ms · {s['audio_ms']/1000:.1f}s{nb}{' · ⚑ ' + esc(fl) if fl else ''}</div></td>""")
    mrows.append(f"""<tr class="{'baseline' if e['baseline'] else ''}" data-id="{esc(e['id'])}"><th class="name">{esc(e['label'])}<br><code>{esc(e['id'])}</code></th>{''.join(cells)}</tr>""")

fails = "".join(f"<li><code>{esc(f['id'])}</code>: <pre>{esc(f['error'])}</pre></li>" for f in R["failures"]) or "<li>none</li>"
skips = "".join(f"<li><b>{esc(s['name'])}</b> — {esc(s['reason'])}</li>" for s in R["skipped"]) or "<li>none</li>"
method = "".join(f"<li><b>{esc(k)}</b>: {esc(v)}</li>" for k, v in R["method"].items())

page = f"""<!doctype html><html lang="en"><head><meta charset="utf-8"><title>Turkish local TTS bake-off — Tellal</title>
<meta name="viewport" content="width=device-width,initial-scale=1">
<style>
:root{{--bg:#0f1115;--fg:#d8dde6;--dim:#7d8594;--line:#262b35;--good:#5fd38d;--ok:#e5c158;--bad:#ff6b6b;--base:#1d2a44}}
*{{box-sizing:border-box}}body{{margin:0;padding:14px 18px;background:var(--bg);color:var(--fg);font:13px/1.35 -apple-system,system-ui,sans-serif}}
h1{{font-size:18px;margin:0 0 4px}}h2{{font-size:15px;margin:18px 0 6px}}a{{color:#7fb3ff}}code{{color:#9aa6b8;font-size:11px}}
.dim{{color:var(--dim);font-size:11px}}.small{{font-size:11px;max-width:260px;color:#b4bbc7}}
table{{border-collapse:collapse;width:100%}}td,th{{border-bottom:1px solid var(--line);padding:4px 6px;vertical-align:top;text-align:left}}
thead th{{position:sticky;top:0;background:#151922;cursor:pointer;white-space:nowrap;font-size:12px}}
tr.baseline{{background:var(--base)}}tr.baseline td:first-child,tr.baseline th:first-child{{border-left:3px solid #7fb3ff}}
.tag{{background:#7fb3ff;color:#0f1115;border-radius:3px;padding:0 4px;font-size:10px;font-weight:700}}
.good{{color:var(--good)}}.ok{{color:var(--ok)}}.bad{{color:var(--bad)}}
.matrix td{{min-width:190px}}.matrix audio{{width:180px;height:28px}}.heard{{font-size:11px;margin-top:2px}}
.name{{min-width:170px;max-width:220px}}.wrap{{overflow-x:auto}}
input{{background:#151922;color:var(--fg);border:1px solid var(--line);padding:4px 8px;border-radius:4px;width:260px}}
details{{margin:6px 0}}pre{{white-space:pre-wrap;font-size:11px;color:#b4bbc7}}
</style></head><body>
<h1>Turkish local TTS bake-off for Tellal</h1>
<div class="dim">Generated {esc(R['generated'])} · {esc(R['machine'])} · real runs, one fresh process per engine/voice · audio = loudness-normalized copies (-16 LUFS), raw wavs in audio/raw/</div>
<details><summary>Method</summary><ul>{method}</ul></details>
<details><summary>Corpus (fed raw, no pre-normalization)</summary><ol type="a">{''.join(f'<li>{esc(c["text"])}</li>' for c in corpus)}</ol></details>
<p><input id="q" placeholder="filter engines (e.g. supertonic, s8, pocket)…"> <span class="dim">click headers to sort · colors: TTFA ≤150 / ≤400 ms, RTF ≤0.1 / ≤0.5, WER ≤3 / ≤8 %</span></p>
<h2>Metrics</h2><div class="wrap"><table id="m"><thead><tr>
<th>Engine / voice</th><th>Device</th><th>Load ms</th><th>TTFA 1st ms</th><th>TTFA p50 ms</th><th>+lead silence p50</th><th>RTF</th><th>RSS MB</th><th>WER</th><th>Numbers OK?</th><th>EN words</th><th>Flags</th><th>License</th><th>Rust integration</th><th>Notes</th>
</tr></thead><tbody>{''.join(rows)}</tbody></table></div>
<h2>Listening matrix (sentence × engine)</h2><div class="wrap"><table class="matrix" id="x"><thead><tr><th>Engine</th>{''.join(f'<th title="{esc(c["text"])}">{c["id"]}: {esc(c["text"][:38])}…</th>' for c in corpus)}</tr></thead><tbody>{''.join(mrows)}</tbody></table></div>
<h2>Failures</h2><ul>{fails}</ul>
<h2>Skipped candidates</h2><ul>{skips}</ul>
<script>
const q=document.getElementById('q');q.oninput=()=>{{const t=q.value.toLowerCase();document.querySelectorAll('tbody tr').forEach(r=>r.style.display=r.dataset.id.toLowerCase().includes(t)||r.textContent.toLowerCase().includes(t)?'':'none')}};
document.querySelectorAll('#m thead th').forEach((th,i)=>th.onclick=()=>{{const tb=th.closest('table').tBodies[0],rs=[...tb.rows],asc=th.dataset.asc!=='1';th.dataset.asc=asc?'1':'0';
rs.sort((a,b)=>{{const x=a.cells[i].dataset.v,y=b.cells[i].dataset.v;const r=(x!==undefined&&y!==undefined)?(+x)-(+y):a.cells[i].textContent.localeCompare(b.cells[i].textContent);return asc?r:-r}});rs.forEach(r=>tb.appendChild(r))}});
document.addEventListener('play',e=>document.querySelectorAll('audio').forEach(a=>{{if(a!==e.target)a.pause()}}),true);
</script></body></html>"""
(ROOT / "index.html").write_text(page)
print("index.html", len(page), "bytes,", len(engines), "engines")
