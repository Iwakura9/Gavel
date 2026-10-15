use crate::agent::{Agent, Ev};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

const JUDGE_MODEL: &str = "claude-opus-5-5";
const DEBATER_MODEL: &str = "claude-sonnet-5-5";
/// Teto de rodadas por eixo, só como trava de custo: o juiz encerra antes, em consenso ou impasse.
const MAX_ROUNDS: usize = 6;
/// Antes disso quem abriu o eixo ainda não respondeu ao oponente.
const MIN_ROUNDS: usize = 2;
/// Rodada registrada em `feito` quando o eixo se encerra.
const CLOSED: usize = usize::MAX;
/// Teto da pauta, mesmo com o juiz incluindo eixos no meio do debate.
const MAX_EIXOS: usize = 6;

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
    /// Com o eixo em aberto: a questão que a próxima réplica deve enfrentar.
    #[serde(default)]
    pub ponto_em_disputa: Option<String>,
    /// em_aberto, consenso ou impasse; só os dois últimos encerram o eixo.
    #[serde(default)]
    pub estado_eixo: String,
    /// Registros antigos, de quando o juiz só dizia se encerrava o eixo.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub encerrar_eixo: bool,
    /// Pauta revisada ao encerrar o eixo: os eixos que vêm depois dele (None mantém a atual).
    pub proximos_eixos: Option<Vec<Eixo>>,
}
impl Check {
    pub fn closes(&self) -> bool {
        self.encerrar_eixo || matches!(self.estado_eixo.as_str(), "consenso" | "impasse")
    }

