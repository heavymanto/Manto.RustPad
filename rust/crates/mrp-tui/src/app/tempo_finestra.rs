//! La finestra "Tempo" (solo in quella con i controlli nativi): il formato dell'orario e il filtro per data in un posto solo.
//! In alto il formato (riconosciuto, scelto fra i noti o scritto a mano), sotto il filtro facoltativo: una casella lo accende e lo
//! spegne senza perdere le date, due campi (Dal / Al) scritti nel formato del file, un calendario e una X per ogni campo; un campo
//! vuoto vuol dire "senza limite". OK applica formato e filtro insieme; se il filtro non si puo' applicare niente cambia.

use super::dialogs::DialogView;
use super::{App, DialogKind};
use mrp_core::timemask::{self, TimeMask};
use mrp_core::timesearch;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::sync::atomic::AtomicBool;

/// I millisecondi di un giorno.
const GIORNO: i64 = 86_400_000;

/// Un mese del calendario: `(anno, mese)`.
pub type Month = (i32, u32);
/// Un giorno del calendario: `(anno, mese, giorno)`.
pub type Day = (i32, u32, u32);

impl App {
    /// Alt+T (`intervallo`) e Alt+O: nella finestra con i controlli nativi aprono la finestra "Tempo" (con il fuoco sul filtro o sul
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
        self.time_bounds = self.limiti_tempo();
        self.time_check = self.window.is_some();
        self.time_field = 2;
        self.time_error = String::new();
        self.dialog = Some(DialogKind::Time);
    }

    /// La prima e l'ultima chiave del file (con la maschera in uso), `None` senza maschera o senza righe con l'orario.
    fn limiti_tempo(&self) -> Option<(i64, i64)> {
        let mask = self.time_mask.as_ref()?;
        let doc = self.doc.as_ref()?;
        let libero = AtomicBool::new(false);
        let primo = timesearch::first_key(doc, mask, &libero).ok()??.1;
        let ultimo = timesearch::last_key(doc, mask, &libero).ok()??.1;
        Some((primo, ultimo))
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
            preview: self.time_mask_preview(),
            auto: cat.t("time.mask.auto").to_string(),
            has_mask: self.time_mask.is_some(),
            has_date: self.time_mask.as_ref().is_some_and(TimeMask::has_date),
            from: self.range_text[0].clone(),
            to: self.range_text[1].clone(),
            check: self.time_check,
            from_hint: suggerimento("time.filter.from_hint", self.time_bounds.map(|b| b.0)),
            to_hint: suggerimento("time.filter.to_hint", self.time_bounds.map(|b| b.1)),
            field: self.time_field,
            error: self.time_error.clone(),
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

    /// Il calendario del campo Dal (2) o Al (3): `(anno, mese)` con cui si apre e il giorno gia' scelto, se il campo ha una data.
    /// Si apre sul mese della data scritta, altrimenti su quello del primo (Dal) o dell'ultimo (Al) orario del file; `None` senza
    /// maschera con la data (non c'e' un calendario da mostrare).
    pub fn time_calendar(&self, i: u8) -> Option<(Month, Option<Day>)> {
        let mask = self.time_mask.as_ref().filter(|m| m.has_date())?;
        if !(2..4).contains(&i) {
            return None;
        }
        let scritto = Self::chiave_a_mano(&self.range_text[usize::from(i) - 2], mask, None);
        if let Some((k, _)) = scritto {
            let (a, m, g) = timemask::civil(k);
            return Some(((a, m), Some((a, m, g))));
        }
        let (primo, ultimo) = self.time_bounds?;
        let (a, m, _) = timemask::civil(if i == 2 { primo } else { ultimo });
        Some(((a, m), None))
    }

    /// Sceglie dal calendario il giorno `anno-mese-giorno` per il campo Dal (2) o Al (3): lo scrive nel formato del file; se il campo
    /// aveva gia' un'ora la tiene, altrimenti Dal parte da mezzanotte e Al arriva a fine giornata. Accende il filtro.
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
            .unwrap_or(if i == 2 { 0 } else { GIORNO - 1 });
        self.range_text[campo] = mask.write(timemask::key_from_civil(anno, mese, giorno) + ora);
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
        let cambia = self
            .time_mask
            .as_ref()
            .is_none_or(|m| m.pattern() != pattern || m.within() != within);
        // com'era, per rimettere tutto se il filtro non si applica
        let prima = (self.time_mask.clone(), self.time_mask_tried, self.window);
        if cambia {
            if let Err(e) = self.set_time_mask(&pattern, within) {
                self.time_error = e;
                return;
            }
        }
        let (da, a) = (self.range_text[0].clone(), self.range_text[1].clone());
        if self.time_check && !(da.trim().is_empty() && a.trim().is_empty()) {
            if let Err(e) = self.apply_time_range(&da, &a) {
                (self.time_mask, self.time_mask_tried, self.window) = prima;
                self.time_error = e;
                return;
            }
        } else if self.window.is_some() {
            self.clear_time_window();
        }
        if cambia {
            // scelta a mano: diventa il formato standard, provato per primo sui file che si apriranno
            self.time_default = Some((pattern, within));
            self.save_settings();
        }
        self.dialog = None;
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
