use crate::debate::Side;
use crate::sprites::{self, Pose, ROWS, W};
use crate::{App, Mode};
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint::*, Layout, Margin, Rect};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Clear, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap};
use std::collections::HashMap;

const GOLD: Color = Color::Rgb(230, 180, 60);
const GREEN: Color = Color::Rgb(76, 175, 80);
const RED: Color = Color::Rgb(229, 57, 53);
const DIM: Color = Color::Rgb(130, 130, 130);
const SW: u16 = W as u16;

fn color(i: usize) -> Color {
    [GREEN, RED, GOLD][i]
}

fn center(a: Rect, w: u16, h: u16) -> Rect {
    let (w, h) = (w.min(a.width), h.min(a.height));
    Rect::new(a.x + (a.width - w) / 2, a.y + (a.height - h) / 2, w, h)
}

fn sprite(f: &mut Frame, area: Rect, side: Side, pose: Pose, tick: u64) {
    let r = Rect { y: area.y + area.height.saturating_sub(ROWS) / 2, height: ROWS.min(area.height), ..area };
    f.render_widget(Paragraph::new(sprites::render(side, pose, tick)), r);
}

/// Destaca citações [P3]/[C1]: verde se o juiz conferiu, vermelho riscado se não confere.
fn cites(text: &str, checks: &HashMap<String, (bool, String)>) -> Line<'static> {
    let mut spans = vec![];
    let mut rest = text;
    while let Some(i) = rest.find('[') {
        let tail = &rest[i..];
        if let Some(j) = tail.find(']') {
            let id = &tail[1..j];
            if id.len() >= 2 && matches!(id.as_bytes()[0], b'P' | b'C') && id[1..].bytes().all(|b| b.is_ascii_digit()) {
                let style = match checks.get(id) {
                    Some((true, _)) => Style::new().fg(GREEN).bold(),
                    Some((false, _)) => Style::new().fg(RED).bold().crossed_out(),
                    None => Style::new().fg(Color::Cyan).bold(),
                };
                spans.push(Span::raw(rest[..i].to_string()));
                spans.push(Span::styled(tail[..=j].to_string(), style));
                rest = &tail[j + 1..];
                continue;
            }
        }
        spans.push(Span::raw(rest[..=i].to_string()));
        rest = &rest[i + 1..];
    }
    spans.push(Span::raw(rest.to_string()));
    Line::from(spans)
}

pub fn draw(f: &mut Frame, app: &App) {
    if app.mode == Mode::Topic {
        return topic_screen(f, app);
    }
    let area = f.area();
    let jh = if area.height >= 32 { 10 } else { 8 };
    let [title, judge, debaters, status] = Layout::vertical([Length(1), Length(jh), Min(6), Length(1)]).areas(area);

    // título
    let mut t = vec![" ⚖ GAVEL ".bold().fg(Color::Black).bg(GOLD), format!(" «{}» ", app.topic).bold()];
    if let Some(i) = app.eixo {
        t.push(format!(" eixo {}/{}: {}", i + 1, app.eixos.len(), app.eixos[i].titulo).fg(GOLD));
    }
    f.render_widget(Line::from(t), title);

    // juiz em cima, no meio
    let bw = area.width.saturating_sub(SW + 6).min(72);
    let [_, js, _, jb, _] = Layout::horizontal([Fill(1), Length(SW), Length(2), Length(bw), Fill(1)]).areas(judge);
    let jpose = app.judge_pose();
    sprite(f, js, Side::Judge, jpose, app.tick);
    let objection = jpose == Pose::Objection;
    let mut block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(if objection { RED } else { GOLD }))
        .title(if objection { " ⚖ OBJEÇÃO! ".bold().fg(RED) } else { " ⚖ Juiz · Opus ".bold().fg(GOLD) });
    if app.pose[2] == Pose::Thinking {
        block = block.title_bottom(" deliberando… ".italic().fg(DIM));
    } else if !app.tool[2].is_empty() {
        block = block.title_bottom(format!(" 🔎 {} ", app.tool[2]).italic().fg(DIM));
    }
    let lines: Vec<Line> = app.balloon.lines().map(|l| cites(l, &app.checks)).collect();
    let p = Paragraph::new(lines).wrap(Wrap { trim: true });
    let scroll = (p.line_count(jb.width.saturating_sub(2)) as u16).saturating_sub(jb.height.saturating_sub(2));
    f.render_widget(p.scroll((scroll, 0)).block(block), jb);

    // debatedores, um de cada lado
    let [l, r] = Layout::horizontal([Fill(1), Fill(1)]).areas(debaters);
    debater(f, app, Side::Pro, l);
    debater(f, app, Side::Con, r);

    status_bar(f, app, status);

    match app.mode {
        Mode::Ask => ask_popup(f, app),
        Mode::Sources => sources_popup(f, app),
        Mode::Verdict => verdict_popup(f, app),
        _ => {}
    }
}

