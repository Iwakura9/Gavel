mod agent;
mod debate;
mod sprites;
mod ui;

use debate::{Cmd, Dossie, Eixo, Msg, Side, Verdict};
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use sprites::Pose;
use std::collections::HashMap;
use std::io::Write;
use std::path::PathBuf;
use std::time::Duration;
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};

#[derive(PartialEq, Clone, Copy)]
pub enum Mode {
    Topic,
    Normal,
    Ask(Side),
    Sources,
    Verdict,
}

pub struct App {
    pub mode: Mode,
    pub input: String,
    pub topic: String,
    pub tick: u64,
    pub phase: String,
    pub eixos: Vec<Eixo>,
    pub eixo: Option<usize>,
    pub speech: [Vec<(String, String)>; 2],
    pub tool: [String; 3],
    pub pose: [Pose; 3],
    pub balloon: String,
    judge_fresh: bool,
    pub objection_until: u64,
    pub talk_until: u64,
    pub infr: [u32; 2],
    pub usage: Option<f64>,
    pub dossies: [Option<Dossie>; 2],
    pub checks: HashMap<String, (bool, String)>,
    pub verdict: Option<Verdict>,
    pub paused: bool,
    pub focus: usize,
    pub back: [u16; 2],
    pub popup_scroll: u16,
    pub saved: Option<PathBuf>,
    pub error: bool,
}

impl App {
    fn new(topic: String) -> Self {
        App {
            mode: if topic.is_empty() { Mode::Topic } else { Mode::Normal },
            input: String::new(),
            topic,
            tick: 0,
            phase: "Preparando".into(),
            eixos: vec![],
            eixo: None,
            speech: Default::default(),
            tool: Default::default(),
            pose: [Pose::Idle; 3],
            balloon: "Ordem no tribunal! O debate vai começar.".into(),
            judge_fresh: false,
            objection_until: 0,
            talk_until: 0,
            infr: [0; 2],
            usage: None,
            dossies: Default::default(),
            checks: HashMap::new(),
            verdict: None,
            paused: false,
            focus: 0,
            back: [0; 2],
            popup_scroll: 0,
            saved: None,
            error: false,
        }
    }

    pub fn judge_pose(&self) -> Pose {
        if self.objection_until > self.tick {
            Pose::Objection
        } else if self.pose[2] == Pose::Idle && self.talk_until > self.tick {
            Pose::Talking
        } else {
            self.pose[2]
        }
    }

