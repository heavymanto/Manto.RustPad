//! Il disegno della griglia di celle (`mrp_tui::cells::Griglia`) con egui: lo sfondo a rettangoli per tratti dello
//! stesso colore, il testo per tratti dello stesso colore (i caratteri non ASCII uno per uno, nella loro cella, perche'
//! un font diverso puo' avere un'altra larghezza) e il cursore.

use eframe::egui::{Align2, Color32, FontId, Painter, Pos2, Rect, Stroke, StrokeKind, Vec2};
use mrp_tui::cells::{Cell, Griglia, BG_PREDEFINITO};

/// Un colore RGB di `cells` come colore di egui.
pub fn colore(c: [u8; 3]) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}

/// Il grassetto senza un font grassetto: il primo piano si schiarisce un po'.
fn schiarito(c: [u8; 3]) -> [u8; 3] {
    let s = |x: u8| x.saturating_add(((255 - u16::from(x)) / 4) as u8);
    [s(c[0]), s(c[1]), s(c[2])]
}

/// Il colore del testo di una cella.
fn primo_piano(c: &Cell) -> Color32 {
    colore(if c.grassetto { schiarito(c.fg) } else { c.fg })
}

/// Gli intervalli di colonne `[da, fino)` di una riga con lo stesso sfondo.
pub fn tratti_sfondo(riga: &[Cell]) -> Vec<(usize, usize, [u8; 3])> {
    let mut v: Vec<(usize, usize, [u8; 3])> = Vec::new();
    for (x, c) in riga.iter().enumerate() {
        match v.last_mut() {
            Some((_, fino, bg)) if *bg == c.bg && *fino == x => *fino = x + 1,
            _ => v.push((x, x + 1, c.bg)),
        }
    }
    v
}

/// I tratti di testo di una riga: `(colonna, testo, colore)`. Un tratto e' una sequenza di celle ASCII dello stesso
/// colore; ogni altro carattere e' un tratto da solo (la sua cella). Le celle vuote (la seconda meta' di un carattere
/// largo) e gli spazi non si scrivono: lo sfondo e' gia' li'.
pub fn tratti_testo(riga: &[Cell]) -> Vec<(usize, String, [u8; 3], bool)> {
    let mut v: Vec<(usize, String, [u8; 3], bool)> = Vec::new();
    let mut aperto: Option<usize> = None;
    for (x, c) in riga.iter().enumerate() {
        let s = c.simbolo.as_str();
        let ascii_stampabile = s.len() == 1 && s.as_bytes()[0] > b' ' && s.as_bytes()[0] < 0x7f;
        if ascii_stampabile {
            // si estende il tratto aperto se e' subito prima, ha lo stesso colore e grassetto
            if let Some(i) = aperto {
                let (inizio, testo, fg, grassetto) = &mut v[i];
                if *inizio + testo.chars().count() == x && *fg == c.fg && *grassetto == c.grassetto
                {
                    testo.push_str(s);
                    continue;
                }
            }
            v.push((x, s.to_string(), c.fg, c.grassetto));
            aperto = Some(v.len() - 1);
        } else {
            aperto = None;
            if !s.is_empty() && s != " " {
                v.push((x, s.to_string(), c.fg, c.grassetto));
            }
        }
    }
    v
}

/// Disegna la griglia con l'angolo in alto a sinistra in `origine`; `modifica` sceglie la forma del cursore (una barra
/// in modifica, un rettangolo in lettura) e `cursore_acceso` lo fa lampeggiare.
pub fn disegna(
    p: &Painter,
    origine: Pos2,
    cella: Vec2,
    g: &Griglia,
    font: &FontId,
    modifica: bool,
    cursore_acceso: bool,
) {
    if g.w == 0 || g.h == 0 {
        return;
    }
    // il fondo di tutta l'area
    p.rect_filled(
        Rect::from_min_size(
            origine,
            Vec2::new(cella.x * f32::from(g.w), cella.y * f32::from(g.h)),
        ),
        0.0,
        colore(BG_PREDEFINITO),
    );
    for y in 0..g.h {
        let inizio = usize::from(y) * usize::from(g.w);
        let riga = &g.celle[inizio..inizio + usize::from(g.w)];
        let py = origine.y + cella.y * f32::from(y);
        for (da, fino, bg) in tratti_sfondo(riga) {
            if bg != BG_PREDEFINITO {
                p.rect_filled(
                    Rect::from_min_max(
                        Pos2::new(origine.x + cella.x * da as f32, py),
                        Pos2::new(origine.x + cella.x * fino as f32, py + cella.y),
                    ),
                    0.0,
                    colore(bg),
                );
            }
        }
        for (x, testo, fg, grassetto) in tratti_testo(riga) {
            let colore_testo = if grassetto {
                colore(schiarito(fg))
            } else {
                colore(fg)
            };
            // centrato nella riga: un font di ripiego (gli ideogrammi) ha altezza e linea di base diverse da quelle del
            // monospace, e a un'altezza fissa dall'alto le righe con ideogrammi risulterebbero spostate
            p.text(
                Pos2::new(origine.x + cella.x * x as f32, py + cella.y / 2.0),
                Align2::LEFT_CENTER,
                testo,
                font.clone(),
                colore_testo,
            );
        }
        // il sottolineato
        for (x, c) in riga.iter().enumerate() {
            if c.sottolineato {
                let ax = origine.x + cella.x * x as f32;
                p.line_segment(
                    [
                        Pos2::new(ax, py + cella.y - 1.0),
                        Pos2::new(ax + cella.x, py + cella.y - 1.0),
                    ],
                    Stroke::new(1.0_f32, primo_piano(c)),
                );
            }
        }
    }
    if let (Some((cx, cy)), true) = (g.cursore, cursore_acceso) {
        let r = Rect::from_min_size(
            Pos2::new(
                origine.x + cella.x * f32::from(cx),
                origine.y + cella.y * f32::from(cy),
            ),
            cella,
        );
        let colore_cursore = Color32::from_rgb(255, 255, 255);
        if modifica {
            p.rect_filled(
                Rect::from_min_size(r.min, Vec2::new(2.0, cella.y)),
                0.0,
                colore_cursore,
            );
        } else {
            p.rect_stroke(
                r,
                0.0,
                Stroke::new(1.5_f32, colore_cursore),
                StrokeKind::Inside,
            );
        }
    }
}
