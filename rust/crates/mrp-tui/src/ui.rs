//! Il disegno del TUI: legge l'`App` e scrive su un `Frame` (layout in `docs/TEAM/DESIGN_TUI.md`, sezione 2).

use crate::app::{App, DialogKind};
use crate::theme::{Theme, ThemeId};
use mrp_core::block::{block_spans, ColumnBlock};
use mrp_core::encoding::Encoding;
use mrp_core::lang::Catalog;
use mrp_core::pattern::Mode;
use mrp_core::view::{self, ViewOptions};
use mrp_core::Document;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::{Frame, Terminal};

/// Le righe dell'aiuto, in ordine: le chiavi `help.l1`..`help.l18` (l'ultima e' sempre Esc, `help.l10`).
pub(crate) const HELP_KEYS: [&str; 18] = [
    "help.l1", "help.l2", "help.l3", "help.l4", "help.l5", "help.l6", "help.l7", "help.l8",
    "help.l9", "help.l11", "help.l12", "help.l13", "help.l14", "help.l15", "help.l16", "help.l17",
    "help.l18", "help.l10",
];

/// Disegna tutto lo schermo e ricorda dove sta il cursore (`App::cursor_xy`).
pub fn draw(f: &mut Frame, app: &mut App) {
    let t = Theme::for_id(app.theme_id());
    let area = f.area();
    // Il fondo del tema su tutto lo schermo.
    f.render_widget(Block::default().style(t.screen), area);
    let (w, h) = (area.width, area.height);
    // con la cornice il contratto chiede h >= 4 (titolo, almeno una riga di testo, stato, tasti); senza basta una riga
    let minima = if app.chrome() { 4 } else { 1 };
    if w == 0 || h < minima {
        app.set_cursor_xy(None);
        return;
    }
    if app.chrome() {
        draw_title(f, app, &t, w);
    }
    draw_text(f, app, &t, w, h);
    if app.chrome() {
        draw_status(f, app, &t, w, h);
        draw_footer(f, app, &t, w, h);
    }
    // senza la cornice del terminale i dialoghi che hanno una vista (`App::dialog_view`) li disegna la finestra
    if app.chrome() || !app.dialog_has_view() {
        draw_dialog(f, app, &t, w, h);
    }
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
    let mut modo = app.catalog().t(mode_key).to_string();
    // La modalita' colonna si dice nel titolo, dopo il modo e con un solo spazio in mezzo
    // ("VIEW COL" / "EDIT COL"); spenta, il titolo e' quello di sempre.
    if app.column_mode() {
        modo.push(' ');
        modo.push_str(app.catalog().t("mode.column"));
    }
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
/// La parte trovata dalla ricerca si disegna con lo stile `found` e la selezione con lo stile `selection`,
/// che vince sul trovato (in entrambi i casi solo le celle visibili, mai il gutter). Senza documento (un
/// salvataggio lo tiene nel suo thread) ridisegna le righe dell'ultimo disegno.
fn draw_text(f: &mut Frame, app: &mut App, t: &Theme, w: u16, h: u16) {
    // la riga da cui comincia il testo (1 sotto il titolo, 0 senza cornice) e quante ne sono
    let sopra = app.top_offset();
    let text_rows = if app.chrome() {
        (h as usize).saturating_sub(3)
    } else {
        h as usize
    };
    let gutter_w = app.gutter_w();
    let cells = (w as usize).saturating_sub(gutter_w);
    let opts = ViewOptions {
        show_eof_row: app.is_edit(),
        ..app.view_opts()
    };
    let (top, hscroll) = (app.top(), app.hscroll());
    let (rows, dal_documento) = match app.document() {
        Some(doc) => (
            view::rows_from(doc, top, text_rows, hscroll, cells, &opts),
            true,
        ),
        None => (Vec::new(), false),
    };
    // La riga e la colonna del cursore, per il cursore del terminale. In modalita' colonna la colonna
    // e' quella virtuale (del blocco, o del cursore), anche oltre la fine della riga.
    let cursor_row = app.cursor_row();
    let col = if app.column_mode() {
        usize::try_from(app.vcol()).unwrap_or(usize::MAX)
    } else {
        app.cursor_col()
    };
    // Il blocco della modalita' colonna, se c'e': le sue righe e le sue colonne (per le celle da
    // colorare riga per riga, piu' sotto).
    let col_block = app.column_block();
    let mut xy: Option<(u16, u16)> = None;
    // Le righe del disegno, gia' pronte come (gutter, testo). Col documento si calcolano ora e si
    // ricordano in `App`; senza documento si ridisegna la copia dell'ultimo disegno.
    let mut righe: Vec<(String, String)> = Vec::new();
    for row in &rows {
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
        if xy.is_none() && row.start == cursor_row && col >= hscroll && col - hscroll < cells {
            xy = Some((
                gutter_w as u16 + (col - hscroll) as u16,
                sopra + righe.len() as u16,
            ));
        }
        righe.push((gutter, text));
    }
    if dal_documento {
        app.set_drawn_rows(&righe);
        app.set_row_starts(rows.iter().map(|r| r.start).collect());
    } else {
        // La copia dell'ultimo disegno: mai piu' righe di quante ne stiano nello schermo di adesso.
        righe = app.drawn_rows().to_vec();
        righe.truncate(text_rows);
    }
    for (i, (gutter, text)) in righe.iter().enumerate() {
        let y = sopra + i as u16;
        // Ogni riga arriva fino in fondo allo schermo.
        let mut text = text.clone();
        let manca = cells.saturating_sub(text.chars().count());
        text.push_str(&" ".repeat(manca));
        let line = Line::from(vec![
            Span::styled(gutter.clone(), t.gutter),
            Span::styled(text, t.fg),
        ]);
        f.render_widget(Paragraph::new(line).style(t.screen), Rect::new(0, y, w, 1));
        // La parte trovata dall'ultima ricerca, se questa riga la contiene: cambia solo lo
        // stile delle celle visibili (il testo no, e il gutter mai); una occorrenza che
        // attraversa piu' righe visuali o esce dalla finestra orizzontale si taglia qui.
        if let (Some(doc), Some(row), Some(found)) = (app.document(), rows.get(i), app.found()) {
            if let Some((da, fino)) = found_cells(doc, row, found, hscroll, cells, &opts) {
                for colonna in da..fino {
                    let x = gutter_w as u16 + (colonna - hscroll) as u16;
                    f.buffer_mut()[(x, y)].set_style(t.found);
                }
            }
        }
        // La selezione, se questa riga la contiene: lo stesso meccanismo del trovato ma con lo stile
        // `selection`, che vince (si applica dopo). Solo le celle dei caratteri: gli a-capo non ne hanno,
        // e un carattere largo o un tabulatore colora tutte le sue celle visibili.
        if let (Some(doc), Some(row), Some(sel)) = (app.document(), rows.get(i), app.selection()) {
            if let Some((da, fino)) = found_cells(doc, row, sel, hscroll, cells, &opts) {
                for colonna in da..fino {
                    let x = gutter_w as u16 + (colonna - hscroll) as u16;
                    f.buffer_mut()[(x, y)].set_style(t.selection);
                }
            }
        }
        // Il blocco della modalita' colonna, se questa riga visuale e' una delle sue (il suo inizio sta
        // fra la prima e l'ultima riga del blocco): le stesse celle dello stile `selection`, quelle dei
        // caratteri che INIZIANO dentro le colonne del blocco (una riga piu' corta non ne ha). La
        // copertura la chiede al core per la SOLA riga a schermo, mai per il blocco intero.
        if let (Some(doc), Some(row), Some(b)) = (app.document(), rows.get(i), col_block) {
            if row.start >= b.first_row && row.start <= b.last_row {
                let sola_riga = ColumnBlock {
                    first_row: row.start,
                    last_row: row.start,
                    col_a: b.col_a,
                    col_b: b.col_b,
                };
                if let Ok(spans) = block_spans(doc, &sola_riga, &opts, 1) {
                    if let Some(span) = spans.first() {
                        let ca = view::col_of(doc, row.start, span.start, &opts);
                        let cb = view::col_of(doc, row.start, span.end, &opts);
                        let (da, fino) = (ca.max(hscroll), cb.min(hscroll + cells));
                        if da < fino {
                            for colonna in da..fino {
                                let x = gutter_w as u16 + (colonna - hscroll) as u16;
                                f.buffer_mut()[(x, y)].set_style(t.selection);
                            }
                        }
                    }
                }
            }
        }
    }
    // Con un dialogo aperto il cursore del terminale e' nascosto: la posizione del testo si ricorda lo stesso
    // (serve quando il dialogo si chiude), ma non si mostra sullo schermo.
    if let Some((x, y)) = xy {
        if app.dialog_kind().is_none() {
            f.set_cursor_position((x, y));
        }
    }
    app.set_cursor_xy(xy);
}

/// Riga h-2: il messaggio transitorio (con uno spazio davanti) o la posizione del cursore; a destra, se un
/// lavoro e' in corso, `status.saving`, `status.searching` o `status.indexing` con la percentuale, seguito
/// da uno spazio; poi, separato da due spazi se c'e' anche quello, il nome della codifica quando non e'
/// UTF-8 (con uno spazio finale, come gli altri testi di destra).
fn draw_status(f: &mut Frame, app: &mut App, t: &Theme, w: u16, h: u16) {
    // Il messaggio si copia prima: sotto serve l'app mutabile.
    let flash = app.flash().map(|msg| msg.to_string());
    let left = match &flash {
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
    // A destra il lavoro in corso: il salvataggio (quando c'e', e' quello che tiene lo schermo
    // congelato), la ricerca o l'indicizzazione.
    let mut right = if let Some(p) = app.save_progress() {
        format!("{} ", app.catalog().tf("status.saving", &[&p.to_string()]))
    } else if let Some(p) = app.search_progress() {
        format!(
            "{} ",
            app.catalog().tf("status.searching", &[&p.to_string()])
        )
    } else if let Some(p) = app.index_progress() {
        format!(
            "{} ",
            app.catalog().tf("status.indexing", &[&p.to_string()])
        )
    } else {
        String::new()
    };
    // Il nome della codifica, solo se non e' UTF-8 e la riga non e' di un messaggio transitorio (che
    // la occupa per conto suo): dopo l'eventuale lavoro in corso, separato da due spazi, e con uno
    // spazio finale come gli altri testi di destra. In UTF-8 la riga di stato e' identica a oggi.
    if flash.is_none() && !app.encoding().is_utf8() {
        if !right.is_empty() {
            right.push(' ');
        }
        right.push_str(app.encoding().name());
        right.push(' ');
    }
    // Lo stile di fine riga, solo se non e' LF (la riga di stato con LF e' identica a oggi): dopo la codifica.
    if flash.is_none() && app.line_ending() != mrp_core::lineending::LineEnding::Lf {
        if !right.is_empty() && !right.ends_with(' ') {
            right.push(' ');
        }
        if !right.is_empty() {
            right.push(' ');
        }
        right.push_str(app.line_ending().label());
        right.push(' ');
    }
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
/// alla larghezza dal disegno. Se con due spazi la riga non sta nello schermo ma con uno sola si', si usa uno
/// spazio solo (cosi' su 80 colonne ci sono tutte le voci, compreso F10); se non sta nemmeno cosi' restano i
/// due spazi e la riga si taglia.
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
        ("F8", "footer.f8"),
        ("F9", "footer.f9"),
        ("F10", "footer.f10"),
    ];
    // Larghezza della riga con un certo separatore: lo spazio iniziale, le voci ("Fn" + spazio + etichetta) e i
    // separatori in mezzo. Le larghezze sono quelle del terminale (un carattere largo, per esempio CJK,
    // vale due celle), non il numero di caratteri.
    let largo = |sep: usize| -> usize {
        1 + voci
            .iter()
            .map(|(tasto, chiave)| Span::raw(*tasto).width() + 1 + Span::raw(cat.t(chiave)).width())
            .sum::<usize>()
            + sep * (voci.len() - 1)
    };
    let sep = if largo(2) > w as usize && largo(1) <= w as usize {
        " "
    } else {
        "  "
    };
    let mut spans = vec![Span::raw(" ")];
    for (i, (tasto, chiave)) in voci.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw(sep));
        }
        spans.push(Span::styled((*tasto).to_string(), t.key));
        spans.push(Span::raw(format!(" {}", cat.t(chiave))));
    }
    f.render_widget(
        Paragraph::new(Line::from(spans)).style(t.screen),
        Rect::new(0, h - 1, w, 1),
    );
}

