//! Il disegno del TUI: legge l'`App` e scrive su un `Frame` (layout in `docs/TEAM/DESIGN_TUI.md`, sezione 2).

use crate::app::App;
use crate::theme::Theme;
use mrp_core::view::{self, ViewOptions};
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};
use ratatui::{Frame, Terminal};

/// Disegna tutto lo schermo e ricorda dove sta il cursore (`App::cursor_xy`).
pub fn draw(f: &mut Frame, app: &mut App) {
    let t = Theme::for_id(app.theme_id());
    let area = f.area();
    // Il fondo del tema su tutto lo schermo.
    f.render_widget(Block::default().style(t.screen), area);
    let (w, h) = (area.width, area.height);
    if w == 0 || h < 4 {
        // Il contratto chiede h >= 4: sotto non c'e' niente da disegnare.
        app.set_cursor_xy(None);
        return;
    }
    draw_title(f, app, &t, w);
    draw_text(f, app, &t, w, h);
    draw_status(f, app, &t, w, h);
    draw_footer(f, app, &t, w, h);
}

/// Riga 0: `" " + nome file + (" *" se modificato)` a sinistra, `dimensione + "  " + modo + " "` a destra.
fn draw_title(f: &mut Frame, app: &mut App, t: &Theme, w: u16) {
    let mut nome = format!(" {}", app.file_name());
    if app.is_modified() {
        nome.push_str(" *");
    }
    let size = fmt_size(app.document_len());
    let mode_key = if app.is_edit() {
        "mode.edit"
    } else {
        "mode.view"
    };
    let modo = app.catalog().t(mode_key).to_string();
    let destra = size.chars().count() + 2 + modo.chars().count() + 1;
    let pad = (w as usize).saturating_sub(nome.chars().count() + destra);
    let mode_style = if app.is_edit() {
        t.mode_edit
    } else {
        t.mode_view
    };
    let line = Line::from(vec![
        Span::styled(nome, t.title),
        Span::raw(" ".repeat(pad)),
        Span::styled(format!("{}  ", size), t.title),
        Span::styled(modo, mode_style),
        Span::raw(" "),
    ]);
    f.render_widget(Paragraph::new(line).style(t.screen), Rect::new(0, 0, w, 1));
}

/// Righe 1..=h-3: il testo con il gutter dei numeri di riga; imposta anche il cursore del terminale.
fn draw_text(f: &mut Frame, app: &mut App, t: &Theme, w: u16, h: u16) {
    let text_rows = (h as usize).saturating_sub(3);
    let gutter_w = app.gutter_w();
    let cells = (w as usize).saturating_sub(gutter_w);
    let opts = ViewOptions {
        show_eof_row: app.is_edit(),
        ..ViewOptions::default()
    };
    let (top, hscroll) = (app.top(), app.hscroll());
    let rows = match app.document() {
        Some(doc) => view::rows_from(doc, top, text_rows, hscroll, cells, &opts),
        None => Vec::new(),
    };
    // La riga e la colonna del cursore, per il cursore del terminale.
    let cursor_row = app.cursor_row();
    let col = app.cursor_col();
    let mut xy: Option<(u16, u16)> = None;
    for (i, row) in rows.iter().enumerate() {
        let y = 1 + i as u16;
        // Il numero c'e' solo se l'indice e' pronto e la riga inizia un a-capo: la continuazione di una riga
        // tagliata non ha numero.
        let num = if row.starts_line {
            app.gutter_number(row.start)
        } else {
            None
        };
        let mut gutter = match num {
            Some(n) => format!("{:>width$}", n, width = gutter_w - 1),
            None => " ".repeat(gutter_w - 1),
        };
        gutter.push('│');
        let mut text = row.text.clone();
        if row.more_left {
            replace_first(&mut text, '«');
        }
        if row.more_right {
            replace_last(&mut text, '»');
        }
        // Ogni riga arriva fino in fondo allo schermo.
        let manca = cells.saturating_sub(text.chars().count());
        text.push_str(&" ".repeat(manca));
        let line = Line::from(vec![
            Span::styled(gutter, t.gutter),
            Span::styled(text, t.fg),
        ]);
        f.render_widget(Paragraph::new(line).style(t.screen), Rect::new(0, y, w, 1));
        if xy.is_none() && row.start == cursor_row && col >= hscroll && col - hscroll < cells {
            xy = Some((gutter_w as u16 + (col - hscroll) as u16, y));
        }
    }
    if let Some((x, y)) = xy {
        f.set_cursor_position((x, y));
    }
    app.set_cursor_xy(xy);
}

