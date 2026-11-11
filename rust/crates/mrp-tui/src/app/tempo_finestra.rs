//! La finestra "Tempo" (solo in quella con i controlli nativi): il formato dell'orario e il filtro per data in un posto solo.
//! In alto il formato (riconosciuto, scelto fra i noti o scritto a mano), sotto il filtro facoltativo: una casella lo accende e lo
//! spegne senza perdere le date, due campi (Dal / Al) scritti nel formato del file, un calendario e una X per ogni campo; un campo
//! vuoto vuol dire "senza limite". OK applica formato e filtro insieme; se il filtro non si puo' applicare niente cambia.

use super::dialogs::DialogView;
use super::timeline::TIMELINE_LEVELS;
use super::{App, DialogKind};
use mrp_core::pattern::{LineFilter, Mode, Pattern};
use mrp_core::timemask::{self, TimeMask};
use mrp_core::timesearch;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::path::Path;
use std::sync::atomic::AtomicBool;

/// I millisecondi di un giorno.
const GIORNO: i64 = 86_400_000;

/// Un mese del calendario: `(anno, mese)`.
pub type Month = (i32, u32);
/// Un giorno del calendario: `(anno, mese, giorno)`.
pub type Day = (i32, u32, u32);

/// Cosa serve a disegnare il calendario di un campo (vedi `App::time_calendar`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CalendarInfo {
    /// Il mese con cui si apre.
    pub month: Month,
    /// Il giorno gia' scritto nel campo.
    pub selected: Option<Day>,
    /// L'ora del giorno, in millisecondi dalla mezzanotte.
    pub time_ms: i64,
    /// Il primo e l'ultimo giorno che il file copre.
    pub first: Option<Day>,
    pub last: Option<Day>,
    /// La piu' piccola unita' che la maschera scrive (1: i millisecondi, 1000: i secondi...).
    pub unit_ms: i64,
}

/// Le condizioni della finestra Filtri: il tempo, i livelli e il testo. Si ricordano dopo OK (la finestra le ripropone), la vista
/// filtrata le porta con se' (la barra di stato le elenca) e si passano al file originale per rifare il filtro.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilterConditions {
    pub levels: [bool; 4],
    pub text: String,
    pub mode: Mode,
    pub case: bool,
    pub invert: bool,
    /// Il filtro per data e' acceso, con questi estremi (un campo vuoto: senza limite).
    pub time: bool,
    pub from: String,
    pub to: String,
    /// Le lunghezze (byte di contenuto) ammesse dal check dei tracciati; `None`: nessuna condizione.
    pub lengths: Option<std::collections::HashSet<u64>>,
}

impl FilterConditions {
    /// Nessuna condizione: il file si vede intero.
    fn is_empty(&self) -> bool {
        !self.levels.contains(&true) && self.text.is_empty() && !self.has_time()
    }

    /// C'e' un filtro per data con almeno un estremo.
    fn has_time(&self) -> bool {
        self.time && !(self.from.trim().is_empty() && self.to.trim().is_empty())
    }
}

impl App {
    /// Alt+I (`intervallo`) e Alt+O: nella finestra con i controlli nativi aprono la finestra "Tempo" (con il fuoco sul filtro o sul
    /// formato); nel terminale i due dialoghi separati.
    pub(super) fn open_time_dialog_for_mode(&mut self, intervallo: bool) {
        if self.chrome {
            if intervallo {
                self.open_time_range_dialog();
            } else {
                self.open_time_mask_dialog();
            }
        } else {
            self.open_time_dialog();
            self.time_field = if intervallo { 2 } else { 0 };
        }
    }

