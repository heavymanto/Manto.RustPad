//! Il disegno del TUI: legge l'`App` e scrive su un `Frame` (layout in `docs/TEAM/DESIGN_TUI.md`, sezione 2).

use crate::app::timeline_pannello::{Pannello, RigaPannello, BARRA_W, ETICHETTA_W};
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

/// Le righe dell'aiuto, in ordine: le chiavi `help.l1`..`help.l26` (senza la 21) (l'ultima e' sempre Esc, `help.l10`). L'aiuto
/// scorre quando lo schermo e' piu' basso (`App::help_scroll`).
pub(crate) const HELP_KEYS: [&str; 25] = [
    "help.l1", "help.l2", "help.l3", "help.l4", "help.l5", "help.l6", "help.l7", "help.l8",
    "help.l9", "help.l11", "help.l12", "help.l13", "help.l14", "help.l15", "help.l16", "help.l17",
    "help.l18", "help.l19", "help.l20", "help.l22", "help.l23", "help.l24", "help.l25", "help.l26",
    "help.l10",
];

/// Disegna tutto lo schermo e ricorda dove sta il cursore (`App::cursor_xy`).
pub fn draw(f: &mut Frame, app: &mut App) {
    let t = Theme::for_id(app.theme_id());
    let area = f.area();
    // Il fondo del tema su tutto lo schermo.
    f.render_widget(Block::default().style(t.screen), area);
    let (w, h) = (area.width, area.height);
    // con la cornice il contratto chiede h >= 3 (almeno una riga di testo, stato, tasti); senza basta una riga
    let minima = if app.chrome() { 3 } else { 1 };
    if w == 0 || h < minima {
        app.set_cursor_xy(None);
        return;
    }
    draw_text(f, app, &t, w, h);
    if app.chrome() {
        draw_timeline(f, app, &t, h);
        draw_status(f, app, &t, w, h);
        draw_footer(f, app, &t, w, h);
    }
    // senza la cornice del terminale i dialoghi che hanno una vista (`App::dialog_view`) li disegna la finestra
    if app.chrome() || !app.dialog_has_view() {
        draw_dialog(f, app, &t, w, h);
    }
}