fn debater(f: &mut Frame, app: &App, side: Side, area: Rect) {
    let i = side as usize;
    let c = color(i);
    let (label, align) = if side == Side::Pro { (" A FAVOR · Sonnet ", Alignment::Left) } else { (" CONTRA · Sonnet ", Alignment::Right) };
    let mut block = Block::bordered()
        .border_type(if app.focus == i { BorderType::Thick } else { BorderType::Rounded })
        .border_style(Style::new().fg(c))
        .title(Line::from(label.bold().fg(c)).alignment(align));
    if app.back[i] > 0 {
        block = block.title_bottom(Line::from(format!(" ↑{} (End volta) ", app.back[i]).fg(DIM)).alignment(align));
    }
    let inner = block.inner(area);
    f.render_widget(block, area);

    let pose = app.pose[i];
    let state = match pose {
        Pose::Thinking => "pensando…",
        Pose::Talking => "falando",
        Pose::Searching => "pesquisando",
        Pose::Win => "🏆 venceu!",
        Pose::Lose => "perdeu… mas com dignidade",
        _ => "aguardando",
    };
    let mut info = vec![Line::from(side.name().bold().fg(c)), Line::from(state.italic())];
    if !app.tool[i].is_empty() {
        info.push(Line::from(format!("🔎 {}", app.tool[i]).fg(DIM)));
    }
    let big = inner.height >= 14;
    let [head, body] = Layout::vertical([Length(if big { ROWS } else { 1 }), Min(1)]).areas(inner);
    if big {
        let (s, t) = if side == Side::Pro {
            let [s, _, t] = Layout::horizontal([Length(SW), Length(1), Min(1)]).areas(head);
            (s, t)
        } else {
            let [t, _, s] = Layout::horizontal([Min(1), Length(1), Length(SW)]).areas(head);
            (s, t)
        };
        sprite(f, s, side, pose, app.tick);
        f.render_widget(Paragraph::new(info).alignment(align).wrap(Wrap { trim: true }), t);
    } else {
        let one = Line::from(vec![format!("{} · ", side.name()).bold().fg(c), state.italic(), format!("  {}", app.tool[i]).fg(DIM)]);
        f.render_widget(one.alignment(align), head);
    }

    let mut lines = vec![];
    for (title, text) in &app.speech[i] {
        lines.push(Line::from(format!("── {title} ──").fg(c).add_modifier(Modifier::DIM | Modifier::BOLD)));
        lines.extend(text.lines().map(|l| cites(l, &app.checks)));
        lines.push(Line::raw(""));
    }
    let p = Paragraph::new(lines).wrap(Wrap { trim: false });
    let total = p.line_count(body.width) as u16;
    let scroll = total.saturating_sub(body.height).saturating_sub(app.back[i]);
    f.render_widget(p.scroll((scroll, 0)), body);
}