    /// Apre la finestra "Tempo": il formato parte dalla maschera attuale, il filtro dalle date che si ricordano, con la casella
    /// accesa se c'e' una finestra di tempo.
    pub fn open_time_dialog(&mut self) {
        self.assicura_maschera();
        let (pattern, within) = self.time_mask.as_ref().map_or((String::new(), 0), |m| {
            (m.pattern().to_string(), m.within())
        });
        self.mask_text = [pattern, within.to_string()];
        self.mask_error = String::new();
        self.calcola_aiuti_formato();
        let limiti = self.limiti_tempo();
        self.time_bounds = limiti.map(|l| l.0);
        self.time_last_known = limiti.is_some_and(|l| l.1);
        // le condizioni dell'ultima volta tornano (livelli, testo, date): con una finestra di tempo accesa le date sono le sue
        self.time_check = self.window.is_some();
        let ricordate = match &self.filter_view {
            Some(v) => v.conditions.clone(),
            None => self.last_filters.clone(),
        };
        if let Some(c) = ricordate {
            self.carica_condizioni(&c, self.window.is_none());
            self.time_check |= c.time;
        }
        self.time_timeline = self.timeline_on();
        // un testo selezionato nel documento e' l'esempio da cui ricavare la regola
        self.example_text = self.esempio_dalla_selezione();
        self.example_result = None;
        self.time_field = 2;
        self.time_error = String::new();
        self.dialog = Some(DialogKind::Time);
        if !self.example_text.is_empty() {
            self.dialog_time_find_rule();
        }
    }

    /// La prima e l'ultima chiave del file (con la maschera in uso), `None` senza maschera o senza righe con l'orario.
    fn limiti_tempo(&self) -> Option<((i64, i64), bool)> {
        let mask = self.time_mask.as_ref()?;
        let doc = self.doc.as_ref()?;
        let libero = AtomicBool::new(false);
        let primo = timesearch::first_key(doc, mask, &libero).ok()??.1;
        // l'ultima si cerca solo nelle ultime righe: una regola che legge righe sparse (un banner ogni tanto) puo' non trovarne in
        // fondo, e senza limiti non si apre nemmeno il calendario; si parte allora dalla prima
        let ultimo = timesearch::last_key(doc, mask, &libero).ok().flatten();
        Some(((primo, ultimo.map_or(primo, |(_, k)| k)), ultimo.is_some()))
    }

    /// Una chiave scritta nel formato del file (o, senza maschera, come `aaaa-mm-gg hh:mm:ss`).
    fn scrivi_chiave(&self, k: i64) -> String {
        match self.time_mask.as_ref() {
            Some(m) => m.write(k),
            None => timemask::iso(k, true),
        }
    }

