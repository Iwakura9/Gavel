//! Clawd em pixel art: cada célula do terminal tem 2 pixels (▀ com fg = de cima, bg = de baixo).
use crate::debate::Side;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};

#[derive(Clone, Copy, PartialEq)]
pub enum Pose {
    Idle,
    Talking,
    Thinking,
    Searching,
    Objection,
    Win,
    Lose,
}

pub const W: usize = 20;
const H: usize = 12;
pub const ROWS: u16 = (H / 2) as u16;

pub const ORANGE: Color = Color::Rgb(217, 119, 87);

const BODY: [&str; 11] = [
    "....................",
    ".....oooooooooo.....",
    ".....oooooooooo.....",
    ".....ooKooooKoo.....",
    ".....ooKooooKoo.....",
    "...oooooooooooooo...",
    "...oooooooooooooo...",
    ".....oooooooooo.....",
    ".....oooooooooo.....",
    ".....o.o....o.o.....",
    ".....o.o....o.o.....",
];
const WIG: [&str; 5] = [
    ".....wwwwwwwwww.....",
    "....w..........w....",
    "....w..........w....",
    "....w..........w....",
    "....w..........w....",
];
const GAVEL_UP: [&str; 5] = ["DDD", "DDD", ".b.", ".b.", ".b."];
const GAVEL_DOWN: [&str; 2] = ["bDD", ".DD"];
/// Lupa na frente de um olho, que aparece ampliado (KK) dentro da lente.
const LENS: [&str; 8] = [".ssss...", "sggggs..", "sgKKgs..", "sgKKgs..", "sggggs..", ".ssssD..", "......D.", ".......D"];
/// Posição x da lupa a cada 2 ticks: olho esquerdo → desliza → olho direito → volta.
const LENS_PATH: [i32; 10] = [5, 5, 5, 5, 7, 10, 10, 10, 10, 8];

type Grid = [[Option<Color>; W]; H];

fn color(c: char) -> Option<Color> {
    Some(match c {
        'o' => ORANGE,
        'K' => Color::Rgb(25, 25, 25),
        'w' => Color::Rgb(240, 240, 240),
        'b' => Color::Rgb(140, 90, 50),
        'D' => Color::Rgb(85, 50, 30),
        'G' => Color::Rgb(76, 175, 80),
        'R' => Color::Rgb(229, 57, 53),
        'c' => Color::Rgb(120, 200, 230),
        's' => Color::Rgb(200, 205, 215),
        'g' => Color::Rgb(170, 220, 240),
        'B' => Color::Rgb(90, 150, 255),
        'y' => Color::Rgb(255, 215, 64),
        'm' => Color::Rgb(220, 90, 220),
        _ => return None, // '_' apaga
    })
}

fn put(g: &mut Grid, x: i32, y: i32, c: char) {
    if (0..W as i32).contains(&x) && (0..H as i32).contains(&y) {
        g[y as usize][x as usize] = color(c);
    }
}

fn draw(g: &mut Grid, art: &[&str], dx: i32, dy: i32) {
    for (y, row) in art.iter().enumerate() {
        for (x, ch) in row.chars().enumerate() {
            if ch != '.' {
                put(g, x as i32 + dx, y as i32 + dy, ch);
            }
        }
    }
}

fn eyes(g: &mut Grid, dy: i32, look: (i32, i32), rows: &[i32]) {
    for y in [3, 4] {
        put(g, 7, y + dy, 'o');
        put(g, 12, y + dy, 'o');
    }
    for &y in rows {
        put(g, 7 + look.0, y + look.1 + dy, 'K');
        put(g, 12 + look.0, y + look.1 + dy, 'K');
    }
}

pub fn render(side: Side, pose: Pose, tick: u64) -> Vec<Line<'static>> {
    let mut g: Grid = [[None; W]; H];
    let dy = match pose {
        Pose::Idle => (tick / 5 % 2) as i32,
        Pose::Win => (tick / 2 % 2) as i32,
        Pose::Lose => 1,
        _ => 0,
    };
    draw(&mut g, &BODY, 0, dy);

    match pose {
        Pose::Talking if tick.is_multiple_of(2) => {
            put(&mut g, 9, 6 + dy, 'K');
            put(&mut g, 10, 6 + dy, 'K');
        }
        Pose::Thinking => {
            eyes(&mut g, dy, (1, -1), &[3, 4]);
            for (i, (x, y)) in [(15, 1), (17, 0), (19, 0)].into_iter().enumerate() {
                if (tick / 3 % 4) as usize > i {
                    put(&mut g, x, y, 'w');
                }
            }
        }
        Pose::Searching => draw(&mut g, &LENS, LENS_PATH[(tick / 2) as usize % LENS_PATH.len()], 0),
        Pose::Win => {
            for (x, y) in [(3, 5), (4, 5), (3, 6), (4, 6), (15, 5), (16, 5), (15, 6), (16, 6)] {
                put(&mut g, x, y + dy, '_');
            }
            for (x, y) in [(4, 4), (3, 3), (3, 2), (2, 1), (15, 4), (16, 3), (16, 2), (17, 1)] {
                put(&mut g, x, y + dy, 'o');
            }
            eyes(&mut g, dy, (0, 0), &[3]);
            for i in 0..7u64 {
                let (x, y) = (((tick * 7 + i * 13) % W as u64) as i32, ((tick + i * 5) % 5) as i32);
                if g[y as usize][x as usize].is_none() {
                    put(&mut g, x, y, ['y', 'm', 'G', 'c', 'R'][(i % 5) as usize]);
                }
            }
        }
        Pose::Lose => {
            eyes(&mut g, dy, (0, 0), &[4]);
            put(&mut g, 7, 5 + dy + (tick / 3 % 2) as i32, 'B');
        }
        _ => {}
    }

    match side {
        Side::Judge => {
            draw(&mut g, &WIG, 0, dy);
            match pose {
                Pose::Objection if tick / 2 % 2 == 1 => {
                    draw(&mut g, &GAVEL_DOWN, 17, 5);
                    for (x, y) in [(16, 8), (19, 8), (18, 9)] {
                        put(&mut g, x, y, 'y');
                    }
                }
                Pose::Idle | Pose::Talking | Pose::Objection => draw(&mut g, &GAVEL_UP, 16, dy),
                _ => {}
            }
        }
        Side::Con => g.iter_mut().for_each(|row| row.reverse()), // espelha: Contra olha para o centro
        Side::Pro => {}
    }

    (0..H / 2)
        .map(|r| {
            Line::from(
                (0..W)
                    .map(|x| match (g[2 * r][x], g[2 * r + 1][x]) {
                        (None, None) => Span::raw(" "),
                        (Some(t), None) => Span::styled("▀", Style::new().fg(t)),
                        (None, Some(b)) => Span::styled("▄", Style::new().fg(b)),
                        (Some(t), Some(b)) => Span::styled("▀", Style::new().fg(t).bg(b)),
                    })
                    .collect::<Vec<_>>(),
            )
        })
        .collect()
}
