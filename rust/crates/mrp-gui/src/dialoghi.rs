//! I dialoghi della finestra senza cornice (B4b): i dialoghi dell'`App` che hanno una vista (`dialog_view`) si disegnano
//! come finestre egui al centro dello schermo, con lista, campo e pulsanti veri. I widget guidano l'`App` con le azioni
//! `dialog_*` (B4a-1), che fanno quello che farebbero i tasti del TUI: cosi' i due modi di mostrarli non possono
//! divergere. `StatoDialogo` ricorda l'ultimo disegno (i rettangoli, per i test) e quello che serve da un giro all'altro
//! (il testo del campo, il fuoco chiesto, la voce da portare in vista).

use eframe::egui::{self, Align2, RichText, ScrollArea, TextEdit};
use mrp_tui::app::DialogKind;
use mrp_tui::dialogs::{ConfirmChoice, DialogView};
use mrp_tui::workspace::Workspace;

/// Il dialogo disegnato nell'ultimo giro: il rettangolo della finestra, il titolo, i pulsanti da sinistra a destra col
/// loro testo, le righe del menu e il campo del prompt (per i test), piu' i ricordi da un giro all'altro (il testo del
/// campo, se ha gia' chiesto il fuoco, la voce evidenziata l'ultima volta). Senza un dialogo da disegnare si azzera.
#[derive(Debug, Default)]
pub struct StatoDialogo {
    /// Il rettangolo della finestra del dialogo (`None`: nessun dialogo disegnato).
    pub rect: Option<egui::Rect>,
    /// Il titolo della finestra (`None`: come sopra).
    pub titolo: Option<String>,
    /// I pulsanti della finestra nell'ultimo disegno, da sinistra a destra, col loro testo.
    pub pulsanti: Vec<(egui::Rect, String)>,
    /// I rettangoli delle righe del menu nell'ultimo disegno, nell'ordine delle voci.
    pub voci: Vec<egui::Rect>,
    /// Il rettangolo del campo di testo del prompt nell'ultimo disegno (`None`: non c'e').
    pub campo: Option<egui::Rect>,
    /// Il dialogo disegnato nel giro scorso: uno nuovo azzera i ricordi del campo e del menu.
    kind: Option<DialogKind>,
    /// Il testo del campo del prompt: il padrone e' l'`App`, il campo lo mostra e le riporta le modifiche.
    testo: String,
    /// Se il campo del prompt ha gia' chiesto il fuoco (lo chiede una volta sola, appena il dialogo si apre).
    campo_fuoco: bool,
    /// La voce del menu evidenziata nel giro scorso: quando cambia, si porta in vista.
    ultima_voce: Option<usize>,
}

