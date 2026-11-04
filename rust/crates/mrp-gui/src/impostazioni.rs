//! La finestra delle impostazioni (Visualizza > Impostazioni, Ctrl+,): tutte le personalizzazioni in un posto, ognuna col suo
//! tasto scritto a destra (`F6`, `Ctrl+B`, `Alt+W`...), piu' una scheda con tutti i tasti. Le scelte si fanno su una BOZZA: non
//! cambia niente finche' non si preme OK (Invio), e Annulla (Esc, la X) la scarta; con OK le differenze vanno al `Workspace` con
//! le stesse chiamate dei menu e dei tasti (quindi si salvano da sole); il corpo del testo lo cambia chi disegna (`Finestra`),
//! qui si dice solo cosa e' stato chiesto. La finestra non e' modale e si sposta trascinando.

use eframe::egui::{self, Align2, RichText};
use mrp_tui::commands::{Cmd, COMMANDS};
use mrp_tui::theme::ThemeId;
use mrp_tui::workspace::Workspace;

/// Le scelte fatte nella finestra e non ancora applicate (si applicano con OK).
#[derive(Debug, Clone, PartialEq)]
pub struct Bozza {
    /// Il tema scelto; `None`: Automatico (come il sistema).
    pub tema: Option<ThemeId>,
    /// Il codice della lingua.
    pub lingua: String,
    /// Il corpo del testo, in punti.
    pub corpo: f32,
    pub numeri_riga: bool,
    pub speciali: bool,
    pub colori_log: bool,
    pub barra_salti: bool,
    /// L'indice righe si costruisce.
    pub indice: bool,
}

impl Bozza {
    /// La bozza che parte da come stanno le cose adesso.
    pub fn da(ws: &Workspace, corpo: f32) -> Bozza {
        Bozza {
            tema: if ws.theme_auto() {
                None
            } else {
                Some(ws.active().theme_id())
            },
            lingua: ws.active().lang_code().to_string(),
            corpo,
            numeri_riga: ws.line_numbers(),
            speciali: ws.special_chars(),
            colori_log: ws.log_colors(),
            barra_salti: ws.nav_bar(),
            indice: ws.index_enabled(),
        }
    }
}

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
    /// Le scelte non ancora applicate (nate all'apertura, scartate alla chiusura).
    pub bozza: Option<Bozza>,
    /// Si e' premuto OK con l'indice righe da spegnere: si aspetta la risposta del dialogo di conferma (OK finisce il lavoro,
    /// Annulla riporta alle impostazioni con la bozza com'era).
    pub in_attesa_indice: bool,
}

/// Cosa chiede la finestra a chi la disegna dopo OK: cose che sta a lui fare.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Richiesta {
    /// Il corpo del testo, in punti.
    Corpo(f32),
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

/// Applica le differenze fra la bozza e come stanno le cose: il tema, la lingua e gli interruttori vanno al `Workspace` (che li
/// salva); il corpo e il tema automatico si restituiscono a chi disegna.
fn applica(ws: &mut Workspace, b: &Bozza, corpo: f32) -> Vec<Richiesta> {
    let mut richieste = Vec::new();
    let auto = ws.theme_auto();
    match b.tema {
        Some(id) if auto || ws.active().theme_id() != id => ws.active_mut().choose_theme(id),
        None if !auto => richieste.push(Richiesta::TemaAutomatico),
        _ => {}
    }
    if b.lingua != ws.active().lang_code() {
        ws.active_mut().choose_lang(&b.lingua);
    }
    for (cmd, voluto, adesso) in [
        (Cmd::LineNumbers, b.numeri_riga, ws.line_numbers()),
        (Cmd::Special, b.speciali, ws.special_chars()),
        (Cmd::LogColors, b.colori_log, ws.log_colors()),
        (Cmd::NavBar, b.barra_salti, ws.nav_bar()),
    ] {
        if voluto != adesso {
            ws.exec(cmd);
        }
    }
    // l'indice: acceso subito; spegnerlo e' passato dalla conferma, che l'ha gia' fatto
    if b.indice && !ws.index_enabled() {
        ws.set_index(true);
    }
    if b.corpo.round() != corpo.round() {
        richieste.push(Richiesta::Corpo(b.corpo));
    }
    richieste
}