/// Il dialogo aperto, sopra lo schermo: prima l'ombra a velo (le celle dietro, una cella a destra e una in
/// basso), poi la finestra centrata con bordo, titolo e contenuto. Nel prompt "vai a" il cursore del terminale
/// va dopo il testo scritto.
fn draw_dialog(f: &mut Frame, app: &mut App, t: &Theme, w: u16, h: u16) {
    let Some(kind) = app.dialog_kind() else {
        return;
    };
    let ContenutoDialogo {
        titolo,
        righe,
        cursore,
    } = contenuto_dialogo(app, t, kind, h);
    // La finestra: il contenuto piu' il bordo, al piu' larga e alta come lo schermo, centrata.
    let larga_contenuto = righe
        .iter()
        .map(|(s, _)| s.chars().count())
        .chain([titolo.chars().count()])
        .max()
        .unwrap_or(0);
    let larga = (larga_contenuto + 2).min(w as usize) as u16;
    let alta = (righe.len() + 2).min(h as usize) as u16;
    if larga < 3 || alta < 3 {
        // Non c'e' spazio nemmeno per il bordo e una riga: niente finestra.
        return;
    }
    let (x, y) = ((w - larga) / 2, (h - alta) / 2);
    // L'ombra si disegna prima: dopo la finestra resta visibile solo la cornice che sporge.
    let mut velo = Style::new().fg(t.shadow_fg);
    if let Some(bg) = t.shadow_bg {
        velo = velo.bg(bg);
    }
    f.render_widget(
        Block::default().style(velo),
        Rect::new(x + 1, y + 1, larga, alta),
    );
    // La finestra copre l'ombra: si ripulisce (il testo dietro non traspare) e poi bordo e fondo del tema.
    f.render_widget(Clear, Rect::new(x, y, larga, alta));
    let finestra = Block::default()
        .borders(Borders::ALL)
        .border_style(t.border)
        .style(t.dialog)
        .title(Line::styled(titolo, t.title));
    f.render_widget(finestra, Rect::new(x, y, larga, alta));
    // Il contenuto, una finestra di una riga per voce (oltre l'altezza interna non c'e' posto).
    for (i, (testo, stile)) in righe.iter().enumerate() {
        if i + 2 >= alta as usize {
            break;
        }
        f.render_widget(
            Paragraph::new(Line::styled(testo.clone(), *stile)),
            Rect::new(x + 1, y + 1 + i as u16, larga - 2, 1),
        );
    }
    // Il cursore del terminale nei prompt: dopo il testo scritto (negli altri dialoghi e' nascosto, se n'e'
    // gia' occupato `draw_text`).
    if let Some((riga, colonna)) = cursore {
        if riga + 2 < alta as usize {
            let colonna = colonna.min(larga as usize - 2);
            let xy = (x + 1 + colonna as u16, y + 1 + riga as u16);
            f.set_cursor_position(xy);
            app.set_cursor_xy(Some(xy));
        }
    }
}