    /// La vista della finestra "Tempo".
    pub(super) fn time_view(&self) -> DialogView {
        let cat = self.catalog();
        let (presets, preset) = self.time_presets();
        let suggerimento = |chiave: &str, k: Option<i64>| match k {
            Some(k) => cat.tf(chiave, &[&self.scrivi_chiave(k)]),
            None => String::new(),
        };
        DialogView::Time {
            title: cat.t("time.dialog.title").to_string(),
            detected: self.time_detected(),
            presets,
            preset,
            matches: self.time_preset_matches().to_vec(),
            pattern_label: cat.t("time.mask.pattern").to_string(),
            within_label: cat.t("time.mask.within").to_string(),
            pattern: self.mask_text[0].clone(),
            within: self.mask_text[1].clone(),
            skip: self.mask_skip,
            sources: self
                .mask_sources
                .iter()
                .map(|s| {
                    let etichetta = match &s.kind {
                        crate::app::SourceKind::Start => cat.t("time.source.start").to_string(),
                        crate::app::SourceKind::After(p) => cat.tf("time.source.after", &[p]),
                        crate::app::SourceKind::Number(n) => {
                            cat.tf("time.source.number", &[&n.to_string()])
                        }
                        crate::app::SourceKind::Rule => cat.tf("time.source.rule", &[&s.pattern]),
                    };
                    (
                        etichetta,
                        s.example.clone(),
                        s.lines,
                        s.kind == crate::app::SourceKind::Rule,
                    )
                })
                .collect(),
            source: self.time_source_selected(),
            example: self.example_text.clone(),
            example_problem: self.time_example_result().and_then(|r| {
                r.problem.as_ref().map(|p| match p {
                    crate::app::ExampleProblem::NoDate => cat.t("time.example.no_date").to_string(),
                    crate::app::ExampleProblem::NotInFile => {
                        cat.t("time.example.not_in_file").to_string()
                    }
                })
            }),
            example_candidates: self
                .time_example_result()
                .map(|r| {
                    let scelta = self.time_example_selected();
                    r.candidates
                        .iter()
                        .enumerate()
                        .map(|(i, c)| {
                            let etichetta = match &c.kind {
                                crate::app::SourceKind::Start => {
                                    cat.t("time.source.start").to_string()
                                }
                                crate::app::SourceKind::After(p) => {
                                    cat.tf("time.source.after", &[p])
                                }
                                crate::app::SourceKind::Number(n) => {
                                    cat.tf("time.source.number", &[&n.to_string()])
                                }
                                crate::app::SourceKind::Rule => {
                                    cat.t("time.source.rule").to_string()
                                }
                            };
                            let mut dettaglio = cat.tf(
                                "time.example.reads",
                                &[&c.lines.to_string(), &c.first, &c.last],
                            );
                            if !c.ordered {
                                dettaglio.push_str(" \u{2014} ");
                                dettaglio.push_str(cat.t("time.example.unordered"));
                            }
                            (etichetta, dettaglio, scelta == Some(i))
                        })
                        .collect()
                })
                .unwrap_or_default(),
            preview: self.time_mask_preview(),
            auto: cat.t("time.mask.auto").to_string(),
            has_mask: self.time_mask.is_some(),
            has_date: self.time_mask.as_ref().is_some_and(TimeMask::has_date),
            from: self.range_text[0].clone(),
            to: self.range_text[1].clone(),
            check: self.time_check,
            timeline: self.time_timeline,
            from_hint: suggerimento("time.filter.from_hint", self.time_bounds.map(|b| b.0)),
            to_hint: suggerimento(
                "time.filter.to_hint",
                self.time_bounds
                    .filter(|_| self.time_last_known)
                    .map(|b| b.1),
            ),
            field: self.time_field,
            error: self.time_error.clone(),
            levels: self.timeline_filter_levels(),
            ftext: self.fl_text.clone(),
            fmode: match self.fl_mode {
                Mode::Text => 0,
                Mode::Glob => 1,
                Mode::Regex => 2,
            },
            fcase: self.fl_case,
            finvert: self.fl_invert,
            summary: self.filters_summary(),
        }
    }

    /// Il testo del campo `i` della finestra "Tempo" (0 pattern, 1 entro quanti caratteri, 2 dal, 3 al).
    pub fn time_field_text(&self, i: usize) -> &str {
        match i {
            0 | 1 => &self.mask_text[i],
            2 | 3 => &self.range_text[i - 2],
            _ => "",
        }
    }

    /// La casella "Filtro attivo" della finestra "Tempo".
    pub fn time_check(&self) -> bool {
        self.time_check
    }

    /// La casella "Timeline" della finestra "Tempo": si applica con OK.
    pub fn time_timeline_check(&self) -> bool {
        self.time_timeline
    }

    /// Accende o spegne la casella "Timeline" della finestra "Tempo".
    pub fn dialog_time_set_timeline(&mut self, acceso: bool) {
        if self.dialog == Some(DialogKind::Time) {
            self.time_timeline = acceso;
        }
    }

    /// L'ultimo errore della finestra "Tempo" (vuoto: nessuno).
    pub fn time_error_text(&self) -> &str {
        &self.time_error
    }

    /// Accende o spegne la casella "Filtro attivo" della finestra "Tempo" (le date restano scritte).
    pub fn dialog_time_set_check(&mut self, acceso: bool) {
        if self.dialog == Some(DialogKind::Time) {
            self.time_check = acceso;
            self.time_error.clear();
        }
    }

