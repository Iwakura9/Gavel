/// Interface language, picked once at startup with `--lang`.
pub static EN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn en() -> bool {
    EN.load(std::sync::atomic::Ordering::Relaxed)
}

/// `tr!("texto", "text")`: picks the string for the current language.
macro_rules! tr {
    ($pt:expr, $en:expr) => {
        if crate::en() { $en } else { $pt }
    };
}

/// `trf!("{n} fontes", "{n} sources")`: like `format!`, one format string per language.
macro_rules! trf {
    ($pt:literal, $en:literal $($args:tt)*) => {
        if crate::en() { format!($en $($args)*) } else { format!($pt $($args)*) }
    };
}

mod agent;
mod debate;
mod sprites;
mod ui;

use debate::{Cmd, Dossie, Eixo, Msg, Record, Side, Verdict};
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use sprites::Pose;
use std::cell::Cell;
use std::collections::HashMap;
use std::io::Write;
use std::path::PathBuf;
use std::time::Duration;
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};

#[derive(PartialEq, Clone, Copy)]
pub enum Mode {
    Topic,
    Normal,
    Ask,
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
    pub manual: bool,
    pub waiting: bool,
    pub focus: usize,
    pub back: [u16; 2],
    pub popup_scroll: u16,
    /// Rolagem máxima do popup aberto, calculada no último desenho.
    pub popup_max: Cell<u16>,
    pub saved: Option<PathBuf>,
    pub error: bool,
    /// Tela inicial: debates salvos e o selecionado (None = campo de tema).
    pub saved_list: Vec<(PathBuf, Record)>,
    pub sel: Option<usize>,
    /// Primeiro Del apertado: o segundo apaga o debate selecionado.
    pub confirm_del: bool,
    open: Option<(PathBuf, Record)>,
    pub readonly: bool,
}

