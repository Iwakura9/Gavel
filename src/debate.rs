use crate::agent::{Agent, Ev};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

const JUDGE_MODEL: &str = "claude-opus-5-5";
const DEBATER_MODEL: &str = "claude-sonnet-5-5";
const MAX_ROUNDS: usize = 3;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Side {
    Pro = 0,
    Con = 1,
    Judge = 2,
}

impl Side {
    pub fn name(self) -> &'static str {
        ["PRÓ", "CONTRA", "JUIZ"][self as usize]
    }
    fn other(self) -> Side {
        if self == Side::Pro { Side::Con } else { Side::Pro }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Eixo {
    pub titulo: String,
    pub pergunta: String,
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Framing {
    pub definicoes: String,
    pub onus_da_prova: String,
    pub eixos: Vec<Eixo>,
    pub regras: String,
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Fonte {
    pub id: String,
    pub url: String,
    pub titulo: String,
    pub trecho: String,
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Dossie {
    pub tese: String,
    pub fontes: Vec<Fonte>,
    pub argumentos: Vec<String>,
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Infracao {
    pub lado: String,
    pub tipo: String,
    pub trecho: String,
    pub explicacao: String,
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Verificacao {
    pub id: String,
    pub ok: bool,
    pub nota: String,
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Check {
    pub intervencao: Option<String>,
    pub infracoes: Vec<Infracao>,
    pub fontes_verificadas: Vec<Verificacao>,
    pub encerrar_eixo: bool,
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct NotaEixo {
    pub titulo: String,
    pub nota_pro: f64,
    pub nota_contra: f64,
    pub comentario: String,
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Verdict {
    pub vencedor: String,
    pub justificativa: String,
    pub eixos: Vec<NotaEixo>,
    pub infracoes_resumo: String,
}

const FRAMING_SCHEMA: &str = r#"{"type":"object","properties":{"definicoes":{"type":"string"},"onus_da_prova":{"type":"string"},"eixos":{"type":"array","minItems":3,"maxItems":5,"items":{"type":"object","properties":{"titulo":{"type":"string"},"pergunta":{"type":"string"}},"required":["titulo","pergunta"]}},"regras":{"type":"string"}},"required":["definicoes","onus_da_prova","eixos","regras"]}"#;
const DOSSIE_SCHEMA: &str = r#"{"type":"object","properties":{"tese":{"type":"string"},"fontes":{"type":"array","items":{"type":"object","properties":{"id":{"type":"string"},"url":{"type":"string"},"titulo":{"type":"string"},"trecho":{"type":"string"}},"required":["id","url","titulo","trecho"]}},"argumentos":{"type":"array","items":{"type":"string"}}},"required":["tese","fontes","argumentos"]}"#;
const CHECK_SCHEMA: &str = r#"{"type":"object","properties":{"intervencao":{"type":["string","null"]},"infracoes":{"type":"array","items":{"type":"object","properties":{"lado":{"enum":["pro","contra"]},"tipo":{"type":"string"},"trecho":{"type":"string"},"explicacao":{"type":"string"}},"required":["lado","tipo","trecho","explicacao"]}},"fontes_verificadas":{"type":"array","items":{"type":"object","properties":{"id":{"type":"string"},"ok":{"type":"boolean"},"nota":{"type":"string"}},"required":["id","ok","nota"]}},"encerrar_eixo":{"type":"boolean"}},"required":["intervencao","infracoes","fontes_verificadas","encerrar_eixo"]}"#;
const ASK_SCHEMA: &str = r#"{"type":"object","properties":{"destino":{"enum":["juiz","pro","contra"]},"resposta":{"type":"string"}},"required":["destino","resposta"]}"#;
const VERDICT_SCHEMA: &str = r#"{"type":"object","properties":{"vencedor":{"enum":["pro","contra","empate"]},"justificativa":{"type":"string"},"eixos":{"type":"array","items":{"type":"object","properties":{"titulo":{"type":"string"},"nota_pro":{"type":"number"},"nota_contra":{"type":"number"},"comentario":{"type":"string"}},"required":["titulo","nota_pro","nota_contra","comentario"]}},"infracoes_resumo":{"type":"string"}},"required":["vencedor","justificativa","eixos","infracoes_resumo"]}"#;

const JUDGE_SYSTEM: &str = "Você é o JUIZ e mediador de um debate entre dois agentes de IA: PRÓ (favorável ao tema) e CONTRA (contrário). \
Você é neutro e fala pouco. Suas funções:
1. Enquadrar o debate: definições, ônus da prova e de 3 a 5 eixos (prefira 3 ou 4), que são os subtemas que orbitam o tema principal.
2. Ao fim de cada rodada, apontar falácias (espantalho, ad hominem, falso dilema, apelo à emoção no lugar de evidência, \
generalização apressada, cherry-picking, etc.), afirmações falsas ou sem fonte e citações distorcidas. Use WebFetch para conferir \
por amostragem as fontes citadas que parecerem duvidosas ou decisivas (no máximo 2 por rodada). Só registre infrações reais e relevantes.
3. Decidir quando um eixo se esgotou (argumentos se repetindo, ou o ponto central já respondido pelos dois).
4. Dar o veredito: quem ARGUMENTOU melhor (evidência, lógica, honestidade, resposta direta ao oponente), não quem \"tem razão\". \
Conceder pontos verdadeiros do oponente é virtude, não fraqueza. Nunca puna um lado pela posição que defende, só pela conduta argumentativa.
Escreva em português. Intervenções têm no máximo 2 frases, com tom de juiz espirituoso mas sério; se não houver nada relevante a dizer, intervencao = null.";

fn debater_system(side: Side, topic: &str) -> String {
    let (pos, id) = if side == Side::Pro { ("FAVORÁVEL", "P") } else { ("CONTRÁRIA", "C") };
    format!(
        "Você é o debatedor {name} em um debate sobre: «{topic}». Sua posição é {pos} ao tema. Um juiz neutro (outro agente) avalia você.
Regras:
- Defenda sua posição com convicção e com a melhor evidência disponível, mas como um advogado honesto: nunca invente fatos, números, citações ou fontes.
- Toda afirmação factual relevante cita uma fonte do seu dossiê pelo id entre colchetes, ex.: [{id}3]. Se pesquisar algo novo no meio do debate, cite a URL.
- Responda diretamente aos argumentos do oponente. Nada de espantalho, ad hominem, falso dilema, apelo à emoção no lugar de evidência ou generalização apressada.
- Se o oponente tiver um ponto verdadeiro, conceda-o e mostre por que sua posição ainda se sustenta: o juiz premia honestidade.
- Se o juiz apontar uma infração sua, reconheça ou defenda-se brevemente no início da próxima fala.
- Falas de no máximo ~200 palavras, em português, prosa direta, sem cabeçalhos nem markdown. Não narre o que vai fazer (\"vou pesquisar...\"): apenas faça e fale.",
        name = side.name()
    )
}

/// Orquestrador → UI.
pub enum Msg {
    Phase(String),
    Framing(Framing),
    Eixo(usize),
    Busy(Side),
    Start(Side, String),
    Token(Side, String),
    Tool(Side, String),
    Said(Side, String),
    Judge(String),
    Check(Check),
    Dossie(Side, Dossie),
    Usage(f64),
    Verdict(Verdict),
    Error(String),
    Saved(PathBuf),
    /// Modo manual: o debate parou e espera o próximo passo.
    Waiting,
}

/// UI → orquestrador (aplicado entre turnos).
pub enum Cmd {
    Manual(bool),
    Next,
    Ask(String),
}

#[derive(Serialize, Default)]
struct Entry {
    quem: String,
    titulo: String,
    texto: String,
}

#[derive(Serialize, Default)]
struct Record {
    tema: String,
    enquadramento: Option<Framing>,
    dossies: [Option<Dossie>; 2],
    transcricao: Vec<Entry>,
    veredito: Option<Verdict>,
}

pub fn data_dir() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".into())).join(".local/share/gavel")
}

fn slug(s: &str) -> String {
    let s: String = s.to_lowercase().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect();
    s.split('-').filter(|p| !p.is_empty()).collect::<Vec<_>>().join("-").chars().take(40).collect()
}

struct Debate {
    agents: [Agent; 3],
    inbox: [Vec<String>; 3],
    tx: UnboundedSender<Msg>,
    rx: UnboundedReceiver<Cmd>,
    manual: bool,
    step: bool,
    rec: Record,
    path: PathBuf,
}

pub async fn run(topic: String, tx: UnboundedSender<Msg>, rx: UnboundedReceiver<Cmd>) {
    let unix = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let path = data_dir().join("debates").join(format!("{unix}-{}", slug(&topic)));
    let mut d = Debate {
        agents: [
            Agent::new(DEBATER_MODEL, debater_system(Side::Pro, &topic)),
            Agent::new(DEBATER_MODEL, debater_system(Side::Con, &topic)),
            Agent::new(JUDGE_MODEL, JUDGE_SYSTEM.into()),
        ],
        inbox: Default::default(),
        tx,
        rx,
        manual: false,
        step: false,
        rec: Record { tema: topic, ..Default::default() },
        path,
    };
    let _ = d.tx.send(Msg::Saved(d.path.with_extension("md")));
    if let Err(e) = d.flow().await {
        let _ = d.tx.send(Msg::Error(e));
    }
    d.save();
}

/// Um turno de um agente, repassando os eventos ao vivo para a UI.
async fn ask_with(agent: &mut Agent, side: Side, tx: &UnboundedSender<Msg>, prompt: &str, schema: Option<&str>) -> Result<crate::agent::Reply, String> {
    agent
        .ask(prompt, schema, |ev| {
            let _ = tx.send(match ev {
                Ev::Token(t) => Msg::Token(side, t),
                Ev::Tool(t) => Msg::Tool(side, t),
                Ev::Usage(u) => Msg::Usage(u),
            });
        })
        .await
        .map_err(|e| format!("{}: {e}", side.name()))
}

fn json<T: for<'a> Deserialize<'a>>(side: Side, r: crate::agent::Reply) -> Result<T, String> {
    let v = r.json.or_else(|| serde_json::from_str::<Value>(&r.text).ok()).ok_or(format!("{}: resposta sem JSON", side.name()))?;
    serde_json::from_value(v).map_err(|e| format!("{}: JSON inválido: {e}", side.name()))
}

fn framing_text(f: &Framing) -> String {
    let eixos: Vec<String> = f.eixos.iter().enumerate().map(|(i, e)| format!("{}. {} — {}", i + 1, e.titulo, e.pergunta)).collect();
    format!("Definições: {}\nÔnus da prova: {}\nEixos:\n{}\nRegras: {}", f.definicoes, f.onus_da_prova, eixos.join("\n"), f.regras)
}

fn check_text(c: &Check) -> String {
    let mut s = c.intervencao.clone().unwrap_or_default();
    for i in &c.infracoes {
        s += &format!("\n⚠ {} — {}: {} («{}»)", if i.lado == "pro" { "PRÓ" } else { "CONTRA" }, i.tipo, i.explicacao, i.trecho);
    }
    for v in &c.fontes_verificadas {
        s += &format!("\n{} [{}] {}", if v.ok { "✓" } else { "✗" }, v.id, v.nota);
    }
    s.trim().to_string()
}

impl Debate {
    fn send(&self, m: Msg) {
        let _ = self.tx.send(m);
    }

    fn log(&mut self, quem: &str, titulo: &str, texto: &str) {
        self.rec.transcricao.push(Entry { quem: quem.into(), titulo: titulo.into(), texto: texto.into() });
        self.save();
    }

    async fn ask(&mut self, side: Side, prompt: &str, schema: Option<&str>) -> Result<crate::agent::Reply, String> {
        ask_with(&mut self.agents[side as usize], side, &self.tx, prompt, schema).await
    }

    /// Prompt de um agente = tudo que aconteceu desde o último turno dele + a instrução atual.
    fn prompt(&mut self, side: Side, instr: &str) -> String {
        let mut p = std::mem::take(&mut self.inbox[side as usize]).join("\n\n");
        if !p.is_empty() {
            p += "\n\n---\n";
        }
        p + instr
    }

    async fn ask_json<T: for<'a> Deserialize<'a>>(&mut self, side: Side, instr: &str, schema: &str) -> Result<T, String> {
        self.send(Msg::Busy(side));
        let p = self.prompt(side, instr);
        let r = self.ask(side, &p, Some(schema)).await?;
        json(side, r)
    }

    /// Fala de um debatedor; o texto vai para o oponente e para o juiz.
    async fn speak(&mut self, side: Side, title: &str, instr: &str) -> Result<(), String> {
        self.send(Msg::Start(side, title.into()));
        let p = self.prompt(side, instr);
        let text = self.ask(side, &p, None).await?.text;
        self.send(Msg::Said(side, text.clone()));
        let news = format!("{} ({title}):\n{text}", side.name());
        self.inbox[side.other() as usize].push(news.clone());
        self.inbox[Side::Judge as usize].push(news);
        self.log(side.name(), title, &text);
        Ok(())
    }

    /// Entre turnos: atende perguntas da plateia e, no modo manual, espera o próximo passo.
    async fn checkpoint(&mut self) -> Result<(), String> {
        loop {
            while let Ok(cmd) = self.rx.try_recv() {
                self.handle(cmd).await?;
            }
            if !self.manual || std::mem::take(&mut self.step) {
                return Ok(());
            }
            self.send(Msg::Waiting);
            match self.rx.recv().await {
                Some(cmd) => self.handle(cmd).await?,
                None => return Ok(()),
            }
        }
    }

    async fn handle(&mut self, cmd: Cmd) -> Result<(), String> {
        match cmd {
            Cmd::Manual(m) => self.manual = m,
            Cmd::Next => self.step = true,
            Cmd::Ask(q) => {
                let instr = format!(
                    "PERGUNTA DA PLATEIA: {q}\nSe ela cabe a você (regras, andamento, falácias, fontes, avaliação), destino = juiz e responda \
em até 3 frases. Se for dirigida a um dos lados ou for melhor respondida por ele, destino = pro ou contra e em resposta escreva \
só uma frase curta passando a palavra."
                );
                #[derive(Deserialize)]
                struct Answer {
                    destino: String,
                    resposta: String,
                }
                let a: Answer = self.ask_json(Side::Judge, &instr, ASK_SCHEMA).await?;
                self.send(Msg::Judge(format!("❓ «{q}»\n{}", a.resposta)));
                self.log("JUIZ", &format!("pergunta da plateia: {q}"), &a.resposta);
                let side = match a.destino.as_str() {
                    "pro" => Side::Pro,
                    "contra" => Side::Con,
                    _ => return Ok(()),
                };
                let instr = format!("PERGUNTA DA PLATEIA, encaminhada a você pelo juiz: {q}\nResponda brevemente (até ~100 palavras) e com honestidade.");
                self.speak(side, &format!("pergunta da plateia: {q}"), &instr).await?;
            }
        }
        Ok(())
    }

    async fn flow(&mut self) -> Result<(), String> {
        let topic = self.rec.tema.clone();
        self.send(Msg::Phase("O juiz está enquadrando o tema".into()));
        let f: Framing = self.ask_json(Side::Judge, &format!("Tema do debate: «{topic}». Enquadre o debate de forma concisa."), FRAMING_SCHEMA).await?;
        self.send(Msg::Framing(f.clone()));
        self.log("JUIZ", "Enquadramento", &framing_text(&f));
        self.rec.enquadramento = Some(f.clone());
        self.checkpoint().await?;

        self.send(Msg::Phase("Pesquisa".into()));
        let research = |id: char| {
            format!(
                "Enquadramento do juiz:\n{}\n\nFASE DE PESQUISA: use WebSearch e WebFetch para montar seu dossiê. Busque fontes confiáveis \
(dados oficiais, estudos, imprensa de qualidade) e antecipe o que o outro lado vai usar. Faça no máximo ~8 buscas. Retorne de 5 a 10 \
fontes com id {id}1, {id}2..., a URL real que você consultou, o título e um trecho ou dado concreto; e seus principais argumentos.",
                framing_text(&f)
            )
        };
        let (pp, pc) = (research('P'), research('C'));
        self.send(Msg::Busy(Side::Pro));
        self.send(Msg::Busy(Side::Con));
        let [pro, con, _] = &mut self.agents;
        let (rp, rc) = tokio::join!(
            ask_with(pro, Side::Pro, &self.tx, &pp, Some(DOSSIE_SCHEMA)),
            ask_with(con, Side::Con, &self.tx, &pc, Some(DOSSIE_SCHEMA))
        );
        for (side, r) in [(Side::Pro, rp), (Side::Con, rc)] {
            let d: Dossie = json(side, r?)?;
            let fontes: Vec<String> = d.fontes.iter().map(|s| format!("[{}] {} — {} — «{}»", s.id, s.titulo, s.url, s.trecho)).collect();
            self.inbox[Side::Judge as usize].push(format!("DOSSIÊ DO {} (para verificação)\nTese: {}\n{}", side.name(), d.tese, fontes.join("\n")));
            self.send(Msg::Dossie(side, d.clone()));
            self.rec.dossies[side as usize] = Some(d);
        }
        self.save();
        self.checkpoint().await?;

        let n = f.eixos.len();
        for (i, eixo) in f.eixos.iter().enumerate() {
            self.send(Msg::Eixo(i));
            self.send(Msg::Phase(format!("Eixo {}/{n}", i + 1)));
            let first = if i % 2 == 0 { Side::Pro } else { Side::Con };
            for round in 1..=MAX_ROUNDS {
                for side in [first, first.other()] {
                    let (title, instr) = if round == 1 {
                        let kind = if side == first { "abertura" } else { "resposta" };
                        (format!("eixo {}: {kind}", i + 1), format!("EIXO {}/{n}: «{}» — {}\nFaça sua fala de {kind} neste eixo.", i + 1, eixo.titulo, eixo.pergunta))
                    } else {
                        (format!("eixo {}: réplica {round}", i + 1), format!("Sua réplica (rodada {round}) no eixo «{}».", eixo.titulo))
                    };
                    self.speak(side, &title, &instr).await?;
                    self.checkpoint().await?;
                }
                let last = round == MAX_ROUNDS;
                let instr = format!(
                    "Fim da rodada {round} do eixo «{}». Avalie as falas desta rodada. encerrar_eixo = true se o eixo se esgotou{}.",
                    eixo.titulo,
                    if last { " (esta é a última rodada permitida; o eixo será encerrado de qualquer forma)" } else { "" }
                );
                let c: Check = self.ask_json(Side::Judge, &instr, CHECK_SCHEMA).await?;
                for inf in &c.infracoes {
                    let side = if inf.lado == "pro" { Side::Pro } else { Side::Con };
                    self.inbox[side as usize].push(format!("O JUIZ apontou uma infração sua: {} — {} («{}»)", inf.tipo, inf.explicacao, inf.trecho));
                }
                for v in c.fontes_verificadas.iter().filter(|v| !v.ok) {
                    let side = if v.id.starts_with('P') { Side::Pro } else { Side::Con };
                    self.inbox[side as usize].push(format!("O JUIZ verificou sua fonte [{}] e ela NÃO confere: {}", v.id, v.nota));
                }
                if let Some(t) = &c.intervencao {
                    for side in [Side::Pro, Side::Con] {
                        self.inbox[side as usize].push(format!("O JUIZ disse: {t}"));
                    }
                }
                self.log("JUIZ", &format!("eixo {}: avaliação da rodada {round}", i + 1), &check_text(&c));
                let close = c.encerrar_eixo;
                self.send(Msg::Check(c));
                self.checkpoint().await?;
                if close {
                    break;
                }
            }
        }

        self.send(Msg::Phase("Considerações finais".into()));
        for side in [Side::Pro, Side::Con] {
            self.speak(side, "considerações finais", "CONSIDERAÇÕES FINAIS: resuma por que sua posição saiu mais forte, em até ~150 palavras.").await?;
            self.checkpoint().await?;
        }

        self.send(Msg::Phase("O juiz está deliberando".into()));
        let v: Verdict = self.ask_json(Side::Judge, "O debate terminou. Dê o veredito: vencedor, notas de 0 a 10 por eixo para cada lado, justificativa e resumo das infrações.", VERDICT_SCHEMA).await?;
        self.rec.veredito = Some(v.clone());
        self.save();
        self.send(Msg::Verdict(v));
        self.send(Msg::Phase("Fim".into()));
        Ok(())
    }

    fn save(&self) {
        let res = std::fs::create_dir_all(self.path.parent().unwrap())
            .and_then(|_| std::fs::write(self.path.with_extension("json"), serde_json::to_string_pretty(&self.rec).unwrap()))
            .and_then(|_| std::fs::write(self.path.with_extension("md"), markdown(&self.rec)));
        if let Err(e) = res {
            self.send(Msg::Error(format!("falha ao salvar o debate: {e}")));
        }
    }
}

fn markdown(r: &Record) -> String {
    let mut s = format!("# ⚖ {}\n\n", r.tema);
    for (side, d) in [Side::Pro, Side::Con].iter().zip(&r.dossies) {
        if let Some(d) = d {
            s += &format!("## Dossiê {}\n\n**Tese:** {}\n\n", side.name(), d.tese);
            for f in &d.fontes {
                s += &format!("- **[{}]** [{}]({}) — {}\n", f.id, f.titulo, f.url, f.trecho);
            }
            s += "\n";
        }
    }
    s += "## Debate\n\n";
    for e in &r.transcricao {
        s += &format!("### {} — {}\n\n{}\n\n", e.quem, e.titulo, e.texto);
    }
    if let Some(v) = &r.veredito {
        s += &format!("## Veredito: {}\n\n{}\n\n| Eixo | PRÓ | CONTRA | Comentário |\n|---|---|---|---|\n", v.vencedor.to_uppercase(), v.justificativa);
        for e in &v.eixos {
            s += &format!("| {} | {} | {} | {} |\n", e.titulo, e.nota_pro, e.nota_contra, e.comentario);
        }
        s += &format!("\n**Infrações:** {}\n", v.infracoes_resumo);
    }
    s
}
