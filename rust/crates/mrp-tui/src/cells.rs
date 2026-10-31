//! Lo schermo del TUI come griglia di celle colorate (testo, primo piano, sfondo, grassetto), per un altro
//! disegnatore dello stesso schermo: la GUI (`mrp-gui`) non rifa la logica di `ui.rs`, disegna le celle che `ui.rs`
//! scrive su un terminale di prova. Cosi' titolo, gutter, selezione, ricerca, dialoghi e temi sono gli stessi del TUI.
//!
//! I colori sono quelli ANSI a 16 (i temi non ne usano altri) piu' i 256 indicizzati e i veri RGB; `Reset` vale il
//! colore predefinito di primo piano o di sfondo (il tema "minimal" non ne impone).

use crate::app::App;
use crate::ui;
use crate::workspace::Workspace;
use ratatui::backend::TestBackend;
use ratatui::style::{Color, Modifier};
use ratatui::text::Span;
use ratatui::{Frame, Terminal};

/// Il colore di primo piano predefinito (`Reset`): grigio chiaro.
pub const FG_PREDEFINITO: [u8; 3] = [204, 204, 204];
/// Il colore di sfondo predefinito (`Reset`): quasi nero.
pub const BG_PREDEFINITO: [u8; 3] = [12, 12, 12];

/// Una cella dello schermo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cell {
    /// Il carattere (o la sequenza) della cella; vuoto per la cella che un carattere largo lascia dietro di se'.
    pub simbolo: String,
    /// Il primo piano, RGB (gia' scambiato con lo sfondo se la cella e' in `REVERSED`, e attenuato se `DIM`).
    pub fg: [u8; 3],
    /// Lo sfondo, RGB.
    pub bg: [u8; 3],
    /// Grassetto.
    pub grassetto: bool,
    /// Sottolineato.
    pub sottolineato: bool,
}

/// Lo schermo: `w` x `h` celle, per righe, e dove sta il cursore (colonna, riga) se l'app ne ha uno.
#[derive(Debug, Clone)]
pub struct Griglia {
    pub w: u16,
    pub h: u16,
    pub celle: Vec<Cell>,
    pub cursore: Option<(u16, u16)>,
}

impl Griglia {
    /// La cella in (colonna, riga).
    pub fn cella(&self, x: u16, y: u16) -> Option<&Cell> {
        if x < self.w && y < self.h {
            self.celle.get(y as usize * self.w as usize + x as usize)
        } else {
            None
        }
    }

    /// Le righe come testo (le celle vuote sono spazi, gli spazi in fondo si tolgono): come `ui::render_lines`.
    pub fn righe(&self) -> Vec<String> {
        (0..self.h)
            .map(|y| {
                let mut s = String::new();
                for x in 0..self.w {
                    match self.cella(x, y) {
                        Some(c) if !c.simbolo.is_empty() => s.push_str(&c.simbolo),
                        _ => s.push(' '),
                    }
                }
                s.trim_end().to_string()
            })
            .collect()
    }
}

/// Il colore ANSI `n` (0-15) in RGB: la tavolozza della console di Windows.
fn ansi(n: u8) -> [u8; 3] {
    const T: [[u8; 3]; 16] = [
        [12, 12, 12],
        [197, 15, 31],
        [19, 161, 14],
        [193, 156, 0],
        [0, 55, 218],
        [136, 23, 152],
        [58, 150, 221],
        [204, 204, 204],
        [118, 118, 118],
        [231, 72, 86],
        [22, 198, 12],
        [249, 241, 165],
        [59, 120, 255],
        [180, 0, 158],
        [97, 214, 214],
        [242, 242, 242],
    ];
    T[(n & 15) as usize]
}

