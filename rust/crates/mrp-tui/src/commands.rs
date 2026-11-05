//! Il registro dei comandi: un elenco unico di quello che l'editor sa fare dal menu o da un pulsante, con il nome (una
//! chiave del catalogo), il tasto dell'`App` che lo esegue, la scorciatoia come si scrive e l'icona. Il menu della
//! finestra (e la sua barra degli strumenti) nascono da qui, cosi' il testo, la scorciatoia e il tasto non possono
//! divergere; `enabled` dice quando un comando ha senso (per spegnere la voce o il pulsante), `exec` lo esegue.
//!
//! Lo zoom non ha un tasto dell'`App`: e' della finestra (`exec` lo lascia a lei).

use crate::workspace::Workspace;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Un comando.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Cmd {
    New,
    Open,
    Save,
    SaveAs,
    CloseTab,
    Quit,
    EditMode,
    Undo,
    Redo,
    Cut,
    Copy,
    Paste,
    SelectAll,
    Find,
    Goto,
    Replace,
    Filter,
    ColumnMode,
    Special,
    LineNumbers,
    /// L'indice righe (acceso: numeri di riga, "vai alla riga N"; spegnerlo chiede conferma).
    LineIndex,
    Follow,
    LogColors,
    NextTab,
    PrevTab,
    NavBar,
    Theme,
    Language,
    Encoding,
    Help,
    Home,
    End,
    ZoomIn,
    ZoomOut,
    ZoomReset,
    Settings,
    /// L'intervallo di tempo (da / a) che restringe la vista.
    TimeRange,
    /// Il formato dell'orario scritto nelle righe del file.
    TimeMask,
    /// Toglie l'intervallo di tempo (la vista torna a tutto il file).
    TimeFilter,
    /// La timeline: la barra mappa del file nel tempo accanto alla scrollbar (si accende e si spegne).
    Timeline,
}

/// La scheda di un comando.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Command {
    pub cmd: Cmd,
    /// La chiave del nome nel catalogo (`gui.*`).
    pub label: &'static str,
    /// Il tasto che l'`App` capisce; `None` per i comandi della sola finestra (lo zoom).
    pub key: Option<KeyEvent>,
    /// La scorciatoia come si scrive (`Ctrl+Shift+S`); con un tasto e' `shortcut_text(tasto)`.
    pub shortcut: &'static str,
    /// Il nome dell'icona (la finestra ne ha il disegno).
    pub icon: &'static str,
}

const CTRL: KeyModifiers = KeyModifiers::CONTROL;
const NIENTE: KeyModifiers = KeyModifiers::NONE;
const ALT: KeyModifiers = KeyModifiers::ALT;
const CTRL_MAIUSC: KeyModifiers = KeyModifiers::CONTROL.union(KeyModifiers::SHIFT);

const fn tasto(c: KeyCode, m: KeyModifiers) -> Option<KeyEvent> {
    Some(KeyEvent::new(c, m))
}

const fn comando(
    cmd: Cmd,
    label: &'static str,
    key: Option<KeyEvent>,
    shortcut: &'static str,
    icon: &'static str,
) -> Command {
    Command {
        cmd,
        label,
        key,
        shortcut,
        icon,
    }
}