    fn on_msg(&mut self, m: Msg) {
        match m {
            Msg::Phase(p) => self.phase = p,
            Msg::Framing(f) => {
                let list: Vec<String> = f.eixos.iter().enumerate().map(|(i, e)| format!("{}) {}", i + 1, e.titulo)).collect();
                self.balloon = format!("Tema enquadrado! Eixos: {}", list.join(" · "));
                self.eixos = f.eixos;
                self.say_judge();
            }
            Msg::Eixo(i) => {
                self.eixo = Some(i);
                let e = &self.eixos[i];
                // anexa: não apaga a objeção/avaliação que acabou de sair
                self.balloon = format!("{}\n\n▶ Eixo {}/{}: {} — {}", self.balloon, i + 1, self.eixos.len(), e.titulo, e.pergunta);
                self.say_judge();
            }
            Msg::Busy(s) => {
                self.pose[s as usize] = Pose::Thinking;
                self.tool[s as usize].clear();
                self.judge_fresh |= s == Side::Judge;
            }
            Msg::Start(s, title) => {
                self.pose[s as usize] = Pose::Thinking;
                self.tool[s as usize].clear();
                self.speech[s as usize].push((title, String::new()));
            }
            Msg::Token(Side::Judge, t) => {
                if std::mem::take(&mut self.judge_fresh) {
                    self.balloon.clear();
                }
                self.balloon += &t;
                self.talk_until = self.tick + 6;
            }
            Msg::Token(s, t) => {
                self.pose[s as usize] = Pose::Talking;
                if let Some(last) = self.speech[s as usize].last_mut() {
                    last.1 += &t;
                }
            }
            Msg::Tool(s, t) => {
                self.pose[s as usize] = Pose::Searching;
                self.tool[s as usize] = t;
            }
            Msg::Said(s, text) => {
                self.pose[s as usize] = Pose::Idle;
                self.tool[s as usize].clear();
                if let Some(last) = self.speech[s as usize].last_mut() {
                    last.1 = text;
                }
            }
            Msg::Judge(text) => {
                self.balloon = text;
                self.say_judge();
            }
            Msg::Check(c) => {
                for i in &c.infracoes {
                    self.infr[if i.lado == "pro" { 0 } else { 1 }] += 1;
                }
                for v in &c.fontes_verificadas {
                    self.checks.insert(v.id.clone(), (v.ok, v.nota.clone()));
                }
                let objection = !c.infracoes.is_empty() || c.fontes_verificadas.iter().any(|v| !v.ok);
                let mut text = c.intervencao.clone().unwrap_or_default();
                for i in &c.infracoes {
                    text += &format!("\n⚠ {}: {} — {}", if i.lado == "pro" { "PRÓ" } else { "CONTRA" }, i.tipo, i.explicacao);
                }
                for v in c.fontes_verificadas.iter().filter(|v| !v.ok) {
                    text += &format!("\n✗ [{}] {}", v.id, v.nota);
                }
                let text = text.trim().to_string();
                self.balloon = if text.is_empty() { "✓ Rodada limpa. Prossigam.".into() } else { text };
                self.say_judge();
                if objection {
                    self.objection_until = self.tick + 20;
                }
            }
            Msg::Dossie(s, d) => {
                self.pose[s as usize] = Pose::Idle;
                self.tool[s as usize].clear();
                self.speech[s as usize].push(("dossiê".into(), format!("{}\n({} fontes · tecla f)", d.tese, d.fontes.len())));
                self.dossies[s as usize] = Some(d);
            }
            Msg::Usage(u) => self.usage = Some(u),
            Msg::Verdict(v) => {
                let (p, c) = match v.vencedor.as_str() {
                    "pro" => (Pose::Win, Pose::Lose),
                    "contra" => (Pose::Lose, Pose::Win),
                    _ => (Pose::Idle, Pose::Idle),
                };
                self.pose = [p, c, Pose::Idle];
                self.balloon = format!("Veredito: {}! (tecla v)", if v.vencedor == "empate" { "EMPATE".into() } else { format!("vence o {}", if p == Pose::Win { "PRÓ" } else { "CONTRA" }) });
                self.objection_until = self.tick + 12;
                self.verdict = Some(v);
                self.mode = Mode::Verdict;
                self.popup_scroll = 0;
            }
            Msg::Error(e) => {
                self.error = true;
                self.pose = [Pose::Idle; 3];
                self.balloon = format!("⚠ {e}");
            }
            Msg::Saved(p) => self.saved = Some(p),
        }
    }

    fn say_judge(&mut self) {
        self.pose[2] = Pose::Idle;
        self.tool[2].clear();
        self.talk_until = self.tick + 14;
    }

    /// Retorna false para sair.
    fn on_key(&mut self, code: KeyCode, ctrl: bool, cmd: &Option<UnboundedSender<Cmd>>) -> bool {
        if ctrl && code == KeyCode::Char('c') {
            return false;
        }
        let send = |c: Cmd| {
            if let Some(tx) = cmd {
                let _ = tx.send(c);
            }
        };
        match self.mode {
            Mode::Topic | Mode::Ask(_) => match code {
                KeyCode::Char(c) => self.input.push(c),
                KeyCode::Backspace => {
                    self.input.pop();
                }
                KeyCode::Esc if self.mode == Mode::Topic => return false,
                KeyCode::Esc => self.mode = Mode::Normal,
                KeyCode::Enter if !self.input.trim().is_empty() => {
                    let text = std::mem::take(&mut self.input).trim().to_string();
                    if let Mode::Ask(side) = self.mode {
                        self.tool[side as usize] = format!("pergunta na fila: {text}");
                        send(Cmd::Ask(side, text));
                    } else {
                        self.topic = text;
                    }
                    self.mode = Mode::Normal;
                }
                _ => {}
            },
            Mode::Sources | Mode::Verdict => match code {
                KeyCode::Up => self.popup_scroll = self.popup_scroll.saturating_sub(1),
                KeyCode::Down => self.popup_scroll += 1,
                KeyCode::PageUp => self.popup_scroll = self.popup_scroll.saturating_sub(10),
                KeyCode::PageDown => self.popup_scroll += 10,
                KeyCode::Char('q') => return false,
                _ => self.mode = Mode::Normal,
            },
            Mode::Normal => match code {
                KeyCode::Char('q') => return false,
                KeyCode::Char(' ') => {
                    self.paused = !self.paused;
                    send(Cmd::Pause(self.paused));
                }
                KeyCode::Char('p') => self.mode = Mode::Ask(Side::Pro),
                KeyCode::Char('c') => self.mode = Mode::Ask(Side::Con),
                KeyCode::Char('j') => self.mode = Mode::Ask(Side::Judge),
                KeyCode::Char('f') => (self.mode, self.popup_scroll) = (Mode::Sources, 0),
                KeyCode::Char('v') if self.verdict.is_some() => (self.mode, self.popup_scroll) = (Mode::Verdict, 0),
                KeyCode::Tab | KeyCode::Left | KeyCode::Right => self.focus = 1 - self.focus,
                KeyCode::Up => self.back[self.focus] += 1,
                KeyCode::Down => self.back[self.focus] = self.back[self.focus].saturating_sub(1),
                KeyCode::PageUp => self.back[self.focus] += 10,
                KeyCode::PageDown => self.back[self.focus] = self.back[self.focus].saturating_sub(10),
                KeyCode::End => self.back[self.focus] = 0,
                _ => {}
            },
        }
        true
    }
}