/// Disegna la finestra del dialogo della scheda attiva (se c'e' e ha una vista) e guida l'`App`: Esc la chiude, nel menu
/// le frecce muovono l'evidenza e Invio sceglie, nel prompt Invio nel campo conferma, i pulsanti fanno quello che
/// dicono. La finestra sta al centro dello schermo, sopra tutto il resto; senza una vista non si disegna niente e lo
/// stato si azzera.
pub fn disegna(ctx: &egui::Context, ws: &mut Workspace, stato: &mut StatoDialogo) {
    {
        let app = ws.active_mut();
        // Esc chiude il dialogo, in ogni variante (nella conferma di uscita vale come Annulla)
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            app.dialog_cancel();
        }
        // nel menu le frecce muovono l'evidenza (ferma alla prima e all'ultima voce), Home e End vanno ai due capi,
        // Invio sceglie la voce evidenziata
        if let Some(DialogView::Menu {
            items, selected, ..
        }) = app.dialog_view()
        {
            let ultima = items.len().saturating_sub(1);
            let mut scelta = selected;
            if ctx.input(|i| i.key_pressed(egui::Key::ArrowDown)) {
                scelta = (scelta + 1).min(ultima);
            }
            if ctx.input(|i| i.key_pressed(egui::Key::ArrowUp)) {
                scelta = scelta.saturating_sub(1);
            }
            if ctx.input(|i| i.key_pressed(egui::Key::Home)) {
                scelta = 0;
            }
            if ctx.input(|i| i.key_pressed(egui::Key::End)) {
                scelta = ultima;
            }
            if scelta != selected {
                app.dialog_select(scelta);
            }
            if ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
                app.dialog_pick(scelta);
            }
        }
    }
    let Some(vista) = ws.active().dialog_view() else {
        *stato = StatoDialogo::default();
        return;
    };
    // un dialogo diverso da quello del giro scorso azzera i ricordi del campo e del menu
    let kind = ws.active().dialog_kind();
    if stato.kind != kind {
        stato.kind = kind;
        stato.testo.clear();
        stato.campo_fuoco = false;
        stato.ultima_voce = None;
    }
    // dei ricordi del giro scorso restano solo testo e fuoco del campo e la voce del menu: il resto si ridisegna
    stato.pulsanti.clear();
    stato.voci.clear();
    stato.campo = None;
    let altezza = ctx.screen_rect().height();
    let t_ok = ws.active().text("gui.ok").to_string();
    let t_cancel = ws.active().text("gui.cancel").to_string();
    let titolo = match &vista {
        DialogView::Help { title, .. }
        | DialogView::Menu { title, .. }
        | DialogView::Prompt { title, .. }
        | DialogView::Overwrite { title, .. }
        | DialogView::Confirm { title, .. }
        | DialogView::Find { title, .. }
        | DialogView::Replace { title, .. }
        | DialogView::Saving { title, .. } => title.clone(),
    };
    // il rettangolo della finestra: lo danno le varianti (una per una), lo legge la fine
    let rect;
    match &vista {
        // l'aiuto: due colonne (il tasto in monospace, la descrizione), in una lista che occupa al piu' il 70% dello
        // schermo; sotto, il pulsante che chiude
        DialogView::Help { rows, .. } => {
            rect = egui::Window::new(titolo.as_str())
                .collapsible(false)
                .resizable(false)
                .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ScrollArea::vertical()
                        .max_height(altezza * 0.7)
                        .show(ui, |ui| {
                            egui::Grid::new("aiuto").num_columns(2).show(ui, |ui| {
                                for (tasto, descrizione) in rows {
                                    ui.label(RichText::new(tasto).monospace());
                                    ui.label(descrizione);
                                    ui.end_row();
                                }
                            });
                        });
                    if pulsante(ui, &mut stato.pulsanti, &t_ok) {
                        ws.active_mut().dialog_cancel();
                    }
                })
                .map(|w| w.response.rect);
        }
        // il menu (tema, lingua, codifica): una riga per voce, col nome e, a fianco, il dettaglio in testo debole, in
        // una lista che occupa al piu' il 60% dello schermo; sotto, l'eventuale nota. Tutta la riga sceglie la voce
        DialogView::Menu {
            items,
            selected,
            note,
            ..
        } => {
            rect = egui::Window::new(titolo.as_str())
                .collapsible(false)
                .resizable(false)
                .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ScrollArea::vertical()
                        .max_height(altezza * 0.6)
                        .show(ui, |ui| {
                            for (i, voce) in items.iter().enumerate() {
                                let riga = ui.horizontal(|ui| {
                                    let r = ui.selectable_label(i == *selected, &voce.label);
                                    // la voce evidenziata si porta in vista quando cambia
                                    if i == *selected && stato.ultima_voce != Some(*selected) {
                                        r.scroll_to_me(None);
                                    }
                                    if !voce.detail.is_empty() {
                                        ui.label(RichText::new(&voce.detail).weak());
                                    }
                                });
                                let rettangolo = riga.response.rect;
                                let clic =
                                    ui.interact(rettangolo, ui.id().with(i), egui::Sense::click());
                                if clic.clicked() {
                                    ws.active_mut().dialog_pick(i);
                                }
                                stato.voci.push(rettangolo);
                            }
                        });
                    if let Some(n) = note {
                        ui.label(RichText::new(n).weak());
                    }
                })
                .map(|w| w.response.rect);
            stato.ultima_voce = Some(*selected);
        }
        // il prompt (vai a, apri, salva con nome): il suggerimento, il campo largo quanto la finestra e i due pulsanti.
        // Il campo prende il fuoco appena il dialogo si apre e mostra sempre il testo dell'App: se l'utente lo cambia,
        // il nuovo testo torna all'App; Invio nel campo conferma (e il campo lascia andare il fuoco: e' egui)
        DialogView::Prompt { hint, text, .. } => {
            if !stato.campo_fuoco {
                stato.testo = text.clone();
            }
            rect = egui::Window::new(titolo.as_str())
                .collapsible(false)
                .resizable(false)
                .min_width(360.0)
                .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label(RichText::new(hint).weak());
                    let r =
                        ui.add(TextEdit::singleline(&mut stato.testo).desired_width(f32::INFINITY));
                    stato.campo = Some(r.rect);
                    if !stato.campo_fuoco {
                        r.request_focus();
                        stato.campo_fuoco = true;
                    }
                    if r.changed() {
                        ws.active_mut().dialog_set_text(&stato.testo);
                    }
                    if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        ws.active_mut().dialog_submit();
                    }
                    ui.horizontal(|ui| {
                        if pulsante(ui, &mut stato.pulsanti, &t_ok) {
                            ws.active_mut().dialog_submit();
                        }
                        if pulsante(ui, &mut stato.pulsanti, &t_cancel) {
                            ws.active_mut().dialog_cancel();
                        }
                    });
                })
                .map(|w| w.response.rect);
        }
        // il file scelto per "salva con nome" esiste gia': la domanda e i due pulsanti
        DialogView::Overwrite { text, .. } => {
            rect = egui::Window::new(titolo.as_str())
                .collapsible(false)
                .resizable(false)
                .min_width(360.0)
                .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label(text.as_str());
                    ui.horizontal(|ui| {
                        if pulsante(ui, &mut stato.pulsanti, &t_ok) {
                            ws.active_mut().dialog_submit();
                        }
                        if pulsante(ui, &mut stato.pulsanti, &t_cancel) {
                            ws.active_mut().dialog_cancel();
                        }
                    });
                })
                .map(|w| w.response.rect);
        }
        // la conferma di uscita con modifiche non salvate: la domanda e le tre scelte, nell'ordine della vista
        DialogView::Confirm {
            text,
            save,
            discard,
            cancel,
            ..
        } => {
            rect = egui::Window::new(titolo.as_str())
                .collapsible(false)
                .resizable(false)
                .min_width(360.0)
                .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label(text.as_str());
                    ui.horizontal(|ui| {
                        if pulsante(ui, &mut stato.pulsanti, save) {
                            ws.active_mut().dialog_confirm(ConfirmChoice::Save);
                        }
                        if pulsante(ui, &mut stato.pulsanti, discard) {
                            ws.active_mut().dialog_confirm(ConfirmChoice::Discard);
                        }
                        if pulsante(ui, &mut stato.pulsanti, cancel) {
                            ws.active_mut().dialog_confirm(ConfirmChoice::Cancel);
                        }
                    });
                })
                .map(|w| w.response.rect);
        }
        // Cerca e Sostituisci: provvisorio (la finestra vera e' il task B4c); intanto un'etichetta e annulla
        DialogView::Find { .. } | DialogView::Replace { .. } => {
            rect = egui::Window::new(titolo.as_str())
                .collapsible(false)
                .resizable(false)
                .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    if pulsante(ui, &mut stato.pulsanti, &t_cancel) {
                        ws.active_mut().dialog_cancel();
                    }
                })
                .map(|w| w.response.rect);
        }
        // il salvataggio in corso: il nome del file, la barra che dice a che punto e', e annulla
        DialogView::Saving { text, percent, .. } => {
            rect = egui::Window::new(titolo.as_str())
                .collapsible(false)
                .resizable(false)
                .min_width(360.0)
                .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label(text.as_str());
                    ui.add(egui::ProgressBar::new(f32::from(*percent) / 100.0));
                    if pulsante(ui, &mut stato.pulsanti, &t_cancel) {
                        ws.active_mut().dialog_cancel();
                    }
                })
                .map(|w| w.response.rect);
        }
    }
    stato.rect = rect;
    stato.titolo = Some(titolo);
}

/// Un pulsante col suo testo: lo disegna, ne ricorda il rettangolo (l'ordine di disegno e' da sinistra a destra) e
/// dice se e' stato premuto.
fn pulsante(ui: &mut egui::Ui, pulsanti: &mut Vec<(egui::Rect, String)>, testo: &str) -> bool {
    let r = ui.button(testo);
    pulsanti.push((r.rect, testo.to_string()));
    r.clicked()
}