fn status_bar(f: &mut Frame, app: &App, area: Rect) {
    let mut s = vec![format!(" {} ", app.phase).bold()];
    if app.waiting {
        s.push(" ⏎ ENTER: próximo turno ".bold().fg(Color::Black).bg(GOLD));
    } else if app.manual {
        s.push(" ✋ MANUAL ".bold().fg(GOLD));
    }
    if !app.eixos.is_empty() {
        let dots: String = (0..app.eixos.len())
            .map(|k| match app.eixo {
                Some(e) if k < e => '●',
                Some(e) if k == e => '◉',
                _ => '○',
            })
            .collect();
        s.push(format!(" {dots} ").fg(GOLD));
    }
    s.push(" ⚠ ".fg(RED));
    s.push(format!("P:{} ", app.infr[0]).fg(GREEN));
    s.push(format!("C:{} ", app.infr[1]).fg(RED));
    if let Some(u) = app.usage {
        s.push(format!("· uso 5h {:.0}% ", u * 100.0).fg(DIM));
    }
    s.push(format!("· ␣ {}  p pergunta  f fontes  v veredito  q sai", if app.manual { "automático" } else { "passo a passo" }).fg(DIM));
    f.render_widget(Line::from(s), area);
}

fn popup(f: &mut Frame, area: Rect, title: Line<'static>, color: Color) -> Rect {
    f.render_widget(Clear, area);
    let block = Block::bordered().border_type(BorderType::Rounded).border_style(Style::new().fg(color)).title(title);
    let inner = block.inner(area);
    f.render_widget(block, area);
    inner
}

fn ask_popup(f: &mut Frame, app: &App) {
    let area = center(f.area(), 70, 6);
    let inner = popup(f, area, Line::from(" ⚖ Pergunta para o juiz ".bold().fg(GOLD)), GOLD);
    let lines = vec![
        Line::from(vec!["› ".fg(GOLD), app.input.clone().into(), "▏".fg(GOLD)]),
        Line::raw(""),
        Line::from("Enter envia · Esc cancela. O juiz responde ou passa a palavra a um dos lados, após o turno atual.".fg(DIM)),
    ];
    f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
}

/// Quebra `text` em linhas de até `w` caracteres; palavras maiores (URLs) são cortadas.
fn wrap(text: &str, w: usize) -> Vec<String> {
    let w = w.max(1);
    let (mut out, mut cur) = (vec![], String::new());
    for word in text.split_whitespace() {
        let mut word: Vec<char> = word.chars().collect();
        let n = cur.chars().count();
        if n > 0 && n + 1 + word.len() <= w {
            cur.push(' ');
            cur.extend(word);
            continue;
        }
        if n > 0 {
            out.push(std::mem::take(&mut cur));
        }
        while word.len() > w {
            out.push(word.drain(..w).collect());
        }
        cur = word.into_iter().collect();
    }
    if !cur.is_empty() || out.is_empty() {
        out.push(cur);
    }
    out
}