/// Modo sem TUI: imprime o debate no stdout (útil para depurar prompts).
async fn plain(topic: String) {
    let (tx, mut rx) = unbounded_channel();
    let (_cmd_tx, cmd_rx) = unbounded_channel();
    tokio::spawn(debate::run(topic, tx, cmd_rx));
    let mut out = std::io::stdout();
    while let Some(m) = rx.recv().await {
        match m {
            Msg::Phase(p) => println!("\n===== {p} ====="),
            Msg::Framing(f) => println!("[JUIZ] eixos: {:?}", f.eixos.iter().map(|e| &e.titulo).collect::<Vec<_>>()),
            Msg::Eixo(i) => println!("\n--- eixo {} ---", i + 1),
            Msg::Busy(s) => println!("[{}] pensando…", s.name()),
            Msg::Start(s, t) => print!("\n[{} · {t}] ", s.name()),
            Msg::Token(_, t) => print!("{t}"),
            Msg::Tool(s, t) => println!("  [{}] {t}", s.name()),
            Msg::Said(..) => println!(),
            Msg::Judge(t) => println!("[JUIZ] {t}"),
            Msg::Check(c) => println!("[JUIZ] {c:?}"),
            Msg::Dossie(s, d) => println!("[{}] dossiê: {} ({} fontes)", s.name(), d.tese, d.fontes.len()),
            Msg::Usage(u) => println!("  (uso 5h: {:.0}%)", u * 100.0),
            Msg::Verdict(v) => println!("\n[VEREDITO] {v:#?}"),
            Msg::Error(e) => println!("\n[ERRO] {e}"),
            Msg::Saved(p) => println!("salvando em {}", p.display()),
        }
        let _ = out.flush();
    }
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let is_plain = args.iter().any(|a| a == "--plain");
    let topic = args.iter().filter(|a| *a != "--plain").cloned().collect::<Vec<_>>().join(" ");
    if is_plain {
        plain(topic).await;
        return Ok(());
    }

    let mut app = App::new(topic);
    let (key_tx, mut key_rx) = unbounded_channel();
    std::thread::spawn(move || {
        while let Ok(ev) = event::read() {
            if key_tx.send(ev).is_err() {
                break;
            }
        }
    });
    let (msg_tx, mut msg_rx) = unbounded_channel();
    let mut cmd_tx: Option<UnboundedSender<Cmd>> = None;
    let mut tick = tokio::time::interval(Duration::from_millis(150));

    let mut terminal = ratatui::init();
    loop {
        if cmd_tx.is_none() && !app.topic.is_empty() {
            let (tx, rx) = unbounded_channel();
            tokio::spawn(debate::run(app.topic.clone(), msg_tx.clone(), rx));
            cmd_tx = Some(tx);
        }
        terminal.draw(|f| ui::draw(f, &app))?;
        tokio::select! {
            Some(ev) = key_rx.recv() => {
                if let Event::Key(k) = ev
                    && k.kind == KeyEventKind::Press
                    && !app.on_key(k.code, k.modifiers.contains(KeyModifiers::CONTROL), &cmd_tx)
                {
                    break;
                }
            }
            Some(m) = msg_rx.recv() => app.on_msg(m),
            _ = tick.tick() => app.tick += 1,
        }
    }
    ratatui::restore();
    if let Some(p) = &app.saved {
        println!("Debate salvo em {}", p.display());
    }
    Ok(())
}
