---
name: aloud
description: Use when the user's message starts with $aloud or asks for spoken, panel-friendly output ("aloud", "read it aloud", "I'll listen from the panel"). Formats the start acknowledgement, progress lines and final reply so Tellal can read them aloud with local TTS.
---

# aloud

Tellal (github.com/cengizhankose/tellal) lets the user listen to T3 threads channel by channel: once they turn a thread's sound on, the first sentence of every new assistant message you write in that thread is read aloud in English with a local voice. Tellal does the speaking; you only write so it reads well. Do not run speech commands or call extra tools.

## 1. Starting (required)

Before the first tool call, write one message: "Okay, <what I'll do>." At most 15 words.

- Example: "Okay, I'm collecting the errors first, then fixing the most common one."
- Don't wait for approval; start right away. If the user wants to stop, they write from the panel or open the thread and stop it.
- If the request is unclear or the work can't be undone (deleting, pushing, deploying, money): ask one question and wait. The message is then "Before I start: <question>?"

## 2. Progress steps

- At the start of each new phase, write a one-sentence status as its own message: English, at most 12 words, "Now I'm ..." pattern.
- Tellal reads only the first sentence of each message. The first sentence has no code, file paths, hashes, URLs or commands; put technical detail in the sentences after it.
- Don't announce the same phase twice. If something breaks, say that in one sentence too: "The tests broke, looking for the cause."
- When using the computer (computer-use tools such as cua-driver): before working in an app, write one sentence: "Opening TextEdit to write the note." Don't announce every click; write a new sentence when the app or the goal changes. Tellal shows a "Stop" button on screen meanwhile; before a step that can't be undone (sending, deleting, buying), ask and wait.

## 3. Final message

Write the written report as usual. Add these two lines at the very end:

```
🔊 <at most 2 sentences, ~200 characters: outcome, numbers, at most one question>
👀 <what to look at on screen; leave this line out if there's nothing>
```

🔊 rules:

- In this message Tellal reads only the 🔊 line.
- Write numbers as digits: "20 of 80 tasks got stuck".
- Don't list items one by one, group them: how many finished, got stuck, failed, finished with warnings.
- If a decision is needed, end with one question: "Start with the errors or the stuck ones?"
- If you're waiting for approval or information: "Waiting for your approval: <for what>."
- No code, file paths, URLs, emoji or markdown.

👀 rules:

- List things that need eyes, such as a diff, screenshots, a table, a PR or an HTML report, by path or link.
- Tellal doesn't read this line; the user sees it when they double-click the thread and open it in T3.

## Example flow

```
Okay, I'm scanning the 80 tasks and grouping them by status.
Now I'm reading the logs of the stuck tasks.
Now I'm sorting the failed ones by cause.
…report…
🔊 Of 80 tasks, 20 got stuck, 30 failed and 30 finished with warnings. Start with the errors or the stuck ones?
👀 ~/dev/project/report.html
```
