#!/usr/bin/env bash
# Finge ser o `claude -p --output-format stream-json` para testar a TUI sem gastar limite.
# Uso: GAVEL_CLAUDE_BIN=tests/fake-claude.sh cargo run -- "tema"
schema=""; prev=""
for a in "$@"; do [[ $prev == --json-schema ]] && schema=$a; prev=$a; done
cat >/dev/null # consome o prompt
sid="fake-$$"
emit() { printf '%s\n' "$1"; }
tok() { emit "{\"type\":\"stream_event\",\"event\":{\"type\":\"content_block_delta\",\"delta\":{\"type\":\"text_delta\",\"text\":\"$1\"}}}"; sleep 0.04; }
tool() { emit "{\"type\":\"assistant\",\"message\":{\"content\":[{\"type\":\"tool_use\",\"name\":\"$1\",\"input\":{\"$2\":\"$3\"}}]}}"; sleep 0.8; }
done_json() { emit "{\"type\":\"result\",\"is_error\":false,\"result\":\"\",\"structured_output\":$1}"; }
emit "{\"type\":\"system\",\"subtype\":\"init\",\"session_id\":\"$sid\"}"
emit "{\"type\":\"rate_limit_event\",\"rate_limit_info\":{\"unifiedWindows\":{\"five_hour\":{\"utilization\":0.$((RANDOM % 90 + 10))}}}}"
sleep 0.6
case $schema in
  *onus_da_prova*) done_json '{"definicoes":"Pizza com abacaxi: pizza com pedaços de abacaxi como cobertura.","onus_da_prova":"Do PRÓ.","eixos":[{"titulo":"Sabor","pergunta":"A combinação é boa?"},{"titulo":"Tradição","pergunta":"Respeita a pizza italiana?"},{"titulo":"Liberdade","pergunta":"Cada um come o que quer?"}],"regras":"Sem ataques pessoais."}' ;;
  *tese*) tool WebSearch query "pizza havaiana história"; tool WebFetch url "https://pt.wikipedia.org/wiki/Pizza_havaiana"; tool WebSearch query "doce e salgado estudos paladar"
          done_json '{"tese":"A pizza com abacaxi é uma combinação legítima e saborosa.","fontes":[{"id":"P1","url":"https://pt.wikipedia.org/wiki/Pizza_havaiana","titulo":"Pizza havaiana","trecho":"Criada em 1962 no Canadá por Sam Panopoulos."},{"id":"C1","url":"https://example.org/tradicao","titulo":"Tradição italiana","trecho":"A AVPN não reconhece abacaxi."}],"argumentos":["contraste doce-salgado"]}' ;;
  *encerrar_eixo*) sleep 1; if (( RANDOM % 2 )); then
          done_json '{"intervencao":"O CONTRA está atacando um espantalho: ninguém disse que abacaxi é obrigatório.","infracoes":[{"lado":"contra","tipo":"espantalho","trecho":"querem obrigar todos","explicacao":"distorce a tese do PRÓ"}],"fontes_verificadas":[{"id":"P1","ok":true,"nota":"confere"},{"id":"C1","ok":false,"nota":"página não existe"}],"encerrar_eixo":true}'
        else done_json '{"intervencao":null,"infracoes":[],"fontes_verificadas":[{"id":"P1","ok":true,"nota":"confere"}],"encerrar_eixo":false}'; fi ;;
  *vencedor*) sleep 1; done_json '{"vencedor":"pro","justificativa":"O PRÓ citou fontes verificáveis e concedeu pontos; o CONTRA usou um espantalho e uma fonte que não confere.","eixos":[{"titulo":"Sabor","nota_pro":8,"nota_contra":6,"comentario":"PRÓ trouxe evidência sensorial."},{"titulo":"Tradição","nota_pro":6,"nota_contra":8,"comentario":"CONTRA foi mais forte aqui."},{"titulo":"Liberdade","nota_pro":9,"nota_contra":5,"comentario":"Argumento simples e sólido."}],"infracoes_resumo":"CONTRA: 1 espantalho, 1 fonte inválida."}' ;;
  *) text="Respeito o ponto do oponente, mas os dados mostram outra coisa: a combinação de doce e salgado é antiga na culinária [P1], e a própria origem da receita prova que ela nasceu da experimentação [C1]. Não se trata de obrigar ninguém, e sim de reconhecer que gosto se discute com evidência."
     for w in $text; do tok "$w "; done
     emit "{\"type\":\"result\",\"is_error\":false,\"result\":\"$text\"}" ;;
esac
