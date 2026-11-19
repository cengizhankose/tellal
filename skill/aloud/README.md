# `aloud` — the agent side of Tellal, in English

Tellal reads agent messages aloud. It can only read what the agent writes, so the agent has
to write for the ear. `aloud` is a small skill that tells coding agents how. [`SKILL.md`](SKILL.md)
is the skill itself; it is the English counterpart of [`sesli`](../sesli/README.md) (same rules,
English wording and examples, for the English voice).

The panel works without it (it then reads the first sentence of each message), but agents
that follow it are much easier to listen to.

## What it makes the agent write

1. **A start line before the first tool call.** One sentence, at most 15 words:
   "Okay, I'm collecting the errors first, then fixing the most common one." The agent does
   not wait for approval, except for unclear or irreversible work (delete, push, deploy,
   money), where it asks one question and stops.
2. **One status sentence per phase.** A separate message at each new phase, at most 12 words,
   "Now I'm …". The panel reads only the first sentence of a message, so that sentence has no
   code, paths, hashes, URLs or commands; details go after it.
3. **A spoken summary at the end.** The normal written report, then two lines:
   ```
   🔊 <at most 2 sentences, ~200 characters: outcome, numbers, at most one question>
   👀 <what to look at on screen: diff, screenshots, PR, report; omitted if nothing>
   ```
   The panel reads only the `🔊` line of that message. Numbers are written as digits
   ("20 of 80 tasks got stuck"), items are grouped instead of listed, and a decision ends in
   a single question. The `👀` line is never read; you see it when you open the thread in T3.

With the panel in English (`TELLAL_LANG=en`), it prepends `$aloud ` to every reply you send
from it (`TELLAL_REPLY_PREFIX`), so the agent keeps using the skill for the rest of the thread.

## Install

The skill is one file. Copy (or symlink) the `aloud` folder into your agent's skills directory:

| Agent | Directory |
|---|---|
| Claude Code | `~/.claude/skills/aloud/` |
| Codex | `~/.codex/skills/aloud/` |
| omp | `~/.omp/agent/skills/aloud/` |
| Shared (agents that read it) | `~/.agents/skills/aloud/` |

```bash
# from the repo root
mkdir -p ~/.claude/skills && cp -R skill/aloud ~/.claude/skills/
# or keep it in sync with the repo
ln -s "$PWD/skill/aloud" ~/.claude/skills/aloud
```

Then start a message with `$aloud` (or say "read it aloud"), or just reply from the panel.
