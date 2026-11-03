//! La navigazione temporale di una scheda (F1): la maschera dell'orario del file (riconosciuta da sola o scelta a mano), l'orario
//! della prima riga visibile e la FINESTRA DI TEMPO, che restringe la vista a un intervallo `[da, a)` come se il file fosse
//! solo quello. Ogni scheda ha la sua maschera e la sua finestra: un file e' un mondo a se'.

use super::{App, DialogKind};
use mrp_core::lines::Cancelled;
use mrp_core::scan::{self, Next};
use mrp_core::timemask::{self, MaskError, TimeMask};
use mrp_core::timesearch;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::sync::atomic::AtomicBool;

/// Quanti byte del file si guardano per riconoscere la maschera, e quante righe al piu'.
const CAMPIONE_BYTE: u64 = 64 * 1024;
const CAMPIONE_RIGHE: usize = 200;

/// Le maschere con cui si leggono gli orari scritti a mano nel dialogo "da / a", se non sono nel formato del file: dalla piu'
/// lunga alla piu' corta. Gli orari soli (la data e' quella del file) sono in `ORARI_A_MANO`.
const FORME_A_MANO: [&str; 7] = [
    "yyyy-MM-dd HH:mm:ss.SSS",
    "yyyy-MM-dd HH:mm:ss",
    "yyyy-MM-ddTHH:mm:ss",
    "yyyy-MM-dd HH:mm",
    "yyyy-MM-ddTHH:mm",
    "yyyy-MM-dd",
    "yyyy/MM/dd HH:mm:ss",
];
const ORARI_A_MANO: [&str; 3] = ["HH:mm:ss.SSS", "HH:mm:ss", "HH:mm"];

/// I millisecondi di un giorno.
const GIORNO: i64 = 86_400_000;

impl App {
    /// Il limite basso della vista: l'inizio della finestra di tempo, o 0.
    pub(crate) fn lo(&self) -> u64 {
        self.window.map_or(0, |w| w.0)
    }

    /// Il limite alto della vista: la fine della finestra di tempo, o la fine del documento.
    pub(crate) fn hi(&self) -> u64 {
        let len = self.document_len();
        self.window.map_or(len, |w| w.1.min(len))
    }

    /// Come `scan::next_row`, ma una riga che comincia oltre la finestra non c'e'.
    pub(crate) fn next_row_w(&self, pos: u64) -> Next {
        let Some(doc) = self.doc.as_ref() else {
            return Next::End;
        };
        let hi = self.hi();
        match scan::next_row(doc, pos, mrp_core::scan::DEFAULT_MAX_SCAN) {
            Next::Line(s) | Next::Cut(s) if s >= hi => Next::End,
            altro => altro,
        }
    }

    /// Come `scan::prev_row_start`, ma una riga che comincia prima della finestra non c'e'.
    pub(crate) fn prev_row_w(&self, pos: u64) -> Option<u64> {
        let doc = self.doc.as_ref()?;
        let lo = self.lo();
        match scan::prev_row_start(doc, pos, mrp_core::scan::DEFAULT_MAX_SCAN) {
            Some(r) if r >= lo => Some(r),
            _ => None,
        }
    }

    /// Le prime righe del file (non vuote), per riconoscere la maschera: solo con gli a-capo a un byte e testo a byte (UTF-16 e
    /// UTF-32 non si riconoscono: le righe sono a unita' piu' grandi).
    fn campione_righe(&self) -> Vec<Vec<u8>> {
        let Some(doc) = self.doc.as_ref() else {
            return Vec::new();
        };
        if self.enc.unit() != 1 {
            return Vec::new();
        }
        let dati = doc.read_range(0, doc.len().min(CAMPIONE_BYTE));
        let mut righe: Vec<&[u8]> = dati.split(|&b| b == b'\n').collect();
        // l'ultimo pezzo puo' essere una riga tagliata dal limite del campione
        if doc.len() > CAMPIONE_BYTE {
            righe.pop();
        }
        righe
            .into_iter()
            .map(|r| r.strip_suffix(b"\r").unwrap_or(r))
            .filter(|r| !r.is_empty())
            .take(CAMPIONE_RIGHE)
            .map(<[u8]>::to_vec)
            .collect()
    }