/// Tutti i comandi, nell'ordine in cui si elencano.
pub const COMMANDS: &[Command] = &[
    comando(
        Cmd::New,
        "gui.new",
        tasto(KeyCode::Char('n'), CTRL),
        "Ctrl+N",
        "new",
    ),
    comando(
        Cmd::Open,
        "gui.open",
        tasto(KeyCode::Char('o'), CTRL),
        "Ctrl+O",
        "open",
    ),
    comando(
        Cmd::Save,
        "gui.save",
        tasto(KeyCode::F(2), NIENTE),
        "F2",
        "save",
    ),
    comando(
        Cmd::SaveAs,
        "gui.save_as",
        tasto(KeyCode::Char('S'), CTRL_MAIUSC),
        "Ctrl+Shift+S",
        "save_as",
    ),
    comando(
        Cmd::CloseTab,
        "gui.close_tab",
        tasto(KeyCode::Char('w'), CTRL),
        "Ctrl+W",
        "close",
    ),
    comando(
        Cmd::Quit,
        "gui.quit",
        tasto(KeyCode::F(10), NIENTE),
        "F10",
        "quit",
    ),
    comando(
        Cmd::EditMode,
        "gui.edit_mode",
        tasto(KeyCode::F(4), NIENTE),
        "F4",
        "edit",
    ),
    comando(
        Cmd::Undo,
        "gui.undo",
        tasto(KeyCode::Char('z'), CTRL),
        "Ctrl+Z",
        "undo",
    ),
    comando(
        Cmd::Redo,
        "gui.redo",
        tasto(KeyCode::Char('y'), CTRL),
        "Ctrl+Y",
        "redo",
    ),
    comando(
        Cmd::Cut,
        "gui.cut",
        tasto(KeyCode::Char('x'), CTRL),
        "Ctrl+X",
        "cut",
    ),
    comando(
        Cmd::Copy,
        "gui.copy",
        tasto(KeyCode::Char('c'), CTRL),
        "Ctrl+C",
        "copy",
    ),
    comando(
        Cmd::Paste,
        "gui.paste",
        tasto(KeyCode::Char('v'), CTRL),
        "Ctrl+V",
        "paste",
    ),
    comando(
        Cmd::SelectAll,
        "gui.select_all",
        tasto(KeyCode::Char('a'), CTRL),
        "Ctrl+A",
        "select_all",
    ),
    comando(
        Cmd::Find,
        "gui.find",
        tasto(KeyCode::Char('f'), CTRL),
        "Ctrl+F",
        "find",
    ),
    comando(
        Cmd::Goto,
        "gui.goto",
        tasto(KeyCode::Char('g'), CTRL),
        "Ctrl+G",
        "goto",
    ),
    comando(
        Cmd::Replace,
        "gui.replace",
        tasto(KeyCode::Char('h'), CTRL),
        "Ctrl+H",
        "replace",
    ),
    comando(
        Cmd::Filter,
        "gui.filter",
        tasto(KeyCode::Char('g'), ALT),
        "Alt+G",
        "filter",
    ),
    comando(
        Cmd::ColumnMode,
        "gui.column_mode",
        tasto(KeyCode::F(8), NIENTE),
        "F8",
        "columns",
    ),
    comando(
        Cmd::Special,
        "gui.special",
        tasto(KeyCode::Char('w'), ALT),
        "Alt+W",
        "special",
    ),
    comando(
        Cmd::LineNumbers,
        "gui.line_numbers",
        tasto(KeyCode::Char('n'), ALT),
        "Alt+N",
        "line_numbers",
    ),
    comando(
        Cmd::LineIndex,
        "gui.line_index",
        tasto(KeyCode::Char('x'), ALT),
        "Alt+X",
        "line_index",
    ),
    comando(
        Cmd::Follow,
        "gui.follow",
        tasto(KeyCode::Char('f'), ALT),
        "Alt+F",
        "follow",
    ),
    comando(
        Cmd::LogColors,
        "gui.log_colors",
        tasto(KeyCode::Char('l'), ALT),
        "Alt+L",
        "log_colors",
    ),
    comando(
        Cmd::TimeRange,
        "gui.time_range",
        tasto(KeyCode::Char('t'), ALT),
        "Alt+T",
        "time_range",
    ),
    comando(
        Cmd::TimeMask,
        "gui.time_mask",
        tasto(KeyCode::Char('o'), ALT),
        "Alt+O",
        "time_mask",
    ),
    comando(Cmd::TimeFilter, "gui.time_filter", None, "", "time_filter"),
    comando(
        Cmd::Timeline,
        "gui.timeline",
        tasto(KeyCode::Char('b'), ALT),
        "Alt+B",
        "timeline",
    ),
    comando(
        Cmd::NextTab,
        "gui.next_tab",
        tasto(KeyCode::PageDown, CTRL),
        "Ctrl+PgDn",
        "next_tab",
    ),
    comando(
        Cmd::PrevTab,
        "gui.prev_tab",
        tasto(KeyCode::PageUp, CTRL),
        "Ctrl+PgUp",
        "prev_tab",
    ),
    comando(
        Cmd::NavBar,
        "gui.nav_bar",
        tasto(KeyCode::Char('b'), CTRL),
        "Ctrl+B",
        "jump_bar",
    ),
    comando(
        Cmd::Theme,
        "gui.theme",
        tasto(KeyCode::F(6), NIENTE),
        "F6",
        "theme",
    ),
    comando(
        Cmd::Language,
        "gui.language",
        tasto(KeyCode::F(7), NIENTE),
        "F7",
        "language",
    ),
    comando(
        Cmd::Encoding,
        "gui.encoding",
        tasto(KeyCode::F(9), NIENTE),
        "F9",
        "encoding",
    ),
    comando(
        Cmd::Help,
        "gui.help",
        tasto(KeyCode::F(1), NIENTE),
        "F1",
        "help",
    ),
    comando(
        Cmd::Home,
        "gui.home",
        tasto(KeyCode::Home, CTRL),
        "Ctrl+Home",
        "home",
    ),
    comando(
        Cmd::End,
        "gui.end",
        tasto(KeyCode::End, CTRL),
        "Ctrl+End",
        "end",
    ),
    comando(Cmd::ZoomIn, "gui.zoom_in", None, "Ctrl++", "zoom_in"),
    comando(Cmd::ZoomOut, "gui.zoom_out", None, "Ctrl+-", "zoom_out"),
    comando(
        Cmd::ZoomReset,
        "gui.zoom_reset",
        None,
        "Ctrl+0",
        "zoom_reset",
    ),
    comando(Cmd::Settings, "gui.settings", None, "Ctrl+,", "settings"),
];

