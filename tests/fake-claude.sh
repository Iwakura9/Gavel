#!/usr/bin/env bash
# Finge ser o `claude -p --output-format stream-json` para testar a TUI sem gastar limite.
# Uso: GAVEL_CLAUDE_BIN=tests/fake-claude.sh cargo run -- "tema"
schema=""; prev=""; side=P
for a in "$@"; do [[ $prev == --json-schema ]] && schema=$a; [[ $a == *"debatedor CONTRA"* || $a == *"CON debater"* ]] && side=C; prev=$a; done
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
          if [[ $side == P ]]; then
          done_json '{"tese":"A pizza com abacaxi é uma combinação legítima e saborosa, com história e respaldo na ciência do paladar.","fontes":[{"id":"P1","url":"https://pt.wikipedia.org/wiki/Pizza_havaiana","titulo":"Pizza havaiana","trecho":"Criada em 1962 em Chatham, Ontário, no Canadá, pelo imigrante grego Sam Panopoulos, que quis experimentar o contraste entre o doce e o salgado inspirado na culinária chinesa."},{"id":"P2","url":"https://www.bbc.com/news/world-us-canada-38912262","titulo":"Hawaiian pizza: the Canadian invention that divides the world","trecho":"Pesquisa YouGov de 2019 indica que 12% dos americanos citam abacaxi entre as coberturas favoritas, à frente de anchovas."},{"id":"P3","url":"https://www.sciencedirect.com/science/article/pii/S0950329318305421?via%3Dihub&utm_source=exemplo-de-url-muito-longa","titulo":"Sweet–salty taste interactions in food pairing: a sensory analysis with 240 participants","trecho":"Combinações de doce e salgado aumentaram a aceitação média em 18% em relação aos controles, efeito atribuído à supressão mútua do amargor."},{"id":"P4","url":"https://www.nytimes.com/2017/02/21/world/canada/hawaiian-pizza-iceland-president.html","titulo":"Presidente da Islândia diz que proibiria abacaxi na pizza, depois volta atrás","trecho":"Guðni Jóhannesson disse que a frase era uma piada e que não tem poder para proibir coberturas."},{"id":"P5","url":"https://example.org/presunto-e-fruta","titulo":"Presunto com melão e outras tradições","trecho":"A cozinha italiana combina fruta e carne salgada há séculos, como no prosciutto e melone."}],"argumentos":["contraste doce-salgado","popularidade","tradição de fruta com salgado"]}'
          else
          done_json '{"tese":"Abacaxi descaracteriza a pizza: a acidez e a umidade prejudicam a massa e a combinação contraria a tradição.","fontes":[{"id":"C1","url":"https://www.pizzanapoletana.org/en/ricetta_pizza_napoletana","titulo":"Disciplinare AVPN — regras da verdadeira pizza napolitana","trecho":"A AVPN lista as coberturas permitidas; abacaxi não está entre elas."},{"id":"C2","url":"https://example.org/umidade-massa","titulo":"Umidade de coberturas e textura da massa","trecho":"Frutas com mais de 85% de água liberam líquido no forno e deixam o centro da pizza encharcado."},{"id":"C3","url":"https://www.bbc.com/news/world-us-canada-38912262","titulo":"Hawaiian pizza: the Canadian invention that divides the world","trecho":"Pesquisas mostram que o abacaxi está também entre as coberturas mais rejeitadas."},{"id":"C4","url":"https://example.org/bromelina","titulo":"Bromelina e queijo","trecho":"A bromelina do abacaxi fresco degrada proteínas do queijo e altera a textura."}],"argumentos":["tradição","textura","rejeição"]}'
          fi ;;
  *encerrar_eixo*) sleep 1; if (( RANDOM % 2 )); then
          done_json '{"intervencao":"O CONTRA está atacando um espantalho: ninguém disse que abacaxi é obrigatório.","infracoes":[{"lado":"contra","tipo":"espantalho","trecho":"querem obrigar todos","explicacao":"distorce a tese do PRÓ"}],"fontes_verificadas":[{"id":"P1","ok":true,"nota":"confere: a Wikipédia traz a data e o autor citados"},{"id":"C2","ok":false,"nota":"a página não existe (404); não há como confirmar o dado de 85%"}],"encerrar_eixo":true}'
        else done_json '{"intervencao":null,"infracoes":[],"fontes_verificadas":[{"id":"P1","ok":true,"nota":"confere"}],"encerrar_eixo":false}'; fi ;;
  *destino*) sleep 1; case $((RANDOM % 3)) in
          0) done_json '{"destino":"juiz","resposta":"Pela regra do debate, só contam argumentos com fonte; opinião pessoal não pontua."}' ;;
          1) done_json '{"destino":"pro","resposta":"Pergunta dirigida ao PRÓ. Com a palavra."}' ;;
          *) done_json '{"destino":"contra","resposta":"Essa cabe ao CONTRA responder."}' ;;
        esac ;;
  *vencedor*) sleep 1; done_json '{"vencedor":"pro","justificativa":"O PRÓ citou fontes verificáveis e concedeu pontos; o CONTRA usou um espantalho e uma fonte que não confere.","eixos":[{"titulo":"Sabor","nota_pro":8,"nota_contra":6,"comentario":"PRÓ trouxe evidência sensorial."},{"titulo":"Tradição","nota_pro":6,"nota_contra":8,"comentario":"CONTRA foi mais forte aqui."},{"titulo":"Liberdade","nota_pro":9,"nota_contra":5,"comentario":"Argumento simples e sólido."}],"infracoes_resumo":"CONTRA: 1 espantalho, 1 fonte inválida."}' ;;
  *) text="Respeito o ponto do oponente, mas os dados mostram outra coisa: a combinação de doce e salgado é antiga na culinária [${side}1], e a própria origem da receita prova que ela nasceu da experimentação [${side}3]. Não se trata de obrigar ninguém, e sim de reconhecer que gosto se discute com evidência [${side}1]."
     for w in $text; do tok "$w "; done
     emit "{\"type\":\"result\",\"is_error\":false,\"result\":\"$text\"}" ;;
esac
