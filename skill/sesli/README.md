# `sesli` — the agent side of t3-panel

t3-panel reads agent messages aloud. It can only read what the agent writes, so the agent has
to write for the ear. `sesli` ("spoken" in Turkish) is a small skill that tells coding agents
how. [`SKILL.md`](SKILL.md) is the skill itself, in Turkish because the voice is Turkish.

The panel works without it (it then reads the first sentence of each message), but agents
that follow it are much easier to listen to.

## What it makes the agent write

1. **A start line before the first tool call.** One sentence, at most 15 words:
   "Tamam, önce hataları topluyorum, sonra en sık olanı düzeltiyorum." ("Okay, I'm collecting
   the errors first, then fixing the most common one.") The agent does not wait for approval,
   except for unclear or irreversible work (delete, push, deploy, money), where it asks one
   question and stops.
2. **One status sentence per phase.** A separate message at each new phase, at most 12 words,
   "Şimdi … yapıyorum" ("Now I'm …"). The panel reads only the first sentence of a message,
   so that sentence has no code, paths, hashes, URLs or commands; details go after it.
3. **A spoken summary at the end.** The normal written report, then two lines:
   ```
   🔊 <at most 2 sentences, ~200 characters: outcome, numbers, at most one question>
   👀 <what to look at on screen: diff, screenshots, PR, report; omitted if nothing>
   ```
   The panel reads only the `🔊` line of that message. Numbers are written as digits
   ("80 görevin 20'si takıldı" = "20 of 80 tasks got stuck"), items are grouped instead of
   listed, and a decision ends in a single question. The `👀` line is never read; you see it
   when you open the thread in T3.

The panel prepends `$sesli ` to every reply you send from it (`T3P_REPLY_PREFIX`), so the agent
keeps using the skill for the rest of the thread.

## Install

The skill is one file. Copy (or symlink) the `sesli` folder into your agent's skills directory:

| Agent | Directory |
|---|---|
| Claude Code | `~/.claude/skills/sesli/` |
| Codex | `~/.codex/skills/sesli/` |
| omp | `~/.omp/agent/skills/sesli/` |
| Shared (agents that read it) | `~/.agents/skills/sesli/` |

```bash
# from the repo root
mkdir -p ~/.claude/skills && cp -R skill/sesli ~/.claude/skills/
# or keep it in sync with the repo
ln -s "$PWD/skill/sesli" ~/.claude/skills/sesli
```

Then start a message with `$sesli` (or say "sesli anlat"), or just reply from the panel.

## Other languages

The rules are language-neutral; only the examples and the voice are Turkish. With a voice in
another language you would translate `SKILL.md` and keep the structure: start line, one short
first sentence per message, a `🔊` line at the end.
