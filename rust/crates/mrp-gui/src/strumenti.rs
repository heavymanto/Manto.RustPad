//! La barra degli strumenti e la barra dei salti della finestra senza cornice (B3): il carattere di ogni icona (i font
//! incorporati in egui li hanno gia', niente dipendenze nuove), i gruppi di comandi della barra, il tooltip di un
//! pulsante e le voci dei salti di pagina. Sono dati puri, quindi provabili senza finestra; il disegno con egui sta in
//! `gui.rs` e usa questi pezzi.

use eframe::egui::Rect;
use mrp_core::lang::Catalog;
use mrp_tui::commands::Cmd;

/// Il carattere di un'icona, dai font gia' incorporati in egui; `"?"` se l'id non e' noto. Ogni `icon` del registro
/// dei comandi (`mrp_tui::commands::COMMANDS`) e' nella tabella.
pub fn glifo(icona: &str) -> &'static str {
    match icona {
        "new" => "📄",
        "open" => "📂",
        "save" => "💾",
        "save_as" => "📥",
        "close" => "✖",
        "quit" => "⏻",
        "edit" => "✏",
        // la sola lettura (VIEW) ha l'occhio; il menu dei modi non ha un pulsante
        "view" => "👁",
        "mode" => "⇄",
        "undo" => "↩",
        "revert" => "🗑",
        "redo" => "↪",
        "cut" => "✂",
        "copy" => "📑",
        "paste" => "📋",
        "select_all" => "🔲",
        "find" => "🔍",
        "goto" => "➡",
        "replace" => "🔁",
        "columns" => "📊",
        "special" => "¶",
        // il tail ha la freccia che si ferma sulla riga di fondo, che nel font non c'e': si disegna a mano (`coda`); qui un segno qualunque
        "follow" => "↓",
        "log_colors" => "≡",
        "filter" => "▽",
        "line_numbers" => "#",
        "jump_config" => "⚙",
        "settings" => "⚙",
        "next_tab" => "▶",
        "prev_tab" => "◀",
        "jump_bar" => "⏩",
        "theme" => "🎨",
        "language" => "🌐",
        "encoding" => "🔤",
        "help" => "❓",
        "home" => "⏮",
        "end" => "⏭",
        "zoom_in" => "➕",
        "zoom_out" => "➖",
        "zoom_reset" => "◎",
        "copy_path" => "📎",
        "line_index" => "☰",
        "time_range" => "⏱",
        "time_mask" => "🕘",
        // il filtro per data ha l'imbuto, che nel font non c'e': si disegna a mano (`imbuto`); qui un segno qualunque, diverso dagli altri
        "time_filter" => "⏷",
        // la timeline ha l'orologio
        "timeline" => "🕒",
        "calendar" => "📅",
        _ => "?",
    }
}

/// I gruppi di pulsanti della barra degli strumenti, da sinistra a destra: un separatore fra un gruppo e l'altro.
pub fn gruppi() -> Vec<Vec<Cmd>> {
    vec![
        vec![Cmd::New, Cmd::Open, Cmd::Save],
        vec![Cmd::Undo, Cmd::Redo],
        vec![Cmd::Cut, Cmd::Copy, Cmd::Paste],
        vec![Cmd::Find, Cmd::Replace, Cmd::Goto],
        vec![Cmd::Timeline, Cmd::TimeFilter],
        vec![
            Cmd::ViewMode,
            Cmd::EditMode,
            Cmd::Follow,
            Cmd::ColumnMode,
            Cmd::NavBar,
        ],
        vec![Cmd::ZoomOut, Cmd::ZoomIn],
    ]
}

/// Il percorso di un file come lo si legge e lo si copia: senza il prefisso `\\?\` che Windows mette ai percorsi canonici
/// (`\\?\C:\dati\a.log` diventa `C:\dati\a.log`, `\\?\UNC\server\x` diventa `\\server\x`).
pub fn percorso_leggibile(p: &std::path::Path) -> String {
    let s = p.display().to_string();
    if let Some(resto) = s.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{resto}")
    } else if let Some(resto) = s.strip_prefix(r"\\?\") {
        resto.to_string()
    } else {
        s
    }
}

/// Il suggerimento del pulsante del percorso: il percorso completo del file (o, per un documento che non e' ancora un file,
/// che non c'e' niente da copiare) e come si copia.
pub fn tooltip_percorso(percorso: Option<&str>, cat: &Catalog) -> String {
    match percorso {
        Some(p) => format!("{p}\n{}", cat.t("gui.tip.copy_path")),
        None => cat.t("gui.tip.not_saved").to_string(),
    }
}