/// Il contenuto di un dialogo pronto da disegnare: il titolo, le righe (testo e stile) e, per i prompt, dove sta
/// il cursore (riga e colonna DENTRO il contenuto). Ogni stringa viene dal `Catalog`.
struct ContenutoDialogo {
    titolo: String,
    righe: Vec<(String, Style)>,
    cursore: Option<(usize, usize)>,
}

/// Il contenuto di un dialogo: il titolo e le righe; per i prompt anche dove sta il cursore (riga e colonna
/// DENTRO il contenuto). Ogni stringa viene dal `Catalog`. `h` e' l'altezza dello schermo: i menu che
/// scorrono (le codifiche) ne limitano la lista a quante righe ci stanno.
fn contenuto_dialogo(app: &App, t: &Theme, kind: DialogKind, h: u16) -> ContenutoDialogo {
    let cat = app.catalog();
    match kind {
        // Il menu delle codifiche: la lista e' quella di `Encoding::all()` (nomi canonici) e scorre:
        // la finestra mostra al piu' quante righe stanno nello schermo meno bordo e suggerimento, e
        // tiene sempre visibile la voce scelta (evidenziata come negli altri menu).
        DialogKind::Encoding => {
            let selezione = app.menu_index().unwrap_or(0);
            let tutte = Encoding::all();
            let altezza = (h as usize).saturating_sub(3);
            let quante = tutte.len().min(altezza);
            let prima = if tutte.len() > altezza {
                selezione
                    .saturating_sub(altezza.saturating_sub(1))
                    .min(tutte.len() - quante)
            } else {
                0
            };
            let mut righe: Vec<(String, Style)> = tutte[prima..prima + quante]
                .iter()
                .enumerate()
                .map(|(i, e)| {
                    let stile = if prima + i == selezione {
                        t.selected
                    } else {
                        t.dialog
                    };
                    (e.name().to_string(), stile)
                })
                .collect();
            // Il suggerimento sta sotto la lista, come nel menu delle lingue.
            righe.push((cat.t("menu.hint").to_string(), t.hint));
            ContenutoDialogo {
                titolo: cat.t("menu.encoding.title").to_string(),
                righe,
                cursore: None,
            }
        }
        DialogKind::Help => {
            let mut righe: Vec<(String, Style)> = HELP_KEYS
                .iter()
                .map(|k| (cat.t(k).to_string(), t.dialog))
                .collect();
            righe.push((cat.t("help.close").to_string(), t.hint));
            ContenutoDialogo {
                titolo: cat.t("help.title").to_string(),
                righe,
                cursore: None,
            }
        }
        DialogKind::Theme => {
            let selezione = app.menu_index().unwrap_or(0);
            let righe = ThemeId::ALL
                .iter()
                .enumerate()
                .map(|(i, tema)| {
                    // Ogni voce: nome + due spazi + descrizione.
                    let testo = format!("{}  {}", cat.t(tema.key()), cat.t(tema.blurb_key()));
                    let stile = if i == selezione { t.selected } else { t.dialog };
                    (testo, stile)
                })
                .collect();
            ContenutoDialogo {
                titolo: cat.t("menu.theme.title").to_string(),
                righe,
                cursore: None,
            }
        }
        DialogKind::Lang => {
            let selezione = app.menu_index().unwrap_or(0);
            let mut righe: Vec<(String, Style)> = app
                .langs()
                .iter()
                .enumerate()
                .map(|(i, l)| {
                    // Ogni voce: il nome della lingua e, se AI, il marchio.
                    let mut testo = l.name.clone();
                    if l.ai {
                        testo.push(' ');
                        testo.push_str(cat.t("menu.lang.ai_tag"));
                    }
                    let stile = if i == selezione { t.selected } else { t.dialog };
                    (testo, stile)
                })
                .collect();
            // Se almeno una lingua e' AI, l'avviso sta sotto la lista.
            if app.langs().iter().any(|l| l.ai) {
                righe.push((cat.t("menu.lang.ai_warning").to_string(), t.hint));
            }
            ContenutoDialogo {
                titolo: cat.t("menu.lang.title").to_string(),
                righe,
                cursore: None,
            }
        }
        DialogKind::Goto => prompt_contenuto(
            cat,
            t,
            "prompt.goto.title",
            "prompt.goto.hint",
            app.goto_text(),
        ),
        DialogKind::Open => prompt_contenuto(
            cat,
            t,
            "prompt.open.title",
            "prompt.open.hint",
            app.open_text(),
        ),
        // Il prompt di ricerca: uguale al "vai a" (cambia il vocabolario), con in piu' la riga delle opzioni.
        DialogKind::Find => {
            let mut c = prompt_contenuto(
                cat,
                t,
                "prompt.find.title",
                "prompt.find.hint",
                app.find_text(),
            );
            c.righe.insert(2, (opzioni_ricerca(app, false), t.hint));
            c
        }
        // Sostituisci: cerca, con, le opzioni, lo stato (l'anteprima) e i tasti.
        DialogKind::Replace => {
            let etichetta_cerca = cat.t("prompt.replace.find").to_string();
            let etichetta_con = cat.t("prompt.replace.with").to_string();
            let larghezza = etichetta_cerca
                .chars()
                .count()
                .max(etichetta_con.chars().count());
            let riga = |etichetta: &str, testo: &str| {
                let pad = larghezza - etichetta.chars().count();
                format!("{}{}> {}", etichetta, " ".repeat(pad), testo)
            };
            let (campo, testo) = if app.rep_field() == 0 {
                (0usize, app.rep_find())
            } else {
                (1usize, app.rep_with())
            };
            let colonna = larghezza + 2 + testo.chars().count();
            let mut righe = vec![
                (riga(&etichetta_cerca, app.rep_find()), t.dialog),
                (riga(&etichetta_con, app.rep_with()), t.dialog),
                (opzioni_ricerca(app, true), t.hint),
            ];
            if !app.rep_status().is_empty() {
                righe.push((app.rep_status().to_string(), t.selected));
            }
            righe.push((cat.t("prompt.replace.keys").to_string(), t.hint));
            ContenutoDialogo {
                titolo: cat.t("prompt.replace.title").to_string(),
                righe,
                cursore: Some((campo, colonna)),
            }
        }
        DialogKind::SaveAs => prompt_contenuto(
            cat,
            t,
            "prompt.saveas.title",
            "prompt.saveas.hint",
            app.save_as_text(),
        ),
        // Il file scelto esiste gia': si chiede se sostituirlo.
        DialogKind::Overwrite => ContenutoDialogo {
            titolo: cat.t("overwrite.title").to_string(),
            righe: vec![
                (
                    cat.tf("overwrite.text", &[&app.save_as_pending_name()]),
                    t.dialog,
                ),
                (cat.t("overwrite.keys").to_string(), t.hint),
            ],
            cursore: None,
        },
        // Conferma all'uscita con modifiche non salvate: il testo e le tre scelte.
        DialogKind::Confirm => ContenutoDialogo {
            titolo: cat.t("confirm.title").to_string(),
            righe: vec![
                (cat.t("confirm.text").to_string(), t.dialog),
                (cat.t("confirm.save").to_string(), t.dialog),
                (cat.t("confirm.discard").to_string(), t.dialog),
                (cat.t("confirm.cancel").to_string(), t.dialog),
            ],
            cursore: None,
        },
        // Salvataggio in corso: il nome del file, la barra di avanzamento e i tasti.
        DialogKind::Saving => {
            let testo = cat.tf("save.text", &[app.file_name()]);
            let chiavi = cat.t("save.keys").to_string();
            let titolo = cat.t("save.title").to_string();
            // La barra occupa la larghezza della finestra meno 4 (il bordo e un margine per lato):
            // `#` per la parte fatta, `-` per il resto.
            let larga_contenuto = [
                titolo.chars().count(),
                testo.chars().count(),
                chiavi.chars().count(),
            ]
            .into_iter()
            .max()
            .unwrap_or(0);
            let barra_larga = (larga_contenuto + 2).saturating_sub(4);
            let fatti = app.save_progress().unwrap_or(0) as usize * barra_larga / 100;
            let barra = format!("{}{}", "#".repeat(fatti), "-".repeat(barra_larga - fatti));
            ContenutoDialogo {
                titolo,
                righe: vec![(testo, t.dialog), (barra, t.dialog), (chiavi, t.hint)],
                cursore: None,
            }
        }
    }
}

