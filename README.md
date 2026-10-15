# ⚖ gavel

Debates between Claude agents in the terminal. You pick the topic; **PRO** and **CON** (Sonnet) research the web
and debate citing sources; the **JUDGE** (Opus) frames the debate, flags fallacies, checks sources and delivers the verdict.

Uses your Claude Code login (subscription): each agent is an isolated `claude -p` with its own session.

## Install

```sh
cargo install --path . --root ~/.local   # installs ~/.local/bin/gavel
```

`~/.local/bin` must be on your `PATH` (in fish: `fish_add_path ~/.local/bin`). Run the same command again to update.

## Usage

```sh
gavel "should pineapple on pizza be accepted?"   # or no topic to type it on the home screen
gavel --lang en "topic"                          # interface and agents in English (default: pt)
gavel --plain "topic"                            # no TUI, prints to stdout
```

## How a debate goes

1. **Framing:** the judge sets definitions, burden of proof and the issues to debate. How many issues depends on the topic: only the ones that decide the question.
2. **Research:** both sides search the web at the same time and build a dossier of sources (`P1`, `C1`, ...).
3. **Issues:** each issue has up to 3 rounds. After every round the judge flags fallacies and unsourced claims, spot-checks sources and decides whether the issue is exhausted. When an issue closes, the judge may revise the agenda: drop issues that became unnecessary and add more important ones that came up.
4. **Closing statements**, then the **verdict**: winner, scores per issue and a summary of the infractions. The judge scores who argued better, not who is right.

## Keys

| Key | Action |
|---|---|
| `space` | toggle automatic / step by step (in step by step, `Enter` releases the next turn) |
| `p` | question for the judge (it answers or hands the floor to one side) |
| `f` | sources (`←→` switches side; ✓/✗ marks what the judge verified) |
| `v` | verdict |
| `tab` `←→` | switch panel |
| `↑↓ PgUp PgDn End` | scroll |
| `q` | back to the home screen (the debate stops and can be resumed) |
| `Esc` / `Ctrl+C` | quit (on the home screen) / quit anywhere |

## Saved debates

Debates are saved to `~/.local/share/gavel/debates/` (`.json` and `.md`) after every turn.
On the home screen press `↓` to list them: `Enter` reopens one (finished debates are read-only, interrupted ones continue where they stopped), `Del` twice deletes it.

## Development

```sh
GAVEL_CLAUDE_BIN=$PWD/tests/fake-claude.sh cargo run -- "topic"   # fake agents, no usage spent
cargo test                                                         # stream-json parser against real CLI output
```
