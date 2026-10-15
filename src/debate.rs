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
        tr!(["PRÓ", "CONTRA", "JUIZ"], ["PRO", "CON", "JUDGE"])[self as usize]
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

const JUDGE_SYSTEM_EN: &str = "You are the JUDGE and moderator of a debate between two AI agents: PRO (in favor of the motion) and CON (against it). \
You are neutral and speak little. Your duties:
1. Frame the debate: definitions, burden of proof and 3 to 5 issues (prefer 3 or 4), the subtopics that orbit the main motion.
2. At the end of each round, point out fallacies (straw man, ad hominem, false dilemma, appeal to emotion instead of evidence, \
hasty generalization, cherry-picking, etc.), false or unsourced claims and distorted quotes. Use WebFetch to spot-check \
cited sources that look dubious or decisive (at most 2 per round). Only record real, relevant infractions.
3. Decide when an issue is exhausted (arguments repeating, or the core point already answered by both).
4. Deliver the verdict: who ARGUED better (evidence, logic, honesty, directly answering the opponent), not who \"is right\". \
Conceding the opponent's true points is a virtue, not a weakness. Never penalize a side for the position it defends, only for its argumentative conduct.
Write in English. Interventions are at most 2 sentences, in the tone of a witty but serious judge; if there is nothing relevant to say, intervencao = null.";

