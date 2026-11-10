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
    /// Nella finestra "Tempo": il calendario aperto sotto un campo, `(campo 2 o 3, anno, mese)` del mese che mostra.
    calendario: Option<(u8, i32, u32)>,
    /// Nella finestra "Tempo": i buffer dei campi Dal e Al (il padrone e' l'`App`, si rileggono a ogni giro).
    filtro: [String; 2],
    /// Dove sta il bottone del calendario aperto (il calendario si apre sotto di lui).
    cal_ancora: Option<egui::Rect>,
    /// Nella finestra "Tempo": il buffer dell'esempio da cui ricavare la regola (il padrone e' l'`App`).
    esempio: String,
    /// Nella finestra dell'aiuto: quale tab e' aperto (0 i tasti del programma, 1 le sue informazioni).
    aiuto_scheda: u8,
    /// Un calendario da aprire appena la finestra e' disegnata (uso da sviluppatore, per le schermate).
    cal_richiesto: Option<u8>,
    /// Nella finestra "Filtri": il buffer del testo del filtro per righe (il padrone e' l'`App`).
    ftesto: String,
}

impl StatoDialogo {
    /// Chiede di aprire il calendario del campo `campo` appena la finestra "Tempo" e' disegnata.
    pub fn apri_calendario(&mut self, campo: u8) {
        self.cal_richiesto = Some(campo);
    }
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
        stato.calendario = None;
        stato.aiuto_scheda = 0;
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
        | DialogView::Time { title, .. }
        | DialogView::IndexOff { title, .. }
        | DialogView::Saving { title, .. } => title.clone(),
    };
    // il rettangolo della finestra: lo danno le varianti (una per una), lo legge la fine
    let rect;
    match &vista {
        // l'aiuto: due tab (i tasti del programma, o chi e' il programma), la tabella in due colonne (il tasto in
        // monospace, la descrizione) in una lista che occupa al piu' il 70% dello schermo; sotto, il pulsante che chiude
        DialogView::Help { rows, info, .. } => {
            let t_tasti = ws.active().text("help.tab.keys").to_string();
            let t_tab_info = ws.active().text("help.tab.info").to_string();
            rect = finestra(
                ctx,
                titolo.as_str(),
                360.0,
                &mut stato.chiudi,
                &mut stato.chiusura_richiesta,
                |ui| {
                    // i due tab: i tasti, o le informazioni del programma
                    ui.horizontal(|ui| {
                        let r = ui.selectable_label(stato.aiuto_scheda == 0, &t_tasti);
                        stato.opzioni.push((r.rect, t_tasti.clone()));
                        if r.clicked() {
                            stato.aiuto_scheda = 0;
                        }
                        let r = ui.selectable_label(stato.aiuto_scheda == 1, &t_tab_info);
                        stato.opzioni.push((r.rect, t_tab_info.clone()));
                        if r.clicked() {
                            stato.aiuto_scheda = 1;
                        }
                    });
                    ui.separator();
                    if stato.aiuto_scheda == 0 {
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
                    } else {
                        // chi e': il nome, e sotto versione, licenza e codice (il sito e' un link vero,
                        // il clic lo apre nel browser)
                        ui.heading("Manto.RustPad");
                        ui.add_space(8.0);
                        for (etichetta, valore) in info {
                            ui.horizontal(|ui| {
                                ui.label(etichetta);
                                if valore.starts_with("http") {
                                    ui.hyperlink(valore.as_str());
                                } else {
                                    ui.monospace(valore.as_str());
                                }
                            });
                        }
                    }
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
        // la finestra "Tempo": il formato dell'orario in alto, sotto il filtro per data (casella, Dal / Al con calendario e X)
        DialogView::Time {
            presets,
            matches,
            pattern_label,
            within_label,
            pattern,
            within,
            skip,
            sources,
            source,
            example,
            example_problem,
            example_candidates,
            preview,
            auto,
            has_mask,
            has_date,
            from,
            to,
            check,
            timeline,
            from_hint,
            to_hint,
            field,
            error,
            levels,
            ftext,
            fmode,
            fcase,
            finvert,
            summary,
            ..
        } => {
            if !stato.campo_fuoco {
                stato.testo = pattern.clone();
                stato.testo_due = within.clone();
            }
            stato.filtro = [from.clone(), to.clone()];
            stato.ftesto = ftext.clone();
            let (t_sez, t_modi, t_maiusc, t_nascondi, t_aiuto_livelli, t_aiuto_testo, t_riassunto) = {
                let a = ws.active();
                (
                    [
                        a.text("filters.tab.time").to_string(),
                        a.text("filters.tab.levels").to_string(),
                        a.text("filters.tab.text").to_string(),
                    ],
                    [
                        a.text("filters.mode.text").to_string(),
                        a.text("filters.mode.glob").to_string(),
                        a.text("filters.mode.regex").to_string(),
                    ],
                    a.text("filters.case").to_string(),
                    a.text("filters.invert").to_string(),
                    a.text("filters.levels_help").to_string(),
                    a.text("filters.text_help").to_string(),
                    a.text("filters.summary").to_string(),
                )
            };
            // i titoli: grassetto su una banda appena piu' chiara (o scura) del fondo, calcolata dal tema: sta bene su ogni tema e font
            let (fondo_titolo, testo_titolo) = {
                let tav = crate::stile::corrente(ctx);
                (
                    crate::stile::miscela(tav.popup_bg, tav.popup_fg, 0.12),
                    tav.popup_fg,
                )
            };
            let colori_livelli = {
                let tav = crate::stile::corrente(ctx);
                [tav.log_errore, tav.log_avviso, tav.log_info, tav.log_debug]
            };
            let t_livelli: [String; 4] = [
                "ERROR".to_string(),
                "WARN".to_string(),
                "INFO".to_string(),
                "DEBUG".to_string(),
            ];
            // un riquadro ha una condizione attiva (il segno sul suo titolo)
            let attiva = [
                *check && !(from.trim().is_empty() && to.trim().is_empty()),
                levels.iter().any(|&x| x),
                !ftext.is_empty(),
            ];
            stato.esempio = example.clone();
            if let Some(c) = stato.cal_richiesto.take() {
                stato.calendario = ws
                    .active()
                    .time_calendar(c)
                    .map(|i| (c, i.month.0, i.month.1));
            }
            let avviso = ctx.style().visuals.warn_fg_color;
            let (t_nessuno, t_filtro, t_personale, t_prime, t_entro) = {
                let a = ws.active();
                (
                    a.text("time.mask.not_detected").to_string(),
                    a.text("time.section.filter").to_string(),
                    a.text("time.mask.custom").to_string(),
                    a.text("time.mask.preview2").to_string(),
                    a.text("time.mask.within_long").to_string(),
                )
            };
            let (t_attivo, t_dal, t_al, t_serve, t_nessun_limite, t_svuota, t_data, t_scegli) = {
                let a = ws.active();
                (
                    a.text("time.filter.active").to_string(),
                    a.text("time.filter.from").to_string(),
                    a.text("time.filter.to").to_string(),
                    a.text("time.filter.need_format").to_string(),
                    a.text("time.filter.no_limit").to_string(),
                    a.text("time.filter.clear_tip").to_string(),
                    a.text("time.filter.cal_tip").to_string(),
                    a.text("time.filter.pick_format").to_string(),
                )
            };
            let nomi_mesi: Vec<String> = (1..=12)
                .map(|m| ws.active().text(&format!("cal.month.{m}")).to_string())
                .collect();
            let t_occorrenza = ws.active().text("time.mask.occurrence").to_string();
            let t_esempio_titolo = ws.active().text("time.example.title").to_string();
            let t_esempio_aiuto = ws.active().text("time.example.hint").to_string();
            let t_trova = ws.active().text("time.example.button").to_string();
            let t_titolo_fonti = ws.active().text("time.source.title").to_string();
            let t_altro = ws.active().text("time.source.other").to_string();
            let t_togli = ws.active().text("time.source.remove").to_string();
            let t_aiuto_regola = ws.active().text("time.source.rule_help").to_string();
            let righe_fonti: Vec<String> = sources
                .iter()
                .map(|(_, _, n, _)| {
                    ws.active()
                        .text_with("time.source.lines", &[&n.to_string()])
                })
                .collect();
            let t_timeline = ws.active().text("time.filter.timeline").to_string();
            let t_ora = ws.active().text("time.filter.time").to_string();
            let t_fatto = ws.active().text("time.filter.done").to_string();
            let giorni_sett: Vec<String> = [
                "cal.mon", "cal.tue", "cal.wed", "cal.thu", "cal.fri", "cal.sat", "cal.sun",
            ]
            .iter()
            .map(|k| ws.active().text(k).to_string())
            .collect();
            // i formati che leggono questo file prima, poi gli altri (ognuno nel suo ordine)
            let mut ordine: Vec<usize> = (0..presets.len()).collect();
            ordine.sort_by_key(|&i| (matches.get(i).copied().unwrap_or(0) == 0, i));
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
            // "Altro" e' scelto se il tempo scritto adesso non e' uno di quelli che il file offre
            let mut pers = *stato
                .personalizzato
                .get_or_insert_with(|| source.is_none() || !example.is_empty());
            rect = finestra(
                ctx,
                titolo.as_str(),
                620.0,
                &mut stato.chiudi,
                &mut stato.chiusura_richiesta,
                |ui| {
                    // tre riquadri nella stessa pagina: Tempo, Livelli, Testo (un punto sul titolo dice che ha una condizione attiva)
                    let titolo_riquadro = |ui: &mut egui::Ui, i: usize| -> egui::Rect {
                        let testo = if attiva[i] {
                            format!("{} \u{25cf}", t_sez[i])
                        } else {
                            t_sez[i].clone()
                        };
                        crate::strumenti::titolo_banda(ui, &testo, fondo_titolo, testo_titolo)
                    };
                    // se lo schermo e' basso (e "Altro" e' aperto) i riquadri scorrono, il titolo e i pulsanti restano
                    let altezza_max = (ctx.screen_rect().height() - 160.0).max(240.0);
                    egui::ScrollArea::vertical()
                        .max_height(altezza_max)
                        .auto_shrink([false, true])
                        .show(ui, |ui| {
                            ui.spacing_mut().item_spacing.y = 6.0;
                            egui::Frame::group(ui.style())
                                .inner_margin(egui::Margin::symmetric(12, 8))
                                .outer_margin(egui::Margin::symmetric(0, 4))
                                .show(ui, |ui| {
                                    ui.set_width(ui.available_width());
                                    let t = titolo_riquadro(ui, 0);
                                    stato.opzioni.push((t, "sezione 0".to_string()));
                                    ui.add_space(6.0);
                                    {
                                        // 1. quale tempo usare: l'elenco di quelli che il file offre, e "Altro" con la regola scritta a mano;
                                        // sulla stessa riga del titolo, a destra, il piccolo pulsante che fa riconoscere di nuovo il tempo
                                        let mut riconosci = false;
                                        ui.horizontal(|ui| {
                                            crate::strumenti::grassetto(
                                                ui,
                                                &t_titolo_fonti,
                                                testo_titolo,
                                            );
                                            ui.with_layout(
                                                egui::Layout::right_to_left(egui::Align::Center),
                                                |ui| {
                                                    let r = ui.small_button(auto.as_str());
                                                    stato
                                                        .opzioni
                                                        .push((r.rect, "riconosci".to_string()));
                                                    riconosci = r.clicked();
                                                },
                                            );
                                        });
                                        if riconosci {
                                            ws.active_mut().dialog_time_auto();
                                            stato.campo_fuoco = false;
                                        }
                                        if sources.is_empty() {
                                            ui.colored_label(avviso, t_nessuno.as_str());
                                        }
                                        // una tendina sola: i tempi che il file offre (le "mie regole" con la X per toglierle) e in fondo "Altro"
                                        let mut scelta_fonte: Option<usize> = None;
                                        let mut togli_regola: Option<usize> = None;
                                        let mut scelto_altro = false;
                                        let in_uso_testo = match (pers, *source) {
                                            (false, Some(i)) if i < sources.len() => {
                                                format!("{}    {}", sources[i].0, sources[i].1)
                                            }
                                            _ => t_altro.clone(),
                                        };
                                        let tendina = egui::ComboBox::from_id_salt("fonte_tempo")
                                            .width(ui.available_width() - 8.0)
                                            .selected_text(RichText::new(in_uso_testo))
                                            .show_ui(ui, |ui| {
                                                for (i, (etichetta, esempio, _, e_regola)) in
                                                    sources.iter().enumerate()
                                                {
                                                    ui.horizontal(|ui| {
                                                        let testo = format!(
                                                            "{etichetta}    {esempio}    {}",
                                                            righe_fonti[i]
                                                        );
                                                        let r = ui.selectable_label(
                                                            !pers && *source == Some(i),
                                                            testo,
                                                        );
                                                        stato
                                                            .opzioni
                                                            .push((r.rect, format!("fonte {i}")));
                                                        if r.clicked() {
                                                            scelta_fonte = Some(i);
                                                        }
                                                        if *e_regola {
                                                            let x = ui
                                                                .add(
                                                                    egui::Button::new("")
                                                                        .frame(false)
                                                                        .min_size(egui::Vec2::new(
                                                                            20.0, 18.0,
                                                                        )),
                                                                )
                                                                .on_hover_text(t_togli.as_str());
                                                            let colore = ui
                                                                .style()
                                                                .interact(&x)
                                                                .fg_stroke
                                                                .color;
                                                            crate::strumenti::croce(
                                                                ui.painter(),
                                                                x.rect.center(),
                                                                colore,
                                                                3.5,
                                                            );
                                                            stato.opzioni.push((
                                                                x.rect,
                                                                format!("togli {i}"),
                                                            ));
                                                            if x.clicked() {
                                                                togli_regola = Some(i);
                                                            }
                                                        }
                                                    });
                                                }
                                                ui.separator();
                                                let r = ui.selectable_label(pers, t_altro.as_str());
                                                stato.opzioni.push((r.rect, "altro".to_string()));
                                                if r.clicked() {
                                                    scelto_altro = true;
                                                }
                                            });
                                        stato
                                            .opzioni
                                            .push((tendina.response.rect, "fonti".to_string()));
                                        if let Some(i) = togli_regola {
                                            ws.active_mut().dialog_time_remove_rule(i);
                                            stato.campo_fuoco = false;
                                        } else if let Some(i) = scelta_fonte {
                                            pers = false;
                                            ws.active_mut().dialog_time_pick_source(i);
                                            stato.campo_fuoco = false;
                                        } else if scelto_altro {
                                            pers = true;
                                            stato.campo_fuoco = false;
                                        }
                                        stato.personalizzato = Some(pers);
                                        // "Altro": un formato fra quelli noti, oppure la regola scritta a mano
                                        if pers || (!error.is_empty() && !*has_mask) {
                                            ui.indent("altro_regola", |ui| {
                                                ui.label(RichText::new(&t_scegli).weak());
                                                let in_uso = (*skip == 0)
                                                    .then(|| {
                                                        presets
                                                            .iter()
                                                            .position(|(p, _)| p == pattern)
                                                    })
                                                    .flatten();
                                                let scelto = in_uso.map_or_else(
                                                    || t_personale.clone(),
                                                    |i| etichette[i].clone(),
                                                );
                                                let mut nuovo: Option<usize> = None;
                                                let combo =
                                                    egui::ComboBox::from_id_salt("formato_orario")
                                                        .width(ui.available_width() - 8.0)
                                                        .selected_text(
                                                            RichText::new(scelto).monospace(),
                                                        )
                                                        .show_ui(ui, |ui| {
                                                            for &i in &ordine {
                                                                let mut testo =
                                                                    RichText::new(&etichette[i])
                                                                        .monospace();
                                                                if !legge[i] {
                                                                    testo = testo.weak();
                                                                }
                                                                let r = ui.selectable_label(
                                                                    in_uso == Some(i),
                                                                    testo,
                                                                );
                                                                stato.voci.push(r.rect);
                                                                if r.clicked() {
                                                                    nuovo = Some(i);
                                                                }
                                                            }
                                                        });
                                                stato.opzioni.push((
                                                    combo.response.rect,
                                                    "formato".to_string(),
                                                ));
                                                if let Some(i) = nuovo {
                                                    ws.active_mut().dialog_time_preset(i);
                                                    stato.campo_fuoco = false;
                                                }
                                                ui.add_space(4.0);
                                                egui::Grid::new("formato").num_columns(2).show(
                                                    ui,
                                                    |ui| {
                                                        for i in 0..2u8 {
                                                            let (etichetta, testo) = if i == 0 {
                                                                (
                                                                    pattern_label.as_str(),
                                                                    &mut stato.testo,
                                                                )
                                                            } else {
                                                                (
                                                                    within_label.as_str(),
                                                                    &mut stato.testo_due,
                                                                )
                                                            };
                                                            ui.label(etichetta);
                                                            let r = ui.add(
                                                                TextEdit::singleline(testo)
                                                                    .desired_width(f32::INFINITY),
                                                            );
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
                                                                ws.active_mut()
                                                                    .dialog_set_field(i, testo);
                                                            }
                                                            if r.lost_focus()
                                                                && ui.input(|i| {
                                                                    i.key_pressed(egui::Key::Enter)
                                                                })
                                                            {
                                                                ws.active_mut().dialog_submit();
                                                            }
                                                            ui.end_row();
                                                        }
                                                        // quale data della riga: la prima, la seconda...
                                                        ui.label(&t_occorrenza);
                                                        let c = egui::ComboBox::from_id_salt(
                                                            "occorrenza",
                                                        )
                                                        .width(60.0)
                                                        .selected_text((*skip + 1).to_string())
                                                        .show_ui(ui, |ui| {
                                                            for n in 0..5usize {
                                                                if ui
                                                                    .selectable_label(
                                                                        n == *skip,
                                                                        (n + 1).to_string(),
                                                                    )
                                                                    .clicked()
                                                                {
                                                                    ws.active_mut()
                                                                        .dialog_time_set_skip(n);
                                                                    stato.campo_fuoco = false;
                                                                }
                                                            }
                                                        });
                                                        stato.opzioni.push((
                                                            c.response.rect,
                                                            "occorrenza".to_string(),
                                                        ));
                                                        ui.end_row();
                                                    },
                                                );
                                                ui.label(RichText::new(&t_entro).weak());
                                                ui.label(
                                                    RichText::new(&t_aiuto_regola).weak().small(),
                                                );
                                                // insegnare con un esempio: si incolla un pezzo di riga e il programma ricava la regola
                                                ui.add_space(6.0);
                                                crate::strumenti::grassetto(
                                                    ui,
                                                    &t_esempio_titolo,
                                                    testo_titolo,
                                                );
                                                ui.label(
                                                    RichText::new(&t_esempio_aiuto).weak().small(),
                                                );
                                                ui.horizontal(|ui| {
                                                    let r = ui.add(
                                                        TextEdit::singleline(&mut stato.esempio)
                                                            .hint_text(
                                                                RichText::new(
                                                                    "funding @ 2022-01-01 08:00",
                                                                )
                                                                .weak(),
                                                            )
                                                            .desired_width(
                                                                (ui.available_width() - 130.0)
                                                                    .max(120.0),
                                                            ),
                                                    );
                                                    stato
                                                        .opzioni
                                                        .push((r.rect, "esempio".to_string()));
                                                    if r.changed() {
                                                        let testo = stato.esempio.clone();
                                                        ws.active_mut()
                                                            .dialog_time_set_example(&testo);
                                                    }
                                                    let b = ui.button(&t_trova);
                                                    stato
                                                        .opzioni
                                                        .push((b.rect, "trova".to_string()));
                                                    if b.clicked()
                                                        || (r.lost_focus()
                                                            && ui.input(|i| {
                                                                i.key_pressed(egui::Key::Enter)
                                                            }))
                                                    {
                                                        ws.active_mut().dialog_time_find_rule();
                                                    }
                                                });
                                                if let Some(problema) = example_problem {
                                                    ui.colored_label(avviso, problema.as_str());
                                                }
                                                for (i, (etichetta, dettaglio, scelta)) in
                                                    example_candidates.iter().enumerate()
                                                {
                                                    let r = ui.horizontal(|ui| {
                                                        let r =
                                                            ui.radio(*scelta, etichetta.as_str());
                                                        ui.label(
                                                            RichText::new(dettaglio).weak().small(),
                                                        );
                                                        r
                                                    });
                                                    stato.opzioni.push((
                                                        r.inner.rect,
                                                        format!("regola {i}"),
                                                    ));
                                                    if r.inner.clicked() {
                                                        ws.active_mut().dialog_time_use_example(i);
                                                        stato.campo_fuoco = false;
                                                    }
                                                }
                                            });
                                        }
                                        ui.add_space(4.0);
                                        ui.label(RichText::new(&t_prime).weak());
                                        egui::Grid::new("formato_anteprima").num_columns(3).show(
                                            ui,
                                            |ui| {
                                                for (testo, letto) in preview {
                                                    ui.label(RichText::new(testo).monospace());
                                                    ui.label("=>");
                                                    crate::strumenti::grassetto(
                                                        ui,
                                                        letto,
                                                        testo_titolo,
                                                    );
                                                    ui.end_row();
                                                }
                                            },
                                        );
                                        ui.separator();
                                        // 2. il filtro per data
                                        crate::strumenti::grassetto(ui, &t_filtro, testo_titolo);
                                        if !*has_mask {
                                            ui.colored_label(avviso, t_serve.as_str());
                                        } else {
                                            let mut acceso = *check;
                                            let mut barra = *timeline;
                                            ui.horizontal(|ui| {
                                                let r = ui.checkbox(&mut acceso, t_attivo.as_str());
                                                stato.opzioni.push((r.rect, "filtro".to_string()));
                                                if r.changed() {
                                                    ws.active_mut().dialog_time_set_check(acceso);
                                                }
                                                ui.add_space(16.0);
                                                let r =
                                                    ui.checkbox(&mut barra, t_timeline.as_str());
                                                stato
                                                    .opzioni
                                                    .push((r.rect, "timeline".to_string()));
                                                if r.changed() {
                                                    ws.active_mut().dialog_time_set_timeline(barra);
                                                }
                                            });
                                            egui::Grid::new("filtro_date").num_columns(2).show(
                                                ui,
                                                |ui| {
                                                    for i in 2..4u8 {
                                                        let (etichetta, suggerimento) = if i == 2 {
                                                            (t_dal.as_str(), from_hint.as_str())
                                                        } else {
                                                            (t_al.as_str(), to_hint.as_str())
                                                        };
                                                        ui.label(etichetta);
                                                        ui.horizontal(|ui| {
                                                            let k = usize::from(i) - 2;
                                                            let r = ui.add(
                                                                TextEdit::singleline(
                                                                    &mut stato.filtro[k],
                                                                )
                                                                .hint_text(
                                                                    RichText::new(suggerimento)
                                                                        .weak(),
                                                                )
                                                                .desired_width(300.0),
                                                            );
                                                            stato.campi.push(r.rect);
                                                            if r.has_focus() && *field != i {
                                                                ws.active_mut().dialog_focus(i);
                                                            }
                                                            if !stato.campo_fuoco && *field == i {
                                                                r.request_focus();
                                                                stato.campo_fuoco = true;
                                                            }
                                                            if r.changed() {
                                                                let testo = stato.filtro[k].clone();
                                                                ws.active_mut()
                                                                    .dialog_set_field(i, &testo);
                                                            }
                                                            if r.lost_focus()
                                                                && ui.input(|i| {
                                                                    i.key_pressed(egui::Key::Enter)
                                                                })
                                                            {
                                                                ws.active_mut().dialog_submit();
                                                            }
                                                            // il calendario (solo con una maschera che ha la data)
                                                            if *has_date {
                                                                let c = ui
                                                                .add(
                                                                    egui::Button::new(
                                                                        RichText::new(
                                                                            crate::strumenti::glifo(
                                                                                "calendar",
                                                                            ),
                                                                        )
                                                                        .size(15.0),
                                                                    )
                                                                    .min_size(egui::Vec2::new(
                                                                        28.0, 24.0,
                                                                    )),
                                                                )
                                                                .on_hover_text(t_data.as_str());
                                                                stato.opzioni.push((
                                                                    c.rect,
                                                                    format!("calendario {i}"),
                                                                ));
                                                                if stato.calendario.is_some_and(
                                                                    |(aperto, _, _)| aperto == i,
                                                                ) {
                                                                    stato.cal_ancora = Some(c.rect);
                                                                }
                                                                if c.clicked() {
                                                                    stato.calendario = match stato
                                                                        .calendario
                                                                    {
                                                                        Some((aperto, _, _))
                                                                            if aperto == i =>
                                                                        {
                                                                            None
                                                                        }
                                                                        _ => ws
                                                                            .active()
                                                                            .time_calendar(i)
                                                                            .map(|info| {
                                                                                stato.cal_ancora =
                                                                                    Some(c.rect);
                                                                                (
                                                                                    i,
                                                                                    info.month.0,
                                                                                    info.month.1,
                                                                                )
                                                                            }),
                                                                    };
                                                                }
                                                            }
                                                            // la X svuota il campo: vuoto = senza limite
                                                            let x = ui
                                                                .add(
                                                                    egui::Button::new("").min_size(
                                                                        egui::Vec2::new(28.0, 24.0),
                                                                    ),
                                                                )
                                                                .on_hover_text(t_svuota.as_str());
                                                            let colore = ui
                                                                .style()
                                                                .interact(&x)
                                                                .fg_stroke
                                                                .color;
                                                            crate::strumenti::croce(
                                                                ui.painter(),
                                                                x.rect.center(),
                                                                colore,
                                                                4.5,
                                                            );
                                                            stato.opzioni.push((
                                                                x.rect,
                                                                format!("svuota {i}"),
                                                            ));
                                                            if x.clicked() {
                                                                ws.active_mut()
                                                                    .dialog_time_clear_field(i);
                                                            }
                                                        });
                                                        ui.end_row();
                                                    }
                                                },
                                            );
                                            // il calendario aperto: un popup sotto il campo (mese e anno in tendina, i giorni, l'ora); ogni scelta scrive subito
                                            // nel campo, il popup resta finche' non si preme "Fatto" o si clicca fuori
                                            if let (Some((campo, anno, mese)), Some(ancora)) =
                                                (stato.calendario, stato.cal_ancora)
                                            {
                                                if let Some(info) = ws.active().time_calendar(campo)
                                                {
                                                    let mut nuovo_mese: Option<(i32, u32)> = None;
                                                    let mut chiudi = false;
                                                    let area = egui::Area::new(egui::Id::new(
                                                        "calendario_popup",
                                                    ))
                                                    .order(egui::Order::Foreground)
                                                    .fixed_pos(egui::Pos2::new(
                                                        ancora.max.x + 40.0,
                                                        ancora.min.y - 6.0,
                                                    ))
                                                    .show(ctx, |ui| {
                                                        egui::Frame::popup(ui.style()).show(
                                                            ui,
                                                            |ui| {
                                                                ui.set_width(236.0);
                                                                // l'intestazione: mese e anno in tendina, le frecce
                                                                ui.horizontal(|ui| {
                                                            let (a_prima, m_prima) = if mese == 1 {
                                                                (anno - 1, 12)
                                                            } else {
                                                                (anno, mese - 1)
                                                            };
                                                            let (a_dopo, m_dopo) = if mese == 12 {
                                                                (anno + 1, 1)
                                                            } else {
                                                                (anno, mese + 1)
                                                            };
                                                            let r = ui.button("<");
                                                            stato.opzioni.push((
                                                                r.rect,
                                                                "mese prima".to_string(),
                                                            ));
                                                            if r.clicked() {
                                                                nuovo_mese =
                                                                    Some((a_prima, m_prima));
                                                            }
                                                            let c = egui::ComboBox::from_id_salt(
                                                                "cal_mese",
                                                            )
                                                            .width(86.0)
                                                            .selected_text(
                                                                nomi_mesi
                                                                    [(mese as usize - 1).min(11)]
                                                                .as_str(),
                                                            )
                                                            .show_ui(ui, |ui| {
                                                                for (m, nome) in
                                                                    nomi_mesi.iter().enumerate()
                                                                {
                                                                    if ui
                                                                        .selectable_label(
                                                                            m as u32 + 1 == mese,
                                                                            nome,
                                                                        )
                                                                        .clicked()
                                                                    {
                                                                        nuovo_mese = Some((
                                                                            anno,
                                                                            m as u32 + 1,
                                                                        ));
                                                                    }
                                                                }
                                                            });
                                                            stato.opzioni.push((
                                                                c.response.rect,
                                                                "mese combo".to_string(),
                                                            ));
                                                            let primo_anno = info
                                                                .first
                                                                .map_or(anno, |d| d.0)
                                                                .min(anno)
                                                                - 3;
                                                            let ultimo_anno = info
                                                                .last
                                                                .map_or(anno, |d| d.0)
                                                                .max(anno)
                                                                + 3;
                                                            let c = egui::ComboBox::from_id_salt(
                                                                "cal_anno",
                                                            )
                                                            .width(60.0)
                                                            .selected_text(anno.to_string())
                                                            .show_ui(ui, |ui| {
                                                                for a in primo_anno..=ultimo_anno {
                                                                    if ui
                                                                        .selectable_label(
                                                                            a == anno,
                                                                            a.to_string(),
                                                                        )
                                                                        .clicked()
                                                                    {
                                                                        nuovo_mese =
                                                                            Some((a, mese));
                                                                    }
                                                                }
                                                            });
                                                            stato.opzioni.push((
                                                                c.response.rect,
                                                                "anno combo".to_string(),
                                                            ));
                                                            let r = ui.button(">");
                                                            stato.opzioni.push((
                                                                r.rect,
                                                                "mese dopo".to_string(),
                                                            ));
                                                            if r.clicked() {
                                                                nuovo_mese = Some((a_dopo, m_dopo));
                                                            }
                                                        });
                                                                ui.add_space(4.0);
                                                                // i giorni: quelli che il file copre normali, gli altri attenuati
                                                                let primo =
                                                            mrp_core::timemask::key_from_civil(
                                                                anno, mese, 1,
                                                            );
                                                                let sfasa =
                                                                    mrp_core::timemask::weekday(
                                                                        primo,
                                                                    )
                                                                        as usize;
                                                                let giorni =
                                                            mrp_core::timemask::days_in_month(
                                                                anno, mese,
                                                            );
                                                                let dentro = |g: u32| match (
                                                                    info.first, info.last,
                                                                ) {
                                                                    (Some(a), Some(b)) => {
                                                                        (anno, mese, g) >= a
                                                                            && (anno, mese, g) <= b
                                                                    }
                                                                    _ => true,
                                                                };
                                                                egui::Grid::new("calendario")
                                                            .num_columns(7)
                                                            .spacing(egui::Vec2::new(2.0, 2.0))
                                                            .show(ui, |ui| {
                                                                for g in &giorni_sett {
                                                                    ui.add_sized(
                                                                        egui::Vec2::new(31.0, 18.0),
                                                                        egui::Label::new(
                                                                            RichText::new(g)
                                                                                .small()
                                                                                .weak(),
                                                                        ),
                                                                    );
                                                                }
                                                                ui.end_row();
                                                                for cella in 0..(sfasa
                                                                    + giorni as usize)
                                                                    .div_ceil(7)
                                                                    * 7
                                                                {
                                                                    if cella < sfasa
                                                                        || cella
                                                                            >= sfasa
                                                                                + giorni as usize
                                                                    {
                                                                        ui.add_sized(
                                                                            egui::Vec2::new(
                                                                                31.0, 24.0,
                                                                            ),
                                                                            egui::Label::new(""),
                                                                        );
                                                                    } else {
                                                                        let giorno = (cella - sfasa
                                                                            + 1)
                                                                            as u32;
                                                                        let scelto = info.selected
                                                                            == Some((
                                                                                anno, mese, giorno,
                                                                            ));
                                                                        let mut testo =
                                                                            RichText::new(
                                                                                giorno.to_string(),
                                                                            );
                                                                        if !dentro(giorno) {
                                                                            testo = testo.weak();
                                                                        }
                                                                        let r = ui.add_sized(
                                                                            egui::Vec2::new(
                                                                                31.0, 24.0,
                                                                            ),
                                                                            egui::Button::new(
                                                                                testo,
                                                                            )
                                                                            .selected(scelto),
                                                                        );
                                                                        stato.opzioni.push((
                                                                            r.rect,
                                                                            format!(
                                                                                "giorno {giorno}"
                                                                            ),
                                                                        ));
                                                                        if r.clicked() {
                                                                            ws.active_mut()
                                                                                .dialog_time_pick(
                                                                                    campo, anno,
                                                                                    mese, giorno,
                                                                                );
                                                                        }
                                                                    }
                                                                    if cella % 7 == 6 {
                                                                        ui.end_row();
                                                                    }
                                                                }
                                                            });
                                                                ui.separator();
                                                                // l'ora: ore, minuti, secondi e (se la maschera li scrive) millisecondi
                                                                ui.horizontal(|ui| {
                                                                    ui.label(&t_ora);
                                                                    let ms = info.time_ms;
                                                                    let mut parti = [
                                                                        (
                                                                            ms / 3_600_000,
                                                                            23_i64,
                                                                            3_600_000_i64,
                                                                            "ora",
                                                                        ),
                                                                        (
                                                                            (ms / 60_000) % 60,
                                                                            59,
                                                                            60_000,
                                                                            "minuti",
                                                                        ),
                                                                        (
                                                                            (ms / 1000) % 60,
                                                                            59,
                                                                            1000,
                                                                            "secondi",
                                                                        ),
                                                                        (
                                                                            ms % 1000,
                                                                            999,
                                                                            1,
                                                                            "millisecondi",
                                                                        ),
                                                                    ];
                                                                    let mut nuovo = ms;
                                                                    let mut primo_pezzo = true;
                                                                    for (v, massimo, peso, nome) in
                                                                        &mut parti
                                                                    {
                                                                        // si mostra fino alla precisione che la maschera scrive
                                                                        if *peso < info.unit_ms {
                                                                            continue;
                                                                        }
                                                                        // fra ore, minuti e secondi i due punti, prima dei millisecondi il punto
                                                                        if !primo_pezzo {
                                                                            ui.label(
                                                                                if *peso == 1 {
                                                                                    "."
                                                                                } else {
                                                                                    ":"
                                                                                },
                                                                            );
                                                                        }
                                                                        primo_pezzo = false;
                                                                        let mut x = *v;
                                                                        let r = ui.add(
                                                                            egui::DragValue::new(
                                                                                &mut x,
                                                                            )
                                                                            .range(0..=*massimo)
                                                                            .speed(0.2)
                                                                            .custom_formatter(
                                                                                |n, _| {
                                                                                    if *massimo
                                                                                        == 999
                                                                                    {
                                                                                        format!(
                                                                                        "{:03}",
                                                                                        n as i64
                                                                                    )
                                                                                    } else {
                                                                                        format!(
                                                                                        "{:02}",
                                                                                        n as i64
                                                                                    )
                                                                                    }
                                                                                },
                                                                            ),
                                                                        );
                                                                        stato.opzioni.push((
                                                                            r.rect,
                                                                            (*nome).to_string(),
                                                                        ));
                                                                        if x != *v {
                                                                            nuovo +=
                                                                                (x - *v) * *peso;
                                                                        }
                                                                    }
                                                                    if nuovo != ms {
                                                                        ws.active_mut()
                                                                            .dialog_time_set_time(
                                                                                campo, nuovo,
                                                                            );
                                                                    }
                                                                });
                                                                ui.add_space(2.0);
                                                                ui.with_layout(
                                                                    egui::Layout::right_to_left(
                                                                        egui::Align::Center,
                                                                    ),
                                                                    |ui| {
                                                                        let r = ui.button(&t_fatto);
                                                                        stato.opzioni.push((
                                                                            r.rect,
                                                                            "fatto".to_string(),
                                                                        ));
                                                                        if r.clicked() {
                                                                            chiudi = true;
                                                                        }
                                                                    },
                                                                );
                                                            },
                                                        );
                                                    });
                                                    if let Some((a, m)) = nuovo_mese {
                                                        stato.calendario = Some((campo, a, m));
                                                    }
                                                    // un clic fuori dal popup (e fuori dal bottone) lo chiude
                                                    let fuori = ctx.input(|i| {
                                                        i.pointer.any_pressed()
                                                            && i.pointer.interact_pos().is_some_and(
                                                                |p| {
                                                                    !area.response.rect.contains(p)
                                                                        && !ancora.contains(p)
                                                                },
                                                            )
                                                    });
                                                    // le tendine del popup sono finestre a parte: finche' una e' aperta non si chiude
                                                    if chiudi
                                                        || (fuori && !egui::Popup::is_any_open(ctx))
                                                    {
                                                        stato.calendario = None;
                                                    }
                                                }
                                            }
                                            if acceso
                                                && from.trim().is_empty()
                                                && to.trim().is_empty()
                                            {
                                                ui.label(RichText::new(&t_nessun_limite).weak());
                                            }
                                        }
                                    }
                                });
                            egui::Frame::group(ui.style())
                                .inner_margin(egui::Margin::symmetric(12, 8))
                                .outer_margin(egui::Margin::symmetric(0, 4))
                                .show(ui, |ui| {
                                    ui.set_width(ui.available_width());
                                    let t = titolo_riquadro(ui, 1);
                                    stato.opzioni.push((t, "sezione 1".to_string()));
                                    ui.add_space(6.0);
                                    ui.label(RichText::new(&t_aiuto_livelli).weak());
                                    ui.horizontal(|ui| {
                                        for (i, nome) in t_livelli.iter().enumerate() {
                                            let mut scelto = levels[i];
                                            let r = ui.checkbox(
                                                &mut scelto,
                                                RichText::new(nome)
                                                    .color(colori_livelli[i])
                                                    .strong(),
                                            );
                                            stato.opzioni.push((r.rect, format!("livello {i}")));
                                            if r.changed() {
                                                ws.active_mut().dialog_filters_set_level(i, scelto);
                                            }
                                        }
                                    });
                                });
                            egui::Frame::group(ui.style())
                                .inner_margin(egui::Margin::symmetric(12, 8))
                                .outer_margin(egui::Margin::symmetric(0, 4))
                                .show(ui, |ui| {
                                    ui.set_width(ui.available_width());
                                    let t = titolo_riquadro(ui, 2);
                                    stato.opzioni.push((t, "sezione 2".to_string()));
                                    ui.add_space(6.0);
                                    ui.label(RichText::new(&t_aiuto_testo).weak());
                                    let r = ui.add(
                                        TextEdit::singleline(&mut stato.ftesto)
                                            .desired_width(f32::INFINITY),
                                    );
                                    stato.campi.push(r.rect);
                                    if stato.campo.is_none() {
                                        stato.campo = Some(r.rect);
                                    }
                                    if r.changed() {
                                        let testo = stato.ftesto.clone();
                                        ws.active_mut().dialog_filters_set_text(&testo);
                                    }
                                    if r.lost_focus()
                                        && ui.input(|i| i.key_pressed(egui::Key::Enter))
                                    {
                                        ws.active_mut().dialog_submit();
                                    }
                                    ui.horizontal(|ui| {
                                        for (i, nome) in t_modi.iter().enumerate() {
                                            let r =
                                                ui.radio(usize::from(*fmode) == i, nome.as_str());
                                            stato.opzioni.push((r.rect, format!("modo {i}")));
                                            if r.clicked() {
                                                ws.active_mut().dialog_filters_set_mode(i);
                                            }
                                        }
                                    });
                                    ui.horizontal(|ui| {
                                        let mut maiusc = *fcase;
                                        let r = ui.checkbox(&mut maiusc, t_maiusc.as_str());
                                        stato.opzioni.push((r.rect, "maiuscole".to_string()));
                                        if r.changed() {
                                            ws.active_mut().dialog_filters_set_case(maiusc);
                                        }
                                        let mut nascondi = *finvert;
                                        let r = ui.checkbox(&mut nascondi, t_nascondi.as_str());
                                        stato.opzioni.push((r.rect, "nascondi".to_string()));
                                        if r.changed() {
                                            ws.active_mut().dialog_filters_set_invert(nascondi);
                                        }
                                    });
                                });
                        });
                    if !error.is_empty() {
                        ui.colored_label(avviso, error.as_str());
                    }
                    ui.separator();
                    ui.add_space(2.0);
                    ui.label(RichText::new(format!("{t_riassunto} {summary}")).weak());
                    ui.add_space(4.0);
                    let testi: Vec<&str> = vec![t_ok.as_str(), t_cancel.as_str()];
                    match pulsanti_centrati(ui, &mut stato.pulsanti, &testi) {
                        Some(0) => ws.active_mut().dialog_submit(),
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
                    // cosa si e' riconosciuto e, a destra sulla stessa riga, il piccolo pulsante che lo fa riconoscere di nuovo
                    let mut riconosci = false;
                    ui.horizontal(|ui| {
                        match detected {
                            Some((tempo, pat)) => {
                                crate::strumenti::grassetto(
                                    ui,
                                    &t_riconosciuto,
                                    crate::stile::corrente(ctx).popup_fg,
                                );
                                ui.label(RichText::new(tempo).monospace());
                                ui.label(RichText::new(format!("({pat})")).weak());
                            }
                            None => {
                                ui.colored_label(avviso, t_nessuno.as_str());
                            }
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let r = ui.small_button(auto.as_str());
                            stato.opzioni.push((r.rect, "riconosci".to_string()));
                            riconosci = r.clicked();
                        });
                    });
                    if riconosci {
                        ws.active_mut().dialog_time_auto();
                        // i campi si rileggono dall'App al prossimo giro
                        stato.campo_fuoco = false;
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
                            let r = ui.selectable_label(
                                pers,
                                RichText::new(&t_personale)
                                    .color(crate::stile::corrente(ctx).popup_fg),
                            );
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
                                crate::strumenti::grassetto(
                                    ui,
                                    letto,
                                    crate::stile::corrente(ctx).popup_fg,
                                );
                                ui.end_row();
                            }
                        });
                    if !error.is_empty() {
                        ui.colored_label(avviso, error.as_str());
                    }
                    match pulsanti_centrati(
                        ui,
                        &mut stato.pulsanti,
                        &[t_ok.as_str(), t_cancel.as_str()],
                    ) {
                        Some(0) => ws.active_mut().dialog_submit(),
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
