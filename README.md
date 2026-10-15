# ⚖ gavel

Debates between Claude agents in the terminal. You pick the topic; **PRO** and **CON** (Sonnet) research the web
and debate citing sources; the **JUDGE** (Opus) sets the issues, flags fallacies, checks sources and delivers the verdict.

Uses your Claude Code login (subscription): each agent is an isolated `claude -p` with its own session.

```sh
cargo run --release -- "should pineapple on pizza be accepted?"   # or no topic to type it on screen
cargo run --release -- --plain "topic"                            # no TUI, prints to stdout
cargo run --release -- --lang en "topic"                          # interface and agents in English (default: pt)
```

Keys: `space` toggles automatic / step by step (in step by step, `Enter` releases the next turn) ·
`p` question for the judge (it answers or hands the floor to one side) · `f` sources (`←→` switches side) · `v` verdict ·
`tab` switches panel · `↑↓ PgUp PgDn End` scroll · `q` quits.

Debates are saved to `~/.local/share/gavel/debates/` (`.json` and `.md`) after every turn.
Run with no topic and press `↓` on the home screen to list them: `Enter` reopens one (finished debates are read-only, interrupted ones continue where they stopped), `Del` twice deletes it.

Test without spending usage: `GAVEL_CLAUDE_BIN=$PWD/tests/fake-claude.sh cargo run -- "topic"`.