/// Un colore di ratatui in RGB; `Reset` e' `predefinito`.
pub fn colore(c: Color, predefinito: [u8; 3]) -> [u8; 3] {
    match c {
        Color::Reset => predefinito,
        Color::Black => ansi(0),
        Color::Red => ansi(1),
        Color::Green => ansi(2),
        Color::Yellow => ansi(3),
        Color::Blue => ansi(4),
        Color::Magenta => ansi(5),
        Color::Cyan => ansi(6),
        Color::Gray => ansi(7),
        Color::DarkGray => ansi(8),
        Color::LightRed => ansi(9),
        Color::LightGreen => ansi(10),
        Color::LightYellow => ansi(11),
        Color::LightBlue => ansi(12),
        Color::LightMagenta => ansi(13),
        Color::LightCyan => ansi(14),
        Color::White => ansi(15),
        Color::Rgb(r, g, b) => [r, g, b],
        Color::Indexed(n) if n < 16 => ansi(n),
        Color::Indexed(n) if n >= 232 => {
            let v = 8 + 10 * (n - 232);
            [v, v, v]
        }
        Color::Indexed(n) => {
            // il cubo 6x6x6 dei colori da 16 a 231
            let i = n - 16;
            let livello = |x: u8| if x == 0 { 0 } else { 55 + 40 * x };
            [livello(i / 36), livello((i / 6) % 6), livello(i % 6)]
        }
    }
}

/// Avvicina `a` a `b` di un terzo (per l'attenuato).
fn verso(a: [u8; 3], b: [u8; 3]) -> [u8; 3] {
    let mix = |x: u8, y: u8| ((u16::from(x) * 2 + u16::from(y)) / 3) as u8;
    [mix(a[0], b[0]), mix(a[1], b[1]), mix(a[2], b[2])]
}

/// Disegna l'app su un terminale di prova `w` x `h` (come `ui::render_lines`) e restituisce le celle e il cursore.
/// Le celle di un carattere largo sono due: la prima ha il simbolo, la seconda e' vuota.
pub fn render_grid(app: &mut App, w: u16, h: u16) -> Griglia {
    let mut g = griglia(w, h, |f| ui::draw(f, app));
    g.cursore = app.cursor_xy();
    g
}

/// Come `render_grid`, ma disegna con il `Workspace`: la barra delle schede (se la disegna lui) e la barra dei salti
/// (Ctrl+B) stanno sopra la scheda attiva. E' quello che usa la GUI.
pub fn render_grid_workspace(ws: &mut Workspace, w: u16, h: u16) -> Griglia {
    let mut g = griglia(w, h, |f| ws.draw(f));
    g.cursore = ws.active().cursor_xy();
    g
}

/// Le celle di un disegno `w` x `h` fatto da `disegna` (senza il cursore, che lo sa chi chiama).
fn griglia(w: u16, h: u16, disegna: impl FnOnce(&mut Frame)) -> Griglia {
    let vuota = Griglia {
        w,
        h,
        celle: Vec::new(),
        cursore: None,
    };
    if w == 0 || h == 0 {
        return vuota;
    }
    // il terminale di prova non fallisce mai (l'errore e' `Infallible`)
    let Ok(mut terminale) = Terminal::new(TestBackend::new(w, h));
    if terminale.draw(disegna).is_err() {
        return vuota;
    }
    let mut celle: Vec<Cell> = terminale
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|c| {
            let mut fg = colore(c.fg, FG_PREDEFINITO);
            let mut bg = colore(c.bg, BG_PREDEFINITO);
            if c.modifier.contains(Modifier::REVERSED) {
                std::mem::swap(&mut fg, &mut bg);
            }
            if c.modifier.contains(Modifier::DIM) {
                fg = verso(fg, bg);
            }
            Cell {
                simbolo: c.symbol().to_string(),
                fg,
                bg,
                grassetto: c.modifier.contains(Modifier::BOLD),
                sottolineato: c.modifier.contains(Modifier::UNDERLINED),
            }
        })
        .collect();
    // La cella dopo un carattere largo (2 celle) e' la sua seconda meta': il terminale di prova ci lascia uno spazio,
    // qui si svuota, cosi' chi disegna sa che non c'e' niente da scrivere.
    for riga in celle.chunks_mut(w as usize) {
        for x in 1..riga.len() {
            if Span::raw(riga[x - 1].simbolo.as_str()).width() == 2 {
                riga[x].simbolo.clear();
            }
        }
    }
    Griglia {
        w,
        h,
        celle,
        cursore: None,
    }
}
