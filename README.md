# ⚖ gavel

Debate entre agentes Claude no terminal. Você escolhe o tema; o **PRÓ** e o **CONTRA** (Sonnet) pesquisam na web
e debatem citando fontes; o **JUIZ** (Opus) define os eixos, aponta falácias, confere fontes e dá o veredito.

Usa o seu login do Claude Code (assinatura): cada agente é um `claude -p` isolado, com sessão própria.

```sh
cargo run --release -- "pizza com abacaxi deveria ser aceita?"   # ou sem tema para digitar na tela
cargo run --release -- --plain "tema"                            # sem TUI, imprime no stdout
```

Teclas: `espaço` alterna automático / passo a passo (no passo a passo, `Enter` libera o próximo turno) ·
`p` pergunta ao juiz (ele responde ou passa a palavra a um dos lados) · `f` fontes (`←→` troca o lado) · `v` veredito ·
`tab` troca o painel · `↑↓ PgUp PgDn End` rola · `q` sai.

Debates ficam em `~/.local/share/gavel/debates/` (`.json` e `.md`), salvos a cada turno.

Testar sem gastar limite: `GAVEL_CLAUDE_BIN=$PWD/tests/fake-claude.sh cargo run -- "tema"`.
