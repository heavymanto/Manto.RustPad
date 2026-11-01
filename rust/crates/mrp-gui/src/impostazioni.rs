//! La finestra delle impostazioni (Visualizza > Impostazioni, Ctrl+,): tutte le personalizzazioni in un posto, ognuna col suo
//! tasto scritto a destra (`F6`, `Ctrl+B`, `Alt+W`...), piu' una scheda con tutti i tasti. Le scelte vanno al `Workspace` con le
//! stesse chiamate dei menu e dei tasti (quindi si salvano da sole); il corpo del testo lo cambia chi disegna (`Finestra`), qui
//! si dice solo cosa e' stato chiesto. La finestra non e' modale e si sposta trascinando il titolo.

use eframe::egui::{self, Align2, RichText};
use mrp_tui::commands::{Cmd, COMMANDS};
use mrp_tui::theme::ThemeId;
use mrp_tui::workspace::Workspace;

/// Lo stato della finestra delle impostazioni tra un giro e l'altro, e cio' che si e' disegnato (per i test).
#[derive(Debug, Default)]
pub struct StatoImpostazioni {
    /// La finestra e' aperta.
    pub aperta: bool,
    /// La scheda in vista: 0 le impostazioni, 1 i tasti.
    pub scheda: u8,
    /// Il rettangolo della finestra nell'ultimo disegno (`None`: chiusa).
    pub rect: Option<egui::Rect>,
    /// Il rettangolo della X della testata nell'ultimo disegno (`None`: chiusa) (per i test, G1).
    pub chiudi: Option<egui::Rect>,
    /// I controlli disegnati nell'ultimo giro (caselle, pulsanti, scelte del tema) col loro testo, nell'ordine di disegno.
    pub controlli: Vec<(egui::Rect, String)>,
    /// Le righe della scheda dei tasti nell'ultimo giro: (nome, scorciatoia).
    pub tasti: Vec<(String, String)>,
}

/// Cosa chiede la finestra a chi la disegna: il corpo del testo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Richiesta {
    Piu,
    Meno,
    Predefinito,
    /// Il tema torna ad essere quello del sistema.
    TemaAutomatico,
}

/// La scorciatoia scritta per `cmd` nella finestra.
fn tasto(cmd: Cmd) -> &'static str {
    match cmd {
        Cmd::Settings => "Ctrl+,",
        c => crate::menu::scorciatoia(c),
    }
}

/// Una riga della scheda delle impostazioni: l'etichetta, il controllo (lo disegna `controllo`) e a destra il tasto.
fn riga<R>(
    ui: &mut egui::Ui,
    etichetta: &str,
    scorciatoia: &str,
    controllo: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    ui.label(etichetta);
    let r = controllo(ui);
    ui.label(RichText::new(scorciatoia).weak().monospace());
    ui.end_row();
    r
}

/// Disegna la finestra (se e' aperta) e restituisce il cambio di corpo chiesto in questo giro, se c'e'.
pub fn disegna(
    ctx: &egui::Context,
    ws: &mut Workspace,
    stato: &mut StatoImpostazioni,
    corpo: f32,
) -> Option<Richiesta> {
    stato.controlli.clear();
    stato.tasti.clear();
    stato.chiudi = None;
    if !stato.aperta {
        stato.rect = None;
        return None;
    }
    let mut richiesta = None;
    let t = |ws: &Workspace, k: &str| ws.active().text(k).to_string();
    let titolo = t(ws, "gui.settings").trim_end_matches('…').to_string();
    let risposta = egui::Window::new(titolo.as_str())
        .title_bar(false)
        .collapsible(false)
        .resizable(true)
        .default_width(560.0)
        .pivot(Align2::CENTER_CENTER)
        .default_pos(ctx.screen_rect().center())
        .show(ctx, |ui| {
            // la testata col titolo e la X che chiude (G1): al posto della barra del titolo di egui
            let (rettangolo, cliccata) = crate::dialoghi::testata(ui, titolo.as_str());
            stato.chiudi = Some(rettangolo);
            if cliccata {
                stato.aperta = false;
            }
            ui.horizontal(|ui| {
                for (i, k) in [(0u8, "settings.tab_general"), (1u8, "settings.tab_keys")] {
                    let nome = t(ws, k);
                    let r = ui.selectable_label(stato.scheda == i, nome.as_str());
                    stato.controlli.push((r.rect, nome));
                    if r.clicked() {
                        stato.scheda = i;
                    }
                }
            });
            ui.separator();
            if stato.scheda == 0 {
                richiesta = generale(ui, ws, stato, corpo);
            } else {
                dei_tasti(ui, ws, stato);
            }
        });
    stato.rect = risposta.map(|r| r.response.rect);
    richiesta
}

