# gavel

Rust TUI (ratatui) for debates between Claude agents. See the README for usage.

## Language
- Everything in the repository is written in English: commit messages, branch names, code, comments and docs.
- Past commits stay as they are; don't rewrite history.
- The app itself is bilingual (`--lang pt|en`, default `pt`): wrap every user-facing string or prompt in `tr!`/`trf!` (`src/main.rs`).

## Git
- Main branch: `main`.
- Create a branch for each new feature or change; don't work directly on `main`.
- Commit after each small working change.

## Notes
- Agents run `claude -p` with the subscription login. Don't use `--bare`: it disables OAuth.
- Test the TUI without spending usage: `GAVEL_CLAUDE_BIN=$PWD/tests/fake-claude.sh cargo run -- "topic"`.
- `cargo test` checks the stream-json parser against `tests/fixtures/search_schema.jsonl` (real CLI output).