impl App {
    fn new(topic: String) -> Self {
        App {
            mode: if topic.is_empty() { Mode::Topic } else { Mode::Normal },
            saved_list: if topic.is_empty() { debate::list() } else { vec![] },
            sel: None,
            confirm_del: false,
            open: None,
            readonly: false,
            input: String::new(),
            topic,
            tick: 0,
            phase: tr!("Preparando", "Preparing").into(),
            eixos: vec![],
            eixo: None,
            speech: Default::default(),
            tool: Default::default(),
            pose: [Pose::Idle; 3],
            balloon: tr!("Ordem no tribunal! O debate vai começar.", "Order in the court! The debate is about to begin.").into(),
            judge_fresh: false,
            objection_until: 0,
            talk_until: 0,
            infr: [0; 2],
            usage: None,
            dossies: Default::default(),
            checks: HashMap::new(),
            verdict: None,
            manual: false,
            waiting: false,
            focus: 0,
            back: [0; 2],
            popup_scroll: 0,
            popup_max: Cell::new(0),
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
                self.balloon = trf!("Tema enquadrado! Eixos: {}", "Topic framed! Issues: {}", list.join(" · "));
                self.eixos = f.eixos;
                self.say_judge();
            }
            Msg::Eixo(i) => {
                self.eixo = Some(i);
                let e = &self.eixos[i];
                // anexa: não apaga a objeção/avaliação que acabou de sair
                self.balloon = trf!("{}\n\n▶ Eixo {}/{}: {} — {}", "{}\n\n▶ Issue {}/{}: {} — {}", self.balloon, i + 1, self.eixos.len(), e.titulo, e.pergunta);
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
                    text += &format!("\n⚠ {}: {} — {}", if i.lado == "pro" { Side::Pro } else { Side::Con }.name(), i.tipo, i.explicacao);
                }
                for v in c.fontes_verificadas.iter().filter(|v| !v.ok) {
                    text += &format!("\n✗ [{}] {}", v.id, v.nota);
                }
                let text = text.trim().to_string();
                self.balloon = if text.is_empty() { tr!("✓ Rodada limpa. Prossigam.", "✓ Clean round. Carry on.").into() } else { text };
                self.say_judge();
                if objection {
                    self.objection_until = self.tick + 20;
                }
            }
            Msg::Dossie(s, d) => {
                self.pose[s as usize] = Pose::Idle;
                self.tool[s as usize].clear();
                self.speech[s as usize].push((tr!("dossiê", "dossier").into(), trf!("{}\n({} fontes · tecla f)", "{}\n({} sources · key f)", d.tese, d.fontes.len())));
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
                let winner = if p == Pose::Win { Side::Pro } else { Side::Con }.name();
                let result = if v.vencedor == "empate" { tr!("EMPATE", "TIE").into() } else { trf!("vence o {winner}", "{winner} wins") };
                self.balloon = trf!("Veredito: {result}! (tecla v)", "Verdict: {result}! (key v)");
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
            Msg::Waiting => self.waiting = true,
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
        let confirm_del = std::mem::take(&mut self.confirm_del);
        let send = |c: Cmd| {
            if let Some(tx) = cmd {
                let _ = tx.send(c);
            }
        };
        match self.mode {
            Mode::Topic | Mode::Ask => match code {
                KeyCode::Down if self.mode == Mode::Topic && !self.saved_list.is_empty() => {
                    self.sel = Some(self.sel.map_or(0, |i| (i + 1).min(self.saved_list.len() - 1)));
                }
                KeyCode::Up => self.sel = self.sel.and_then(|i| i.checked_sub(1)),
                KeyCode::Esc if self.sel.is_some() => self.sel = None,
                KeyCode::Delete if self.sel.is_some() && !confirm_del => self.confirm_del = true,
                KeyCode::Delete if self.sel.is_some() => {
                    let i = self.sel.unwrap();
                    let path = &self.saved_list[i].0;
                    if std::fs::remove_file(path.with_extension("json")).is_ok() {
                        let _ = std::fs::remove_file(path.with_extension("md"));
                        self.saved_list.remove(i);
                        self.sel = self.saved_list.len().checked_sub(1).map(|last| i.min(last));
                    }
                }
                KeyCode::Enter if self.sel.is_some() => {
                    let (path, rec) = std::mem::take(&mut self.saved_list).swap_remove(self.sel.unwrap());
                    self.topic = rec.tema.clone();
                    self.open = Some((path, rec));
                    self.mode = Mode::Normal;
                }
                KeyCode::Char(c) => {
                    self.sel = None;
                    self.input.push(c);
                }
                KeyCode::Backspace => {
                    self.input.pop();
                }
                KeyCode::Esc if self.mode == Mode::Topic => return false,
                KeyCode::Esc => self.mode = Mode::Normal,
                KeyCode::Enter if !self.input.trim().is_empty() => {
                    let text = std::mem::take(&mut self.input).trim().to_string();
                    if self.mode == Mode::Ask {
                        self.tool[2] = trf!("pergunta na fila: {text}", "question queued: {text}");
                        send(Cmd::Ask(text));
                    } else {
                        self.topic = text;
                    }
                    self.mode = Mode::Normal;
                }
                _ => {}
            },
            Mode::Sources | Mode::Verdict => match code {
                KeyCode::Up => self.popup_scroll = self.popup_scroll.saturating_sub(1),
                KeyCode::Down => self.popup_scroll = (self.popup_scroll + 1).min(self.popup_max.get()),
                KeyCode::PageUp => self.popup_scroll = self.popup_scroll.saturating_sub(10),
                KeyCode::PageDown => self.popup_scroll = (self.popup_scroll + 10).min(self.popup_max.get()),
                KeyCode::Home => self.popup_scroll = 0,
                KeyCode::End => self.popup_scroll = self.popup_max.get(),
                KeyCode::Tab | KeyCode::Left | KeyCode::Right if self.mode == Mode::Sources => {
                    (self.focus, self.popup_scroll) = (1 - self.focus, 0);
                }
                KeyCode::Char('q') => return false,
                _ => self.mode = Mode::Normal,
            },
            Mode::Normal => match code {
                KeyCode::Char('q') => return false,
                KeyCode::Char(' ') if !self.readonly => {
                    self.manual = !self.manual;
                    self.waiting = false;
                    send(Cmd::Manual(self.manual));
                }
                KeyCode::Enter if self.waiting => {
                    self.waiting = false;
                    send(Cmd::Next);
                }
                KeyCode::Char('p') if !self.readonly => self.mode = Mode::Ask,
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
            Msg::Framing(f) => println!("[{}] {}: {:?}", Side::Judge.name(), tr!("eixos", "issues"), f.eixos.iter().map(|e| &e.titulo).collect::<Vec<_>>()),
            Msg::Eixo(i) => println!("\n--- {} {} ---", tr!("eixo", "issue"), i + 1),
            Msg::Busy(s) => println!("[{}] {}", s.name(), tr!("pensando…", "thinking…")),
            Msg::Start(s, t) => print!("\n[{} · {t}] ", s.name()),
            Msg::Token(_, t) => print!("{t}"),
            Msg::Tool(s, t) => println!("  [{}] {t}", s.name()),
            Msg::Said(..) => println!(),
            Msg::Judge(t) => println!("[{}] {t}", Side::Judge.name()),
            Msg::Check(c) => println!("[{}] {c:?}", Side::Judge.name()),
            Msg::Dossie(s, d) => println!("{}", trf!("[{}] dossiê: {} ({} fontes)", "[{}] dossier: {} ({} sources)", s.name(), d.tese, d.fontes.len())),
            Msg::Usage(u) => println!("{}", trf!("  (uso 5h: {:.0}%)", "  (5h usage: {:.0}%)", u * 100.0)),
            Msg::Verdict(v) => println!("\n[{}] {v:#?}", tr!("VEREDITO", "VERDICT")),
            Msg::Error(e) => println!("\n[{}] {e}", tr!("ERRO", "ERROR")),
            Msg::Saved(p) => println!("{}", trf!("salvando em {}", "saving to {}", p.display())),
            Msg::Waiting => {}
        }
        let _ = out.flush();
    }
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let (mut is_plain, mut words) = (false, vec![]);
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--plain" => is_plain = true,
            "--lang" => match args.next().as_deref() {
                Some("pt") => {}
                Some("en") => EN.store(true, std::sync::atomic::Ordering::Relaxed),
                _ => {
                    eprintln!("usage: gavel [--lang pt|en] [--plain] [topic]");
                    std::process::exit(2);
                }
            },
            _ => words.push(a),
        }
    }
    let topic = words.join(" ");
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
    let mut started = false;
    let mut tick = tokio::time::interval(Duration::from_millis(150));

    let mut terminal = ratatui::init();
    loop {
        if !started && !app.topic.is_empty() {
            started = true;
            let (tx, rx) = unbounded_channel();
            match app.open.take() {
                None => {
                    tokio::spawn(debate::run(app.topic.clone(), msg_tx.clone(), rx));
                }
                Some((path, rec)) => {
                    // o debate segue no idioma em que começou
                    EN.store(rec.idioma == "en", std::sync::atomic::Ordering::Relaxed);
                    if rec.resumable() {
                        tokio::spawn(debate::resume(path, rec, msg_tx.clone(), rx));
                    } else {
                        for m in debate::replay(&path, &rec) {
                            app.on_msg(m);
                        }
                        app.readonly = true;
                        app.phase = tr!("Somente leitura", "Read-only").into();
                    }
                }
            }
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
        println!("{}", trf!("Debate salvo em {}", "Debate saved to {}", p.display()));
    }
    Ok(())
}