    /// Riconosce la maschera dalle prime righe, una volta sola per scheda (la scelta a mano la sostituisce).
    fn assicura_maschera(&mut self) {
        if self.time_mask_tried {
            return;
        }
        self.time_mask_tried = true;
        let righe = self.campione_righe();
        let rif: Vec<&[u8]> = righe.iter().map(Vec::as_slice).collect();
        self.time_mask = timemask::detect(&rif).map(|d| d.mask);
    }

    /// Il pattern della maschera dell'orario di questo file (riconosciuta o scelta a mano), `None` se non c'e'.
    pub fn time_mask_pattern(&mut self) -> Option<String> {
        self.assicura_maschera();
        self.time_mask.as_ref().map(|m| m.pattern().to_string())
    }

    /// Quanto la maschera puo' stare lontana dall'inizio della riga (la finestra di ricerca).
    pub fn time_mask_within(&mut self) -> usize {
        self.assicura_maschera();
        self.time_mask.as_ref().map_or(0, TimeMask::within)
    }

    /// L'orario della prima riga visibile, scritto `aaaa-mm-gg hh:mm:ss` (senza la data con le maschere della sola ora), o
    /// `None` se la riga non ne ha uno (o il file non ha una maschera).
    pub fn time_at_top(&mut self) -> Option<String> {
        self.assicura_maschera();
        let mask = self.time_mask.as_ref()?;
        let doc = self.doc.as_ref()?;
        let chiave = timesearch::key_of_line(doc, mask, self.top)?;
        Some(timemask::iso(chiave, mask.has_date()))
    }

    /// Sceglie a mano la maschera dell'orario: `pattern` (vedi `TimeMask::parse`) e quanto lontano dall'inizio della riga puo'
    /// stare l'orario. Se il pattern non va, `Err` col motivo e non cambia niente. Cambiare la maschera toglie la finestra di
    /// tempo (le posizioni dipendevano da quella di prima).
    pub fn set_time_mask(&mut self, pattern: &str, within: usize) -> Result<(), String> {
        let mask = TimeMask::parse(pattern, within).map_err(|e| match e {
            MaskError::Incomplete => self.cat.t("time.err.incomplete").to_string(),
            MaskError::BadToken(t) => self.cat.tf("time.err.bad_token", &[&t]),
        })?;
        self.time_mask = Some(mask);
        self.time_mask_tried = true;
        self.window = None;
        Ok(())
    }

    /// La finestra di tempo attuale: l'inizio della prima riga e l'inizio di quella che la segue.
    pub fn time_window(&self) -> Option<(u64, u64)> {
        self.window
    }

    /// Toglie la finestra di tempo: si torna a vedere tutto il file (il cursore resta dov'e').
    pub fn clear_time_window(&mut self) {
        if self.window.take().is_some() {
            self.found = None;
            self.flash = Some(self.cat.t("time.range_off").to_string());
        }
    }

    /// La chiave di un orario scritto a mano: nel formato del file (la sua maschera), poi nelle forme comuni; senza data prende
    /// quella della prima riga con orario (`giorno_primo`: i millisecondi dal 1970 della mezzanotte di quel giorno).
    fn chiave_a_mano(testo: &str, mask: &TimeMask, giorno_primo: Option<i64>) -> Option<i64> {
        let t = testo.trim();
        if let Some(k) = mask.read(t.as_bytes()) {
            return Some(k);
        }
        for forma in FORME_A_MANO {
            if let Some(k) = TimeMask::parse(forma, 0)
                .ok()
                .and_then(|m| m.read(t.as_bytes()))
            {
                return Some(k);
            }
        }
        for forma in ORARI_A_MANO {
            let Some(ms) = TimeMask::parse(forma, 0)
                .ok()
                .and_then(|m| m.read(t.as_bytes()))
            else {
                continue;
            };
            // un orario solo: la data e' quella del file (o, se il file ha la sola ora, resta l'ora)
            return Some(if mask.has_date() {
                giorno_primo? + ms
            } else {
                ms
            });
        }
        None
    }

