//! La barra dei menu: ogni voce e' un'`Azione`, cioe' un tasto che l'`App` del TUI gia' conosce (cosi' menu e tastiera
//! fanno la stessa cosa), l'apertura di un file o lo zoom. La struttura (`voci`) e' dati puri, quindi provabile senza
//! finestra; `disegna` la mostra con egui.

use eframe::egui::{self, Ui};
use mrp_tui::app::App;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Cosa fa una voce di menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Azione {
    /// Sceglie un file e lo apre (il dialogo nativo).
    Apri,
    /// Manda questo tasto all'`App`.
    Tasto(KeyEvent),
    ZoomPiu,
    ZoomMeno,
    ZoomNormale,
}

/// Una voce: la chiave del testo (`gui.*` nel catalogo), la scorciatoia da mostrare, l'azione.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Voce {
    pub chiave: &'static str,
    pub scorciatoia: &'static str,
    pub azione: Azione,
}

/// Un menu: la chiave del titolo e le voci; una `None` e' un separatore.
pub type Menu = (&'static str, &'static [Option<Voce>]);

const fn tasto(c: KeyCode, m: KeyModifiers) -> Azione {
    Azione::Tasto(KeyEvent::new(c, m))
}

const fn voce(chiave: &'static str, scorciatoia: &'static str, azione: Azione) -> Option<Voce> {
    Some(Voce {
        chiave,
        scorciatoia,
        azione,
    })
}

const CTRL: KeyModifiers = KeyModifiers::CONTROL;
const NIENTE: KeyModifiers = KeyModifiers::NONE;

const FILE: &[Option<Voce>] = &[
    voce("gui.open", "Ctrl+O", Azione::Apri),
    voce("gui.save", "F2", tasto(KeyCode::F(2), NIENTE)),
    voce("gui.close_tab", "Ctrl+W", tasto(KeyCode::Char('w'), CTRL)),
    None,
    voce("gui.quit", "F10", tasto(KeyCode::F(10), NIENTE)),
];

const MODIFICA: &[Option<Voce>] = &[
    voce("gui.edit_mode", "F4", tasto(KeyCode::F(4), NIENTE)),
    None,
    voce("gui.undo", "Ctrl+Z", tasto(KeyCode::Char('z'), CTRL)),
    voce("gui.redo", "Ctrl+Y", tasto(KeyCode::Char('y'), CTRL)),
    None,
    voce("gui.cut", "Ctrl+X", tasto(KeyCode::Char('x'), CTRL)),
    voce("gui.copy", "Ctrl+C", tasto(KeyCode::Char('c'), CTRL)),
    voce("gui.paste", "Ctrl+V", tasto(KeyCode::Char('v'), CTRL)),
    voce("gui.select_all", "Ctrl+A", tasto(KeyCode::Char('a'), CTRL)),
    None,
    voce("gui.find", "Ctrl+F", tasto(KeyCode::Char('f'), CTRL)),
    voce("gui.goto", "Ctrl+G", tasto(KeyCode::Char('g'), CTRL)),
];

const VISTA: &[Option<Voce>] = &[
    voce("gui.next_tab", "Ctrl+PgDn", tasto(KeyCode::PageDown, CTRL)),
    voce("gui.prev_tab", "Ctrl+PgUp", tasto(KeyCode::PageUp, CTRL)),
    None,
    voce("gui.column_mode", "F8", tasto(KeyCode::F(8), NIENTE)),
    None,
    voce("gui.theme", "F6", tasto(KeyCode::F(6), NIENTE)),
    voce("gui.language", "F7", tasto(KeyCode::F(7), NIENTE)),
    voce("gui.encoding", "F9", tasto(KeyCode::F(9), NIENTE)),
    None,
    voce("gui.zoom_in", "Ctrl++", Azione::ZoomPiu),
    voce("gui.zoom_out", "Ctrl+-", Azione::ZoomMeno),
    voce("gui.zoom_reset", "Ctrl+0", Azione::ZoomNormale),
];

const AIUTO: &[Option<Voce>] = &[voce("gui.help", "F1", tasto(KeyCode::F(1), NIENTE))];

/// I menu della barra, nell'ordine in cui compaiono.
pub const MENU: [Menu; 4] = [
    ("gui.menu.file", FILE),
    ("gui.menu.edit", MODIFICA),
    ("gui.menu.view", VISTA),
    ("gui.menu.help", AIUTO),
];

/// Disegna la barra dei menu e restituisce l'azione scelta in questo giro, se c'e'. Il testo viene dal catalogo
/// dell'`App` (la lingua attiva).
pub fn disegna(ui: &mut Ui, app: &App) -> Option<Azione> {
    let mut scelta = None;
    egui::MenuBar::new().ui(ui, |ui| {
        for (titolo, voci) in MENU {
            ui.menu_button(app.text(titolo), |ui| {
                for v in voci {
                    match v {
                        None => {
                            ui.separator();
                        }
                        Some(v) => {
                            let b =
                                egui::Button::new(app.text(v.chiave)).shortcut_text(v.scorciatoia);
                            if ui.add(b).clicked() {
                                scelta = Some(v.azione);
                                ui.close();
                            }
                        }
                    }
                }
            });
        }
    });
    scelta
}