/// Il contenuto di un prompt ("vai a" e ricerca): il suggerimento, la riga `"> " + testo` e i
/// tasti; il cursore del terminale sta nella riga di input, dopo "> " e il testo scritto.
fn prompt_contenuto(
    cat: &Catalog,
    t: &Theme,
    titolo: &str,
    suggerimento: &str,
    testo: &str,
) -> ContenutoDialogo {
    ContenutoDialogo {
        titolo: cat.t(titolo).to_string(),
        righe: vec![
            (cat.t(suggerimento).to_string(), t.hint),
            (format!("> {}", testo), t.dialog),
            (cat.t("prompt.keys").to_string(), t.hint),
        ],
        cursore: Some((1, 2 + testo.chars().count())),
    }
}

/// La riga delle opzioni di ricerca: il modo, le maiuscole e (per Sostituisci) l'ambito.
fn opzioni_ricerca(app: &App, con_ambito: bool) -> String {
    let cat = app.catalog();
    let (modo, maiuscole) = app.search_options();
    let modo = match modo {
        Mode::Text => cat.t("prompt.opt.text"),
        Mode::Glob => cat.t("prompt.opt.glob"),
        Mode::Regex => cat.t("prompt.opt.regex"),
    };
    let caso = if maiuscole {
        cat.t("prompt.opt.case")
    } else {
        cat.t("prompt.opt.nocase")
    };
    let mut s = format!("{}: {} | {}", cat.t("prompt.opt.mode"), modo, caso);
    if con_ambito {
        s.push_str(" | ");
        s.push_str(if app.rep_all() {
            cat.t("prompt.opt.all")
        } else {
            cat.t("prompt.opt.one")
        });
    } else {
        s.push_str("  (Alt+M, Alt+C)");
    }
    s
}

