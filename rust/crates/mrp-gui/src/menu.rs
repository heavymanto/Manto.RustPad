//! La barra dei menu: ogni voce e' un comando del registro (`mrp_tui::commands`), che porta il nome, la scorciatoia e il
//! tasto; qui c'e' solo la disposizione nei menu. Le voci non attive (per esempio Annulla senza storia) si vedono spente.
//! La struttura (`MENU`, `voce_di`) e' dati puri, quindi provabile senza finestra; `disegna` la mostra con egui.

use eframe::egui::{self, Ui};
use mrp_tui::commands::{self, Cmd};
use mrp_tui::workspace::Workspace;

/// Cosa fa una voce di menu: un comando del registro (le voci non hanno altro).
pub type Azione = Cmd;

/// Una voce: la chiave del testo (`gui.*` nel catalogo), la scorciatoia da mostrare, il comando.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Voce {
    pub chiave: &'static str,
    pub scorciatoia: &'static str,
    pub azione: Cmd,
}

/// La voce di un comando, dalla sua scheda nel registro.
pub fn voce_di(cmd: Cmd) -> Voce {
    let i = cmd.info();
    Voce {
        chiave: i.label,
        scorciatoia: i.shortcut,
        azione: cmd,
    }
}

/// Un menu: la chiave del titolo e le voci; una `None` e' un separatore.
pub type Menu = (&'static str, &'static [Option<Cmd>]);

const FILE: &[Option<Cmd>] = &[
    Some(Cmd::New),
    Some(Cmd::Open),
    Some(Cmd::Save),
    Some(Cmd::SaveAs),
    Some(Cmd::CloseTab),
    None,
    Some(Cmd::Quit),
];

const MODIFICA: &[Option<Cmd>] = &[
    Some(Cmd::EditMode),
    None,
    Some(Cmd::Undo),
    Some(Cmd::Redo),
    None,
    Some(Cmd::Cut),
    Some(Cmd::Copy),
    Some(Cmd::Paste),
    Some(Cmd::SelectAll),
    None,
    Some(Cmd::Find),
    Some(Cmd::Goto),
    Some(Cmd::Replace),
];

const VISTA: &[Option<Cmd>] = &[
    Some(Cmd::NextTab),
    Some(Cmd::PrevTab),
    None,
    Some(Cmd::NavBar),
    None,
    Some(Cmd::ColumnMode),
    None,
    Some(Cmd::Theme),
    Some(Cmd::Language),
    Some(Cmd::Encoding),
    None,
    Some(Cmd::ZoomIn),
    Some(Cmd::ZoomOut),
    Some(Cmd::ZoomReset),
];

const AIUTO: &[Option<Cmd>] = &[Some(Cmd::Help)];

/// I menu della barra, nell'ordine in cui compaiono.
pub const MENU: [Menu; 4] = [
    ("gui.menu.file", FILE),
    ("gui.menu.edit", MODIFICA),
    ("gui.menu.view", VISTA),
    ("gui.menu.help", AIUTO),
];

/// Disegna la barra dei menu e restituisce il comando scelto in questo giro, se c'e'. Il testo viene dal catalogo della
/// scheda attiva (la lingua attiva); le voci che non hanno senso adesso sono spente.
pub fn disegna(ui: &mut Ui, ws: &Workspace) -> Option<Cmd> {
    let app = ws.active();
    let mut scelta = None;
    egui::MenuBar::new().ui(ui, |ui| {
        for (titolo, voci) in MENU {
            ui.menu_button(app.text(titolo), |ui| {
                for v in voci {
                    match v {
                        None => {
                            ui.separator();
                        }
                        Some(cmd) => {
                            let v = voce_di(*cmd);
                            let b =
                                egui::Button::new(app.text(v.chiave)).shortcut_text(v.scorciatoia);
                            if ui.add_enabled(commands::enabled(ws, *cmd), b).clicked() {
                                scelta = Some(*cmd);
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