    /// Svuota il campo Dal (2) o Al (3): un campo vuoto vuol dire "senza limite" (la X accanto al campo).
    pub fn dialog_time_clear_field(&mut self, i: u8) {
        if self.dialog == Some(DialogKind::Time) && (2..4).contains(&i) {
            self.range_text[usize::from(i) - 2].clear();
            self.time_field = i;
            self.time_error.clear();
        }
    }

    /// Cosa serve a disegnare il calendario del campo Dal (2) o Al (3): il mese con cui si apre, il giorno gia' scelto, l'ora del giorno
    /// (quella scritta nel campo, altrimenti mezzanotte per Dal e fine giornata per Al), i giorni che il file copre e la precisione
    /// della maschera. Si apre sul mese della data scritta, altrimenti su quello del primo (Dal) o dell'ultimo (Al) orario del file;
    /// `None` senza maschera con la data (non c'e' un calendario da mostrare).
    pub fn time_calendar(&self, i: u8) -> Option<CalendarInfo> {
        let mask = self.time_mask.as_ref().filter(|m| m.has_date())?;
        if !(2..4).contains(&i) {
            return None;
        }
        let giorno_di = |k: i64| timemask::civil(k);
        let scritto = Self::chiave_a_mano(&self.range_text[usize::from(i) - 2], mask, None);
        let (primo, ultimo) = match self.time_bounds {
            Some((a, b)) => (Some(giorno_di(a)), Some(giorno_di(b))),
            None => (None, None),
        };
        let (mese, scelto, ora) = match scritto {
            Some((k, _)) => {
                let (a, m, g) = giorno_di(k);
                ((a, m), Some((a, m, g)), k.rem_euclid(GIORNO))
            }
            None => {
                let (a, m, _) = if i == 2 { primo? } else { ultimo? };
                ((a, m), None, if i == 2 { 0 } else { GIORNO - 1 })
            }
        };
        Some(CalendarInfo {
            month: mese,
            selected: scelto,
            time_ms: ora,
            first: primo,
            last: ultimo,
            unit_ms: mask.unit_ms(),
        })
    }

    /// L'ora del giorno con cui scrivere un giorno scelto dal calendario nel campo `i`, se il campo non ha gia' un orario: per Dal la
    /// mezzanotte (o l'ora della prima riga se il giorno e' quello della prima riga), per Al la fine giornata (o l'ora dell'ultima
    /// riga se il giorno e' quello dell'ultima).
    fn ora_predefinita(&self, i: u8, giorno: Day) -> i64 {
        let Some((primo, ultimo)) = self.time_bounds else {
            return if i == 2 { 0 } else { GIORNO - 1 };
        };
        let (k, predefinita) = if i == 2 {
            (primo, 0)
        } else {
            (ultimo, GIORNO - 1)
        };
        let (a, m, g) = timemask::civil(k);
        if (a, m, g) == giorno {
            k.rem_euclid(GIORNO)
        } else {
            predefinita
        }
    }

    /// Scrive nel campo Dal (2) o Al (3) il giorno `anno-mese-giorno` nel formato del file: se il campo aveva gia' un'ora la tiene,
    /// altrimenti vale quella predefinita (vedi sopra). Accende il filtro.
    pub fn dialog_time_pick(&mut self, i: u8, anno: i32, mese: u32, giorno: u32) {
        if self.dialog != Some(DialogKind::Time) || !(2..4).contains(&i) {
            return;
        }
        // senza una maschera con la data (la sola ora) un giorno non si puo' scrivere
        let Some(mask) = self.time_mask.clone().filter(TimeMask::has_date) else {
            return;
        };
        let campo = usize::from(i) - 2;
        let ora = Self::chiave_a_mano(&self.range_text[campo], &mask, None)
            .map(|(k, _)| k.rem_euclid(GIORNO))
            .unwrap_or_else(|| self.ora_predefinita(i, (anno, mese, giorno)));
        self.range_text[campo] = mask.write(timemask::key_from_civil(anno, mese, giorno) + ora);
        self.time_check = true;
        self.time_field = i;
        self.time_error.clear();
    }