/// Righe 0..=h-3: il testo con il gutter dei numeri di riga; imposta anche il cursore del terminale.
/// La parte trovata dalla ricerca si disegna con lo stile `found` e la selezione con lo stile `selection`,
/// che vince sul trovato (in entrambi i casi solo le celle visibili, mai il gutter). Senza documento (un
/// salvataggio lo tiene nel suo thread) ridisegna le righe dell'ultimo disegno.
fn draw_text(f: &mut Frame, app: &mut App, t: &Theme, w: u16, h: u16) {
    // la riga da cui comincia il testo e quante ne sono (con la cornice le altre due sono stato e tasti)
    let sopra = app.top_offset();
    let text_rows = if app.chrome() {
        (h as usize).saturating_sub(2)
    } else {
        h as usize
    };
    let gutter_w = app.gutter_w();
    // la timeline prende una colonna di schermo (a sinistra o a destra): il testo sta nel resto, da `x0`
    let (x0, larghezza) = (app.text_x0(), w - app.timeline_panel_w());
    let cells = (larghezza as usize).saturating_sub(gutter_w);
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
        // senza i numeri di riga il margine non c'e' proprio (ne' i numeri ne' il filetto)
        let gutter = if gutter_w == 0 {
            String::new()
        } else {
            let mut g = match num {
                Some(n) => format!("{:>width$}", n, width = gutter_w - 1),
                None => " ".repeat(gutter_w - 1),
            };
            g.push('│');
            g
        };
        let mut text = row.text.clone();
        if row.more_left {
            replace_first(&mut text, '«');
        }
        if row.more_right {
            replace_last(&mut text, '»');
        }
        if xy.is_none() && row.start == cursor_row && col >= hscroll && col - hscroll < cells {
            xy = Some((
                x0 + gutter_w as u16 + (col - hscroll) as u16,
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
    // gli orari si colorano solo dove li legge la maschera del file (riconosciuta o scelta): senza maschera, niente
    let maschera = if app.log_colors() {
        app.time_mask_for_colors()
    } else {
        None
    };
    for (i, (gutter, text)) in righe.iter().enumerate() {
        let y = sopra + i as u16;
        // Ogni riga arriva fino in fondo allo schermo.
        let mut text = text.clone();
        // La colorazione dei log (Alt+L): il colore della riga (errori e avvisi) e le celle dei livelli, degli orari e degli
        // indirizzi; si calcola sul testo visibile (con lo scorrimento orizzontale una parola tagliata non si riconosce).
        let mut base = t.fg;
        let mut celle_log: Vec<(usize, usize, ratatui::style::Style)> = Vec::new();
        if app.log_colors() {
            use mrp_core::loghl::{self, Kind};
            match loghl::line_level(&text) {
                Some(Kind::Error) => base = base.patch(t.log.error),
                Some(Kind::Warn) => base = base.patch(t.log.warn),
                _ => {}
            }
            for sp in loghl::highlight(&text) {
                if sp.kind == Kind::Time {
                    // gli orari li decide la maschera, non la forma
                    continue;
                }
                let stile = match sp.kind {
                    Kind::Error => t.log.error,
                    Kind::Warn => t.log.warn,
                    Kind::Info => t.log.info,
                    Kind::Debug => t.log.debug,
                    Kind::Time => t.log.time,
                    Kind::Ip => t.log.ip,
                };
                let da = Span::raw(&text[..sp.start]).width();
                let fino = Span::raw(&text[..sp.end]).width();
                celle_log.push((da, fino, stile));
            }
            if let Some((da, fino, _)) = maschera.as_ref().and_then(|m| m.find(text.as_bytes())) {
                if let (Some(prima), Some(fino)) = (text.get(..da), text.get(..fino)) {
                    celle_log.push((
                        Span::raw(prima).width(),
                        Span::raw(fino).width(),
                        t.log.time,
                    ));
                }
            }
        }
        let manca = cells.saturating_sub(text.chars().count());
        text.push_str(&" ".repeat(manca));
        let line = Line::from(vec![
            Span::styled(gutter.clone(), t.gutter),
            Span::styled(text, base),
        ]);
        f.render_widget(
            Paragraph::new(line).style(t.screen),
            Rect::new(x0, y, larghezza, 1),
        );
        for (da, fino, stile) in celle_log {
            for colonna in da..fino.min(cells) {
                let x = x0 + gutter_w as u16 + colonna as u16;
                f.buffer_mut()[(x, y)].set_style(stile);
            }
        }
        // La parte trovata dall'ultima ricerca, se questa riga la contiene: cambia solo lo
        // stile delle celle visibili (il testo no, e il gutter mai); una occorrenza che
        // attraversa piu' righe visuali o esce dalla finestra orizzontale si taglia qui.
        if let (Some(doc), Some(row), Some(found)) = (app.document(), rows.get(i), app.found()) {
            if let Some((da, fino)) = found_cells(doc, row, found, hscroll, cells, &opts) {
                for colonna in da..fino {
                    let x = x0 + gutter_w as u16 + (colonna - hscroll) as u16;
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
                    let x = x0 + gutter_w as u16 + (colonna - hscroll) as u16;
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
                                let x = x0 + gutter_w as u16 + (colonna - hscroll) as u16;
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

/// Le colonne della legenda dei livelli nell'intestazione del pannello: quattro volte la forma e la lettera.
const LEGENDA_W: usize = 8;

/// Il testo ridotto a `larghezza` colonne esatte: tagliato se e' piu' lungo, completato con spazi se e' piu' corto.
fn in_colonne(testo: &str, larghezza: usize) -> String {
    let mut fuori = String::new();
    let mut usate = 0;
    for c in testo.chars() {
        let w = Span::raw(c.to_string()).width();
        if usate + w > larghezza {
            break;
        }
        fuori.push(c);
        usate += w;
    }
    fuori.push_str(&" ".repeat(larghezza - usate));
    fuori
}

/// La colonna della timeline (se c'e'), accanto al testo: l'intestazione, e sotto una riga per periodo (la data e la barra divisa
/// fra i livelli) o il messaggio che dice perche' non ci sono. Il filetto sta dal lato del testo ed e' in risalto dove passa il
/// tratto di file che si legge. Le forme della barra distinguono i livelli anche senza i colori (`█` errori, `▓` avvisi, `▒`
/// notizie, `░` dettagli, `▒` fioco quelli non ancora contati).
fn draw_timeline(f: &mut Frame, app: &mut App, t: &Theme, h: u16) {
    let larghezza = app.timeline_panel_w();
    if larghezza == 0 {
        return;
    }
    let Some(corpo) = app.timeline_panel() else {
        return;
    };
    let x = app.timeline_panel_x();
    let sopra = app.top_offset();
    let alte = usize::from(h).saturating_sub(2);
    let dentro = usize::from(larghezza) - 1;
    let titolo = app.timeline_panel_title();
    // dei colori dei livelli si tiene il primo piano: lo sfondo e' quello dello schermo
    let primo_piano = |s: Style| s.fg.map_or_else(Style::default, |c| Style::default().fg(c));
    let barre: [(&str, Style); 5] = [
        ("█", primo_piano(t.log.error)),
        ("▓", primo_piano(t.log.warn)),
        ("▒", primo_piano(t.log.info)),
        ("░", primo_piano(t.log.debug)),
        ("▒", t.gutter),
    ];
    let riga_periodo = |r: &RigaPannello| -> Vec<Span<'static>> {
        let stile_data = if r.in_vista { t.title } else { t.gutter };
        let mut spans = vec![
            Span::styled(in_colonne(&r.etichetta, ETICHETTA_W), stile_data),
            Span::raw(" "),
        ];
        let mut usate = 0;
        for (k, &n) in r.celle.iter().enumerate() {
            if n > 0 {
                spans.push(Span::styled(barre[k].0.repeat(n), barre[k].1));
                usate += n;
            }
        }
        spans.push(Span::raw(" ".repeat(BARRA_W - usate)));
        spans
    };
    for i in 0..alte {
        let (spans, in_vista): (Vec<Span<'static>>, bool) = if i == 0 {
            // il nome a sinistra e, a destra, la legenda dei livelli: la forma e il colore di ciascuno con la sua lettera
            // (le stesse E W I D dei pulsanti della finestra)
            let mut intestazione = vec![Span::styled(
                in_colonne(&titolo, dentro - LEGENDA_W),
                t.status,
            )];
            for (k, lettera) in ["E", "W", "I", "D"].into_iter().enumerate() {
                intestazione.push(Span::styled(format!("{}{lettera}", barre[k].0), barre[k].1));
            }
            (intestazione, false)
        } else {
            match &corpo {
                Pannello::Testo(righe) => {
                    let s = righe.get(i - 1).map_or("", String::as_str);
                    (
                        vec![Span::styled(in_colonne(&format!(" {s}"), dentro), t.hint)],
                        false,
                    )
                }
                Pannello::Righe(righe) => match righe.get(i - 1) {
                    Some(r) => (riga_periodo(r), r.in_vista),
                    None => (vec![Span::raw(" ".repeat(dentro))], false),
                },
            }
        };
        let filetto = if in_vista {
            Span::styled("┃", t.title)
        } else {
            Span::styled("│", t.gutter)
        };
        let mut celle = spans;
        // il filetto sta dal lato del testo
        if x > 0 {
            celle.insert(0, filetto);
        } else {
            celle.push(filetto);
        }
        f.render_widget(
            Paragraph::new(Line::from(celle)).style(t.screen),
            Rect::new(x, sopra + i as u16, larghezza, 1),
        );
    }
}

/// Gli spazi che restano almeno fra la sinistra e la destra della riga di stato.
const GAP_STATO: usize = 3;

/// Riga h-2: a sinistra il messaggio transitorio (con uno spazio davanti) o la posizione del cursore; a destra, separati
/// da due spazi e con uno spazio finale: il lavoro in corso (`status.saving`, `status.searching` o `status.indexing` con la
/// percentuale), il nome del file (con ` *` se e' modificato), la dimensione, la codifica se non e' UTF-8, lo stile di fine
/// riga se non e' LF e, per ultimo e col suo colore, il modo (lettura, modifica, segui; colonna). Quello che non ci sta cade
/// (vedi `destra_stato`).
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
    // Il lavoro in corso: il salvataggio (quando c'e', e' quello che tiene lo schermo congelato), la ricerca o l'indicizzazione.
    let lavoro = if let Some(p) = app.save_progress() {
        Some(app.catalog().tf("status.saving", &[&p.to_string()]))
    } else if let Some(p) = app.search_progress() {
        Some(app.catalog().tf("status.searching", &[&p.to_string()]))
    } else {
        app.index_progress()
            .map(|p| app.catalog().tf("status.indexing", &[&p.to_string()]))
    };
    let mut nome = app.file_name().to_string();
    if app.is_modified() {
        nome.push_str(" *");
    }
    // Dopo il nome: la dimensione e, solo se non e' la solita e la riga non e' di un messaggio transitorio (che la occupa per
    // conto suo), la codifica e lo stile di fine riga.
    let mut dopo = vec![fmt_size(app.document_len())];
    if flash.is_none() && !app.encoding().is_utf8() {
        dopo.push(app.encoding().name().to_string());
    }
    if flash.is_none() && app.line_ending() != mrp_core::lineending::LineEnding::Lf {
        dopo.push(app.line_ending().label().to_string());
    }
    let mode_key = if app.is_edit() {
        "mode.edit"
    } else if app.follow() {
        "mode.follow"
    } else {
        "mode.view"
    };
    let mut modo = app.catalog().t(mode_key).to_string();
    // La modalita' colonna si dice dopo il modo e con un solo spazio in mezzo ("VIEW COL" / "EDIT COL").
    if app.column_mode() {
        modo.push(' ');
        modo.push_str(app.catalog().t("mode.column"));
    }
    let mode_style = if app.is_edit() {
        t.mode_edit
    } else {
        t.mode_view
    };
    let (destra, modo) = destra_stato(
        w as usize,
        left.chars().count(),
        lavoro.into_iter().collect(),
        nome,
        dopo,
        modo,
    );
    let pad = (w as usize).saturating_sub(
        left.chars().count()
            + destra.chars().count()
            + modo.as_ref().map_or(0, |m| m.chars().count() + 1),
    );
    let mut pezzi = vec![
        Span::styled(left, t.status),
        Span::raw(" ".repeat(pad)),
        Span::styled(destra, t.status),
    ];
    if let Some(modo) = modo {
        pezzi.push(Span::styled(modo, mode_style));
        pezzi.push(Span::raw(" "));
    }
    let line = Line::from(pezzi);
    f.render_widget(
        Paragraph::new(line).style(t.screen),
        Rect::new(0, h - 2, w, 1),
    );
}

/// La parte destra della riga di stato di uno schermo largo `w` con una sinistra larga `sinistra` celle: il testo dei pezzi
/// (`lavoro in corso`, `nome`, poi `dopo`: dimensione, codifica, fine riga) ciascuno con due spazi dopo, e il modo, che va per ultimo
/// col suo colore (`None` se non c'e' posto). Fra la sinistra e la destra restano almeno `GAP_STATO` spazi; quello che non ci sta
/// cade, nell'ordine: il nome (prima accorciato, tenendo la fine, con `…` davanti; sotto gli 8 caratteri sparisce), `dopo`, il modo,
/// il lavoro in corso. A sinistra non si toglie niente.
#[doc(hidden)]
pub fn destra_stato(
    w: usize,
    sinistra: usize,
    mut prima: Vec<String>,
    nome: String,
    mut dopo: Vec<String>,
    modo: String,
) -> (String, Option<String>) {
    let larga = |pezzi: &[String]| pezzi.iter().map(|s| s.chars().count() + 2).sum::<usize>();
    let spazio = w.saturating_sub(sinistra + GAP_STATO);
    let con_modo = modo.chars().count() + 1;
    let libero = spazio.saturating_sub(larga(&prima) + larga(&dopo) + con_modo + 2);
    let nome = if nome.chars().count() <= libero {
        Some(nome)
    } else if libero >= 8 {
        let coda: Vec<char> = nome.chars().rev().take(libero - 1).collect();
        Some(format!("…{}", coda.into_iter().rev().collect::<String>()))
    } else {
        None
    };
    let mut modo = Some(modo);
    if nome.is_none() && larga(&prima) + larga(&dopo) + con_modo > spazio {
        dopo.clear();
    }
    if nome.is_none() && larga(&prima) + con_modo > spazio {
        modo = None;
    }
    if nome.is_none() && modo.is_none() && larga(&prima) > spazio {
        prima.clear();
    }
    let mut destra = String::new();
    for pezzo in prima.iter().chain(nome.iter()).chain(dopo.iter()) {
        destra.push_str(pezzo);
        destra.push_str("  ");
    }
    (destra, modo)
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
    let mut voci = vec![
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
    let largo = |voci: &[(&str, &str)], sep: usize| -> usize {
        1 + voci
            .iter()
            .map(|(tasto, chiave)| Span::raw(*tasto).width() + 1 + Span::raw(cat.t(chiave)).width())
            .sum::<usize>()
            + sep * (voci.len() - 1)
    };
    // Dopo F10, se lo schermo e' largo abbastanza (a 80 colonne non c'e' posto): scartare le modifiche (solo quando ce ne sono) e
    // la timeline. Ognuna entra solo se ci sta con lo stesso separatore che hanno le voci di base (le voci in piu' non fanno
    // stringere gli spazi alle altre: con spazio di sobra restano due spazi).
    let sep_base = if largo(&voci, 2) <= w as usize { 2 } else { 1 };
    let mut in_piu = Vec::new();
    if app.is_modified() && !app.is_untitled() {
        in_piu.push(("Ctrl+R", "footer.revert"));
    }
    in_piu.push(("Alt+B", "footer.timeline"));
    for voce in in_piu {
        voci.push(voce);
        if largo(&voci, sep_base) > w as usize {
            voci.pop();
        }
    }
    let sep = if largo(&voci, 2) > w as usize && largo(&voci, 1) <= w as usize {
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
            // le righe che stanno nello schermo, a partire da quella scelta scorrendo; sotto, sempre, come si chiude
            let (prima, quante) = (app.help_scroll(), app.help_visible());
            // i tasti e le descrizioni stanno in due colonne: la larghezza della prima e' quella del tasto piu' lungo
            // di tutta la tabella (non solo delle righe in vista), cosi' scorrendo la colonna non si muove
            let tabella = app.help_rows();
            let larga_tasti = tabella
                .iter()
                .map(|(k, _)| k.chars().count())
                .max()
                .unwrap_or(0);
            let mut righe: Vec<(String, Style)> = tabella[prima..prima + quante]
                .iter()
                .map(|(k, d)| {
                    let testo = if d.is_empty() {
                        k.clone()
                    } else {
                        format!("{:<larga_tasti$}  {}", k, d)
                    };
                    (testo, t.dialog)
                })
                .collect();
            let mut chiudi = cat.t("help.close").to_string();
            if quante < HELP_KEYS.len() {
                chiudi.push_str("   ");
                chiudi.push_str(cat.t("help.scroll"));
            }
            righe.push((chiudi, t.hint));
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
        // I salti: una riga per numero, quella col fuoco col cursore
        DialogKind::Jumps => {
            let etichette = [
                cat.t("jump.label1").to_string(),
                cat.t("jump.label2").to_string(),
                cat.t("jump.label3").to_string(),
            ];
            let larghezza = etichette
                .iter()
                .map(|e| e.chars().count())
                .max()
                .unwrap_or(0);
            let campo = usize::from(app.jump_field());
            let mut righe = vec![(cat.t("jump.hint").to_string(), t.hint)];
            for (i, e) in etichette.iter().enumerate() {
                let pad = larghezza - e.chars().count();
                righe.push((
                    format!("{}{} > {}", e, " ".repeat(pad), app.jump_value(i)),
                    t.dialog,
                ));
            }
            righe.push((cat.t("prompt.keys").to_string(), t.hint));
            let colonna = larghezza + 3 + app.jump_value(campo).chars().count();
            ContenutoDialogo {
                titolo: cat.t("jump.title").to_string(),
                righe,
                cursore: Some((1 + campo, colonna)),
            }
        }
        // Spegnere l'indice righe: la spiegazione, le funzioni che non ci saranno piu' e la nota; Invio conferma, Esc rinuncia
        DialogKind::IndexOff => {
            let mut righe = vec![(cat.t("index_off.text").to_string(), t.dialog)];
            for k in ["f1", "f2", "f3", "f4", "f5"] {
                righe.push((
                    format!("  - {}", cat.t(&format!("index_off.{k}"))),
                    t.dialog,
                ));
            }
            righe.push((cat.t("index_off.note").to_string(), t.hint));
            righe.push((cat.t("overwrite.keys").to_string(), t.hint));
            ContenutoDialogo {
                titolo: cat.t("index_off.title").to_string(),
                righe,
                cursore: None,
            }
        }
        // L'intervallo di tempo: il suggerimento, i due campi (da, a; il fuoco col cursore), l'errore e i tasti
        DialogKind::TimeRange => {
            let etichette = [
                cat.t("time.range.from").to_string(),
                cat.t("time.range.to").to_string(),
            ];
            let larghezza = etichette
                .iter()
                .map(|e| e.chars().count())
                .max()
                .unwrap_or(0);
            let campo = usize::from(app.time_range_field());
            let mut righe = vec![(cat.t("time.range.hint").to_string(), t.hint)];
            for (i, e) in etichette.iter().enumerate() {
                let pad = larghezza - e.chars().count();
                righe.push((
                    format!("{}{} > {}", e, " ".repeat(pad), app.time_range_text(i)),
                    t.dialog,
                ));
            }
            if !app.time_range_error().is_empty() {
                righe.push((app.time_range_error().to_string(), t.status));
            }
            righe.push((cat.t("prompt.keys").to_string(), t.hint));
            let colonna = larghezza + 3 + app.time_range_text(campo).chars().count();
            ContenutoDialogo {
                titolo: cat.t("time.range.title").to_string(),
                righe,
                cursore: Some((1 + campo, colonna)),
            }
        }
        // Il formato dell'orario: il pattern, entro quanti caratteri, le prime righe lette e l'errore
        DialogKind::TimeMask | DialogKind::Time => {
            let etichette = [
                cat.t("time.mask.pattern").to_string(),
                cat.t("time.mask.within").to_string(),
            ];
            let larghezza = etichette
                .iter()
                .map(|e| e.chars().count())
                .max()
                .unwrap_or(0);
            let campo = usize::from(app.time_mask_field());
            let mut righe = vec![(cat.t("time.mask.hint").to_string(), t.hint)];
            for (i, e) in etichette.iter().enumerate() {
                let pad = larghezza - e.chars().count();
                righe.push((
                    format!("{}{} > {}", e, " ".repeat(pad), app.time_mask_text(i)),
                    t.dialog,
                ));
            }
            // il formato noto evidenziato (Alt+N / Alt+P per scorrerli)
            let (noti, scelto) = app.time_presets();
            if let Some((pattern, esempio)) = noti.get(scelto) {
                righe.push((
                    format!(
                        "{}  {pattern}   ({esempio})   Alt+N / Alt+P",
                        cat.t("time.mask.known")
                    ),
                    t.hint,
                ));
            }
            righe.push((cat.t("time.mask.preview").to_string(), t.hint));
            for (testo, letto) in app.time_mask_preview() {
                righe.push((format!("  {testo}  =>  {letto}"), t.dialog));
            }
            if !app.time_mask_error().is_empty() {
                righe.push((app.time_mask_error().to_string(), t.status));
            }
            righe.push((cat.t("prompt.keys").to_string(), t.hint));
            let colonna = larghezza + 3 + app.time_mask_text(campo).chars().count();
            ContenutoDialogo {
                titolo: cat.t("time.mask.title").to_string(),
                righe,
                cursore: Some((1 + campo, colonna)),
            }
        }
        DialogKind::Open => prompt_contenuto(
            cat,
            t,
            "prompt.open.title",
            "prompt.open.hint",
            app.open_text(),
        ),
        // Il prompt di ricerca: uguale al "vai a" (cambia il vocabolario), con in piu' la riga delle opzioni.
        DialogKind::Find => {
            let (titolo, suggerimento) = if app.filter_mode() {
                ("prompt.filter.title", "prompt.filter.hint")
            } else {
                ("prompt.find.title", "prompt.find.hint")
            };
            let mut c = prompt_contenuto(cat, t, titolo, suggerimento, app.find_text());
            let mut opzioni = opzioni_ricerca(app, false);
            if app.filter_mode() {
                opzioni.push_str(&format!(
                    " | Alt+I {} {}",
                    cat.t("filter.invert"),
                    if app.find_invert() { "[x]" } else { "[ ]" }
                ));
            }
            c.righe.insert(2, (opzioni, t.hint));
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
        // La codifica del file da salvare: la lista (scorre come il menu delle codifiche), la casella del BOM e i tasti.
        DialogKind::SaveEncoding => {
            let selezione = app.menu_index().unwrap_or(0);
            let tutte = Encoding::all();
            // fuori dalla lista restano il titolo, il BOM e il suggerimento
            let altezza = (h as usize).saturating_sub(5);
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
            let ha_bom = tutte
                .get(selezione)
                .is_some_and(|e| e.is_utf8() || e.is_utf16() || e.is_utf32());
            if ha_bom {
                let casella = if app.save_encoding_bom() {
                    "[x]"
                } else {
                    "[ ]"
                };
                righe.push((format!("{casella} {}", cat.t("saveenc.bom")), t.dialog));
            }
            righe.push((cat.t("saveenc.hint").to_string(), t.hint));
            ContenutoDialogo {
                titolo: cat.t("saveenc.title").to_string(),
                righe,
                cursore: None,
            }
        }
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
        // Scartare le modifiche non salvate: la domanda col nome del file e i tasti.
        DialogKind::Revert => ContenutoDialogo {
            titolo: cat.t("revert.title").to_string(),
            righe: vec![
                (cat.tf("revert.text", &[app.file_name()]), t.dialog),
                (cat.t("revert.keys").to_string(), t.hint),
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
