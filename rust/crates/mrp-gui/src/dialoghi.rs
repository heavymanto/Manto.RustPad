//! I dialoghi della finestra senza cornice (B4b): i dialoghi dell'`App` che hanno una vista (`dialog_view`) si disegnano
//! come finestre egui al centro dello schermo, con lista, campo e pulsanti veri. I widget guidano l'`App` con le azioni
//! `dialog_*` (B4a-1), che fanno quello che farebbero i tasti del TUI: cosi' i due modi di mostrarli non possono
//! divergere. `StatoDialogo` ricorda l'ultimo disegno (i rettangoli, per i test) e quello che serve da un giro all'altro
//! (il testo del campo, il fuoco chiesto, la voce da portare in vista).

use eframe::egui::{self, Align2, RichText, ScrollArea, TextEdit};
use mrp_tui::app::{App, DialogKind};
use mrp_tui::dialogs::{ConfirmChoice, DialogView, Mode, SearchLabels};
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
    /// I rettangoli dei campi di testo del dialogo nell'ultimo disegno, nell'ordine di disegno (per i test); il primo
    /// e' anche `campo`.
    pub campi: Vec<egui::Rect>,
    /// Le opzioni del dialogo (i radio del modo e dell'ambito, la casella delle maiuscole) nell'ultimo disegno,
    /// nell'ordine di disegno, con la loro etichetta (per i test).
    pub opzioni: Vec<(egui::Rect, String)>,
    /// Il dialogo disegnato nel giro scorso: uno nuovo azzera i ricordi del campo e del menu.
    kind: Option<DialogKind>,
    /// Il testo del campo del prompt: il padrone e' l'`App`, il campo lo mostra e le riporta le modifiche.
    testo: String,
    /// Il testo del secondo campo di Sostituisci (con cosa sostituire; il primo, cosa cercare, usa `testo`).
    testo_due: String,
    /// I tre numeri del dialogo dei salti.
    numeri: [String; 3],
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
        stato.testo_due.clear();
        stato.campo_fuoco = false;
        stato.ultima_voce = None;
    }
    // dei ricordi del giro scorso restano solo testo e fuoco del campo e la voce del menu: il resto si ridisegna
    stato.pulsanti.clear();
    stato.voci.clear();
    stato.campo = None;
    stato.campi.clear();
    stato.opzioni.clear();
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
        | DialogView::Jumps { title, .. }
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
                .pivot(Align2::CENTER_CENTER)
                .default_pos(ctx.screen_rect().center())
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
                .pivot(Align2::CENTER_CENTER)
                .default_pos(ctx.screen_rect().center())
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
                .pivot(Align2::CENTER_CENTER)
                .default_pos(ctx.screen_rect().center())
                .show(ctx, |ui| {
                    ui.label(RichText::new(hint).weak());
                    let r =
                        ui.add(TextEdit::singleline(&mut stato.testo).desired_width(f32::INFINITY));
                    stato.campo = Some(r.rect);
                    stato.campi.push(r.rect);
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
                .pivot(Align2::CENTER_CENTER)
                .default_pos(ctx.screen_rect().center())
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
                .pivot(Align2::CENTER_CENTER)
                .default_pos(ctx.screen_rect().center())
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
        // Cerca: il suggerimento, il campo largo quanto la finestra, la riga delle opzioni (il modo e le maiuscole) e
        // i due pulsanti. Come il prompt: il campo prende il fuoco appena il dialogo si apre, mostra il testo dell'App
        // e le riporta le modifiche; Invio nel campo cerca (e chiude), come il pulsante.
        // i salti: tre numeri (le pagine dei salti piccolo, medio e grande) con la loro etichetta, e i due pulsanti
        DialogView::Jumps {
            hint,
            labels,
            values,
            field,
            ..
        } => {
            // il padrone dei numeri e' l'App: il campo li rilegge a ogni giro (le modifiche dell'utente le riportano a lei)
            stato.numeri = values.clone();
            rect = egui::Window::new(titolo.as_str())
                .collapsible(false)
                .resizable(false)
                .min_width(380.0)
                .pivot(Align2::CENTER_CENTER)
                .default_pos(ctx.screen_rect().center())
                .show(ctx, |ui| {
                    ui.label(RichText::new(hint).weak());
                    egui::Grid::new("salti_numeri")
                        .num_columns(2)
                        .show(ui, |ui| {
                            for i in 0..3u8 {
                                ui.label(labels[usize::from(i)].as_str());
                                let r = ui.add(
                                    TextEdit::singleline(&mut stato.numeri[usize::from(i)])
                                        .desired_width(80.0),
                                );
                                stato.campi.push(r.rect);
                                if stato.campo.is_none() {
                                    stato.campo = Some(r.rect);
                                }
                                // il fuoco iniziale e' sul campo che dice l'App
                                if !stato.campo_fuoco && *field == i {
                                    r.request_focus();
                                }
                                if r.has_focus() && *field != i {
                                    ws.active_mut().dialog_set_jump(i, &values[usize::from(i)]);
                                }
                                if r.changed() {
                                    ws.active_mut()
                                        .dialog_set_jump(i, &stato.numeri[usize::from(i)]);
                                    // l'App tiene solo le cifre: il campo le rilegge
                                    stato.numeri[usize::from(i)] = stato.numeri[usize::from(i)]
                                        .chars()
                                        .filter(char::is_ascii_digit)
                                        .take(5)
                                        .collect();
                                }
                                if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                    ws.active_mut().dialog_submit();
                                }
                                ui.end_row();
                            }
                        });
                    stato.campo_fuoco = true;
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
        DialogView::Find {
            hint,
            text,
            mode,
            case,
            labels,
            go,
            invert,
            ..
        } => {
            if !stato.campo_fuoco {
                stato.testo = text.clone();
            }
            rect = egui::Window::new(titolo.as_str())
                .collapsible(false)
                .resizable(false)
                .min_width(420.0)
                .pivot(Align2::CENTER_CENTER)
                .default_pos(ctx.screen_rect().center())
                .show(ctx, |ui| {
                    ui.label(RichText::new(hint).weak());
                    let r =
                        ui.add(TextEdit::singleline(&mut stato.testo).desired_width(f32::INFINITY));
                    stato.campo = Some(r.rect);
                    stato.campi.push(r.rect);
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
                    riga_opzioni(
                        ui,
                        ws.active_mut(),
                        &mut stato.opzioni,
                        *mode,
                        *case,
                        labels,
                    );
                    // nel dialogo del filtro: nascondere le righe che corrispondono invece di mostrarle
                    if let Some(inv) = invert {
                        let mut v = *inv;
                        let r = ui.checkbox(&mut v, labels.invert.as_str());
                        stato.opzioni.push((r.rect, labels.invert.clone()));
                        if r.changed() {
                            ws.active_mut().dialog_set_invert(v);
                        }
                    }
                    ui.horizontal(|ui| {
                        if pulsante(ui, &mut stato.pulsanti, go) {
                            ws.active_mut().dialog_submit();
                        }
                        if pulsante(ui, &mut stato.pulsanti, &t_cancel) {
                            ws.active_mut().dialog_cancel();
                        }
                    });
                })
                .map(|w| w.response.rect);
        }
        // Sostituisci: i due campi allineati in una griglia (l'etichetta a sinistra), le opzioni come in Cerca piu'
        // l'ambito, il messaggio dell'anteprima e i due pulsanti. Il fuoco parte dal campo che dice l'App e la segue
        // (Tab e i clic lo spostano: lo fa egui); le modifiche tornano all'App e tolgono l'anteprima, Invio in un
        // campo fa l'anteprima e, se gia' pronta, applica.
        DialogView::Replace {
            find_label,
            with_label,
            find,
            with,
            field,
            mode,
            case,
            all_tabs,
            status,
            ready,
            labels,
            preview,
            apply,
            ..
        } => {
            if !stato.campo_fuoco {
                stato.testo = find.clone();
                stato.testo_due = with.clone();
            }
            let avviso = ctx.style().visuals.warn_fg_color;
            rect = egui::Window::new(titolo.as_str())
                .collapsible(false)
                .resizable(false)
                .min_width(420.0)
                .pivot(Align2::CENTER_CENTER)
                .default_pos(ctx.screen_rect().center())
                .show(ctx, |ui| {
                    egui::Grid::new("sostituisci")
                        .num_columns(2)
                        .show(ui, |ui| {
                            // i due campi (0 cosa cercare, 1 con cosa sostituire): l'etichetta a sinistra e il testo che
                            // riempie la riga; il primo rettangolo e' anche il campo del prompt
                            for i in 0..2u8 {
                                let (etichetta, testo) = if i == 0 {
                                    (find_label.as_str(), &mut stato.testo)
                                } else {
                                    (with_label.as_str(), &mut stato.testo_due)
                                };
                                ui.label(etichetta);
                                let r = ui
                                    .add(TextEdit::singleline(testo).desired_width(f32::INFINITY));
                                stato.campi.push(r.rect);
                                if stato.campo.is_none() {
                                    stato.campo = Some(r.rect);
                                }
                                // il fuoco arrivato a un campo diverso da quello della vista si dice all'App
                                if r.has_focus() && *field != i {
                                    ws.active_mut().dialog_focus(i);
                                }
                                // il fuoco iniziale, una volta sola: sul campo che dice la vista
                                if !stato.campo_fuoco && *field == i {
                                    r.request_focus();
                                    stato.campo_fuoco = true;
                                }
                                if r.changed() {
                                    ws.active_mut().dialog_set_field(i, testo);
                                }
                                if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                    ws.active_mut().dialog_submit();
                                }
                                ui.end_row();
                            }
                        });
                    riga_opzioni(
                        ui,
                        ws.active_mut(),
                        &mut stato.opzioni,
                        *mode,
                        *case,
                        labels,
                    );
                    riga_ambito(ui, ws.active_mut(), &mut stato.opzioni, *all_tabs, labels);
                    if !status.is_empty() {
                        ui.colored_label(avviso, status.as_str());
                    }
                    ui.horizontal(|ui| {
                        if pulsante(
                            ui,
                            &mut stato.pulsanti,
                            if *ready { apply } else { preview },
                        ) {
                            ws.active_mut().dialog_submit();
                        }
                        if pulsante(ui, &mut stato.pulsanti, &t_cancel) {
                            ws.active_mut().dialog_cancel();
                        }
                    });
                })
                .map(|w| w.response.rect);
        }
        // il salvataggio in corso: il nome del file, la barra che dice a che punto e', e annulla
        DialogView::Saving { text, percent, .. } => {
            rect = egui::Window::new(titolo.as_str())
                .collapsible(false)
                .resizable(false)
                .min_width(360.0)
                .pivot(Align2::CENTER_CENTER)
                .default_pos(ctx.screen_rect().center())
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

/// La riga delle opzioni di Cerca e Sostituisci (come `opzioni_ricerca` del TUI): l'etichetta del modo, le tre scelte
/// (testo, glob, regex) e la casella delle maiuscole. I valori vengono dall'App e i clic tornano a lei
/// (`dialog_set_mode`, `dialog_set_case`); i rettangoli delle scelte finiscono in `opzioni` con la loro etichetta,
/// nell'ordine di disegno.
fn riga_opzioni(
    ui: &mut egui::Ui,
    app: &mut App,
    opzioni: &mut Vec<(egui::Rect, String)>,
    modo: Mode,
    caso: bool,
    labels: &SearchLabels,
) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(&labels.mode).weak());
        let mut corrente = modo;
        for (scelta, etichetta) in [
            (Mode::Text, labels.text.as_str()),
            (Mode::Glob, labels.glob.as_str()),
            (Mode::Regex, labels.regex.as_str()),
        ] {
            let r = ui.radio_value(&mut corrente, scelta, etichetta);
            opzioni.push((r.rect, etichetta.to_string()));
            if r.changed() {
                app.dialog_set_mode(corrente);
            }
        }
        let mut maiuscole = caso;
        let r = ui.checkbox(&mut maiuscole, labels.case.as_str());
        opzioni.push((r.rect, labels.case.clone()));
        if r.changed() {
            app.dialog_set_case(maiuscole);
        }
    });
}

/// La riga dell'ambito di Sostituisci (questa scheda o tutte): come le opzioni, i clic tornano all'App con
/// `dialog_set_scope_all`.
fn riga_ambito(
    ui: &mut egui::Ui,
    app: &mut App,
    opzioni: &mut Vec<(egui::Rect, String)>,
    tutte: bool,
    labels: &SearchLabels,
) {
    ui.horizontal(|ui| {
        let mut corrente = tutte;
        let r = ui.radio_value(&mut corrente, false, labels.scope_one.as_str());
        opzioni.push((r.rect, labels.scope_one.clone()));
        if r.changed() {
            app.dialog_set_scope_all(false);
        }
        let r = ui.radio_value(&mut corrente, true, labels.scope_all.as_str());
        opzioni.push((r.rect, labels.scope_all.clone()));
        if r.changed() {
            app.dialog_set_scope_all(true);
        }
    });
}