/// Disegna la finestra (se e' aperta) e restituisce cosa chiede a chi la disegna dopo un OK (il corpo del testo, il tema
/// automatico).
pub fn disegna(
    ctx: &egui::Context,
    ws: &mut Workspace,
    stato: &mut StatoImpostazioni,
    corpo: f32,
) -> Vec<Richiesta> {
    stato.controlli.clear();
    stato.tasti.clear();
    stato.chiudi = None;
    if !stato.aperta {
        stato.rect = None;
        stato.bozza = None;
        stato.in_attesa_indice = false;
        return Vec::new();
    }
    // all'apertura la bozza parte da come stanno le cose
    if stato.bozza.is_none() {
        stato.bozza = Some(Bozza::da(ws, corpo));
    }
    // la conferma dello spegnimento dell'indice: se l'utente ha detto OK si finisce il lavoro (le altre scelte si applicano e la
    // finestra si chiude), se ha detto Annulla si torna alle impostazioni
    let mut finisci_ora = false;
    if stato.in_attesa_indice {
        if ws.take_index_off_done() {
            stato.in_attesa_indice = false;
            finisci_ora = true;
        } else if ws.active().dialog_kind().is_none() {
            stato.in_attesa_indice = false;
        }
    }
    // Esc annulla, Invio conferma (la finestra non ha campi di testo che li vogliano per se')
    // (non quando sopra c'e' un dialogo, per esempio quello dei salti: li' Esc e Invio sono suoi)
    let libera = ws.active().dialog_kind().is_none() && !stato.in_attesa_indice;
    let (esc, invio) = ctx.input(|i| {
        (
            libera && i.key_pressed(egui::Key::Escape),
            libera && i.key_pressed(egui::Key::Enter),
        )
    });
    let mut conferma = invio || finisci_ora;
    let mut annulla = esc;
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
            let tav = crate::stile::corrente(ui.ctx());
            crate::stile::popup(ui.style_mut(), &tav);
            // la testata col titolo e la X che chiude (G1): al posto della barra del titolo di egui
            let (rettangolo, cliccata) = crate::dialoghi::testata(ui, titolo.as_str());
            stato.chiudi = Some(rettangolo);
            if cliccata {
                annulla = true;
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
                let mut bozza = stato.bozza.take().unwrap_or_else(|| Bozza::da(ws, corpo));
                generale(ui, ws, stato, &mut bozza);
                stato.bozza = Some(bozza);
            } else {
                dei_tasti(ui, ws, stato);
            }
            // i pulsanti: OK applica le scelte, Annulla le scarta
            ui.add_space(6.0);
            let ok = t(ws, "gui.ok");
            let no = t(ws, "gui.cancel");
            match crate::dialoghi::pulsanti_centrati(
                ui,
                &mut stato.controlli,
                &[ok.as_str(), no.as_str()],
            ) {
                Some(0) => conferma = true,
                Some(1) => annulla = true,
                _ => {}
            }
        });
    stato.rect = risposta.map(|r| r.response.rect);
    let mut richieste = Vec::new();
    if annulla {
        // la bozza si butta: non cambia niente
        stato.aperta = false;
        stato.bozza = None;
    } else if conferma {
        // l'indice da spegnere chiede prima conferma (il dialogo spiega cosa non ci sara' piu'): la finestra resta aperta
        let spegne =
            !finisci_ora && ws.index_enabled() && stato.bozza.as_ref().is_some_and(|b| !b.indice);
        if spegne {
            stato.in_attesa_indice = true;
            ws.active_mut().request_index_off();
        } else {
            if let Some(b) = stato.bozza.take() {
                richieste = applica(ws, &b, corpo);
            }
            stato.aperta = false;
        }
    }
    richieste
}

