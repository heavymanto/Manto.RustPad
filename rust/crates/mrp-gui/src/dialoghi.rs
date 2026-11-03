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
    /// Il rettangolo della X della testata nell'ultimo disegno (`None`: non c'e'); non finisce in `pulsanti`
    /// perche' chiude il dialogo, non e' un'azione sua (per i test, G1).
    pub chiudi: Option<egui::Rect>,
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
    /// Se la X della testata e' stata cliccata in questo giro: alla fine del disegno vale come Annulla.
    chiusura_richiesta: bool,
    /// Nel dialogo del formato orario: "formato personalizzato" scelto (`None`: ancora da decidere dal pattern in uso).
    personalizzato: Option<bool>,
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
        stato.personalizzato = None;
    }
    // dei ricordi del giro scorso restano solo testo e fuoco del campo e la voce del menu: il resto si ridisegna
    stato.pulsanti.clear();
    stato.voci.clear();
    stato.campo = None;
    stato.campi.clear();
    stato.opzioni.clear();
    stato.chiudi = None;
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
        | DialogView::SaveEncoding { title, .. }
        | DialogView::TimeRange { title, .. }
        | DialogView::TimeMask { title, .. }
        | DialogView::IndexOff { title, .. }
        | DialogView::Saving { title, .. } => title.clone(),
    };
    // il rettangolo della finestra: lo danno le varianti (una per una), lo legge la fine
    let rect;
    match &vista {
        // l'aiuto: due colonne (il tasto in monospace, la descrizione), in una lista che occupa al piu' il 70% dello
        // schermo; sotto, il pulsante che chiude
        DialogView::Help { rows, .. } => {
            rect = finestra(
                ctx,
                titolo.as_str(),
                360.0,
                &mut stato.chiudi,
                &mut stato.chiusura_richiesta,
                |ui| {
                    ScrollArea::vertical()
                        .scroll_source(
                            egui::scroll_area::ScrollSource::SCROLL_BAR
                                | egui::scroll_area::ScrollSource::MOUSE_WHEEL,
                        )
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
                    if pulsanti_centrati(ui, &mut stato.pulsanti, &[t_ok.as_str()]) == Some(0) {
                        ws.active_mut().dialog_cancel();
                    }
                },
            );
        }
        // il menu (tema, lingua, codifica): una riga per voce, col nome e, a fianco, il dettaglio in testo debole, in
        // una lista che occupa al piu' il 60% dello schermo; sotto, l'eventuale nota. Tutta la riga sceglie la voce
        DialogView::Menu {
            items,
            selected,
            note,
            ..
        } => {
            rect = finestra(
                ctx,
                titolo.as_str(),
                420.0,
                &mut stato.chiudi,
                &mut stato.chiusura_richiesta,
                |ui| {
                    ScrollArea::vertical()
                        .scroll_source(
                            egui::scroll_area::ScrollSource::SCROLL_BAR
                                | egui::scroll_area::ScrollSource::MOUSE_WHEEL,
                        )
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
                                // un clic sceglie la voce (la evidenzia); la scelta si applica con OK (o con Invio)
                                if clic.clicked() {
                                    ws.active_mut().dialog_select(i);
                                }
                                stato.voci.push(rettangolo);
                            }
                        });
                    if let Some(n) = note {
                        ui.label(RichText::new(n).weak());
                    }
                    match pulsanti_centrati(
                        ui,
                        &mut stato.pulsanti,
                        &[t_ok.as_str(), t_cancel.as_str()],
                    ) {
                        Some(0) => ws.active_mut().dialog_submit(),
                        Some(1) => ws.active_mut().dialog_cancel(),
                        _ => {}
                    }
                },
            );
            stato.ultima_voce = Some(*selected);
        }
        // il prompt (vai a, apri, salva con nome): il suggerimento, il campo largo quanto la finestra e i due pulsanti.
        // Il campo prende il fuoco appena il dialogo si apre e mostra sempre il testo dell'App: se l'utente lo cambia,
        // il nuovo testo torna all'App; Invio nel campo conferma (e il campo lascia andare il fuoco: e' egui)
        DialogView::Prompt { hint, text, .. } => {
            if !stato.campo_fuoco {
                stato.testo = text.clone();
            }
            rect = finestra(
                ctx,
                titolo.as_str(),
                360.0,
                &mut stato.chiudi,
                &mut stato.chiusura_richiesta,
                |ui| {
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
                    match pulsanti_centrati(
                        ui,
                        &mut stato.pulsanti,
                        &[t_ok.as_str(), t_cancel.as_str()],
                    ) {
                        Some(0) => ws.active_mut().dialog_submit(),
                        Some(1) => ws.active_mut().dialog_cancel(),
                        _ => {}
                    }
                },
            );
        }
        // la codifica del file di "salva con nome": la lista delle codifiche (un clic evidenzia la voce, non salva), la casella
        // del BOM (solo per le codifiche che lo hanno) e i due pulsanti
        DialogView::SaveEncoding {
            items,
            selected,
            bom,
            bom_label,
            ok,
            cancel,
            ..
        } => {
            rect = finestra(
                ctx,
                titolo.as_str(),
                420.0,
                &mut stato.chiudi,
                &mut stato.chiusura_richiesta,
                |ui| {
                    ScrollArea::vertical()
                        .scroll_source(
                            egui::scroll_area::ScrollSource::SCROLL_BAR
                                | egui::scroll_area::ScrollSource::MOUSE_WHEEL,
                        )
                        .max_height(altezza * 0.5)
                        .show(ui, |ui| {
                            for (i, voce) in items.iter().enumerate() {
                                let r = ui.selectable_label(i == *selected, &voce.label);
                                // la voce evidenziata si porta in vista quando cambia
                                if i == *selected && stato.ultima_voce != Some(*selected) {
                                    r.scroll_to_me(None);
                                }
                                if r.clicked() {
                                    ws.active_mut().dialog_select(i);
                                }
                                stato.voci.push(r.rect);
                            }
                        });
                    if let Some(acceso) = bom {
                        let mut v = *acceso;
                        let r = ui.checkbox(&mut v, bom_label.as_str());
                        stato.opzioni.push((r.rect, bom_label.clone()));
                        if r.changed() {
                            ws.active_mut().dialog_set_bom(v);
                        }
                    }
                    match pulsanti_centrati(
                        ui,
                        &mut stato.pulsanti,
                        &[ok.as_str(), cancel.as_str()],
                    ) {
                        Some(0) => ws.active_mut().dialog_submit(),
                        Some(1) => ws.active_mut().dialog_cancel(),
                        _ => {}
                    }
                },
            );
            stato.ultima_voce = Some(*selected);
        }
        // spegnere l'indice righe: la spiegazione, le funzioni che non ci saranno piu', la nota e i pulsanti OK / Annulla
        DialogView::IndexOff {
            text,
            features,
            note,
            ..
        } => {
            rect = finestra(
                ctx,
                titolo.as_str(),
                480.0,
                &mut stato.chiudi,
                &mut stato.chiusura_richiesta,
                |ui| {
                    ui.label(text.as_str());
                    ui.add_space(4.0);
                    for f in features {
                        ui.label(format!("\u{2022}  {f}"));
                    }
                    ui.add_space(6.0);
                    ui.label(RichText::new(note).weak());
                    ui.add_space(4.0);
                    match pulsanti_centrati(
                        ui,
                        &mut stato.pulsanti,
                        &[t_ok.as_str(), t_cancel.as_str()],
                    ) {
                        Some(0) => ws.active_mut().dialog_submit(),
                        Some(1) => ws.active_mut().dialog_cancel(),
                        _ => {}
                    }
                },
            );
        }
        // l'intervallo di tempo: il suggerimento, i due campi (da, a), l'errore e i pulsanti (OK, Togli intervallo se c'e', Annulla)
        DialogView::TimeRange {
            hint,
            from_label,
            to_label,
            from,
            to,
            field,
            error,
            clear,
            ..
        } => {
            if !stato.campo_fuoco {
                stato.testo = from.clone();
                stato.testo_due = to.clone();
            }
            let avviso = ctx.style().visuals.warn_fg_color;
            rect = finestra(
                ctx,
                titolo.as_str(),
                420.0,
                &mut stato.chiudi,
                &mut stato.chiusura_richiesta,
                |ui| {
                    ui.label(RichText::new(hint).weak());
                    egui::Grid::new("intervallo").num_columns(2).show(ui, |ui| {
                        for i in 0..2u8 {
                            let (etichetta, testo) = if i == 0 {
                                (from_label.as_str(), &mut stato.testo)
                            } else {
                                (to_label.as_str(), &mut stato.testo_due)
                            };
                            ui.label(etichetta);
                            let r =
                                ui.add(TextEdit::singleline(testo).desired_width(f32::INFINITY));
                            stato.campi.push(r.rect);
                            if stato.campo.is_none() {
                                stato.campo = Some(r.rect);
                            }
                            if r.has_focus() && *field != i {
                                ws.active_mut().dialog_focus(i);
                            }
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
                    if !error.is_empty() {
                        ui.colored_label(avviso, error.as_str());
                    }
                    let mut testi = vec![t_ok.as_str()];
                    if let Some(c) = clear {
                        testi.push(c.as_str());
                    }
                    testi.push(t_cancel.as_str());
                    let con_togli = clear.is_some();
                    match pulsanti_centrati(ui, &mut stato.pulsanti, &testi) {
                        Some(0) => ws.active_mut().dialog_submit(),
                        Some(1) if con_togli => ws.active_mut().dialog_time_clear(),
                        Some(_) => ws.active_mut().dialog_cancel(),
                        None => {}
                    }
                },
            );
        }
        // il formato dell'orario: in alto cosa si e' riconosciuto in questo file, poi l'elenco dei formati noti (quelli che leggono
        // questo file per primi, con quante righe leggono), un "formato personalizzato" che si apre solo se serve, e come il
        // formato scelto legge le prime righe del file
        DialogView::TimeMask {
            pattern_label,
            within_label,
            pattern,
            within,
            field,
            error,
            preview,
            auto,
            presets,
            preset,
            matches,
            detected,
            ..
        } => {
            if !stato.campo_fuoco {
                stato.testo = pattern.clone();
                stato.testo_due = within.clone();
            }
            let avviso = ctx.style().visuals.warn_fg_color;
            let (t_riconosciuto, t_nessuno, t_scegli, t_personale, t_prime, t_entro) = {
                let a = ws.active();
                (
                    a.text("time.mask.detected").to_string(),
                    a.text("time.mask.not_detected").to_string(),
                    a.text("time.mask.pick").to_string(),
                    a.text("time.mask.custom").to_string(),
                    a.text("time.mask.preview2").to_string(),
                    a.text("time.mask.within_long").to_string(),
                )
            };
            // i formati che leggono questo file prima, poi gli altri (ognuno nel suo ordine)
            let mut ordine: Vec<usize> = (0..presets.len()).collect();
            ordine.sort_by_key(|&i| (matches.get(i).copied().unwrap_or(0) == 0, i));
            // la riga di ogni formato nel menu a tendina: l'orario com'e' nel log, il pattern e quante righe legge
            let legge: Vec<bool> = (0..presets.len())
                .map(|i| matches.get(i).copied().unwrap_or(0) > 0)
                .collect();
            let etichette: Vec<String> = (0..presets.len())
                .map(|i| {
                    let (pat, esempio) = &presets[i];
                    match matches.get(i).copied().unwrap_or(0) {
                        0 => format!("{esempio}    {pat}"),
                        n => format!(
                            "{esempio}    {pat}    {}",
                            ws.active().text_with("time.mask.reads", &[&n.to_string()])
                        ),
                    }
                })
                .collect();
            // "formato personalizzato" scelto: all'apertura lo e' se il pattern in uso non e' uno dei noti
            let mut pers = *stato
                .personalizzato
                .get_or_insert_with(|| !presets.iter().any(|(p, _)| p == pattern));
            rect = finestra(
                ctx,
                titolo.as_str(),
                620.0,
                &mut stato.chiudi,
                &mut stato.chiusura_richiesta,
                |ui| {
                    // cosa si e' riconosciuto
                    match detected {
                        Some((tempo, pat)) => {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(&t_riconosciuto).strong());
                                ui.label(RichText::new(tempo).monospace());
                                ui.label(RichText::new(format!("({pat})")).weak());
                            });
                        }
                        None => {
                            ui.colored_label(avviso, t_nessuno.as_str());
                        }
                    }
                    ui.add_space(4.0);
                    ui.label(RichText::new(&t_scegli).weak());
                    // un menu a tendina: "formato personalizzato" in cima, poi i formati noti (quelli che leggono questo
                    // file per primi)
                    let scelto = if pers {
                        t_personale.clone()
                    } else {
                        etichette[(*preset).min(etichette.len() - 1)].clone()
                    };
                    let mut nuovo: Option<Option<usize>> = None;
                    let combo = egui::ComboBox::from_id_salt("formato_orario")
                        .width(ui.available_width() - 8.0)
                        .selected_text(RichText::new(scelto).monospace())
                        .show_ui(ui, |ui| {
                            let r = ui.selectable_label(pers, RichText::new(&t_personale).strong());
                            stato.voci.push(r.rect);
                            if r.clicked() {
                                nuovo = Some(None);
                            }
                            ui.separator();
                            for &i in &ordine {
                                let mut testo = RichText::new(&etichette[i]).monospace();
                                if !legge[i] {
                                    testo = testo.weak();
                                }
                                let r = ui.selectable_label(!pers && i == *preset, testo);
                                stato.voci.push(r.rect);
                                if r.clicked() {
                                    nuovo = Some(Some(i));
                                }
                            }
                        });
                    stato
                        .opzioni
                        .push((combo.response.rect, "formato".to_string()));
                    match nuovo {
                        Some(None) => {
                            pers = true;
                            // il fuoco va al campo del formato
                            stato.campo_fuoco = false;
                        }
                        Some(Some(i)) => {
                            pers = false;
                            ws.active_mut().dialog_time_preset(i);
                            // i campi si rileggono dall'App al prossimo giro
                            stato.campo_fuoco = false;
                        }
                        None => {}
                    }
                    stato.personalizzato = Some(pers);
                    // i campi del formato solo se lo si scrive a mano (o c'e' un errore da correggere)
                    let campi_visibili = pers || !error.is_empty();
                    if campi_visibili {
                        ui.add_space(4.0);
                        egui::Grid::new("formato").num_columns(2).show(ui, |ui| {
                            for i in 0..2u8 {
                                let (etichetta, testo) = if i == 0 {
                                    (pattern_label.as_str(), &mut stato.testo)
                                } else {
                                    (within_label.as_str(), &mut stato.testo_due)
                                };
                                ui.label(etichetta);
                                let r = ui
                                    .add(TextEdit::singleline(testo).desired_width(f32::INFINITY));
                                stato.campi.push(r.rect);
                                if stato.campo.is_none() {
                                    stato.campo = Some(r.rect);
                                }
                                if r.has_focus() && *field != i {
                                    ws.active_mut().dialog_focus(i);
                                }
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
                        ui.label(RichText::new(&t_entro).weak());
                    } else {
                        // senza campi Invio conferma (a meno che la tendina sia aperta e lo usi per scegliere)
                        if !egui::Popup::is_any_open(ui.ctx())
                            && ui.input(|i| i.key_pressed(egui::Key::Enter))
                        {
                            ws.active_mut().dialog_submit();
                        }
                    }
                    // come il formato scelto legge le prime righe del file
                    ui.add_space(4.0);
                    ui.label(RichText::new(&t_prime).weak());
                    egui::Grid::new("formato_anteprima")
                        .num_columns(3)
                        .show(ui, |ui| {
                            for (testo, letto) in preview {
                                ui.label(RichText::new(testo).monospace());
                                ui.label("=>");
                                ui.label(RichText::new(letto).strong());
                                ui.end_row();
                            }
                        });
                    if !error.is_empty() {
                        ui.colored_label(avviso, error.as_str());
                    }
                    match pulsanti_centrati(
                        ui,
                        &mut stato.pulsanti,
                        &[t_ok.as_str(), auto.as_str(), t_cancel.as_str()],
                    ) {
                        Some(0) => ws.active_mut().dialog_submit(),
                        Some(1) => {
                            ws.active_mut().dialog_time_auto();
                            // i campi si rileggono dall'App al prossimo giro
                            stato.campo_fuoco = false;
                        }
                        Some(_) => ws.active_mut().dialog_cancel(),
                        None => {}
                    }
                },
            );
        }
        // il file scelto per "salva con nome" esiste gia': la domanda e i due pulsanti
        DialogView::Overwrite { text, .. } => {
            rect = finestra(
                ctx,
                titolo.as_str(),
                360.0,
                &mut stato.chiudi,
                &mut stato.chiusura_richiesta,
                |ui| {
                    ui.label(text.as_str());
                    match pulsanti_centrati(
                        ui,
                        &mut stato.pulsanti,
                        &[t_ok.as_str(), t_cancel.as_str()],
                    ) {
                        Some(0) => ws.active_mut().dialog_submit(),
                        Some(1) => ws.active_mut().dialog_cancel(),
                        _ => {}
                    }
                },
            );
        }
        // la conferma di uscita con modifiche non salvate: la domanda e le tre scelte, nell'ordine della vista
        DialogView::Confirm {
            text,
            save,
            discard,
            cancel,
            ..
        } => {
            rect = finestra(
                ctx,
                titolo.as_str(),
                360.0,
                &mut stato.chiudi,
                &mut stato.chiusura_richiesta,
                |ui| {
                    ui.label(text.as_str());
                    match pulsanti_centrati(
                        ui,
                        &mut stato.pulsanti,
                        &[save.as_str(), discard.as_str(), cancel.as_str()],
                    ) {
                        Some(0) => ws.active_mut().dialog_confirm(ConfirmChoice::Save),
                        Some(1) => ws.active_mut().dialog_confirm(ConfirmChoice::Discard),
                        Some(2) => ws.active_mut().dialog_confirm(ConfirmChoice::Cancel),
                        _ => {}
                    }
                },
            );
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
            rect = finestra(
                ctx,
                titolo.as_str(),
                380.0,
                &mut stato.chiudi,
                &mut stato.chiusura_richiesta,
                |ui| {
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
                    match pulsanti_centrati(
                        ui,
                        &mut stato.pulsanti,
                        &[t_ok.as_str(), t_cancel.as_str()],
                    ) {
                        Some(0) => ws.active_mut().dialog_submit(),
                        Some(1) => ws.active_mut().dialog_cancel(),
                        _ => {}
                    }
                },
            );
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
            rect = finestra(
                ctx,
                titolo.as_str(),
                420.0,
                &mut stato.chiudi,
                &mut stato.chiusura_richiesta,
                |ui| {
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
                    match pulsanti_centrati(
                        ui,
                        &mut stato.pulsanti,
                        &[go.as_str(), t_cancel.as_str()],
                    ) {
                        Some(0) => ws.active_mut().dialog_submit(),
                        Some(1) => ws.active_mut().dialog_cancel(),
                        _ => {}
                    }
                },
            );
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
            rect = finestra(
                ctx,
                titolo.as_str(),
                420.0,
                &mut stato.chiudi,
                &mut stato.chiusura_richiesta,
                |ui| {
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
                    // il primo pulsante dice cosa fa adesso: l'anteprima, o l'applica se e' gia' pronta
                    let primo = if *ready { apply } else { preview };
                    match pulsanti_centrati(
                        ui,
                        &mut stato.pulsanti,
                        &[primo.as_str(), t_cancel.as_str()],
                    ) {
                        Some(0) => ws.active_mut().dialog_submit(),
                        Some(1) => ws.active_mut().dialog_cancel(),
                        _ => {}
                    }
                },
            );
        }
        // il salvataggio in corso: il nome del file, la barra che dice a che punto e', e annulla
        DialogView::Saving { text, percent, .. } => {
            rect = finestra(
                ctx,
                titolo.as_str(),
                360.0,
                &mut stato.chiudi,
                &mut stato.chiusura_richiesta,
                |ui| {
                    ui.label(text.as_str());
                    ui.add(egui::ProgressBar::new(f32::from(*percent) / 100.0));
                    if pulsanti_centrati(ui, &mut stato.pulsanti, &[t_cancel.as_str()]) == Some(0) {
                        ws.active_mut().dialog_cancel();
                    }
                },
            );
        }
    }
    // la X della testata chiude il dialogo come Annulla / Esc (nella conferma di uscita vale Annulla)
    if stato.chiusura_richiesta {
        stato.chiusura_richiesta = false;
        ws.active_mut().dialog_cancel();
    }
    stato.rect = rect;
    stato.titolo = Some(titolo);
}

/// La finestra di un dialogo (G1): come le `egui::Window` di prima, ma senza la barra del titolo di
/// egui (che la X non ce l'ha) e con la testata nostra sopra il corpo: cosi' ogni finestrella ha la
/// X in alto a destra, che vale Annulla. `chiudi` riceve il rettangolo della X e
/// `chiusura_richiesta` dice se e' stata cliccata (li chiede chi chiama, che li registra nel suo
/// stato); il corpo lo disegna `contenuto`. Restituisce il rettangolo della finestra.
fn finestra(
    ctx: &egui::Context,
    titolo: &str,
    larghezza_min: f32,
    chiudi: &mut Option<egui::Rect>,
    chiusura_richiesta: &mut bool,
    contenuto: impl FnOnce(&mut egui::Ui),
) -> Option<egui::Rect> {
    egui::Window::new(titolo)
        .title_bar(false)
        .collapsible(false)
        .resizable(false)
        .min_width(larghezza_min)
        .pivot(Align2::CENTER_CENTER)
        .default_pos(ctx.screen_rect().center())
        .show(ctx, |ui| {
            let tav = crate::stile::corrente(ui.ctx());
            crate::stile::popup(ui.style_mut(), &tav);
            let (rettangolo, cliccata) = testata(ui, titolo);
            *chiudi = Some(rettangolo);
            if cliccata {
                *chiusura_richiesta = true;
            }
            contenuto(ui);
        })
        .map(|w| w.response.rect)
}

/// La testata di una finestra senza barra del titolo (G1, G2): una banda col risalto del tema (in tutti e quattro i temi il titolo
/// si stacca dal corpo) con il titolo forte a sinistra e la X che chiude a destra. Restituisce il rettangolo della X e se e'
/// stata cliccata. La usa `finestra` e la finestra delle impostazioni.
pub(crate) fn testata(ui: &mut egui::Ui, titolo: &str) -> (egui::Rect, bool) {
    let tav = crate::stile::corrente(ui.ctx());
    let mut rettangolo = egui::Rect::ZERO;
    let mut cliccata = false;
    // la banda si dipinge dopo, quando si sa quanto e' alta la riga: qui se ne prenota il posto (sta dietro il testo)
    let posto_banda = ui.painter().add(egui::Shape::Noop);
    let margine = ui.spacing().window_margin;
    let angoli = ui.visuals().window_corner_radius;
    let riga = ui.horizontal(|ui| {
        ui.label(
            RichText::new(titolo)
                .strong()
                .size(17.0)
                .color(tav.risalto_fg),
        );
        // la X sta sull'orlo destro della riga
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.scope(|ui| {
                {
                    // col tema chiaro la X e' rossa (come nelle finestre di Windows); negli altri e' un riquadro piu' scuro
                    // della banda; al passaggio del mouse diventa rossa viva col testo bianco, in tutti i temi
                    let (fondo, testo) = if tav.scuro {
                        (
                            crate::stile::miscela(tav.risalto_bg, egui::Color32::BLACK, 0.3),
                            tav.risalto_fg,
                        )
                    } else {
                        (
                            egui::Color32::from_rgb(0xc4, 0x2b, 0x1c),
                            egui::Color32::WHITE,
                        )
                    };
                    let v = ui.visuals_mut();
                    v.widgets.inactive.weak_bg_fill = fondo;
                    v.widgets.inactive.bg_fill = fondo;
                    v.widgets.inactive.fg_stroke = egui::Stroke::new(1.0_f32, testo);
                    for vis in [&mut v.widgets.hovered, &mut v.widgets.active] {
                        vis.weak_bg_fill = egui::Color32::from_rgb(0xe8, 0x11, 0x23);
                        vis.bg_fill = egui::Color32::from_rgb(0xe8, 0x11, 0x23);
                        vis.fg_stroke = egui::Stroke::new(1.0_f32, egui::Color32::WHITE);
                    }
                }
                // la croce si disegna a mano: il glifo "✕" nel font non c'e' e uscirebbe un quadratino
                let r = ui.add(egui::Button::new("").min_size(egui::Vec2::new(28.0, 24.0)));
                let colore = ui.style().interact(&r).fg_stroke.color;
                let c = r.rect.center();
                for (a, b) in [(-1.0_f32, -1.0_f32), (-1.0, 1.0)] {
                    ui.painter().line_segment(
                        [
                            c + egui::Vec2::new(a * 5.0, b * 5.0),
                            c + egui::Vec2::new(-a * 5.0, -b * 5.0),
                        ],
                        egui::Stroke::new(1.6_f32, colore),
                    );
                }
                rettangolo = r.rect;
                cliccata = r.clicked();
            });
        });
    });
    // la banda va da un bordo all'altro della finestra (sopra i margini) e prende gli angoli alti della finestra
    let r = riga.response.rect;
    let area = ui.max_rect();
    let banda = egui::Rect::from_min_max(
        egui::pos2(
            area.min.x - f32::from(margine.left),
            r.min.y - f32::from(margine.top),
        ),
        egui::pos2(area.max.x + f32::from(margine.right), r.max.y + 5.0),
    );
    ui.painter().set(
        posto_banda,
        egui::Shape::rect_filled(
            banda,
            egui::CornerRadius {
                nw: angoli.nw,
                ne: angoli.ne,
                sw: 0,
                se: 0,
            },
            tav.risalto_bg,
        ),
    );
    ui.add_space(8.0);
    (rettangolo, cliccata)
}