/// Un testo in grassetto: nei font che abbiamo non c'e' un peso bold, quindi si disegna due volte con mezzo punto di scarto. Vale
/// per qualunque font e tema. Ritorna il rettangolo occupato.
pub fn grassetto(
    ui: &mut eframe::egui::Ui,
    testo: &str,
    colore: eframe::egui::Color32,
) -> eframe::egui::Rect {
    use eframe::egui::{FontSelection, Sense, Vec2, WidgetText};
    let galleria = WidgetText::from(testo).color(colore).into_galley(
        ui,
        Some(eframe::egui::TextWrapMode::Extend),
        f32::INFINITY,
        FontSelection::Default,
    );
    let (rect, _) = ui.allocate_exact_size(galleria.size() + Vec2::new(0.6, 0.0), Sense::hover());
    for dx in [0.0_f32, 0.6] {
        ui.painter()
            .galley(rect.min + Vec2::new(dx, 0.0), galleria.clone(), colore);
    }
    rect
}

/// Il titolo di un riquadro: testo in grassetto su una banda di `fondo` larga quanto il riquadro. Ritorna il rettangolo del testo (la larghezza dice se e' cambiato).
pub fn titolo_banda(
    ui: &mut eframe::egui::Ui,
    testo: &str,
    fondo: eframe::egui::Color32,
    colore: eframe::egui::Color32,
) -> eframe::egui::Rect {
    use eframe::egui::{FontSelection, Sense, Vec2, WidgetText};
    let galleria = WidgetText::from(testo).color(colore).into_galley(
        ui,
        Some(eframe::egui::TextWrapMode::Extend),
        f32::INFINITY,
        FontSelection::Default,
    );
    let (rect, _) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), galleria.size().y + 6.0),
        Sense::hover(),
    );
    ui.painter().rect_filled(rect, 3.0, fondo);
    let punto = rect.min + Vec2::new(6.0, 3.0);
    for dx in [0.0_f32, 0.6] {
        ui.painter()
            .galley(punto + Vec2::new(dx, 0.0), galleria.clone(), colore);
    }
    eframe::egui::Rect::from_min_size(punto, galleria.size())
}

/// Disegna una croce (X) centrata in `c`, di colore `colore`, con i bracci lunghi `mezza` punti dal centro: nel font non c'e' il glifo.
pub fn croce(
    painter: &eframe::egui::Painter,
    c: eframe::egui::Pos2,
    colore: eframe::egui::Color32,
    mezza: f32,
) {
    for (a, b) in [(-1.0_f32, -1.0_f32), (-1.0, 1.0)] {
        painter.line_segment(
            [
                c + eframe::egui::Vec2::new(a * mezza, b * mezza),
                c + eframe::egui::Vec2::new(-a * mezza, -b * mezza),
            ],
            eframe::egui::Stroke::new(1.5_f32, colore),
        );
    }
}

/// Disegna un imbuto (il segno del filtro) centrato in `c`, di colore `colore`, alto circa `alto` punti: nel font non c'e' il glifo.
pub fn imbuto(
    painter: &eframe::egui::Painter,
    c: eframe::egui::Pos2,
    colore: eframe::egui::Color32,
    alto: f32,
) {
    let k = alto / 10.0;
    painter.add(eframe::egui::Shape::convex_polygon(
        vec![
            eframe::egui::Pos2::new(c.x - 5.0 * k, c.y - 4.0 * k),
            eframe::egui::Pos2::new(c.x + 5.0 * k, c.y - 4.0 * k),
            eframe::egui::Pos2::new(c.x + 1.0 * k, c.y + 1.0 * k),
            eframe::egui::Pos2::new(c.x + 1.0 * k, c.y + 5.0 * k),
            eframe::egui::Pos2::new(c.x - 1.0 * k, c.y + 5.0 * k),
            eframe::egui::Pos2::new(c.x - 1.0 * k, c.y + 1.0 * k),
        ],
        colore,
        eframe::egui::Stroke::NONE,
    ));
}

/// Disegna il segno del tail (una freccia verso il basso che si ferma su una riga: "vai in fondo e resta li'") centrato in `c`,
/// di colore `colore`, alto circa `alto` punti: nel font non c'e' il glifo.
pub fn coda(
    painter: &eframe::egui::Painter,
    c: eframe::egui::Pos2,
    colore: eframe::egui::Color32,
    alto: f32,
) {
    use eframe::egui::{Pos2, Shape, Stroke};
    let k = alto / 10.0;
    let tratto = Stroke::new(1.6_f32, colore);
    // l'asta, la punta e la riga di fondo
    painter.line_segment(
        [Pos2::new(c.x, c.y - 5.0 * k), Pos2::new(c.x, c.y + 1.0 * k)],
        tratto,
    );
    painter.add(Shape::convex_polygon(
        vec![
            Pos2::new(c.x - 3.5 * k, c.y - 1.5 * k),
            Pos2::new(c.x + 3.5 * k, c.y - 1.5 * k),
            Pos2::new(c.x, c.y + 2.5 * k),
        ],
        colore,
        Stroke::NONE,
    ));
    painter.line_segment(
        [
            Pos2::new(c.x - 5.0 * k, c.y + 5.0 * k),
            Pos2::new(c.x + 5.0 * k, c.y + 5.0 * k),
        ],
        tratto,
    );
}