impl Cmd {
    /// La scheda di questo comando.
    pub fn info(self) -> &'static Command {
        COMMANDS
            .iter()
            .find(|c| c.cmd == self)
            .expect("ogni comando e' nell'elenco")
    }
}

/// Come si scrive la scorciatoia di un tasto: `Ctrl+Shift+S`, `F2`, `Ctrl+PgDn` (Ctrl, Alt, Maiusc, poi il tasto).
pub fn shortcut_text(k: KeyEvent) -> String {
    let mut s = String::new();
    if k.modifiers.contains(KeyModifiers::CONTROL) {
        s.push_str("Ctrl+");
    }
    if k.modifiers.contains(KeyModifiers::ALT) {
        s.push_str("Alt+");
    }
    if k.modifiers.contains(KeyModifiers::SHIFT) {
        s.push_str("Shift+");
    }
    match k.code {
        KeyCode::F(n) => s.push_str(&format!("F{n}")),
        KeyCode::PageUp => s.push_str("PgUp"),
        KeyCode::PageDown => s.push_str("PgDn"),
        KeyCode::Char(c) => s.push(c.to_ascii_uppercase()),
        KeyCode::Enter => s.push_str("Enter"),
        KeyCode::Esc => s.push_str("Esc"),
        KeyCode::Tab => s.push_str("Tab"),
        KeyCode::Home => s.push_str("Home"),
        KeyCode::End => s.push_str("End"),
        altro => s.push_str(&format!("{altro:?}")),
    }
    s
}

/// Il comando che ha questo tasto (si guardano il tasto e i modificatori, non il tipo di evento).
pub fn from_key(k: KeyEvent) -> Option<Cmd> {
    COMMANDS
        .iter()
        .find(|c| {
            c.key
                .is_some_and(|t| t.code == k.code && t.modifiers == k.modifiers)
        })
        .map(|c| c.cmd)
}

/// Il comando ha senso adesso? Salva se c'e' qualcosa da salvare; annulla e ripeti in modifica con una storia; copia con una
/// selezione; taglia con una selezione in modifica; incolla in modifica; le schede se ce n'e' piu' di una.
pub fn enabled(ws: &Workspace, cmd: Cmd) -> bool {
    let a = ws.active();
    match cmd {
        Cmd::Save => a.is_modified(),
        Cmd::Undo => a.is_edit() && a.can_undo(),
        Cmd::Redo => a.is_edit() && a.can_redo(),
        Cmd::Copy => a.has_selection(),
        Cmd::Cut => a.is_edit() && a.has_selection(),
        Cmd::Paste => a.is_edit(),
        Cmd::NextTab | Cmd::PrevTab => ws.len() > 1,
        _ => true,
    }
}