/// La dimensione leggibile del titolo: `N B` sotto 1024, altrimenti due decimali in `KiB`/`MiB`/`GiB`/`TiB`.
pub fn fmt_size(len: u64) -> String {
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

/// Le celle (colonne assolute della riga) coperte dalla parte trovata che stanno nella riga
/// visuale `row` e nella finestra orizzontale: quelle da colorare con lo stile `found`. Le
/// colonne si ricavano dai confini di byte dell'occorrenza con `view::col_of`.
fn found_cells(
    doc: &Document,
    row: &view::Row,
    found: (u64, u64),
    hscroll: usize,
    cells: usize,
    opts: &ViewOptions,
) -> Option<(usize, usize)> {
    // La parte dell'occorrenza che sta nei byte di questa riga (puo' attraversarne piu' d'una
    // o stare tutta altrove).
    let (a, b) = (found.0.max(row.start), found.1.min(row.end));
    if a >= b {
        return None;
    }
    let ca = view::col_of(doc, row.start, a, opts);
    let cb = view::col_of(doc, row.start, b, opts);
    // La finestra orizzontale taglia quello che non si vede.
    let (da, fino) = (ca.max(hscroll), cb.min(hscroll + cells));
    if da < fino {
        Some((da, fino))
    } else {
        None
    }
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
    // Una cella = un carattere; le celle saltate dai caratteri larghi si leggono come spazi. Gli spazi
    // in fondo a ogni riga non si restituiscono: la riga e' il suo contenuto, cosi' chi la guarda
    // (i test, `--dump`) non dipende dalla larghezza dello schermo.
    let mut out = Vec::with_capacity(h as usize);
    for riga in terminal.backend().buffer().content().chunks(w as usize) {
        let mut s = String::with_capacity(w as usize);
        for cella in riga {
            match cella.symbol() {
                "" => s.push(' '),
                simbolo => s.push_str(simbolo),
            }
        }
        out.push(s.trim_end().to_string());
    }
    out
}
