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

/// La scorciatoia di un comando come la scrive la finestra: quella del registro, tranne Trova (F3), Sostituisci (F4) e
/// Lettura / modifica (F12), che nella finestra hanno i tasti standard (`input::tasto` li traduce).
pub fn scorciatoia(cmd: Cmd) -> &'static str {
    match cmd {
        Cmd::Find => "F3",
        Cmd::Replace => "F4",
        Cmd::EditMode => "F12",
        _ => cmd.info().shortcut,
    }
}

/// La voce di un comando, dalla sua scheda nel registro.
pub fn voce_di(cmd: Cmd) -> Voce {
    let i = cmd.info();
    Voce {
        chiave: i.label,
        scorciatoia: scorciatoia(cmd),
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
    Some(Cmd::Settings),
    None,
    Some(Cmd::NextTab),
    Some(Cmd::PrevTab),
    None,
    Some(Cmd::NavBar),
    None,
    Some(Cmd::ColumnMode),
    Some(Cmd::Special),
    Some(Cmd::LineNumbers),
    Some(Cmd::LineIndex),
    None,
    Some(Cmd::Encoding),
    None,
    Some(Cmd::ZoomIn),
    Some(Cmd::ZoomOut),
    Some(Cmd::ZoomReset),
];

/// Gli strumenti per i log: filtrare le righe, seguire la coda, colorare.
const LOG: &[Option<Cmd>] = &[
    Some(Cmd::Filter),
    None,
    Some(Cmd::Follow),
    Some(Cmd::LogColors),
    None,
    Some(Cmd::TimeFilter),
    Some(Cmd::TimeRange),
    Some(Cmd::TimeMask),
];

const AIUTO: &[Option<Cmd>] = &[Some(Cmd::Help)];

/// I menu della barra, nell'ordine in cui compaiono.
pub const MENU: [Menu; 5] = [
    ("gui.menu.file", FILE),
    ("gui.menu.edit", MODIFICA),
    ("gui.menu.view", VISTA),
    ("gui.menu.log", LOG),
    ("gui.menu.help", AIUTO),
];

/// La voce e' un interruttore: `Some(acceso)`; `None` se e' un comando qualunque. Nel menu Visualizza gli interruttori
/// parlano del lavoro attuale (le impostazioni di tema e lingua stanno nella finestra delle impostazioni).
pub fn acceso(ws: &Workspace, cmd: Cmd) -> Option<bool> {
    let a = ws.active();
    match cmd {
        Cmd::NavBar => Some(ws.nav_bar()),
        Cmd::ColumnMode => Some(a.column_mode()),
        Cmd::Special => Some(ws.special_chars()),
        Cmd::LineNumbers => Some(a.line_numbers()),
        Cmd::LineIndex => Some(ws.index_enabled()),
        Cmd::LogColors => Some(a.log_colors()),
        Cmd::Follow => Some(a.follow()),
        Cmd::TimeFilter => Some(a.time_filter_on()),
        _ => None,
    }
}

/// La spunta di una voce accesa, disegnata a mano (il glifo non e' nel font), nel margine a sinistra del bottone.
fn spunta(ui: &Ui, r: egui::Rect) {
    let colore = ui.visuals().text_color();
    let c = egui::Pos2::new(r.min.x + 14.0, r.center().y);
    let tratto = egui::Stroke::new(1.6_f32, colore);
    ui.painter().line_segment(
        [
            c + egui::Vec2::new(-4.0, 0.0),
            c + egui::Vec2::new(-1.0, 3.5),
        ],
        tratto,
    );
    ui.painter().line_segment(
        [
            c + egui::Vec2::new(-1.0, 3.5),
            c + egui::Vec2::new(5.0, -4.0),
        ],
        tratto,
    );
}

/// Disegna la barra dei menu e restituisce il comando scelto in questo giro, se c'e'. Il testo viene dal catalogo della
/// scheda attiva (la lingua attiva); le voci che non hanno senso adesso sono spente.
pub fn disegna(ui: &mut Ui, ws: &Workspace) -> Option<Cmd> {
    let app = ws.active();
    let mut scelta = None;
    egui::MenuBar::new().ui(ui, |ui| {
        for (titolo, voci) in MENU {
            ui.menu_button(app.text(titolo), |ui| {
                // i menu a tendina sono popup: i colori dei popup del tema
                let tav = crate::stile::corrente(ui.ctx());
                crate::stile::popup(ui.style_mut(), &tav);
                crate::stile::voci_piatte(ui.style_mut());
                for v in voci {
                    match v {
                        None => {
                            ui.separator();
                        }
                        Some(cmd) => {
                            let v = voce_di(*cmd);
                            let acceso = acceso(ws, *cmd);
                            // le voci che si accendono e spengono hanno un margine per la spunta, che c'e' solo se sono accese
                            let testo = match acceso {
                                Some(_) => format!("      {}", app.text(v.chiave)),
                                None => app.text(v.chiave).to_string(),
                            };
                            let b = egui::Button::new(testo).shortcut_text(v.scorciatoia);
                            let r = ui.add_enabled(commands::enabled(ws, *cmd), b);
                            if acceso == Some(true) {
                                spunta(ui, r.rect);
                            }
                            if r.clicked() {
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

/// Il menu contestuale della griglia (tasto destro sul testo): i comandi di modifica e la ricerca; una `None` e' un
/// separatore.
pub const CONTESTUALE: &[Option<Cmd>] = &[
    Some(Cmd::Undo),
    Some(Cmd::Redo),
    None,
    Some(Cmd::Cut),
    Some(Cmd::Copy),
    Some(Cmd::Paste),
    Some(Cmd::SelectAll),
    None,
    Some(Cmd::Find),
];

/// Disegna il menu contestuale con l'angolo in `pos` e restituisce i rettangoli delle voci (nell'ordine, senza i
/// separatori), il rettangolo di tutto il menu e il comando scelto in questo giro, se c'e'. Le voci spente restano
/// visibili ma non rispondono.
pub fn disegna_contestuale(
    ctx: &egui::Context,
    ws: &Workspace,
    pos: egui::Pos2,
) -> (Vec<(egui::Rect, Cmd)>, egui::Rect, Option<Cmd>) {
    let app = ws.active();
    let mut voci = Vec::new();
    let mut scelta = None;
    let r = egui::Area::new(egui::Id::new("mrp_contestuale"))
        .order(egui::Order::Foreground)
        .fixed_pos(pos)
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                let tav = crate::stile::corrente(ui.ctx());
                crate::stile::popup(ui.style_mut(), &tav);
                crate::stile::voci_piatte(ui.style_mut());
                ui.set_min_width(190.0);
                for v in CONTESTUALE {
                    match v {
                        None => {
                            ui.separator();
                        }
                        Some(cmd) => {
                            let v = voce_di(*cmd);
                            let b =
                                egui::Button::new(app.text(v.chiave)).shortcut_text(v.scorciatoia);
                            let r = ui.add_enabled(commands::enabled(ws, *cmd), b);
                            voci.push((r.rect, *cmd));
                            if r.clicked() {
                                scelta = Some(*cmd);
                            }
                        }
                    }
                }
            });
        });
    (voci, r.response.rect, scelta)
}
