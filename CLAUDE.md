# gavel

Rust TUI (ratatui) for debates between Claude agents. See the README for usage.

## Language
- Everything in the repository is written in English: commit messages, branch names, code, comments and docs.
- Past commits stay as they are; don't rewrite history.

## Translation (pt/en)
The app is bilingual: `--lang pt|en`, Portuguese is the default and the primary language. Every change must keep both working:
- Wrap every user-facing string (UI, status bar, popups, `--plain` output, errors, saved `.md`) and every agent prompt in `tr!("pt", "en")` / `trf!("pt {x}", "en {x}", ...)` (`src/main.rs`). Write the Portuguese first, then the English.
- Side names come from `Side::name()`; don't hardcode `PRÓ`/`CONTRA`/`JUIZ`.
- Keep JSON schema keys (`destino`, `encerrar_eixo`, ...) and their enum values in Portuguese in both languages: the code parses them.
- `tests/fake-claude.sh` detects the side from the system prompt in both languages; update it if a prompt it matches changes.
- Before committing, run the fake-claude TUI with and without `--lang en`.

## Git
- Main branch: `main`.
- Create a branch for each new feature or change; don't work directly on `main`.
- Commit after each small working change.
- After every merge into `main`, install the new version: `cargo install --path . --root ~/.local` (puts `gavel` in `~/.local/bin`, which is on the fish `PATH`).

## Notes
- Agents run `claude -p` with the subscription login. Don't use `--bare`: it disables OAuth.
- Test the TUI without spending usage: `GAVEL_CLAUDE_BIN=$PWD/tests/fake-claude.sh cargo run -- "topic"`.
- `cargo test` checks the stream-json parser against `tests/fixtures/search_schema.jsonl` (real CLI output).