    /// Scrive nel campo Dal (2) o Al (3) l'ora del giorno `ms` (millisecondi dalla mezzanotte, da 0 a un giorno meno uno), tenendo la
    /// data che il campo ha (o, se e' vuoto, il giorno della prima / ultima riga del file). Accende il filtro.
    pub fn dialog_time_set_time(&mut self, i: u8, ms: i64) {
        if self.dialog != Some(DialogKind::Time) || !(2..4).contains(&i) {
            return;
        }
        let Some(mask) = self.time_mask.clone().filter(TimeMask::has_date) else {
            return;
        };
        let campo = usize::from(i) - 2;
        let giorno = match Self::chiave_a_mano(&self.range_text[campo], &mask, None) {
            Some((k, _)) => timemask::civil(k),
            None => match self.time_bounds {
                Some((primo, ultimo)) => timemask::civil(if i == 2 { primo } else { ultimo }),
                None => return,
            },
        };
        let ms = ms.clamp(0, GIORNO - 1);
        self.range_text[campo] =
            mask.write(timemask::key_from_civil(giorno.0, giorno.1, giorno.2) + ms);
        self.time_check = true;
        self.time_field = i;
        self.time_error.clear();
    }

    /// OK della finestra "Tempo": applica il formato (se e' cambiato, e diventa quello standard) e poi il filtro (se la casella e'
    /// accesa e c'e' almeno un estremo); con la casella spenta, o senza estremi, il file si vede intero. Un errore resta nella
    /// finestra e non cambia niente.
    pub(super) fn time_submit(&mut self) {
        self.time_error.clear();
        let within = self.mask_text[1].trim().parse::<usize>().unwrap_or(0);
        let pattern = self.mask_text[0].trim().to_string();
        let skip = self.mask_skip;
        // un file senza orario riconosciuto non ha formato: se non se ne scrive uno, non c'e' niente da cambiare (i filtri per testo e
        // per livello funzionano lo stesso)
        let cambia = if pattern.is_empty() && self.time_mask.is_none() {
            false
        } else {
            self.time_mask
                .as_ref()
                .is_none_or(|m| m.pattern() != pattern || m.within() != within || m.skip() != skip)
        };
        // com'era, per rimettere tutto se il filtro non si applica
        let prima = (self.time_mask.clone(), self.time_mask_tried, self.window);
        if cambia {
            if let Err(e) = self.set_time_mask_with(&pattern, within, skip) {
                self.time_error = e;
                return;
            }
        }
        let (da, a) = (self.range_text[0].clone(), self.range_text[1].clone());
        let con_date = self.time_check && !(da.trim().is_empty() && a.trim().is_empty());
        let livelli = self.timeline_filter_levels();
        let con_livelli = livelli.iter().any(|&x| x);
        let con_testo = !self.fl_text.is_empty();
        // in una vista filtrata le condizioni non si applicano alle sue righe (sono gia' quelle filtrate): le rifa' il file originale
        let in_vista = self.filter_view.is_some();
        // livelli e/o testo: una vista filtrata sola con tutte le condizioni (anche il tempo, come intervallo); la finestra di tempo di
        // questa scheda non cambia. Senza: il filtro per data restringe la vista di adesso
        let mut vista = None;
        if con_livelli || con_testo {
            let pattern = if con_testo {
                match Pattern::new(&self.fl_text, self.fl_mode, self.fl_case) {
                    Ok(p) => Some(p),
                    Err(e) => {
                        (self.time_mask, self.time_mask_tried, self.window) = prima;
                        self.time_error = self.cat.tf("replace.bad", &[&e.to_string()]);
                        return;
                    }
                }
            } else {
                None
            };
            if !self.enc.is_utf8() {
                (self.time_mask, self.time_mask_tried, self.window) = prima;
                self.time_error = self.cat.t("flash.regex_utf8").to_string();
                return;
            }
            let intervallo = if in_vista {
                None
            } else if con_date {
                match self.intervallo_tempo(&da, &a) {
                    Ok(r) => Some(r),
                    Err(e) => {
                        (self.time_mask, self.time_mask_tried, self.window) = prima;
                        self.time_error = e;
                        return;
                    }
                }
            } else {
                None
            };
            let filtro = LineFilter {
                pattern,
                invert: self.fl_invert,
                levels: livelli,
                lengths: self.lengths_filter.clone(),
            };
            let nome = self.nome_vista_filtri(&da, &a, con_date);
            vista = Some((filtro, intervallo, nome));
        } else if in_vista {
            // niente da fare qui: le date (o nessuna condizione) le applica il file originale
        } else if con_date {
            if let Err(e) = self.apply_time_range(&da, &a) {
                (self.time_mask, self.time_mask_tried, self.window) = prima;
                self.time_error = e;
                return;
            }
        } else if self.window.is_some() {
            self.clear_time_window();
        }
        // scelta a mano: diventa il formato standard, provato per primo sui file che si apriranno; non se vale un'occorrenza che non
        // sia la prima (e' una scelta per questo file, non per tutti)
        // la timeline si accende o si spegne come dice la casella (non dipende dal filtro: sono due cose, ma chi accende il filtro se
        // la aspetta, quindi la casella e' li' accanto)
        self.set_timeline(self.time_timeline);
        let standard = cambia && skip == 0;
        if standard {
            self.time_default = Some((pattern.clone(), within));
        }
        // una regola scritta a mano (ne' uno dei tempi offerti ne' un formato noto all'inizio della riga) si ricorda: la volta dopo e'
        // una riga dell'elenco, nei file dove legge qualcosa
        let offerto = self
            .mask_sources
            .iter()
            .any(|s| s.pattern == pattern && s.within == within && s.skip == skip);
        let noto = skip == 0 && timemask::PRESETS.iter().any(|p| p.pattern == pattern);
        let regola = !pattern.is_empty() && !offerto && !noto;
        if regola {
            self.aggiungi_regola(&pattern, within, skip);
        }
        if standard || regola {
            self.save_settings();
        }
        self.dialog = None;
        let condizioni = self.condizioni_attuali();
        if in_vista {
            self.refilter = Some(condizioni);
            return;
        }
        self.last_filters = Some(condizioni.clone());
        if let Some((filtro, intervallo, nome)) = vista {
            if !self.fl_text.is_empty() {
                self.last_find = Some(self.fl_text.clone());
            }
            self.filter_start(filtro, intervallo, &nome, condizioni);
        }
    }