/// La scheda delle impostazioni: i controlli leggono e scrivono la bozza.
fn generale(ui: &mut egui::Ui, ws: &mut Workspace, stato: &mut StatoImpostazioni, b: &mut Bozza) {
    let t = |ws: &Workspace, k: &str| ws.active().text(k).to_string();
    egui::Grid::new("impostazioni")
        .num_columns(3)
        .spacing([16.0, 8.0])
        .show(ui, |ui| {
            // il tema: Automatico (come il sistema) o uno dei quattro: Blu (stile Total Commander), Debian, Scuro, Chiaro
            let etichetta = t(ws, "settings.theme");
            riga(ui, &etichetta, "F6", |ui| {
                // una tendina, come la lingua: il tema scelto e' il testo, l'elenco ha Automatico e i quattro temi
                let scelto = match b.tema {
                    None => t(ws, "settings.theme_auto"),
                    Some(id) => t(ws, id.key()),
                };
                let combo = egui::ComboBox::from_id_salt("tema")
                    .selected_text(scelto.as_str())
                    .show_ui(ui, |ui| {
                        let nome = t(ws, "settings.theme_auto");
                        let r = ui.selectable_label(b.tema.is_none(), nome.as_str());
                        stato.controlli.push((r.rect, nome));
                        if r.clicked() {
                            b.tema = None;
                        }
                        for id in [
                            ThemeId::Night,
                            ThemeId::Debian,
                            ThemeId::Dark,
                            ThemeId::Light,
                        ] {
                            let nome = t(ws, id.key());
                            let r = ui.selectable_label(b.tema == Some(id), nome.as_str());
                            stato.controlli.push((r.rect, nome));
                            if r.clicked() {
                                b.tema = Some(id);
                            }
                        }
                    });
                stato
                    .controlli
                    .push((combo.response.rect, "tema".to_string()));
            });
            // la lingua
            let lingue = ws.active().language_list();
            let nome_scelto = lingue
                .iter()
                .find(|(c, _)| *c == b.lingua)
                .map_or(b.lingua.clone(), |(_, n)| n.clone());
            let etichetta = t(ws, "settings.language");
            riga(ui, &etichetta, "F7", |ui| {
                let combo = egui::ComboBox::from_id_salt("lingua")
                    .selected_text(nome_scelto.as_str())
                    .show_ui(ui, |ui| {
                        for (codice, nome) in &lingue {
                            if ui
                                .selectable_label(*codice == b.lingua, nome.as_str())
                                .clicked()
                            {
                                b.lingua = codice.clone();
                            }
                        }
                    });
                stato
                    .controlli
                    .push((combo.response.rect, nome_scelto.clone()));
            });
            // il corpo del testo
            let etichetta = t(ws, "settings.font");
            riga(ui, &etichetta, "Ctrl+- / Ctrl++ / Ctrl+0", |ui| {
                ui.horizontal(|ui| {
                    let b_meno = ui.button("-");
                    stato.controlli.push((b_meno.rect, "-".to_string()));
                    if b_meno.clicked() {
                        b.corpo = (b.corpo - 1.0).max(crate::gui::CORPO_MIN);
                    }
                    ui.label(format!("{} pt", b.corpo.round()));
                    let b_piu = ui.button("+");
                    stato.controlli.push((b_piu.rect, "+".to_string()));
                    if b_piu.clicked() {
                        b.corpo = (b.corpo + 1.0).min(crate::gui::CORPO_MAX);
                    }
                    let ripristina = t(ws, "settings.reset");
                    let r = ui.button(ripristina.as_str());
                    stato.controlli.push((r.rect, ripristina));
                    if r.clicked() {
                        b.corpo = crate::gui::CORPO;
                    }
                });
            });
            // gli interruttori: ognuno col suo tasto (i numeri di riga si spengono da soli senza l'indice righe)
            let con_indice = b.indice;
            for (cmd, valore) in [
                (Cmd::LineIndex, &mut b.indice),
                (Cmd::LineNumbers, &mut b.numeri_riga),
                (Cmd::Special, &mut b.speciali),
                (Cmd::LogColors, &mut b.colori_log),
                (Cmd::NavBar, &mut b.barra_salti),
            ] {
                let etichetta = t(ws, cmd.info().label);
                let attivo = cmd != Cmd::LineNumbers || con_indice;
                riga(ui, "", tasto(cmd), |ui| {
                    let r = ui.add_enabled(attivo, egui::Checkbox::new(valore, etichetta.as_str()));
                    if cmd == Cmd::LineIndex {
                        r.clone().on_hover_text(t(ws, "settings.index_hint"));
                    }
                    stato.controlli.push((r.rect, etichetta.clone()));
                });
            }
            // i salti: i tre numeri e il pulsante che li cambia (ha il suo dialogo, con OK e Annulla)
            let [a, c, d] = ws.jump_pages();
            let etichetta = t(ws, "settings.jumps");
            riga(ui, &etichetta, "Alt+PgUp / Alt+PgDn", |ui| {
                ui.horizontal(|ui| {
                    ui.label(format!("{a} / {c} / {d}"));
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
        // trascinare sulla lista non la scorre: muove la finestra
        .scroll_source(
            egui::scroll_area::ScrollSource::SCROLL_BAR
                | egui::scroll_area::ScrollSource::MOUSE_WHEEL,
        )
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