/// La riga dei pulsanti di un dialogo (G1): grandi (almeno 120 x 34), tutti larghi come il testo
/// piu' lungo, il gruppo centrato nello spazio disponibile; il primo e' l'azione principale (pieno
/// di colore), gli altri neutri col contorno. Registra i rettangoli in `registro` da sinistra a
/// destra col loro testo, come faceva `pulsante`, e restituisce l'indice del pulsante cliccato.
pub(crate) fn pulsanti_centrati(
    ui: &mut egui::Ui,
    registro: &mut Vec<(egui::Rect, String)>,
    testi: &[&str],
) -> Option<usize> {
    // la larghezza di tutti: il testo piu' lungo deve starci comodo
    let mut larghezza = 120.0f32;
    for testo in testi {
        let galley = egui::WidgetText::from(RichText::new(*testo).size(16.0)).into_galley(
            ui,
            Some(egui::TextWrapMode::Extend),
            f32::INFINITY,
            egui::FontSelection::Default,
        );
        larghezza = larghezza.max(galley.size().x + 40.0);
    }
    let totale = larghezza * testi.len() as f32 + 12.0 * testi.len().saturating_sub(1) as f32;
    let mut cliccato = None;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 12.0;
        // il gruppo sta nel mezzo dello spazio disponibile
        ui.add_space(((ui.available_width() - totale) / 2.0).max(0.0));
        for (i, testo) in testi.iter().enumerate() {
            let (fondo, colore, contorno) = colori_pulsante(ui, i == 0);
            let r = ui.add_sized(
                egui::Vec2::new(larghezza, 34.0),
                egui::Button::new(RichText::new(*testo).size(16.0).color(colore))
                    .min_size(egui::Vec2::new(120.0, 34.0))
                    .fill(fondo)
                    .stroke(contorno),
            );
            registro.push((r.rect, testo.to_string()));
            if r.clicked() {
                cliccato = Some(i);
            }
        }
    });
    cliccato
}

/// I colori di un pulsante della riga (G1, G2): il principale ha il risalto del tema (nel tema Blu giallo su nero, in Debian
/// bianco su rosso, in Scuro e Chiaro bianco su blu), gli altri sono neutri col contorno e il testo del popup.
fn colori_pulsante(
    ui: &egui::Ui,
    principale: bool,
) -> (egui::Color32, egui::Color32, egui::Stroke) {
    let tav = crate::stile::corrente(ui.ctx());
    if principale {
        (tav.risalto_bg, tav.risalto_fg, egui::Stroke::NONE)
    } else {
        (
            tav.pulsante_neutro(),
            tav.popup_fg,
            egui::Stroke::new(
                1.0_f32,
                crate::stile::miscela(tav.popup_bg, tav.popup_fg, 0.45),
            ),
        )
    }
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