    /// Il nome della scheda filtrata dalla finestra Filtri: le condizioni scelte, una dopo l'altra (`levels: ERROR, WARN + "timeout"`).
    fn nome_vista_filtri(&self, da: &str, a: &str, con_date: bool) -> String {
        let mut parti = Vec::new();
        let nomi: Vec<&str> = (0..4)
            .filter(|&i| self.timeline_filter_levels()[i])
            .map(|i| TIMELINE_LEVELS[i])
            .collect();
        if !nomi.is_empty() {
            parti.push(self.cat.tf("timeline.filter_name", &[&nomi.join(", ")]));
        }
        if !self.fl_text.is_empty() {
            let chiave = if self.fl_invert {
                "filters.part.not_text"
            } else {
                "filters.part.text"
            };
            parti.push(self.cat.tf(chiave, &[&self.fl_text]));
        }
        if con_date {
            parti.push(self.cat.tf("filters.part.time", &[&punti(da), &punti(a)]));
        }
        parti.join(" + ")
    }

    /// Il riassunto delle condizioni della finestra Filtri, in una riga ("Time: ... \u{b7} Levels: ... \u{b7} Text: ..."), o "nessun filtro".
    pub(super) fn filters_summary(&self) -> String {
        self.riassumi(&self.condizioni_attuali())
    }