    /// Restringe la vista alle righe fra due orari: da `da` (compreso) ad `a` (escluso); `da` vuoto = dall'inizio del file, `a`
    /// vuoto = fino alla fine. Gli orari si scrivono nel formato del file o in quelli comuni (`2026-11-01 02:30`, `02:30`...); senza
    /// data valgono nel giorno della prima riga. `Err` col motivo (e niente cambia) se non c'e' una maschera, se un orario non si
    /// capisce, se l'inizio non precede la fine o se nell'intervallo non c'e' nessuna riga. Il cursore va all'inizio della finestra.
    pub fn apply_time_range(&mut self, da: &str, a: &str) -> Result<(), String> {
        self.assicura_maschera();
        let Some(mask) = self.time_mask.clone() else {
            return Err(self.cat.t("time.err.no_mask").to_string());
        };
        let Some(doc) = self.doc.as_ref() else {
            return Err(self.cat.t("time.err.no_mask").to_string());
        };
        let libero = AtomicBool::new(false);
        let annullato = |_: Cancelled| self.cat.t("time.err.cancelled").to_string();
        // il giorno della prima riga con orario, per gli orari senza data
        let giorno_primo = timesearch::first_key(doc, &mask, &libero)
            .map_err(annullato)?
            .map(|(_, k)| k.div_euclid(GIORNO) * GIORNO);
        let leggi = |testo: &str| -> Result<Option<i64>, String> {
            if testo.trim().is_empty() {
                return Ok(None);
            }
            match Self::chiave_a_mano(testo, &mask, giorno_primo) {
                Some(k) => Ok(Some(k)),
                None => Err(self.cat.tf("time.err.bad_time", &[testo.trim()])),
            }
        };
        let (k_da, k_a) = (leggi(da)?, leggi(a)?);
        if let (Some(x), Some(y)) = (k_da, k_a) {
            if x >= y {
                return Err(self.cat.t("time.err.order").to_string());
            }
        }
        let lo = match k_da {
            Some(k) => timesearch::find_key(doc, &mask, k, &libero)
                .map_err(annullato)?
                .ok_or_else(|| self.cat.t("time.err.empty").to_string())?,
            None => 0,
        };
        let hi = match k_a {
            Some(k) => timesearch::find_key(doc, &mask, k, &libero)
                .map_err(annullato)?
                .unwrap_or(doc.len()),
            None => doc.len(),
        };
        if lo >= hi {
            return Err(self.cat.t("time.err.empty").to_string());
        }
        self.window = Some((lo, hi));
        self.top = lo;
        self.cursor = lo;
        self.hscroll = 0;
        self.anchor = None;
        self.block = None;
        self.found = None;
        self.remember_col();
        self.scroll_adjust();
        let da_testo = da.trim();
        let a_testo = a.trim();
        self.flash = Some(self.cat.tf(
            "time.range_set",
            &[
                if da_testo.is_empty() { "…" } else { da_testo },
                if a_testo.is_empty() { "…" } else { a_testo },
            ],
        ));
        Ok(())
    }

    /// L'orario (scritto `aaaa-mm-gg hh:mm:ss`) della riga che contiene `pos`, vuoto se non ne ha uno.
    fn etichetta_a(&self, pos: u64) -> String {
        let (Some(mask), Some(doc)) = (self.time_mask.as_ref(), self.doc.as_ref()) else {
            return String::new();
        };
        timesearch::key_of_line(doc, mask, pos)
            .map(|k| timemask::iso(k, mask.has_date()))
            .unwrap_or_default()
    }