/// La scheda delle impostazioni.
fn generale(
    ui: &mut egui::Ui,
    ws: &mut Workspace,
    stato: &mut StatoImpostazioni,
    corpo: f32,
) -> Option<Richiesta> {
    let mut richiesta = None;
    let t = |ws: &Workspace, k: &str| ws.active().text(k).to_string();
    egui::Grid::new("impostazioni")
        .num_columns(3)
        .spacing([16.0, 8.0])
        .show(ui, |ui| {
            // il tema: Automatico (come il sistema), Chiaro, Scuro
            let corrente = ws.active().theme_id();
            let automatico = ws.theme_auto();
            let etichetta = t(ws, "settings.theme");
            let mut scelto = None;
            let mut auto = false;
            riga(ui, &etichetta, "F6", |ui| {
                ui.horizontal(|ui| {
                    let nome = t(ws, "settings.theme_auto");
                    let r = ui.selectable_label(automatico, nome.as_str());
                    stato.controlli.push((r.rect, nome));
                    if r.clicked() && !automatico {
                        auto = true;
                    }
                    for id in [ThemeId::Light, ThemeId::Dark] {
                        let nome = t(ws, id.key());
                        let r = ui.selectable_label(!automatico && corrente == id, nome.as_str());
                        stato.controlli.push((r.rect, nome));
                        if r.clicked() && (automatico || corrente != id) {
                            scelto = Some(id);
                        }
                    }
                });
            });
            if let Some(id) = scelto {
                ws.active_mut().choose_theme(id);
            }
            if auto {
                richiesta = Some(Richiesta::TemaAutomatico);
            }
            // la lingua
            let lingue = ws.active().language_list();
            let attuale = ws.active().lang_code().to_string();
            let nome_attuale = lingue
                .iter()
                .find(|(c, _)| *c == attuale)
                .map_or(attuale.clone(), |(_, n)| n.clone());
            let etichetta = t(ws, "settings.language");
            let mut nuova = None;
            riga(ui, &etichetta, "F7", |ui| {
                let combo = egui::ComboBox::from_id_salt("lingua")
                    .selected_text(nome_attuale.as_str())
                    .show_ui(ui, |ui| {
                        for (codice, nome) in &lingue {
                            if ui
                                .selectable_label(*codice == attuale, nome.as_str())
                                .clicked()
                            {
                                nuova = Some(codice.clone());
                            }
                        }
                    });
                stato
                    .controlli
                    .push((combo.response.rect, nome_attuale.clone()));
            });
            if let Some(c) = nuova {
                ws.active_mut().choose_lang(&c);
            }
            // il corpo del testo
            let etichetta = t(ws, "settings.font");
            riga(ui, &etichetta, "Ctrl+- / Ctrl++ / Ctrl+0", |ui| {
                ui.horizontal(|ui| {
                    for (testo, r) in [("-", Richiesta::Meno), ("+", Richiesta::Piu)] {
                        let b = ui.button(testo);
                        stato.controlli.push((b.rect, testo.to_string()));
                        if b.clicked() {
                            richiesta = Some(r);
                        }
                        if r == Richiesta::Meno {
                            ui.label(format!("{} pt", corpo.round()));
                        }
                    }
                    let ripristina = t(ws, "settings.reset");
                    let b = ui.button(ripristina.as_str());
                    stato.controlli.push((b.rect, ripristina));
                    if b.clicked() {
                        richiesta = Some(Richiesta::Predefinito);
                    }
                });
            });
            // gli interruttori: ognuno col suo tasto, e agiscono come il tasto
            let interruttori = [
                (Cmd::LineNumbers, ws.line_numbers()),
                (Cmd::Special, ws.special_chars()),
                (Cmd::LogColors, ws.log_colors()),
                (Cmd::NavBar, ws.nav_bar()),
            ];
            for (cmd, acceso) in interruttori {
                let etichetta = t(ws, cmd.info().label);
                let mut v = acceso;
                riga(ui, "", tasto(cmd), |ui| {
                    let r = ui.checkbox(&mut v, etichetta.as_str());
                    stato.controlli.push((r.rect, etichetta.clone()));
                    if r.changed() {
                        ws.exec(cmd);
                    }
                });
            }
            // i salti: i tre numeri e il pulsante che li cambia
            let [a, b, c] = ws.jump_pages();
            let etichetta = t(ws, "settings.jumps");
            riga(ui, &etichetta, "Alt+PgUp / Alt+PgDn", |ui| {
                ui.horizontal(|ui| {
                    ui.label(format!("{a} / {b} / {c}"));
                    let cambia = t(ws, "settings.change");
                    let r = ui.button(cambia.as_str());
                    stato.controlli.push((r.rect, cambia));
                    if r.clicked() {
                        ws.active_mut().open_jump_dialog();
                    }
                });
            });
        });
    ui.separator();
    // dove si salva tutto
    match ws.active().settings_file() {
        Some(p) => {
            let testo = ws
                .active()
                .text_with("settings.saved_in", &[&p.display().to_string()]);
            ui.label(RichText::new(testo).weak());
        }
        None => {
            ui.label(RichText::new(t(ws, "settings.not_saved")).weak());
        }
    }
    richiesta
}

/// La scheda dei tasti: i comandi come nei menu (nome e scorciatoia), poi gli altri tasti dell'aiuto.
fn dei_tasti(ui: &mut egui::Ui, ws: &mut Workspace, stato: &mut StatoImpostazioni) {
    let t = |ws: &Workspace, k: &str| ws.active().text(k).to_string();
    let mut righe: Vec<(String, String)> = COMMANDS
        .iter()
        .map(|c| {
            (
                t(ws, c.label).trim_end_matches('…').to_string(),
                tasto(c.cmd).to_string(),
            )
        })
        .collect();
    // le righe dell'aiuto sono (tasti, descrizione): qui il nome viene prima
    righe.extend(ws.active().help_rows().into_iter().map(|(k, d)| (d, k)));
    egui::ScrollArea::vertical()
        .max_height(420.0)
        .show(ui, |ui| {
            egui::Grid::new("tasti")
                .num_columns(2)
                .striped(true)
                .show(ui, |ui| {
                    for (nome, scorciatoia) in &righe {
                        ui.label(nome.as_str());
                        ui.label(RichText::new(scorciatoia).monospace());
                        ui.end_row();
                    }
                });
        });
    stato.tasti = righe;
}