    /// Le condizioni di `c` in una riga, o "nessun filtro". Il testo dice anche il modo se non e' semplice (`glob`, `regex`).
    fn riassumi(&self, c: &FilterConditions) -> String {
        let cat = &self.cat;
        let mut parti = Vec::new();
        if c.has_time() {
            parti.push(cat.tf("filters.sum.time", &[&punti(&c.from), &punti(&c.to)]));
        }
        let nomi: Vec<&str> = (0..4)
            .filter(|&i| c.levels[i])
            .map(|i| TIMELINE_LEVELS[i])
            .collect();
        if !nomi.is_empty() {
            parti.push(cat.tf("filters.sum.levels", &[&nomi.join(", ")]));
        }
        if !c.text.is_empty() {
            let chiave = if c.invert {
                "filters.sum.not_text"
            } else {
                "filters.sum.text"
            };
            let modo = match c.mode {
                Mode::Text => "",
                Mode::Glob => " (glob)",
                Mode::Regex => " (regex)",
            };
            parti.push(format!("{}{modo}", cat.tf(chiave, &[&c.text])));
        }
        if parti.is_empty() {
            cat.t("filters.sum.none").to_string()
        } else {
            parti.join(" \u{b7} ")
        }
    }

    /// Le condizioni scritte adesso nella finestra Filtri.
    fn condizioni_attuali(&self) -> FilterConditions {
        FilterConditions {
            levels: self.timeline_filter_levels(),
            text: self.fl_text.clone(),
            mode: self.fl_mode,
            case: self.fl_case,
            invert: self.fl_invert,
            time: self.time_check,
            from: self.range_text[0].clone(),
            to: self.range_text[1].clone(),
            lengths: self.lengths_filter.clone(),
        }
    }

    /// Scrive nella finestra Filtri le condizioni di `c` (le date solo se `con_date`: con una finestra di tempo accesa sono le sue).
    fn carica_condizioni(&mut self, c: &FilterConditions, con_date: bool) {
        for (i, &scelto) in c.levels.iter().enumerate() {
            self.set_timeline_filter_level(i, scelto);
        }
        self.fl_text = c.text.clone();
        self.fl_mode = c.mode;
        self.fl_case = c.case;
        self.fl_invert = c.invert;
        if con_date {
            self.range_text = [c.from.clone(), c.to.clone()];
        }
    }

    /// Le condizioni che mostrano le righe di questa scheda, in una riga per la barra di stato; `None` se la scheda mostra tutto.
    /// Una vista filtrata ha quelle che l'hanno fatta; una scheda con una finestra di tempo, le sue date.
    pub fn active_filters(&self) -> Option<String> {
        let c = if let Some(v) = &self.filter_view {
            v.conditions.clone()?
        } else if self.window.is_some() {
            FilterConditions {
                levels: [false; 4],
                text: String::new(),
                mode: Mode::Text,
                case: false,
                invert: false,
                time: true,
                from: self.range_text[0].clone(),
                to: self.range_text[1].clone(),
                lengths: None,
            }
        } else {
            return None;
        };
        (!c.is_empty()).then(|| self.riassumi(&c))
    }

    /// Una vista filtrata ha chiesto nuove condizioni (OK nella finestra Filtri): le rifa' il file originale, non la vista.
    pub fn take_refilter(&mut self) -> Option<FilterConditions> {
        self.refilter.take()
    }

    /// Il file di cui questa scheda e' la vista filtrata.
    pub fn filter_origin(&self) -> Option<&Path> {
        self.filter_view.as_ref().map(|v| v.origin.as_path())
    }

    /// Un filtro (o una ricerca) e' in corso in questa scheda.
    pub fn filter_job_running(&self) -> bool {
        self.search_job.is_some()
    }