    /// Alt+T: apre il dialogo dell'intervallo di tempo. I campi partono dalla finestra attuale, se c'e', altrimenti da
    /// "dall'orario della prima riga visibile, senza fine".
    pub fn open_time_range_dialog(&mut self) {
        self.assicura_maschera();
        let (da, a) = match self.window {
            Some((lo, hi)) => (self.etichetta_a(lo), self.etichetta_a(hi)),
            None => (self.time_at_top().unwrap_or_default(), String::new()),
        };
        self.range_text = [da, a];
        self.range_field = 0;
        self.range_error = String::new();
        self.dialog = Some(DialogKind::TimeRange);
    }

    /// Alt+O: apre il dialogo del formato dell'orario, che parte dalla maschera attuale.
    pub fn open_time_mask_dialog(&mut self) {
        self.assicura_maschera();
        let (pattern, within) = self.time_mask.as_ref().map_or((String::new(), 0), |m| {
            (m.pattern().to_string(), m.within())
        });
        self.mask_text = [pattern, within.to_string()];
        self.mask_field = 0;
        self.mask_error = String::new();
        self.dialog = Some(DialogKind::TimeMask);
    }

    /// Il testo del campo `i` (0 da, 1 a) del dialogo dell'intervallo.
    pub fn time_range_text(&self, i: usize) -> &str {
        self.range_text.get(i).map_or("", String::as_str)
    }

    /// Il campo che ha il fuoco nel dialogo dell'intervallo (0 o 1).
    pub fn time_range_field(&self) -> u8 {
        self.range_field
    }

    /// L'ultimo errore del dialogo dell'intervallo (vuoto: nessuno).
    pub fn time_range_error(&self) -> &str {
        &self.range_error
    }

    /// Il testo del campo `i` (0 pattern, 1 entro quanti caratteri) del dialogo del formato.
    pub fn time_mask_text(&self, i: usize) -> &str {
        self.mask_text.get(i).map_or("", String::as_str)
    }

    /// Il campo che ha il fuoco nel dialogo del formato (0 o 1).
    pub fn time_mask_field(&self) -> u8 {
        self.mask_field
    }

    /// L'ultimo errore del dialogo del formato (vuoto: nessuno).
    pub fn time_mask_error(&self) -> &str {
        &self.mask_error
    }

    /// L'anteprima del dialogo del formato: le prime righe del file (al piu' tre, accorciate) e come le legge il pattern che c'e'
    /// scritto adesso (l'orario, o "(nessun orario)"; un pattern che non va non legge nessuna riga).
    pub fn time_mask_preview(&self) -> Vec<(String, String)> {
        let within = self.mask_text[1].trim().parse::<usize>().unwrap_or(0);
        let mask = TimeMask::parse(self.mask_text[0].trim(), within).ok();
        let nessuno = self.cat.t("time.mask.none").to_string();
        self.campione_righe()
            .iter()
            .take(3)
            .map(|riga| {
                let testo: String = String::from_utf8_lossy(riga).chars().take(44).collect();
                let letto = mask
                    .as_ref()
                    .and_then(|m| m.read(riga).map(|k| timemask::iso(k, m.has_date())))
                    .unwrap_or_else(|| nessuno.clone());
                (testo, letto)
            })
            .collect()
    }

    /// Imposta il testo di un campo dei dialoghi del tempo (`campo` 0 o 1); in un altro dialogo non fa niente.
    pub(crate) fn time_set_field(&mut self, campo: u8, testo: &str) {
        let i = usize::from(campo);
        match self.dialog {
            Some(DialogKind::TimeRange) if i < 2 => {
                self.range_text[i] = testo.to_string();
                self.range_error.clear();
            }
            Some(DialogKind::TimeMask) if i < 2 => {
                self.mask_text[i] = testo.to_string();
                self.mask_error.clear();
            }
            _ => {}
        }
    }