/// Tela cheia, um lado por vez (←→ troca): cada fonte é um cartão com selo de status, título, URL, trecho e nota do juiz.
fn sources_popup(f: &mut Frame, app: &App) {
    let i = app.focus;
    let c = color(i);
    let tab = |k: usize| {
        let n = app.dossies[k].as_ref().map_or(0, |d| d.fontes.len());
        let label = format!(" {} · {n} ", [Side::Pro, Side::Con][k].name());
        if k == i { label.bold().fg(Color::Black).bg(color(k)) } else { label.fg(color(k)) }
    };
    let title = Line::from(vec![" 📚 FONTES  ".bold().fg(GOLD), tab(0), " ".into(), tab(1), " ".into()]);
    let inner = popup(f, f.area(), title, c);
    let [body, _, hint] = Layout::vertical([Min(1), Length(1), Length(1)]).areas(inner);
    f.render_widget(Line::from("←→ troca o lado · ↑↓ PgUp PgDn Home End rola · Esc fecha".fg(DIM)).centered(), hint);
    let [text, _, bar] = Layout::horizontal([Min(1), Length(1), Length(1)]).areas(body.inner(Margin::new(1, 0)));

    const IND: usize = 6;
    let w = (text.width as usize).max(IND + 10); // evita underflow em terminal minúsculo
    let pad = || Span::raw(" ".repeat(IND));
    let mut lines: Vec<Line> = vec![];
    match &app.dossies[i] {
        None => lines.push(Line::from("(ainda pesquisando…)".italic().fg(DIM))),
        Some(d) => {
            for (k, l) in wrap(&d.tese, w - 6).into_iter().enumerate() {
                lines.push(Line::from(vec![if k == 0 { "Tese: ".bold().fg(c) } else { "      ".into() }, l.bold()]));
            }
            let st = |id: &String| app.checks.get(id).map(|(ok, _)| *ok);
            let ok = d.fontes.iter().filter(|s| st(&s.id) == Some(true)).count();
            let bad = d.fontes.iter().filter(|s| st(&s.id) == Some(false)).count();
            lines.push(Line::from(vec![
                format!("✓ conferidas: {ok}").fg(GREEN),
                "  ·  ".fg(DIM),
                format!("✗ não conferem: {bad}").fg(RED),
                "  ·  ".fg(DIM),
                format!("não verificadas: {}", d.fontes.len() - ok - bad).fg(DIM),
            ]));
            for s in &d.fontes {
                lines.push(Line::raw(""));
                let (bg, verdict) = match app.checks.get(&s.id) {
                    Some((true, nota)) => (GREEN, Some(("✓ juiz: ", nota))),
                    Some((false, nota)) => (RED, Some(("✗ juiz: ", nota))),
                    None => (DIM, None),
                };
                let badge = format!(" {} ", s.id);
                let gap = " ".repeat(IND.saturating_sub(badge.chars().count()).max(1));
                for (k, l) in wrap(&s.titulo, w - IND).into_iter().enumerate() {
                    lines.push(if k == 0 {
                        Line::from(vec![badge.clone().bold().fg(Color::Black).bg(bg), gap.clone().into(), l.bold()])
                    } else {
                        Line::from(vec![pad(), l.bold()])
                    });
                }
                for l in wrap(&s.url, w - IND) {
                    lines.push(Line::from(vec![pad(), l.fg(Color::Cyan).add_modifier(Modifier::DIM)]));
                }
                for l in wrap(&s.trecho, w - IND - 2) {
                    lines.push(Line::from(vec![pad(), "│ ".fg(c), l.into()]));
                }
                if let Some((mark, nota)) = verdict {
                    for (k, l) in wrap(nota, w - IND - mark.chars().count()).into_iter().enumerate() {
                        let head = if k == 0 { mark.to_string() } else { " ".repeat(mark.chars().count()) };
                        lines.push(Line::from(vec![pad(), head.fg(bg), l.italic().fg(GOLD)]));
                    }
                }
                let cited: usize = app.speech[i].iter().map(|(_, t)| t.matches(&format!("[{}]", s.id)).count()).sum();
                if cited > 0 {
                    lines.push(Line::from(vec![pad(), format!("citada {cited}× no debate").fg(DIM)]));
                }
            }
        }
    }
    let max = (lines.len() as u16).saturating_sub(text.height);
    app.popup_max.set(max);
    let scroll = app.popup_scroll.min(max);
    f.render_widget(Paragraph::new(lines).scroll((scroll, 0)), text);
    if max > 0 {
        let mut state = ScrollbarState::new(max as usize).position(scroll as usize);
        f.render_stateful_widget(Scrollbar::new(ScrollbarOrientation::VerticalRight).begin_symbol(None).end_symbol(None).thumb_style(Style::new().fg(c)), bar, &mut state);
    }
}

