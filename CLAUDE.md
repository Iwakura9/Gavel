# gavel

TUI em Rust (ratatui) para debates entre agentes Claude. Veja o README para uso.

## Git
- Branch principal: `main`.
- Crie uma branch para cada feature nova ou modificação; não trabalhe direto na `main`.
- Faça commit a cada pequena alteração funcional (mensagens em português).

## Notas
- Os agentes rodam `claude -p` com o login da assinatura. Não use `--bare`: ele desliga o OAuth.
- Teste a TUI sem gastar limite: `GAVEL_CLAUDE_BIN=$PWD/tests/fake-claude.sh cargo run -- "tema"`.
- `cargo test` valida o parser do stream-json contra `tests/fixtures/search_schema.jsonl` (saída real do CLI).