    /// Le condizioni di `c` come se si fossero scritte nella finestra Filtri di questa scheda e si fosse premuto OK: e' il modo in
    /// cui il file originale rifa' la vista filtrata. Un errore resta nella finestra (aperta).
    pub fn apply_filter_conditions(&mut self, c: &FilterConditions) {
        self.open_time_dialog();
        self.carica_condizioni(c, true);
        self.time_check = c.time;
        self.time_submit();
    }

    /// Il testo del filtro per righe della finestra Filtri.
    pub fn filters_text(&self) -> &str {
        &self.fl_text
    }

    /// Scrive il testo del filtro per righe della finestra Filtri.
    pub fn dialog_filters_set_text(&mut self, testo: &str) {
        if self.dialog == Some(DialogKind::Time) {
            self.fl_text = testo.to_string();
        }
    }

    /// Sceglie il modo del testo: 0 testo semplice, 1 glob, 2 espressione regolare.
    pub fn dialog_filters_set_mode(&mut self, modo: usize) {
        if self.dialog == Some(DialogKind::Time) {
            self.fl_mode = match modo {
                1 => Mode::Glob,
                2 => Mode::Regex,
                _ => Mode::Text,
            };
        }
    }

    /// La casella "maiuscole/minuscole" del filtro per righe.
    pub fn dialog_filters_set_case(&mut self, scelto: bool) {
        if self.dialog == Some(DialogKind::Time) {
            self.fl_case = scelto;
        }
    }

    /// La casella "nascondi le righe che corrispondono" del filtro per righe.
    pub fn dialog_filters_set_invert(&mut self, scelto: bool) {
        if self.dialog == Some(DialogKind::Time) {
            self.fl_invert = scelto;
        }
    }

    /// Accende o spegne il livello `i` (0 ERROR .. 3 DEBUG) nella finestra Filtri (e' lo stesso dei bottoni della timeline).
    pub fn dialog_filters_set_level(&mut self, i: usize, scelto: bool) {
        if self.dialog == Some(DialogKind::Time) {
            self.set_timeline_filter_level(i, scelto);
        }
    }

    /// I tasti della finestra "Tempo": le scritte arrivano dai campi nativi (`dialog_set_field`); qui solo Invio (applica), Esc
    /// (annulla) e Alt+A (riconosci di nuovo il formato).
    pub(super) fn time_key(&mut self, key: KeyEvent) {
        let alt = key.modifiers.contains(KeyModifiers::ALT);
        match key.code {
            KeyCode::Esc => self.dialog = None,
            KeyCode::Enter => self.time_submit(),
            KeyCode::Char('a') | KeyCode::Char('A') if alt => self.dialog_time_auto(),
            _ => {}
        }
    }

    /// Il filtro per data si accende o si spegne senza aprire niente (il pulsante della barra, il menu Log): se c'e' una finestra di
    /// tempo la toglie; altrimenti, se si ricordano delle date, la rimette; altrimenti apre la finestra "Tempo". Un errore nel
    /// rimetterla (le date non valgono piu') va nel messaggio della barra di stato.
    pub fn toggle_time_filter(&mut self) {
        if self.window.is_some() {
            self.clear_time_window();
            return;
        }
        self.assicura_maschera();
        let (da, a) = (self.range_text[0].clone(), self.range_text[1].clone());
        if self.time_mask.is_some() && !(da.trim().is_empty() && a.trim().is_empty()) {
            if let Err(e) = self.apply_time_range(&da, &a) {
                self.flash = Some(e);
            }
            return;
        }
        self.open_time_dialog();
    }

    /// Il filtro per data e' acceso (c'e' una finestra di tempo).
    pub fn time_filter_on(&self) -> bool {
        self.window.is_some()
    }
}

/// Un estremo del filtro per data, per scriverlo in una frase: vuoto = "..." (nessun limite).
fn punti(t: &str) -> String {
    if t.trim().is_empty() {
        "\u{2026}".to_string()
    } else {
        t.trim().to_string()
    }
}
