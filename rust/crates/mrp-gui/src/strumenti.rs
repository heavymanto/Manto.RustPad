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
        "undo" => "↩",
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
        "follow" => "↓",
        "log_colors" => "≡",
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
        vec![Cmd::EditMode, Cmd::ColumnMode, Cmd::NavBar],
    ]
}

/// Il tooltip di un pulsante: il nome del comando nella lingua attiva e la sua scorciatoia, `nome (scorciatoia)`.
pub fn tooltip(cmd: Cmd, cat: &Catalog) -> String {
    let info = cmd.info();
    format!("{} ({})", cat.t(info.label), info.shortcut)
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