fn verdict_popup(f: &mut Frame, app: &App) {
    let Some(v) = &app.verdict else { return };
    let a = f.area();
    let area = center(a, 90, a.height.saturating_sub(4));
    let inner = popup(f, area, Line::from(" ⚖ VEREDITO · Esc fecha ".bold().fg(GOLD)), GOLD);
    let (who, c) = match v.vencedor.as_str() {
        "pro" => ("🏆 VENCE O PRÓ", GREEN),
        "contra" => ("🏆 VENCE O CONTRA", RED),
        _ => ("🤝 EMPATE", GOLD),
    };
    let mut lines = vec![Line::from(who.bold().fg(c)).centered(), Line::raw("")];
    let (tp, tc): (f64, f64) = v.eixos.iter().fold((0.0, 0.0), |(p, c), e| (p + e.nota_pro, c + e.nota_contra));
    for e in &v.eixos {
        lines.push(Line::from(vec![
            format!("{}  ", e.titulo).bold(),
            format!("PRÓ {:.1}", e.nota_pro).fg(GREEN),
            "  ·  ".into(),
            format!("CONTRA {:.1}", e.nota_contra).fg(RED),
        ]));
        lines.push(Line::from(e.comentario.clone().fg(DIM)));
    }
    lines.push(Line::from(vec!["Total  ".bold(), format!("PRÓ {tp:.1}").fg(GREEN).bold(), "  ·  ".into(), format!("CONTRA {tc:.1}").fg(RED).bold()]));
    lines.push(Line::raw(""));
    lines.push(Line::from(v.justificativa.clone()));
    lines.push(Line::raw(""));
    lines.push(Line::from(vec!["Infrações: ".bold().fg(RED), v.infracoes_resumo.clone().into()]));
    if let Some(p) = &app.saved {
        lines.push(Line::raw(""));
        lines.push(Line::from(format!("Salvo em {}", p.display()).fg(DIM)));
    }
    let p = Paragraph::new(lines).wrap(Wrap { trim: true });
    let max = (p.line_count(inner.width) as u16).saturating_sub(inner.height);
    app.popup_max.set(max);
    f.render_widget(p.scroll((app.popup_scroll.min(max), 0)), inner);
}

fn topic_screen(f: &mut Frame, app: &App) {
    let a = f.area();
    let area = center(a, 66, 15);
    let inner = popup(f, area, Line::from(" ⚖ GAVEL ".bold().fg(Color::Black).bg(GOLD)).centered(), GOLD);
    let [_, sprites_row, _, label, input, _, hint] = Layout::vertical([Length(1), Length(ROWS), Length(1), Length(1), Length(1), Fill(1), Length(1)]).areas(inner);
    let cols: [Rect; 3] = Layout::horizontal([Length(SW); 3]).flex(ratatui::layout::Flex::SpaceAround).areas(sprites_row);
    for (side, col) in [Side::Pro, Side::Judge, Side::Con].into_iter().zip(cols) {
        sprite(f, col, side, if side == Side::Judge { Pose::Idle } else { Pose::Thinking }, app.tick);
    }
    f.render_widget(Line::from("Qual o tema do debate?".bold()).centered(), label);
    f.render_widget(Line::from(vec!["› ".fg(GOLD), app.input.clone().into(), "▏".fg(GOLD)]), input);
    f.render_widget(Line::from("Enter começa · Esc sai".fg(DIM)).centered(), hint);
}

#[cfg(test)]
mod tests {
    #[test]
    fn wrap_respects_width() {
        assert_eq!(super::wrap("a bb ccc dddd", 6), ["a bb", "ccc", "dddd"]);
        assert_eq!(super::wrap("https://exemplo.org/x", 8), ["https://", "exemplo.", "org/x"]);
        assert_eq!(super::wrap("", 5), [""]);
    }
}