/// Il tooltip di un pulsante: il nome del comando nella lingua attiva e la sua scorciatoia, `nome (scorciatoia)`.
pub fn tooltip(cmd: Cmd, cat: &Catalog) -> String {
    // la timeline e il filtro per data non hanno un tasto: il suggerimento dice cosa fa un clic
    match cmd {
        Cmd::Timeline => return cat.t("gui.tip.timeline").to_string(),
        Cmd::TimeFilter => return cat.t("gui.tip.time_filter").to_string(),
        _ => {}
    }
    let info = cmd.info();
    let tasto = crate::menu::scorciatoia(info.cmd);
    if tasto.is_empty() {
        cat.t(info.label).to_string()
    } else {
        format!("{} ({tasto})", cat.t(info.label))
    }
}

/// Cosa fa un pulsante della barra dei salti.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaltoAzione {
    /// L'inizio del file.
    Inizio,
    /// La fine del file.
    Fine,
    /// Salta di `quante` pagine, avanti o indietro (`avanti`).
    Pagine(bool, usize),
}

/// Un pulsante della barra dei salti: l'etichetta e cosa fa al clic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaltoVoce {
    pub etichetta: String,
    pub azione: SaltoAzione,
}

/// Il suggerimento di un pulsante della barra dei salti: cosa fa e con che tasti (nel catalogo, nella lingua attiva). Le pagine
/// dei tre salti delle impostazioni stanno su Alt, Alt+Maiusc e Alt+Ctrl; la pagina sola su PgSu / PgGiu.
pub fn tooltip_salto(voce: &SaltoVoce, pagine: [usize; 3], cat: &Catalog) -> String {
    match voce.azione {
        SaltoAzione::Inizio => cat.t("gui.tip.jump_start").to_string(),
        SaltoAzione::Fine => cat.t("gui.tip.jump_end").to_string(),
        SaltoAzione::Pagine(avanti, n) => {
            let pg = if avanti { "PgDn" } else { "PgUp" };
            let tasti = match pagine.iter().position(|&p| p == n) {
                _ if n == 1 => pg.to_string(),
                Some(0) => format!("Alt+{pg}"),
                Some(1) => format!("Alt+Shift+{pg}"),
                Some(_) => format!("Alt+Ctrl+{pg}"),
                None => pg.to_string(),
            };
            cat.tf(
                if avanti {
                    "gui.tip.jump_forward"
                } else {
                    "gui.tip.jump_back"
                },
                &[&n.to_string(), &tasti],
            )
        }
    }
}

/// Le voci della barra dei salti, da sinistra a destra: l'inizio del file, i salti indietro e quelli avanti (i
/// moltiplicatori delle impostazioni, dal piu' grande al piu' piccolo, piu' la pagina sola), la fine del file.
pub fn salti(pagine: [usize; 3]) -> Vec<SaltoVoce> {
    let [a, b, c] = pagine;
    let voce = |etichetta: String, azione: SaltoAzione| SaltoVoce { etichetta, azione };
    vec![
        voce(glifo("home").to_string(), SaltoAzione::Inizio),
        voce(format!("-{c}"), SaltoAzione::Pagine(false, c)),
        voce(format!("-{b}"), SaltoAzione::Pagine(false, b)),
        voce(format!("-{a}"), SaltoAzione::Pagine(false, a)),
        voce("-1".to_string(), SaltoAzione::Pagine(false, 1)),
        voce("+1".to_string(), SaltoAzione::Pagine(true, 1)),
        voce(format!("+{a}"), SaltoAzione::Pagine(true, a)),
        voce(format!("+{b}"), SaltoAzione::Pagine(true, b)),
        voce(format!("+{c}"), SaltoAzione::Pagine(true, c)),
        voce(glifo("end").to_string(), SaltoAzione::Fine),
    ]
}

/// Un pulsante della barra degli strumenti nell'ultimo disegno: dove sta, che comando fa, se era attivo (spento quando
/// il comando non ha senso) e se era acceso (i toggle: modifica, colonna, barra dei salti).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pulsante {
    pub rect: Rect,
    pub cmd: Cmd,
    pub attivo: bool,
    pub acceso: bool,
}