fn debater_system(side: Side, topic: &str) -> String {
    let (pos, id) = if side == Side::Pro { (tr!("FAVORÁVEL", "IN FAVOR OF"), "P") } else { (tr!("CONTRÁRIA", "AGAINST"), "C") };
    trf!(
        "Você é o debatedor {name} em um debate sobre: «{topic}». Sua posição é {pos} ao tema. Um juiz neutro (outro agente) avalia você.
Regras:
- Defenda sua posição com convicção e com a melhor evidência disponível, mas como um advogado honesto: nunca invente fatos, números, citações ou fontes.
- Toda afirmação factual relevante cita uma fonte do seu dossiê pelo id entre colchetes, ex.: [{id}3]. Se pesquisar algo novo no meio do debate, cite a URL.
- Responda diretamente aos argumentos do oponente. Nada de espantalho, ad hominem, falso dilema, apelo à emoção no lugar de evidência ou generalização apressada.
- Se o oponente tiver um ponto verdadeiro, conceda-o e mostre por que sua posição ainda se sustenta: o juiz premia honestidade.
- Se o juiz apontar uma infração sua, reconheça ou defenda-se brevemente no início da próxima fala.
- Falas de no máximo ~200 palavras, em português, prosa direta, sem cabeçalhos nem markdown. Não narre o que vai fazer (\"vou pesquisar...\"): apenas faça e fale.",
        "You are the {name} debater in a debate about: «{topic}». Your position is {pos} the motion. A neutral judge (another agent) evaluates you.
Rules:
- Defend your position with conviction and the best evidence available, but as an honest lawyer: never invent facts, numbers, quotes or sources.
- Every relevant factual claim cites a source from your dossier by its id in brackets, e.g. [{id}3]. If you research something new mid-debate, cite the URL.
- Answer your opponent's arguments directly. No straw man, ad hominem, false dilemma, appeal to emotion instead of evidence or hasty generalization.
- If your opponent has a true point, concede it and show why your position still holds: the judge rewards honesty.
- If the judge flags an infraction of yours, acknowledge it or briefly defend yourself at the start of your next speech.
- Speeches of at most ~200 words, in English, plain prose, no headings or markdown. Don't narrate what you are about to do (\"I'll search...\"): just do it and speak.",
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
            Agent::new(JUDGE_MODEL, tr!(JUDGE_SYSTEM, JUDGE_SYSTEM_EN).into()),
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
    let v = r.json.or_else(|| serde_json::from_str::<Value>(&r.text).ok()).ok_or(trf!("{}: resposta sem JSON", "{}: reply without JSON", side.name()))?;
    serde_json::from_value(v).map_err(|e| trf!("{}: JSON inválido: {e}", "{}: invalid JSON: {e}", side.name()))
}

fn framing_text(f: &Framing) -> String {
    let eixos: Vec<String> = f.eixos.iter().enumerate().map(|(i, e)| format!("{}. {} — {}", i + 1, e.titulo, e.pergunta)).collect();
    trf!("Definições: {}\nÔnus da prova: {}\nEixos:\n{}\nRegras: {}", "Definitions: {}\nBurden of proof: {}\nIssues:\n{}\nRules: {}", f.definicoes, f.onus_da_prova, eixos.join("\n"), f.regras)
}

fn check_text(c: &Check) -> String {
    let mut s = c.intervencao.clone().unwrap_or_default();
    for i in &c.infracoes {
        s += &format!("\n⚠ {} — {}: {} («{}»)", if i.lado == "pro" { Side::Pro } else { Side::Con }.name(), i.tipo, i.explicacao, i.trecho);
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
                let instr = trf!(
                    "PERGUNTA DA PLATEIA: {q}\nSe ela cabe a você (regras, andamento, falácias, fontes, avaliação), destino = juiz e responda \
em até 3 frases. Se for dirigida a um dos lados ou for melhor respondida por ele, destino = pro ou contra e em resposta escreva \
só uma frase curta passando a palavra.",
                    "AUDIENCE QUESTION: {q}\nIf it is yours to answer (rules, progress, fallacies, sources, assessment), destino = juiz and answer \
in at most 3 sentences. If it is addressed to one of the sides or better answered by it, destino = pro or contra and in resposta write \
only one short sentence handing over the floor."
                );
                #[derive(Deserialize)]
                struct Answer {
                    destino: String,
                    resposta: String,
                }
                let a: Answer = self.ask_json(Side::Judge, &instr, ASK_SCHEMA).await?;
                self.send(Msg::Judge(format!("❓ «{q}»\n{}", a.resposta)));
                self.log(Side::Judge.name(), &trf!("pergunta da plateia: {q}", "audience question: {q}"), &a.resposta);
                let side = match a.destino.as_str() {
                    "pro" => Side::Pro,
                    "contra" => Side::Con,
                    _ => return Ok(()),
                };
                let instr = trf!("PERGUNTA DA PLATEIA, encaminhada a você pelo juiz: {q}\nResponda brevemente (até ~100 palavras) e com honestidade.", "AUDIENCE QUESTION, forwarded to you by the judge: {q}\nAnswer briefly (up to ~100 words) and honestly.");
                self.speak(side, &trf!("pergunta da plateia: {q}", "audience question: {q}"), &instr).await?;
            }
        }
        Ok(())
    }

    async fn flow(&mut self) -> Result<(), String> {
        let topic = self.rec.tema.clone();
        self.send(Msg::Phase(tr!("O juiz está enquadrando o tema", "The judge is framing the topic").into()));
        let f: Framing = self.ask_json(Side::Judge, &trf!("Tema do debate: «{topic}». Enquadre o debate de forma concisa.", "Debate motion: «{topic}». Frame the debate concisely."), FRAMING_SCHEMA).await?;
        self.send(Msg::Framing(f.clone()));
        self.log(Side::Judge.name(), tr!("Enquadramento", "Framing"), &framing_text(&f));
        self.rec.enquadramento = Some(f.clone());
        self.checkpoint().await?;

        self.send(Msg::Phase(tr!("Pesquisa", "Research").into()));
        let research = |id: char| {
            trf!(
                "Enquadramento do juiz:\n{}\n\nFASE DE PESQUISA: use WebSearch e WebFetch para montar seu dossiê. Busque fontes confiáveis \
(dados oficiais, estudos, imprensa de qualidade) e antecipe o que o outro lado vai usar. Faça no máximo ~8 buscas. Retorne de 5 a 10 \
fontes com id {id}1, {id}2..., a URL real que você consultou, o título e um trecho ou dado concreto; e seus principais argumentos.",
                "Judge's framing:\n{}\n\nRESEARCH PHASE: use WebSearch and WebFetch to build your dossier. Look for reliable sources \
(official data, studies, quality press) and anticipate what the other side will use. Do at most ~8 searches. Return 5 to 10 \
sources with ids {id}1, {id}2..., the real URL you consulted, the title and a concrete excerpt or data point; and your main arguments.",
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
            self.inbox[Side::Judge as usize].push(trf!("DOSSIÊ DO {} (para verificação)\nTese: {}\n{}", "{} DOSSIER (for verification)\nThesis: {}\n{}", side.name(), d.tese, fontes.join("\n")));
            self.send(Msg::Dossie(side, d.clone()));
            self.rec.dossies[side as usize] = Some(d);
        }
        self.save();
        self.checkpoint().await?;

        let n = f.eixos.len();
        for (i, eixo) in f.eixos.iter().enumerate() {
            self.send(Msg::Eixo(i));
            self.send(Msg::Phase(trf!("Eixo {}/{n}", "Issue {}/{n}", i + 1)));
            let first = if i % 2 == 0 { Side::Pro } else { Side::Con };
            for round in 1..=MAX_ROUNDS {
                for side in [first, first.other()] {
                    let (title, instr) = if round == 1 {
                        let kind = if side == first { tr!("abertura", "opening") } else { tr!("resposta", "response") };
                        (trf!("eixo {}: {kind}", "issue {}: {kind}", i + 1), trf!("EIXO {}/{n}: «{}» — {}\nFaça sua fala de {kind} neste eixo.", "ISSUE {}/{n}: «{}» — {}\nGive your {kind} speech on this issue.", i + 1, eixo.titulo, eixo.pergunta))
                    } else {
                        (trf!("eixo {}: réplica {round}", "issue {}: rebuttal {round}", i + 1), trf!("Sua réplica (rodada {round}) no eixo «{}».", "Your rebuttal (round {round}) on the issue «{}».", eixo.titulo))
                    };
                    self.speak(side, &title, &instr).await?;
                    self.checkpoint().await?;
                }
                let last = round == MAX_ROUNDS;
                let instr = trf!(
                    "Fim da rodada {round} do eixo «{}». Avalie as falas desta rodada. encerrar_eixo = true se o eixo se esgotou{}.",
                    "End of round {round} of the issue «{}». Evaluate this round's speeches. encerrar_eixo = true if the issue is exhausted{}.",
                    eixo.titulo,
                    if last { tr!(" (esta é a última rodada permitida; o eixo será encerrado de qualquer forma)", " (this is the last round allowed; the issue will be closed either way)") } else { "" }
                );
                let c: Check = self.ask_json(Side::Judge, &instr, CHECK_SCHEMA).await?;
                for inf in &c.infracoes {
                    let side = if inf.lado == "pro" { Side::Pro } else { Side::Con };
                    self.inbox[side as usize].push(trf!("O JUIZ apontou uma infração sua: {} — {} («{}»)", "The JUDGE flagged an infraction of yours: {} — {} («{}»)", inf.tipo, inf.explicacao, inf.trecho));
                }
                for v in c.fontes_verificadas.iter().filter(|v| !v.ok) {
                    let side = if v.id.starts_with('P') { Side::Pro } else { Side::Con };
                    self.inbox[side as usize].push(trf!("O JUIZ verificou sua fonte [{}] e ela NÃO confere: {}", "The JUDGE checked your source [{}] and it does NOT hold up: {}", v.id, v.nota));
                }
                if let Some(t) = &c.intervencao {
                    for side in [Side::Pro, Side::Con] {
                        self.inbox[side as usize].push(trf!("O JUIZ disse: {t}", "The JUDGE said: {t}"));
                    }
                }
                self.log(Side::Judge.name(), &trf!("eixo {}: avaliação da rodada {round}", "issue {}: round {round} assessment", i + 1), &check_text(&c));
                let close = c.encerrar_eixo;
                self.send(Msg::Check(c));
                self.checkpoint().await?;
                if close {
                    break;
                }
            }
        }

        self.send(Msg::Phase(tr!("Considerações finais", "Closing statements").into()));
        for side in [Side::Pro, Side::Con] {
            self.speak(side, tr!("considerações finais", "closing statement"), tr!("CONSIDERAÇÕES FINAIS: resuma por que sua posição saiu mais forte, em até ~150 palavras.", "CLOSING STATEMENT: sum up why your position came out stronger, in up to ~150 words.")).await?;
            self.checkpoint().await?;
        }

        self.send(Msg::Phase(tr!("O juiz está deliberando", "The judge is deliberating").into()));
        let v: Verdict = self.ask_json(Side::Judge, tr!("O debate terminou. Dê o veredito: vencedor, notas de 0 a 10 por eixo para cada lado, justificativa e resumo das infrações.", "The debate is over. Deliver the verdict: winner, scores from 0 to 10 per issue for each side, justification and a summary of the infractions."), VERDICT_SCHEMA).await?;
        self.rec.veredito = Some(v.clone());
        self.save();
        self.send(Msg::Verdict(v));
        self.send(Msg::Phase(tr!("Fim", "End").into()));
        Ok(())
    }

    fn save(&self) {
        let res = std::fs::create_dir_all(self.path.parent().unwrap())
            .and_then(|_| std::fs::write(self.path.with_extension("json"), serde_json::to_string_pretty(&self.rec).unwrap()))
            .and_then(|_| std::fs::write(self.path.with_extension("md"), markdown(&self.rec)));
        if let Err(e) = res {
            self.send(Msg::Error(trf!("falha ao salvar o debate: {e}", "failed to save the debate: {e}")));
        }
    }
}

fn markdown(r: &Record) -> String {
    let mut s = format!("# ⚖ {}\n\n", r.tema);
    for (side, d) in [Side::Pro, Side::Con].iter().zip(&r.dossies) {
        if let Some(d) = d {
            s += &trf!("## Dossiê {}\n\n**Tese:** {}\n\n", "## {} dossier\n\n**Thesis:** {}\n\n", side.name(), d.tese);
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
        let winner = match v.vencedor.as_str() {
            "pro" => Side::Pro.name(),
            "contra" => Side::Con.name(),
            _ => tr!("EMPATE", "TIE"),
        };
        let (pro, con) = (Side::Pro.name(), Side::Con.name());
        s += &trf!("## Veredito: {winner}\n\n{}\n\n| Eixo | {pro} | {con} | Comentário |\n|---|---|---|---|\n", "## Verdict: {winner}\n\n{}\n\n| Issue | {pro} | {con} | Comment |\n|---|---|---|---|\n", v.justificativa);
        for e in &v.eixos {
            s += &format!("| {} | {} | {} | {} |\n", e.titulo, e.nota_pro, e.nota_contra, e.comentario);
        }
        s += &trf!("\n**Infrações:** {}\n", "\n**Infractions:** {}\n", v.infracoes_resumo);
    }
    s
}