    /// Porta il fuoco su un campo dei dialoghi del tempo.
    pub(crate) fn time_focus(&mut self, campo: u8) {
        match self.dialog {
            Some(DialogKind::TimeRange) if campo < 2 => self.range_field = campo,
            Some(DialogKind::TimeMask) if campo < 2 => self.mask_field = campo,
            _ => {}
        }
    }

    /// Toglie la finestra di tempo e chiude il dialogo dell'intervallo (il pulsante "Togli intervallo", Alt+R).
    pub fn dialog_time_clear(&mut self) {
        if self.dialog == Some(DialogKind::TimeRange) {
            self.dialog = None;
            self.clear_time_window();
        }
    }

    /// Fa riconoscere di nuovo la maschera dalle prime righe e la scrive nel dialogo del formato (il pulsante, Alt+A).
    pub fn dialog_time_auto(&mut self) {
        if self.dialog != Some(DialogKind::TimeMask) {
            return;
        }
        self.time_mask_tried = false;
        self.time_mask = None;
        self.assicura_maschera();
        let (pattern, within) = self.time_mask.as_ref().map_or((String::new(), 0), |m| {
            (m.pattern().to_string(), m.within())
        });
        self.mask_text = [pattern, within.to_string()];
        self.mask_error = if self.time_mask.is_none() {
            self.cat.t("time.err.no_mask").to_string()
        } else {
            String::new()
        };
    }

    /// I tasti del dialogo dell'intervallo: Tab / frecce cambiano campo, i caratteri si scrivono, Invio applica (un errore resta nel
    /// dialogo), Alt+R toglie l'intervallo, Esc annulla.
    pub(crate) fn time_range_key(&mut self, key: KeyEvent) {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let alt = key.modifiers.contains(KeyModifiers::ALT);
        let campo = usize::from(self.range_field);
        match key.code {
            KeyCode::Esc => self.dialog = None,
            KeyCode::Tab | KeyCode::Down | KeyCode::Up | KeyCode::BackTab => {
                self.range_field ^= 1;
            }
            KeyCode::Enter => {
                let (da, a) = (self.range_text[0].clone(), self.range_text[1].clone());
                match self.apply_time_range(&da, &a) {
                    Ok(()) => self.dialog = None,
                    Err(e) => self.range_error = e,
                }
            }
            KeyCode::Char('r') | KeyCode::Char('R') if alt && !ctrl => self.dialog_time_clear(),
            KeyCode::Backspace => {
                self.range_text[campo].pop();
                self.range_error.clear();
            }
            KeyCode::Char(c) if !ctrl && !alt => {
                self.range_text[campo].push(c);
                self.range_error.clear();
            }
            _ => {}
        }
    }

    /// I tasti del dialogo del formato: come quelli dell'intervallo; il secondo campo vuole cifre; Alt+A lo fa riconoscere di
    /// nuovo.
    pub(crate) fn time_mask_key(&mut self, key: KeyEvent) {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let alt = key.modifiers.contains(KeyModifiers::ALT);
        let campo = usize::from(self.mask_field);
        match key.code {
            KeyCode::Esc => self.dialog = None,
            KeyCode::Tab | KeyCode::Down | KeyCode::Up | KeyCode::BackTab => {
                self.mask_field ^= 1;
            }
            KeyCode::Enter => {
                let within = self.mask_text[1].trim().parse::<usize>().unwrap_or(0);
                let pattern = self.mask_text[0].trim().to_string();
                match self.set_time_mask(&pattern, within) {
                    Ok(()) => self.dialog = None,
                    Err(e) => self.mask_error = e,
                }
            }
            KeyCode::Char('a') | KeyCode::Char('A') if alt && !ctrl => self.dialog_time_auto(),
            KeyCode::Backspace => {
                self.mask_text[campo].pop();
                self.mask_error.clear();
            }
            KeyCode::Char(c)
                if !ctrl
                    && !alt
                    && (campo == 0 || (c.is_ascii_digit() && self.mask_text[1].len() < 3)) =>
            {
                self.mask_text[campo].push(c);
                self.mask_error.clear();
            }
            _ => {}
        }
    }
}