/// Riga h-2: il messaggio transitorio (con uno spazio davanti) o la posizione del cursore; a destra, se un lavoro
/// e' in corso, `status.indexing` con la percentuale, seguito da uno spazio.
fn draw_status(f: &mut Frame, app: &mut App, t: &Theme, w: u16, h: u16) {
    // Il messaggio si copia prima: sotto serve l'app mutabile.
    let flash = app.flash().map(|msg| msg.to_string());
    let left = match flash {
        Some(msg) => format!(" {}", msg),
        None => {
            let riga = match app.cursor_line() {
                Some(n) => n.to_string(),
                None => app.catalog().t("status.unknown_line").to_string(),
            };
            let col = app.cursor_col() + 1;
            let pct = percent(app.cursor(), app.document_len());
            app.catalog().tf(
                "status.pos",
                &[
                    &riga,
                    &col.to_string(),
                    &app.cursor().to_string(),
                    &pct.to_string(),
                ],
            )
        }
    };
    let right = match app.index_progress() {
        Some(p) => format!(
            "{} ",
            app.catalog().tf("status.indexing", &[&p.to_string()])
        ),
        None => String::new(),
    };
    let pad = (w as usize).saturating_sub(left.chars().count() + right.chars().count());
    let line = Line::from(vec![
        Span::styled(left, t.status),
        Span::raw(" ".repeat(pad)),
        Span::styled(right, t.status),
    ]);
    f.render_widget(
        Paragraph::new(line).style(t.screen),
        Rect::new(0, h - 2, w, 1),
    );
}

/// Riga h-1: `" "` + `F1 Help  F2 Save  ...` (voci separate da due spazi, F4 cambia etichetta col modo), tagliata
/// alla larghezza dal disegno.
fn draw_footer(f: &mut Frame, app: &mut App, t: &Theme, w: u16, h: u16) {
    let cat = app.catalog();
    let f4 = if app.is_edit() {
        "footer.f4.view"
    } else {
        "footer.f4.edit"
    };
    let voci = [
        ("F1", "footer.f1"),
        ("F2", "footer.f2"),
        ("F3", "footer.f3"),
        ("F4", f4),
        ("F5", "footer.f5"),
        ("F6", "footer.f6"),
        ("F7", "footer.f7"),
        ("F10", "footer.f10"),
    ];
    let mut spans = vec![Span::raw(" ")];
    for (i, (tasto, chiave)) in voci.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw("  "));
        }
        spans.push(Span::styled((*tasto).to_string(), t.key));
        spans.push(Span::raw(format!(" {}", cat.t(chiave))));
    }
    f.render_widget(
        Paragraph::new(Line::from(spans)).style(t.screen),
        Rect::new(0, h - 1, w, 1),
    );
}

/// La dimensione leggibile del titolo: `N B` sotto 1024, altrimenti due decimali in `KiB`/`MiB`/`GiB`/`TiB`.
fn fmt_size(len: u64) -> String {
    const UNITA: [&str; 4] = ["KiB", "MiB", "GiB", "TiB"];
    if len < 1024 {
        return format!("{} B", len);
    }
    let mut v = len as f64 / 1024.0;
    let mut u = 0;
    while v >= 1024.0 && u < UNITA.len() - 1 {
        v /= 1024.0;
        u += 1;
    }
    format!("{:.2} {}", v, UNITA[u])
}

/// La percentuale intera del cursore nel documento (0 se vuoto).
fn percent(cursor: u64, len: u64) -> u64 {
    cursor.saturating_mul(100).checked_div(len).unwrap_or(0)
}

/// Il primo carattere diventa `c` (il segno di prosecuzione a sinistra).
fn replace_first(s: &mut String, c: char) {
    let Some(primo) = s.chars().next() else {
        return;
    };
    let mut buf = [0u8; 4];
    s.replace_range(0..primo.len_utf8(), c.encode_utf8(&mut buf));
}

/// L'ultimo carattere diventa `c` (il segno di prosecuzione a destra).
fn replace_last(s: &mut String, c: char) {
    let Some(ultimo) = s.chars().next_back() else {
        return;
    };
    let mut buf = [0u8; 4];
    let fine = s.len();
    s.replace_range(fine - ultimo.len_utf8()..fine, c.encode_utf8(&mut buf));
}

/// Disegna su un terminale di prova `w` x `h` e restituisce le `h` righe di testo (una cella = un carattere, le celle
/// vuote sono spazi): serve ai test e alla modalita' `--dump` del binario.
pub fn render_lines(app: &mut App, w: u16, h: u16) -> Vec<String> {
    if w == 0 || h == 0 {
        return vec![String::new(); h as usize];
    }
    let mut terminal = match Terminal::new(TestBackend::new(w, h)) {
        Ok(t) => t,
        Err(_) => return vec![String::new(); h as usize],
    };
    if terminal.draw(|f| draw(f, app)).is_err() {
        return vec![String::new(); h as usize];
    }
    // Una cella = un carattere; le celle saltate dai caratteri larghi si leggono come spazi.
    let mut out = Vec::with_capacity(h as usize);
    for riga in terminal.backend().buffer().content().chunks(w as usize) {
        let mut s = String::with_capacity(w as usize);
        for cella in riga {
            match cella.symbol() {
                "" => s.push(' '),
                simbolo => s.push_str(simbolo),
            }
        }
        out.push(s);
    }
    out
}