    /// O andamento do eixo em uma linha, para a tela e o registro.
    pub fn status(&self) -> Option<String> {
        match self.estado_eixo.as_str() {
            "consenso" => Some(tr!("■ Eixo encerrado: consenso.", "■ Issue closed: consensus.").into()),
            "impasse" => Some(tr!("■ Eixo encerrado: impasse.", "■ Issue closed: impasse.").into()),
            _ => self.ponto_em_disputa.as_ref().map(|p| trf!("→ Em disputa: {p}", "→ In dispute: {p}")),
        }
    }
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

const FRAMING_SCHEMA: &str = r#"{"type":"object","properties":{"definicoes":{"type":"string"},"onus_da_prova":{"type":"string"},"eixos":{"type":"array","minItems":1,"maxItems":6,"items":{"type":"object","properties":{"titulo":{"type":"string"},"pergunta":{"type":"string"}},"required":["titulo","pergunta"]}},"regras":{"type":"string"}},"required":["definicoes","onus_da_prova","eixos","regras"]}"#;
const DOSSIE_SCHEMA: &str = r#"{"type":"object","properties":{"tese":{"type":"string"},"fontes":{"type":"array","items":{"type":"object","properties":{"id":{"type":"string"},"url":{"type":"string"},"titulo":{"type":"string"},"trecho":{"type":"string"}},"required":["id","url","titulo","trecho"]}},"argumentos":{"type":"array","items":{"type":"string"}}},"required":["tese","fontes","argumentos"]}"#;
const CHECK_SCHEMA: &str = r#"{"type":"object","properties":{"intervencao":{"type":["string","null"]},"infracoes":{"type":"array","items":{"type":"object","properties":{"lado":{"enum":["pro","contra"]},"tipo":{"type":"string"},"trecho":{"type":"string"},"explicacao":{"type":"string"}},"required":["lado","tipo","trecho","explicacao"]}},"fontes_verificadas":{"type":"array","items":{"type":"object","properties":{"id":{"type":"string"},"ok":{"type":"boolean"},"nota":{"type":"string"}},"required":["id","ok","nota"]}},"ponto_em_disputa":{"type":["string","null"]},"estado_eixo":{"enum":["em_aberto","consenso","impasse"]},"proximos_eixos":{"type":["array","null"],"items":{"type":"object","properties":{"titulo":{"type":"string"},"pergunta":{"type":"string"}},"required":["titulo","pergunta"]}}},"required":["intervencao","infracoes","fontes_verificadas","ponto_em_disputa","estado_eixo","proximos_eixos"]}"#;
const ASK_SCHEMA: &str = r#"{"type":"object","properties":{"destino":{"enum":["juiz","pro","contra"]},"resposta":{"type":"string"}},"required":["destino","resposta"]}"#;
const VERDICT_SCHEMA: &str = r#"{"type":"object","properties":{"vencedor":{"enum":["pro","contra","empate"]},"justificativa":{"type":"string"},"eixos":{"type":"array","items":{"type":"object","properties":{"titulo":{"type":"string"},"nota_pro":{"type":"number"},"nota_contra":{"type":"number"},"comentario":{"type":"string"}},"required":["titulo","nota_pro","nota_contra","comentario"]}},"infracoes_resumo":{"type":"string"}},"required":["vencedor","justificativa","eixos","infracoes_resumo"]}"#;

const JUDGE_SYSTEM: &str = "Você é o JUIZ e mediador de um debate entre dois agentes de IA: PRÓ (favorável ao tema) e CONTRA (contrário). \
Você é neutro e fala pouco. Suas funções:
1. Enquadrar o debate: definições, ônus da prova e os eixos, que são os subtemas que orbitam o tema principal. A quantidade depende do tema: \
só os eixos que realmente decidem a questão (um tema simples pode ter 1 ou 2; um complexo, até 6), sem eixos redundantes ou periféricos.
2. Ao fim de cada rodada, apontar falácias (espantalho, ad hominem, falso dilema, apelo à emoção no lugar de evidência, \
generalização apressada, cherry-picking, etc.), afirmações falsas ou sem fonte e citações distorcidas. Use WebFetch para conferir \
por amostragem as fontes citadas que parecerem duvidosas ou decisivas (no máximo 2 por rodada). Só registre infrações reais e relevantes.
3. Conduzir cada eixo até ele se resolver; não há número fixo de rodadas. O eixo só termina em consenso (os dois lados aceitam explicitamente \
a mesma resposta à pergunta do eixo; concessões parciais não bastam) ou em impasse (uma rodada inteira sem argumento nem evidência nova, \
com a discordância reduzida a uma premissa que nenhum dos dois vai abandonar). Enquanto estiver em aberto, indique o ponto em disputa que a \
próxima réplica deve enfrentar. Ao encerrar um eixo, revise a pauta: retirar eixos que ficaram desnecessários (já cobertos ou irrelevantes) e incluir os mais importantes que surgiram no debate.
4. Dar o veredito: quem ARGUMENTOU melhor (evidência, lógica, honestidade, resposta direta ao oponente), não quem \"tem razão\". \
Nunca puna um lado pela posição que defende, só pela conduta argumentativa.
Os debatedores são advogados das suas posições: defender a própria tese com firmeza é o papel deles. Conceder não é mérito em si: \
corrigir um erro factual próprio é obrigação, e ceder a tese sem que o oponente tenha provado o ponto é fraqueza argumentativa. \
Não diga nas regras nem nas intervenções que concessões são premiadas, e não elogie concessões.
Escreva em português. Intervenções têm no máximo 2 frases, com tom de juiz espirituoso mas sério; se não houver nada relevante a dizer, intervencao = null.";

const JUDGE_SYSTEM_EN: &str = "You are the JUDGE and moderator of a debate between two AI agents: PRO (in favor of the motion) and CON (against it). \
You are neutral and speak little. Your duties:
1. Frame the debate: definitions, burden of proof and the issues, the subtopics that orbit the main motion. How many depends on the motion: \
only the issues that really decide the question (a simple motion may have 1 or 2; a complex one, up to 6), no redundant or peripheral ones.
2. At the end of each round, point out fallacies (straw man, ad hominem, false dilemma, appeal to emotion instead of evidence, \
hasty generalization, cherry-picking, etc.), false or unsourced claims and distorted quotes. Use WebFetch to spot-check \
cited sources that look dubious or decisive (at most 2 per round). Only record real, relevant infractions.
3. Carry each issue until it is resolved; there is no fixed number of rounds. An issue only ends in consensus (both sides explicitly accept \
the same answer to the issue's question; partial concessions are not enough) or in impasse (a whole round with no new argument or evidence, \
with the disagreement narrowed down to a premise neither side will give up). While it is open, name the point in dispute that the next \
rebuttal must address. When closing an issue, revise the agenda: drop issues that became unnecessary (already covered or irrelevant) and add the more important ones that came up in the debate.
4. Deliver the verdict: who ARGUED better (evidence, logic, honesty, directly answering the opponent), not who \"is right\". \
Never penalize a side for the position it defends, only for its argumentative conduct.
The debaters are advocates for their positions: defending their own thesis firmly is their job. Conceding is not a merit in itself: \
correcting one's own factual error is an obligation, and giving up the thesis without the opponent having proven the point is argumentative weakness. \
Don't say in the rules or in your interventions that concessions are rewarded, and don't praise concessions.
Write in English. Interventions are at most 2 sentences, in the tone of a witty but serious judge; if there is nothing relevant to say, intervencao = null.";

fn debater_system(side: Side, topic: &str) -> String {
    let (pos, id) = if side == Side::Pro { (tr!("FAVORÁVEL", "IN FAVOR OF"), "P") } else { (tr!("CONTRÁRIA", "AGAINST"), "C") };
    trf!(
        "Você é o debatedor {name} em um debate sobre: «{topic}». Sua posição é {pos} ao tema. Seu objetivo é vencer o debate; \
um juiz (outro agente) decide quem argumentou melhor.
Regras:
- Você é advogado da sua causa, não um mediador. Defenda sua posição com firmeza e com a melhor evidência disponível: destaque o que favorece seu lado, \
enquadre as evidências a seu favor e não ceda terreno. Mas seja um advogado honesto: nunca invente fatos, números, citações ou fontes.
- Toda afirmação factual relevante cita uma fonte do seu dossiê pelo id entre colchetes, ex.: [{id}3]. Se pesquisar algo novo no meio do debate, cite a URL.
- Responda diretamente aos argumentos do oponente. Nada de espantalho, ad hominem, falso dilema, apelo à emoção no lugar de evidência ou generalização apressada.
- Corrigir um erro não é ceder a tese. Se errou um fato ou uma fonte, corrija em uma frase e siga em frente. Só conceda um ponto do oponente quando ele trouxer \
evidência que você não consegue contestar, e de forma estreita: delimite o alcance e mostre por que isso não decide a questão. Não busque meio-termo por cortesia \
nem ceda a conclusão do eixo para encerrar a discussão: mude de posição só se o oponente provar o ponto.
- Ataque a maior fraqueza do argumento do oponente (falta de evidência, salto lógico, ônus da prova não cumprido) e termine cada fala com o ponto mais forte do seu lado.
- Se o juiz apontar uma infração sua, no início da próxima fala reconheça brevemente se a crítica for justa, ou defenda-se se não for.
- Falas de no máximo ~200 palavras, em português, prosa direta, sem cabeçalhos nem markdown. Não narre o que vai fazer (\"vou pesquisar...\"): apenas faça e fale.",
        "You are the {name} debater in a debate about: «{topic}». Your position is {pos} the motion. Your goal is to win the debate; \
a judge (another agent) decides who argued better.
Rules:
- You are an advocate for your side, not a mediator. Defend your position firmly and with the best evidence available: highlight what favors your side, \
frame the evidence in your favor and don't give up ground. But be an honest advocate: never invent facts, numbers, quotes or sources.
- Every relevant factual claim cites a source from your dossier by its id in brackets, e.g. [{id}3]. If you research something new mid-debate, cite the URL.
- Answer your opponent's arguments directly. No straw man, ad hominem, false dilemma, appeal to emotion instead of evidence or hasty generalization.
- Correcting a mistake is not giving up your thesis. If you got a fact or a source wrong, correct it in one sentence and move on. Only concede an opponent's point \
when they bring evidence you cannot contest, and narrowly: limit its scope and show why it doesn't decide the question. Don't seek middle ground out of courtesy \
or give up the issue's conclusion to end the discussion: change your position only if the opponent proves the point.
- Attack the biggest weakness in your opponent's argument (missing evidence, logical leap, unmet burden of proof) and end each speech with your side's strongest point.
- If the judge flags an infraction of yours, at the start of your next speech briefly acknowledge it if the criticism is fair, or defend yourself if it isn't.
- Speeches of at most ~200 words, in English, plain prose, no headings or markdown. Don't narrate what you are about to do (\"I'll search...\"): just do it and speak.",
        name = side.name()
    )
}

/// Orquestrador → UI.
pub enum Msg {
    Phase(String),
    Framing(Framing),
    Eixo(usize),
    /// O juiz revisou a pauta: a lista completa de eixos.
    Pauta(Vec<Eixo>),
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

#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
struct Entry {
    quem: String,
    titulo: String,
    texto: String,
    /// Avaliação do juiz em formato estruturado, para reconstruir a tela ao reabrir.
    #[serde(skip_serializing_if = "Option::is_none")]
    avaliacao: Option<Check>,
}

/// O `.json` salvo: o bastante para reler o debate ou retomá-lo de onde parou.
#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Record {
    pub tema: String,
    pub idioma: String,
    enquadramento: Option<Framing>,
    dossies: [Option<Dossie>; 2],
    transcricao: Vec<Entry>,
    pub veredito: Option<Verdict>,
    sessoes: [Option<String>; 3],
    /// O que cada agente ainda não recebeu (vai no início do próximo prompt dele).
    pendentes: [Vec<String>; 3],
    /// Último passo concluído: (eixo, rodada, vez); vez 0/1 = debatedores, 2 = avaliação do juiz.
    feito: Option<(usize, usize, usize)>,
}

impl Record {
    pub fn resumable(&self) -> bool {
        self.veredito.is_none()
    }

    /// Registros antigos não guardam `feito`: deduz o último passo concluído pela transcrição.
    fn infer_feito(&self) -> Option<(usize, usize, usize)> {
        let n = self.enquadramento.as_ref()?.eixos.len();
        self.transcricao.iter().rev().find_map(|e| {
            let side = [Side::Pro, Side::Con].into_iter().find(|s| s.name() == e.quem);
            if e.titulo == tr!("considerações finais", "closing statement") {
                return Some((n, 0, side? as usize));
            }
            // "eixo 2: réplica 3", "eixo 2: avaliação da rodada 1"; perguntas da plateia e o enquadramento não contam
            let rest = e.titulo.strip_prefix(tr!("eixo ", "issue "))?;
            let nums: Vec<usize> = rest.split(|c: char| !c.is_ascii_digit()).filter_map(|x| x.parse().ok()).collect();
            let (i, round) = (nums.first()?.checked_sub(1)?, nums.get(1).copied().unwrap_or(1));
            Some(match side {
                Some(s) => (i, round, (s != if i % 2 == 0 { Side::Pro } else { Side::Con }) as usize),
                None if e.avaliacao.as_ref().is_some_and(Check::closes) => (i, CLOSED, 2),
                None => (i, round, 2),
            })
        })
    }

    /// Para um agente sem sessão salva: o histórico do debate, que ele não lembra.
    fn recap(&self, side: Side) -> String {
        let mut s = tr!(
            "RETOMADA: este debate foi interrompido e continua agora, mas você não tem memória do que já aconteceu. Eis o histórico:",
            "RESUMING: this debate was interrupted and continues now, but you have no memory of what already happened. Here is the record:"
        )
        .to_string();
        for (d_side, d) in [Side::Pro, Side::Con].into_iter().zip(&self.dossies) {
            if let Some(d) = d
                && (side == Side::Judge || side == d_side)
            {
                s += &format!("\n\n{}", dossie_text(d_side, d));
            }
        }
        for e in &self.transcricao {
            s += &format!("\n\n{} ({}):\n{}", e.quem, e.titulo, e.texto);
        }
        s
    }
}

/// Debates salvos (caminho sem extensão + registro), do mais novo ao mais antigo.
pub fn list() -> Vec<(PathBuf, Record)> {
    let mut v: Vec<(PathBuf, Record)> = std::fs::read_dir(data_dir().join("debates"))
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .filter_map(|p| Some((p.with_extension(""), serde_json::from_str(&std::fs::read_to_string(&p).ok()?).ok()?)))
        .collect();
    v.sort_by(|a, b| b.0.cmp(&a.0));
    v
}

/// Mensagens que reconstroem na UI tudo o que já aconteceu no debate.
pub fn replay(path: &std::path::Path, rec: &Record) -> Vec<Msg> {
    let mut v = vec![Msg::Saved(path.with_extension("md"))];
    if let Some(f) = &rec.enquadramento {
        v.push(Msg::Framing(f.clone()));
    }
    for (side, d) in [Side::Pro, Side::Con].into_iter().zip(&rec.dossies) {
        if let Some(d) = d {
            v.push(Msg::Dossie(side, d.clone()));
        }
    }
    // a primeira entrada é o enquadramento, já mostrado acima
    for e in rec.transcricao.iter().skip(rec.enquadramento.is_some() as usize) {
        match ([Side::Pro, Side::Con].into_iter().find(|s| s.name() == e.quem), &e.avaliacao) {
            (Some(s), _) => {
                v.push(Msg::Start(s, e.titulo.clone()));
                v.push(Msg::Said(s, e.texto.clone()));
            }
            (None, Some(c)) => v.push(Msg::Check(c.clone())),
            (None, None) => v.push(Msg::Judge(e.texto.clone())),
        }
    }
    // ao retomar, o próprio fluxo anuncia o eixo atual
    let n = rec.enquadramento.as_ref().map_or(0, |f| f.eixos.len());
    if let (Some(f), false, true) = (rec.feito, rec.resumable(), n > 0) {
        v.push(Msg::Eixo(f.0.min(n - 1)));
    }
    if let Some(verdict) = &rec.veredito {
        v.push(Msg::Verdict(verdict.clone()));
    }
    v
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
    let rec = Record { tema: topic, idioma: tr!("pt", "en").into(), ..Default::default() };
    resume(path, rec, tx, rx).await
}

/// Continua um debate salvo (ou começa um novo, com `rec` vazio) a partir do último passo concluído.
// ponytail: depende das sessões do claude, que ele apaga após cleanupPeriodDays (30 dias por padrão); debates mais velhos falham ao retomar.
pub async fn resume(path: PathBuf, mut rec: Record, tx: UnboundedSender<Msg>, rx: UnboundedReceiver<Cmd>) {
    if rec.feito.is_none() {
        rec.feito = rec.infer_feito();
    }
    // sem sessão salva o agente começa do zero; o histórico já cobre o que estava pendente
    if !rec.transcricao.is_empty() {
        for side in [Side::Pro, Side::Con, Side::Judge] {
            if rec.sessoes[side as usize].is_none() {
                rec.pendentes[side as usize] = vec![rec.recap(side)];
            }
        }
    }
    for m in replay(&path, &rec) {
        let _ = tx.send(m);
    }
    let mut agents = [
        Agent::new(DEBATER_MODEL, debater_system(Side::Pro, &rec.tema)),
        Agent::new(DEBATER_MODEL, debater_system(Side::Con, &rec.tema)),
        Agent::new(JUDGE_MODEL, tr!(JUDGE_SYSTEM, JUDGE_SYSTEM_EN).into()),
    ];
    for (a, s) in agents.iter_mut().zip(&rec.sessoes) {
        a.session = s.clone();
    }
    let mut d = Debate { agents, tx, rx, manual: false, step: false, rec, path };
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

fn dossie_text(side: Side, d: &Dossie) -> String {
    let fontes: Vec<String> = d.fontes.iter().map(|s| format!("[{}] {} — {} — «{}»", s.id, s.titulo, s.url, s.trecho)).collect();
    trf!("DOSSIÊ DO {}\nTese: {}\n{}", "{} DOSSIER\nThesis: {}\n{}", side.name(), d.tese, fontes.join("\n"))
}

fn check_text(c: &Check) -> String {
    let mut s = c.intervencao.clone().unwrap_or_default();
    for i in &c.infracoes {
        s += &format!("\n⚠ {} — {}: {} («{}»)", if i.lado == "pro" { Side::Pro } else { Side::Con }.name(), i.tipo, i.explicacao, i.trecho);
    }
    for v in &c.fontes_verificadas {
        s += &format!("\n{} [{}] {}", if v.ok { "✓" } else { "✗" }, v.id, v.nota);
    }
    if let Some(st) = c.status() {
        s += &format!("\n{st}");
    }
    s.trim().to_string()
}

impl Debate {
    fn send(&self, m: Msg) {
        let _ = self.tx.send(m);
    }

    fn log(&mut self, quem: &str, titulo: &str, texto: &str) {
        self.rec.transcricao.push(Entry { quem: quem.into(), titulo: titulo.into(), texto: texto.into(), avaliacao: None });
        self.save();
    }

    async fn ask(&mut self, side: Side, prompt: &str, schema: Option<&str>) -> Result<crate::agent::Reply, String> {
        ask_with(&mut self.agents[side as usize], side, &self.tx, prompt, schema).await
    }

    /// Prompt de um agente = tudo que aconteceu desde o último turno dele + a instrução atual.
    fn prompt(&mut self, side: Side, instr: &str) -> String {
        let mut p = std::mem::take(&mut self.rec.pendentes[side as usize]).join("\n\n");
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
        self.rec.pendentes[side.other() as usize].push(news.clone());
        self.rec.pendentes[Side::Judge as usize].push(news);
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
                self.send(Msg::Judge(format!("? «{q}»\n{}", a.resposta)));
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
        let f = match self.rec.enquadramento.clone() {
            Some(f) => f,
            None => {
                self.send(Msg::Phase(tr!("O juiz está enquadrando o tema", "The judge is framing the topic").into()));
                let f: Framing = self.ask_json(Side::Judge, &trf!("Tema do debate: «{topic}». Enquadre o debate de forma concisa.", "Debate motion: «{topic}». Frame the debate concisely."), FRAMING_SCHEMA).await?;
                self.send(Msg::Framing(f.clone()));
                self.rec.enquadramento = Some(f.clone());
                self.log(Side::Judge.name(), tr!("Enquadramento", "Framing"), &framing_text(&f));
                self.checkpoint().await?;
                f
            }
        };

        let need = self.rec.dossies.each_ref().map(Option::is_none);
        if need[0] || need[1] {
            self.research(&f, need).await?;
        }

        // ao retomar, pula os passos já feitos
        let feito = self.rec.feito;
        let todo = |p: (usize, usize, usize)| feito.is_none_or(|d| p > d);
        // a pauta pode mudar a cada eixo encerrado, então relê a cada volta
        let len = |r: &Record| r.enquadramento.as_ref().map_or(0, |f| f.eixos.len());
        for i in 0.. {
            let Some(eixo) = self.rec.enquadramento.as_ref().and_then(|f| f.eixos.get(i)).cloned() else { break };
            let n = len(&self.rec);
            if !todo((i, CLOSED, 2)) {
                continue;
            }
            self.send(Msg::Eixo(i));
            self.send(Msg::Phase(trf!("Eixo {}/{n}", "Issue {}/{n}", i + 1)));
            let first = if i % 2 == 0 { Side::Pro } else { Side::Con };
            for round in 1..=MAX_ROUNDS {
                for (k, side) in [first, first.other()].into_iter().enumerate() {
                    if !todo((i, round, k)) {
                        continue;
                    }
                    let (title, instr) = if round == 1 {
                        let kind = if side == first { tr!("abertura", "opening") } else { tr!("resposta", "response") };
                        (trf!("eixo {}: {kind}", "issue {}: {kind}", i + 1), trf!("EIXO {}/{n}: «{}» — {}\nFaça sua fala de {kind} neste eixo.", "ISSUE {}/{n}: «{}» — {}\nGive your {kind} speech on this issue.", i + 1, eixo.titulo, eixo.pergunta))
                    } else {
                        (trf!("eixo {}: réplica {round}", "issue {}: rebuttal {round}", i + 1), trf!("Sua réplica (rodada {round}) no eixo «{}».", "Your rebuttal (round {round}) on the issue «{}».", eixo.titulo))
                    };
                    self.speak(side, &title, &instr).await?;
                    self.mark((i, round, k));
                    self.checkpoint().await?;
                }
                if !todo((i, round, 2)) {
                    continue;
                }
                let last = round == MAX_ROUNDS;
                let when = if round < MIN_ROUNDS {
                    trf!(" Antes da rodada {MIN_ROUNDS} o eixo não pode ser encerrado: estado_eixo = em_aberto.", " Before round {MIN_ROUNDS} the issue cannot be closed: estado_eixo = em_aberto.")
                } else if last {
                    tr!(" Esta é a última rodada permitida: o eixo será encerrado de qualquer forma, em consenso ou impasse.", " This is the last round allowed: the issue will be closed either way, in consensus or impasse.").into()
                } else {
                    String::new()
                };
                let rest = self.rec.enquadramento.as_ref().map(|f| f.eixos[i + 1..].iter().map(|e| e.titulo.clone()).collect::<Vec<_>>().join(" · ")).unwrap_or_default();
                let instr = trf!(
                    "Fim da rodada {round} do eixo «{}». Avalie as falas desta rodada.{when}\n\
estado_eixo = consenso, impasse ou em_aberto. Se em_aberto, ponto_em_disputa = a questão concreta que a próxima réplica deve enfrentar; senão null.\n\
Pauta restante: {}. Se o eixo for encerrado, proximos_eixos = a lista completa dos eixos que virão depois dele (retire os desnecessários, \
inclua os mais importantes que surgiram; lista vazia leva às considerações finais), ou null para manter a pauta.",
                    "End of round {round} of the issue «{}». Evaluate this round's speeches.{when}\n\
estado_eixo = consenso, impasse or em_aberto. If em_aberto, ponto_em_disputa = the concrete question the next rebuttal must address; otherwise null.\n\
Remaining agenda: {}. If the issue is closed, proximos_eixos = the full list of issues to come after it (drop the unnecessary ones, \
add the more important ones that came up; an empty list goes to closing statements), or null to keep the agenda.",
                    eixo.titulo,
                    if rest.is_empty() { tr!("nenhum", "none").to_string() } else { rest }
                );
                let mut c: Check = self.ask_json(Side::Judge, &instr, CHECK_SCHEMA).await?;
                // o mínimo e o teto de rodadas valem mesmo se o juiz discordar
                let close = last || (round >= MIN_ROUNDS && c.closes());
                if close {
                    c.ponto_em_disputa = None;
                    if !c.closes() {
                        c.estado_eixo = "impasse".into();
                    }
                } else {
                    c.estado_eixo = "em_aberto".into();
                    c.proximos_eixos = None;
                }
                for inf in &c.infracoes {
                    let side = if inf.lado == "pro" { Side::Pro } else { Side::Con };
                    self.rec.pendentes[side as usize].push(trf!("O JUIZ apontou uma infração sua: {} — {} («{}»)", "The JUDGE flagged an infraction of yours: {} — {} («{}»)", inf.tipo, inf.explicacao, inf.trecho));
                }
                for v in c.fontes_verificadas.iter().filter(|v| !v.ok) {
                    let side = if v.id.starts_with('P') { Side::Pro } else { Side::Con };
                    self.rec.pendentes[side as usize].push(trf!("O JUIZ verificou sua fonte [{}] e ela NÃO confere: {}", "The JUDGE checked your source [{}] and it does NOT hold up: {}", v.id, v.nota));
                }
                if let Some(t) = &c.intervencao {
                    for side in [Side::Pro, Side::Con] {
                        self.rec.pendentes[side as usize].push(trf!("O JUIZ disse: {t}", "The JUDGE said: {t}"));
                    }
                }
                if let Some(p) = &c.ponto_em_disputa {
                    for side in [Side::Pro, Side::Con] {
                        self.rec.pendentes[side as usize].push(trf!("O JUIZ indicou o ponto em disputa, que sua réplica deve enfrentar: {p}", "The JUDGE named the point in dispute, which your rebuttal must address: {p}"));
                    }
                }
                let titulo = trf!("eixo {}: avaliação da rodada {round}", "issue {}: round {round} assessment", i + 1);
                self.rec.transcricao.push(Entry { quem: Side::Judge.name().into(), titulo, texto: check_text(&c), avaliacao: Some(c.clone()) });
                self.send(Msg::Check(c.clone()));
                if let (true, Some(mut next)) = (close, c.proximos_eixos) {
                    next.truncate(MAX_EIXOS.saturating_sub(i + 1));
                    let titles = |v: &[Eixo]| v.iter().map(|e| e.titulo.clone()).collect::<Vec<_>>();
                    if let Some(f) = self.rec.enquadramento.as_mut()
                        && titles(&f.eixos[i + 1..]) != titles(&next)
                    {
                        f.eixos.truncate(i + 1);
                        f.eixos.extend(next);
                        let eixos = f.eixos.clone();
                        let list: Vec<String> = eixos.iter().enumerate().map(|(k, e)| format!("{}. {} — {}", k + 1, e.titulo, e.pergunta)).collect();
                        self.log(Side::Judge.name(), tr!("pauta revisada", "revised agenda"), &list.join("\n"));
                        self.send(Msg::Pauta(eixos));
                    }
                }
                // eixo encerrado conta como todas as rodadas feitas
                self.mark((i, if close { CLOSED } else { round }, 2));
                self.checkpoint().await?;
                if close {
                    break;
                }
            }
        }

        let n = len(&self.rec);
        self.send(Msg::Phase(tr!("Considerações finais", "Closing statements").into()));
        for (k, side) in [Side::Pro, Side::Con].into_iter().enumerate() {
            if !todo((n, 0, k)) {
                continue;
            }
            self.speak(side, tr!("considerações finais", "closing statement"), tr!("CONSIDERAÇÕES FINAIS: resuma por que sua posição saiu mais forte, em até ~150 palavras.", "CLOSING STATEMENT: sum up why your position came out stronger, in up to ~150 words.")).await?;
            self.mark((n, 0, k));
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

    /// Fase de pesquisa, só para os lados que ainda não têm dossiê.
    async fn research(&mut self, f: &Framing, need: [bool; 2]) -> Result<(), String> {
        self.send(Msg::Phase(tr!("Pesquisa", "Research").into()));
        let research = |id: char| {
            trf!(
                "Enquadramento do juiz:\n{}\n\nFASE DE PESQUISA: use WebSearch e WebFetch para montar seu dossiê. Busque fontes confiáveis \
(dados oficiais, estudos, imprensa de qualidade) e antecipe o que o outro lado vai usar. Faça no máximo ~8 buscas. Retorne de 5 a 10 \
fontes com id {id}1, {id}2..., a URL real que você consultou, o título e um trecho ou dado concreto; e seus principais argumentos.",
                "Judge's framing:\n{}\n\nRESEARCH PHASE: use WebSearch and WebFetch to build your dossier. Look for reliable sources \
(official data, studies, quality press) and anticipate what the other side will use. Do at most ~8 searches. Return 5 to 10 \
sources with ids {id}1, {id}2..., the real URL you consulted, the title and a concrete excerpt or data point; and your main arguments.",
                framing_text(f)
            )
        };
        let (pp, pc) = (research('P'), research('C'));
        for side in [Side::Pro, Side::Con].into_iter().filter(|s| need[*s as usize]) {
            self.send(Msg::Busy(side));
        }
        let [pro, con, _] = &mut self.agents;
        let tx = &self.tx;
        let (rp, rc) = tokio::join!(
            async { if need[0] { Some(ask_with(pro, Side::Pro, tx, &pp, Some(DOSSIE_SCHEMA)).await) } else { None } },
            async { if need[1] { Some(ask_with(con, Side::Con, tx, &pc, Some(DOSSIE_SCHEMA)).await) } else { None } }
        );
        // guarda o dossiê de um lado mesmo se o outro falhar: pesquisa é cara
        let mut err = None;
        for (side, r) in [(Side::Pro, rp), (Side::Con, rc)] {
            let Some(r) = r else { continue };
            match r.and_then(|r| json::<Dossie>(side, r)) {
                Ok(d) => {
                    self.rec.pendentes[Side::Judge as usize].push(dossie_text(side, &d));
                    self.send(Msg::Dossie(side, d.clone()));
                    self.rec.dossies[side as usize] = Some(d);
                }
                Err(e) => err = err.or(Some(e)),
            }
        }
        self.save();
        if let Some(e) = err {
            return Err(e);
        }
        self.checkpoint().await
    }

    fn mark(&mut self, p: (usize, usize, usize)) {
        self.rec.feito = Some(p);
        self.save();
    }

    fn save(&mut self) {
        self.rec.sessoes = self.agents.each_ref().map(|a| a.session.clone());
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

#[cfg(test)]
mod tests {
    use super::{CLOSED, Record, Side};

    #[test]
    fn old_records_resume_where_they_stopped() {
        let entry = |quem: &str, titulo: &str| format!(r#"{{"quem":"{quem}","titulo":"{titulo}","texto":"y"}}"#);
        let old = |entries: &[String]| -> Record {
            let json = format!(r#"{{"tema":"x","enquadramento":{{"definicoes":"","onus_da_prova":"","eixos":[{{"titulo":"a","pergunta":"?"}},{{"titulo":"b","pergunta":"?"}}],"regras":""}},"transcricao":[{}]}}"#, entries.join(","));
            serde_json::from_str(&json).unwrap()
        };
        let framing = entry("JUIZ", "Enquadramento");
        assert_eq!(old(&[framing.clone()]).infer_feito(), None);
        let r = old(&[framing.clone(), entry("PRÓ", "eixo 1: abertura"), entry("CONTRA", "eixo 1: resposta"), entry("JUIZ", "eixo 1: avaliação da rodada 1"), entry("CONTRA", "eixo 1: réplica 2")]);
        assert!(r.resumable());
        assert_eq!(r.infer_feito(), Some((0, 2, 1)));
        assert!(r.recap(Side::Pro).contains("CONTRA (eixo 1: réplica 2):"));
        // eixo 2 começa pelo CONTRA; perguntas da plateia não contam
        let r = old(&[framing.clone(), entry("CONTRA", "eixo 2: abertura"), entry("JUIZ", "pergunta da plateia: 3 rodadas?")]);
        assert_eq!(r.infer_feito(), Some((1, 1, 0)));
        assert_eq!(old(&[framing.clone(), entry("CONTRA", "considerações finais")]).infer_feito(), Some((2, 0, 1)));
        // eixo encerrado, no formato antigo (encerrar_eixo) e no atual (estado_eixo)
        let check = |extra: &str| format!(r#"{{"quem":"JUIZ","titulo":"eixo 1: avaliação da rodada 2","texto":"y","avaliacao":{{"intervencao":null,"infracoes":[],"fontes_verificadas":[],{extra}}}}}"#);
        assert_eq!(old(&[framing.clone(), check(r#""encerrar_eixo":true"#)]).infer_feito(), Some((0, CLOSED, 2)));
        assert_eq!(old(&[framing.clone(), check(r#""estado_eixo":"impasse""#)]).infer_feito(), Some((0, CLOSED, 2)));
        assert_eq!(old(&[framing, check(r#""estado_eixo":"em_aberto","ponto_em_disputa":"z""#)]).infer_feito(), Some((0, 2, 2)));
        let new: Record = serde_json::from_str(r#"{"tema":"x","sessoes":[null,null,"s"],"feito":[1,2,0]}"#).unwrap();
        assert!(new.resumable());
        assert_eq!(new.feito, Some((1, 2, 0)));
    }
}
