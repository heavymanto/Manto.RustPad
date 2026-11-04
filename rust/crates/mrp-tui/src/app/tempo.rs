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
const CAMPIONE_RIGHE: usize = 1000;

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

/// Quanti byte dall'inizio del file si guardano al piu' per trovare le righe che leggono un formato (anteprima e suggerimento), e
/// a pezzi di quanti.
pub(super) const SCANSIONE_BYTE: u64 = 4 * 1024 * 1024;
const PEZZO_SCANSIONE: u64 = 256 * 1024;
/// "Entro quanti caratteri" con cui si legge la seconda data di una riga.
const SUGGERIMENTO_ENTRO: usize = 200;

/// Da dove viene un tempo che il file offre da usare.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceKind {
    /// L'orario all'inizio della riga.
    Start,
    /// Una data dentro la riga, dopo questo testo (`@ `).
    After(String),
    /// La N-esima data della riga.
    Number(usize),
    /// Una regola scritta a mano dall'utente.
    Rule,
}

/// Un tempo che il file offre da usare nel dialogo del formato: come si legge (pattern, quanto lontano dall'inizio riga, quale
/// occorrenza), un esempio di come e' scritto nel file e quante righe lo hanno (fra quelle guardate).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimeSource {
    pub kind: SourceKind,
    pub pattern: String,
    pub within: usize,
    pub skip: usize,
    pub example: String,
    pub lines: usize,
}

/// Che tempo e' una maschera: all'inizio della riga, dopo un testo fra apici, la N-esima data.
fn tipo_fonte(pattern: &str, skip: usize) -> SourceKind {
    if let Some(resto) = pattern.strip_prefix('\'') {
        if let Some(fine) = resto.find('\'') {
            return SourceKind::After(resto[..fine].to_string());
        }
    }
    if skip > 0 {
        SourceKind::Number(skip + 1)
    } else {
        SourceKind::Start
    }
}

/// L'anteprima del formato gia' calcolata, con la chiave (pattern, finestra, occorrenza, lunghezza) con cui lo e' stata.
pub(super) type PreviewMemo = (String, Vec<(String, String)>);

/// Il suggerimento "in queste righe c'e' anche un'altra data": il formato che la legge come seconda occorrenza, un esempio di come
/// e' scritta e quante righe (fra quelle guardate) ce l'hanno.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimeSuggestion {
    pub pattern: String,
    /// Quale occorrenza dell'orario nella riga vale con quel pattern (0: la prima).
    pub skip: usize,
    pub example: String,
    pub lines: usize,
}

/// Quanti gruppi di quattro cifre di seguito ci sono nella riga (grossolano: serve a scartare le righe che non possono avere due date).
fn gruppi_di_quattro_cifre(riga: &[u8]) -> usize {
    let mut gruppi = 0;
    let mut di_fila = 0;
    for &b in riga {
        if b.is_ascii_digit() {
            di_fila += 1;
            if di_fila == 4 {
                gruppi += 1;
                di_fila = 0;
            }
        } else {
            di_fila = 0;
        }
    }
    gruppi
}

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

    /// Le righe del file con cui riconoscere la maschera: quelle all'inizio e, se il file e' piu' lungo del campione, quelle in
    /// fondo (non vuote, al piu' `CAMPIONE_RIGHE` per parte). Solo con gli a-capo a un byte e testo a byte (UTF-16 e UTF-32 non si
    /// riconoscono: le righe sono a unita' piu' grandi). Un log con grandi tabelle fra le righe con l'orario si riconosce lo stesso.
    fn campione_righe(&self) -> Vec<Vec<u8>> {
        let Some(doc) = self.doc.as_ref() else {
            return Vec::new();
        };
        if self.enc.unit() != 1 {
            return Vec::new();
        }
        let len = doc.len();
        // un pezzo di file in righe: la prima e l'ultima possono essere tagliate dai bordi del pezzo e si scartano
        let righe_di =
            |da: u64, quanti: u64, taglio_inizio: bool, taglio_fine: bool| -> Vec<Vec<u8>> {
                let dati = doc.read_range(da, quanti);
                let mut righe: Vec<&[u8]> = dati.split(|&b| b == b'\n').collect();
                if taglio_fine {
                    righe.pop();
                }
                if taglio_inizio && !righe.is_empty() {
                    righe.remove(0);
                }
                righe
                    .into_iter()
                    .map(|r| r.strip_suffix(b"\r").unwrap_or(r))
                    .filter(|r| !r.is_empty())
                    .take(CAMPIONE_RIGHE)
                    .map(<[u8]>::to_vec)
                    .collect()
            };
        let mut v = righe_di(0, len.min(CAMPIONE_BYTE), false, len > CAMPIONE_BYTE);
        if len > 2 * CAMPIONE_BYTE {
            v.extend(righe_di(len - CAMPIONE_BYTE, CAMPIONE_BYTE, true, false));
        }
        v
    }

    /// Riconosce la maschera dalle prime righe, una volta sola per scheda (la scelta a mano la sostituisce).
    pub(super) fn assicura_maschera(&mut self) {
        if self.time_mask_tried {
            return;
        }
        self.time_mask_tried = true;
        let righe = self.campione_righe();
        let rif: Vec<&[u8]> = righe.iter().map(Vec::as_slice).collect();
        // il formato standard scelto a mano si prova per primo
        let preferita = self
            .time_default
            .as_ref()
            .and_then(|(p, w)| TimeMask::parse(p, *w).ok());
        self.time_mask = timemask::detect_with(&rif, preferita.as_ref()).map(|d| d.mask);
    }

    /// Il pattern della maschera dell'orario di questo file (riconosciuta o scelta a mano), `None` se non c'e'.
    pub fn time_mask_pattern(&mut self) -> Option<String> {
        self.assicura_maschera();
        self.time_mask.as_ref().map(|m| m.pattern().to_string())
    }

    /// La maschera dell'orario per colorarlo nella riga (con i colori dei log accesi): solo gli orari che la maschera legge
    /// si colorano, quindi senza maschera niente.
    pub fn time_mask_for_colors(&mut self) -> Option<TimeMask> {
        self.assicura_maschera();
        self.time_mask.clone()
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
        self.set_time_mask_with(pattern, within, 0)
    }

    /// Come `set_time_mask`, e dice quale occorrenza dell'orario nella riga vale (`skip`: 0 la prima, 1 la seconda...): per i log con
    /// piu' date, quando quella che interessa non e' la prima (vedi `TimeMask::with_skip`).
    pub fn set_time_mask_with(
        &mut self,
        pattern: &str,
        within: usize,
        skip: usize,
    ) -> Result<(), String> {
        let mask = TimeMask::parse(pattern, within)
            .map_err(|e| match e {
                MaskError::Incomplete => self.cat.t("time.err.incomplete").to_string(),
                MaskError::BadToken(t) => self.cat.tf("time.err.bad_token", &[&t]),
            })?
            .with_skip(skip);
        self.time_mask = Some(mask);
        self.time_mask_tried = true;
        self.window = None;
        Ok(())
    }

    /// Quale occorrenza dell'orario nella riga vale per la maschera in uso (0: la prima).
    pub fn time_mask_skip(&self) -> usize {
        self.time_mask.as_ref().map_or(0, TimeMask::skip)
    }

    /// Il pattern della maschera per scriverlo in barra di stato: con l'occorrenza diversa dalla prima ci si aggiunge `#N`.
    pub(super) fn time_mask_label(&mut self) -> Option<String> {
        let skip = {
            self.assicura_maschera();
            self.time_mask_skip()
        };
        self.time_mask_pattern().map(|p| {
            if skip > 0 {
                format!("{p} #{}", skip + 1)
            } else {
                p
            }
        })
    }

    /// Il formato standard dell'orario (pattern e finestra), se l'utente ne ha scelto uno a mano.
    pub fn time_default(&self) -> Option<(String, usize)> {
        self.time_default.clone()
    }

    /// Cambia il formato standard senza salvare (lo usa il `Workspace` per allineare le schede).
    pub fn set_time_default(&mut self, formato: Option<(String, usize)>) {
        self.time_default = formato;
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
    pub(super) fn chiave_a_mano(
        testo: &str,
        mask: &TimeMask,
        giorno_primo: Option<i64>,
    ) -> Option<(i64, i64)> {
        let t = testo.trim();
        if let Some(k) = mask.read(t.as_bytes()) {
            return Some((k, mask.unit_ms()));
        }
        for forma in FORME_A_MANO {
            if let Ok(m) = TimeMask::parse(forma, 0) {
                if let Some(k) = m.read(t.as_bytes()) {
                    return Some((k, m.unit_ms()));
                }
            }
        }
        for forma in ORARI_A_MANO {
            let Some((m, ms)) = TimeMask::parse(forma, 0)
                .ok()
                .and_then(|m| m.read(t.as_bytes()).map(|k| (m, k)))
            else {
                continue;
            };
            // un orario solo: la data e' quella del file (o, se il file ha la sola ora, resta l'ora)
            let k = if mask.has_date() {
                giorno_primo? + ms
            } else {
                ms
            };
            return Some((k, m.unit_ms()));
        }
        None
    }

    /// Restringe la vista alle righe fra due orari: da `da` ad `a`, estremi compresi (un estremo scritto "02:30" copre tutto quel
    /// minuto, "2026-11-01" tutto quel giorno: vale l'unita' piu' piccola che ha scritto); `da` vuoto = dall'inizio del file, `a`
    /// vuoto = fino alla fine. Gli orari si scrivono nel formato del file o in quelli comuni (`2026-11-01 02:30`, `02:30`...); senza
    /// data valgono nel giorno della prima riga. `Err` col motivo (e niente cambia) se non c'e' una maschera, se un orario non si
    /// capisce, se l'inizio non precede la fine o se nell'intervallo non c'e' nessuna riga. Il cursore va all'inizio della finestra.
    pub fn apply_time_range(&mut self, da: &str, a: &str) -> Result<(), String> {
        let (lo, hi) = self.intervallo_tempo(da, a)?;
        self.window = Some((lo, hi));
        // le date si ricordano (per riaccendere il filtro dopo averlo spento senza riscriverle)
        self.range_text = [da.trim().to_string(), a.trim().to_string()];
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

    /// I byte `[lo, hi)` delle righe fra due orari (le regole di `apply_time_range`), senza cambiare la vista: serve a chi usa l'intervallo
    /// come una condizione fra altre (la vista filtrata).
    pub fn intervallo_tempo(&mut self, da: &str, a: &str) -> Result<(u64, u64), String> {
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
        let leggi = |testo: &str| -> Result<Option<(i64, i64)>, String> {
            if testo.trim().is_empty() {
                return Ok(None);
            }
            match Self::chiave_a_mano(testo, &mask, giorno_primo) {
                Some(k) => Ok(Some(k)),
                None => Err(self.cat.tf("time.err.bad_time", &[testo.trim()])),
            }
        };
        let (k_da, k_a) = (leggi(da)?.map(|(k, _)| k), leggi(a)?);
        // l'estremo finale e' compreso: finisce dove finisce l'unita' che ha scritto
        let k_a = k_a.map(|(k, unita)| k + unita);
        if let (Some(x), Some(y)) = (k_da, k_a) {
            if x >= y {
                return Err(self.cat.t("time.err.order").to_string());
            }
        }
        // "nessuna riga": il messaggio dice anche quali orari copre il file, cosi' si vede subito che la data era fuori
        let nessuna_riga = || -> String {
            let scritto = |k: i64| mask.write(k);
            let primo = timesearch::first_key(doc, &mask, &AtomicBool::new(false))
                .ok()
                .flatten();
            let ultimo = timesearch::last_key(doc, &mask, &AtomicBool::new(false))
                .ok()
                .flatten();
            let ellissi = |t: &str| {
                if t.trim().is_empty() {
                    "\u{2026}".to_string()
                } else {
                    t.trim().to_string()
                }
            };
            match (primo, ultimo) {
                (Some((_, p)), Some((_, u))) => self.cat.tf(
                    "time.err.empty_in",
                    &[&ellissi(da), &ellissi(a), &scritto(p), &scritto(u)],
                ),
                _ => self.cat.t("time.err.empty").to_string(),
            }
        };
        let lo = match k_da {
            Some(k) => timesearch::find_key(doc, &mask, k, &libero)
                .map_err(annullato)?
                .ok_or_else(nessuna_riga)?,
            None => 0,
        };
        let hi = match k_a {
            Some(k) => timesearch::find_key(doc, &mask, k, &libero)
                .map_err(annullato)?
                .unwrap_or(doc.len()),
            None => doc.len(),
        };
        if lo >= hi {
            return Err(nessuna_riga());
        }
        Ok((lo, hi))
    }

    /// Alt+T: apre il dialogo dell'intervallo di tempo. I campi partono dalla finestra attuale, se c'e', altrimenti da
    /// "dall'orario della prima riga visibile, senza fine".
    pub fn open_time_range_dialog(&mut self) {
        self.assicura_maschera();
        // con una finestra attiva i campi sono le date che l'hanno fatta (l'estremo finale e' compreso: rileggere l'orario della
        // riga che segue la allargherebbe); senza, "dall'orario della prima riga visibile, senza fine"
        if self.window.is_none() {
            self.range_text = [self.time_at_top().unwrap_or_default(), String::new()];
        }
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
        self.calcola_aiuti_formato();
        self.dialog = Some(DialogKind::TimeMask);
    }

    /// Per il dialogo del formato: quante righe del campione legge ognuno dei formati noti, e l'orario della prima riga che legge
    /// la maschera in uso. Si calcola una volta all'apertura (il campione non cambia mentre il dialogo e' aperto).
    pub(super) fn calcola_aiuti_formato(&mut self) {
        self.mask_skip = self.time_mask_skip();
        self.calcola_suggerimento();
        let righe = self.campione_righe();
        self.mask_matches = timemask::PRESETS
            .iter()
            .map(|p| {
                TimeMask::parse(p.pattern, p.within)
                    .map_or(0, |m| righe.iter().filter(|r| m.read(r).is_some()).count())
            })
            .collect();
        self.mask_detected = self.time_mask.as_ref().and_then(|m| {
            righe
                .iter()
                .find_map(|r| m.read(r))
                .map(|k| (timemask::iso(k, m.has_date()), m.pattern().to_string()))
        });
        self.calcola_fonti();
        // il formato noto evidenziato e' quello della maschera in uso, se e' uno di loro
        self.mask_preset = self
            .time_mask
            .as_ref()
            .and_then(|m| {
                timemask::PRESETS
                    .iter()
                    .position(|p| p.pattern == m.pattern())
            })
            .unwrap_or(0);
    }

    /// Quante righe del campione legge ognuno dei formati noti (nell'ordine di `timemask::PRESETS`).
    pub fn time_preset_matches(&self) -> &[usize] {
        &self.mask_matches
    }

    /// L'orario della prima riga letta dalla maschera in uso e il suo pattern, se ce n'e' una.
    pub fn time_detected(&self) -> Option<(String, String)> {
        self.mask_detected.clone()
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

    /// Chiama `f` per ogni riga (senza a-capo) dall'inizio del file, fino a `SCANSIONE_BYTE` byte, finche' ritorna vero. Solo con gli
    /// a-capo a un byte e testo a byte (come `campione_righe`).
    fn scansiona_righe(&self, f: impl FnMut(&[u8]) -> bool) {
        self.scansiona_righe_per(SCANSIONE_BYTE, f);
    }

    /// Come `scansiona_righe`, ma guardando al piu' `max` byte.
    pub(super) fn scansiona_righe_per(&self, max: u64, mut f: impl FnMut(&[u8]) -> bool) {
        let Some(doc) = self.doc.as_ref() else {
            return;
        };
        if self.enc.unit() != 1 {
            return;
        }
        let len = doc.len();
        let fine = len.min(max);
        let mut pos = 0u64;
        let mut resto: Vec<u8> = Vec::new();
        while pos < fine {
            let dati = doc.read_range(pos, PEZZO_SCANSIONE.min(fine - pos));
            if dati.is_empty() {
                break;
            }
            pos += dati.len() as u64;
            resto.extend_from_slice(&dati);
            let mut da = 0;
            while let Some(i) = resto[da..].iter().position(|&b| b == b'\n') {
                let riga = &resto[da..da + i];
                if !f(riga.strip_suffix(b"\r").unwrap_or(riga)) {
                    return;
                }
                da += i + 1;
            }
            resto.drain(..da);
        }
        // l'ultima riga, se il file finisce senza a-capo (non se la scansione si e' fermata a meta' di una riga)
        if pos >= len && !resto.is_empty() {
            f(resto.strip_suffix(b"\r").unwrap_or(&resto));
        }
    }

    /// L'anteprima del dialogo del formato: le righe del file che il pattern scritto adesso LEGGE (al piu' tre, dalle prime righe
    /// fino a `SCANSIONE_BYTE`), ognuna con l'orario letto; se non ne legge nessuna, le prime tre righe con "(nessun orario)". Le
    /// righe lunghe si mostrano attorno alla data letta. Si ricalcola solo quando cambia il pattern, la finestra o l'occorrenza.
    pub fn time_mask_preview(&self) -> Vec<(String, String)> {
        let within = self.mask_text[1].trim().parse::<usize>().unwrap_or(0);
        let pattern = self.mask_text[0].trim();
        let chiave = format!(
            "{pattern}|{within}|{}|{}",
            self.mask_skip,
            self.document_len()
        );
        if let Some((k, v)) = self.preview_memo.borrow().as_ref() {
            if *k == chiave {
                return v.clone();
            }
        }
        let mask = TimeMask::parse(pattern, within)
            .ok()
            .map(|m| m.with_skip(self.mask_skip));
        let nessuno = self.cat.t("time.mask.none").to_string();
        let mut lette: Vec<(String, String)> = Vec::new();
        let mut prime: Vec<String> = Vec::new();
        self.scansiona_righe(|riga| {
            if prime.len() < 3 {
                prime.push(String::from_utf8_lossy(riga).chars().take(44).collect());
            }
            if let Some(m) = mask.as_ref() {
                if let Some((a, _, k)) = m.find(riga) {
                    // la riga attorno alla data: se comincia piu' avanti si mostra da poco prima
                    let da = a.saturating_sub(12);
                    let mut testo: String = String::from_utf8_lossy(&riga[da..])
                        .chars()
                        .take(44)
                        .collect();
                    if da > 0 {
                        testo.insert(0, '\u{2026}');
                    }
                    lette.push((testo, timemask::iso(k, m.has_date())));
                }
            }
            lette.len() < 3 && (mask.is_some() || prime.len() < 3)
        });
        let v: Vec<(String, String)> = if lette.is_empty() {
            prime.into_iter().map(|t| (t, nessuno.clone())).collect()
        } else {
            lette
        };
        *self.preview_memo.borrow_mut() = Some((chiave, v.clone()));
        v
    }

    /// Cerca nelle prime righe del file un'altra data oltre alla prima (l'ora di scrittura all'inizio, la data dei dati dentro la
    /// riga). Le seconde date si raggruppano per formato e per i due caratteri che le precedono (`@ ` in `funding @ 2022-01-01 00:00`):
    /// un gruppo e' un buon suggerimento se ha almeno cinque righe, almeno cinque date diverse e se vanno avanti nel file (quasi sempre non
    /// decrescenti): una data che si ripete uguale (l'inizio del backtest nelle tabelle) o che salta a caso non si suggerisce. Vince il
    /// gruppo con piu' righe (a parita' il formato piu' preciso); il suggerimento e' un pattern con il testo che precede fra apici
    /// (`'@ 'yyyy-MM-dd HH:mm`), che legge solo quelle righe.
    pub(super) fn calcola_suggerimento(&mut self) {
        const CANDIDATI: [&str; 9] = [
            "yyyy-MM-dd HH:mm:ss.SSS",
            "yyyy-MM-dd HH:mm:ss,SSS",
            "yyyy-MM-dd HH:mm:ss",
            "yyyy-MM-ddTHH:mm:ss",
            "yyyy-MM-dd HH:mm",
            "yyyy/MM/dd HH:mm:ss",
            "dd/MM/yyyy HH:mm:ss",
            "dd/MM/yyyy HH:mm",
            "yyyy-MM-dd",
        ];
        const MIN_RIGHE: usize = 5;
        /// Statistiche di un gruppo (formato, testo che precede).
        #[derive(Default)]
        struct Gruppo {
            righe: usize,
            chiavi: Vec<i64>,
            esempio: String,
        }
        self.mask_suggestion = None;
        let maschere: Vec<TimeMask> = CANDIDATI
            .iter()
            .filter_map(|p| TimeMask::parse(p, SUGGERIMENTO_ENTRO).ok())
            .map(|m| m.with_skip(1))
            .collect();
        let mut gruppi: std::collections::HashMap<(usize, String), Gruppo> =
            std::collections::HashMap::new();
        let mut righe = 0usize;
        self.scansiona_righe(|riga| {
            righe += 1;
            // una seconda data vuole almeno due anni di quattro cifre nella riga
            if gruppi_di_quattro_cifre(riga) >= 2 {
                for (i, m) in maschere.iter().enumerate() {
                    let Some((a, b, k)) = m.find(riga) else {
                        continue;
                    };
                    // i due caratteri che precedono la data, se sono testo (non un apice, che romperebbe il pattern)
                    let Ok(prima) = std::str::from_utf8(&riga[a.saturating_sub(2)..a]) else {
                        continue;
                    };
                    if prima.contains('\'') {
                        continue;
                    }
                    let g = gruppi.entry((i, prima.to_string())).or_default();
                    g.righe += 1;
                    if g.chiavi.len() < 20_000 {
                        g.chiavi.push(k);
                    }
                    if g.esempio.is_empty() {
                        g.esempio = String::from_utf8_lossy(&riga[a..b]).into_owned();
                    }
                }
            }
            righe < 80_000
        });
        let utile = |g: &Gruppo| {
            let mut diversi = g.chiavi.clone();
            diversi.sort_unstable();
            diversi.dedup();
            let in_ordine = g.chiavi.windows(2).filter(|w| w[0] <= w[1]).count();
            g.righe >= MIN_RIGHE
                && diversi.len() >= 5
                && in_ordine * 100 >= g.chiavi.len().saturating_sub(1) * 95
        };
        // per ogni testo che precede la data, il formato piu' preciso fra quelli che la leggono (quasi) quante il migliore; poi i tre
        // testi con piu' righe
        let mut per_testo: std::collections::HashMap<String, Vec<(usize, &Gruppo)>> =
            std::collections::HashMap::new();
        for (chiave, g) in gruppi.iter().filter(|(_, g)| utile(g)) {
            per_testo
                .entry(chiave.1.clone())
                .or_default()
                .push((chiave.0, g));
        }
        let mut scelti: Vec<TimeSuggestion> = Vec::new();
        for (prima, candidati) in per_testo {
            let massimo = candidati.iter().map(|(_, g)| g.righe).max().unwrap_or(0);
            let meglio = candidati
                .iter()
                .filter(|(_, g)| g.righe * 10 >= massimo * 8)
                .min_by_key(|(i, g)| (maschere[*i].unit_ms(), usize::MAX - g.righe));
            if let Some((i, g)) = meglio {
                scelti.push(TimeSuggestion {
                    pattern: format!("'{prima}'{}", CANDIDATI[*i]),
                    skip: 0,
                    example: g.esempio.clone(),
                    lines: g.righe,
                });
            }
        }
        // l'ordine della mappa non conta: piu' righe prima, a parita' il testo
        scelti.sort_by(|a, b| {
            b.lines
                .cmp(&a.lines)
                .then_with(|| a.pattern.cmp(&b.pattern))
        });
        scelti.truncate(3);
        self.mask_suggestion = scelti.first().cloned();
        self.mask_groups = scelti;
    }

    /// I tempi che il file offre da usare, per il dialogo del formato: quello in uso (il primo), le date dentro le righe trovate da
    /// `calcola_suggerimento` e le regole scritte dall'utente che leggono almeno una riga delle prime del file. Una passata sola.
    pub(super) fn calcola_fonti(&mut self) {
        // le candidate: (tipo, pattern, entro, occorrenza)
        let mut candidate: Vec<(SourceKind, String, usize, usize)> = Vec::new();
        let mut aggiungi = |k: SourceKind, p: String, w: usize, s: usize| {
            if !candidate.iter().any(|c| c.1 == p && c.2 == w && c.3 == s) {
                candidate.push((k, p, w, s));
            }
        };
        // l'orario all'inizio della riga che il riconoscimento sceglierebbe da solo, e quello in uso: cosi' l'elenco e' lo stesso a ogni
        // apertura, qualunque sia il tempo scelto
        let righe_campione = self.campione_righe();
        let rif: Vec<&[u8]> = righe_campione.iter().map(Vec::as_slice).collect();
        if let Some(d) = timemask::detect(&rif) {
            aggiungi(
                SourceKind::Start,
                d.mask.pattern().to_string(),
                d.mask.within(),
                0,
            );
        }
        let attiva = self
            .time_mask
            .as_ref()
            .map(|m| (m.pattern().to_string(), m.within(), m.skip()));
        for g in &self.mask_groups {
            aggiungi(
                tipo_fonte(&g.pattern, g.skip),
                g.pattern.clone(),
                SUGGERIMENTO_ENTRO,
                g.skip,
            );
        }
        for (p, w, k) in &self.time_rules {
            aggiungi(SourceKind::Rule, p.clone(), *w, *k);
        }
        // quello in uso per ultimo: se e' gia' fra i precedenti resta dov'e' (e col suo tipo), altrimenti e' una voce in piu' in coda al suo
        // tipo. Cosi' le voci non cambiano posto a ogni OK (prima il tempo in uso saltava in cima e le altre si rimescolavano)
        if let Some(m) = self.time_mask.as_ref() {
            aggiungi(
                tipo_fonte(m.pattern(), m.skip()),
                m.pattern().to_string(),
                m.within(),
                m.skip(),
            );
        }
        // l'ordine non dipende da quale e' in uso: prima gli orari all'inizio della riga, poi le date dentro le righe, poi quelle
        // numerate, poi le regole dell'utente (a parita' resta l'ordine in cui sono state trovate)
        let rango = |k: &SourceKind| match k {
            SourceKind::Start => 0,
            SourceKind::After(_) => 1,
            SourceKind::Number(_) => 2,
            SourceKind::Rule => 3,
        };
        candidate.sort_by_key(|c| rango(&c.0));
        let maschere: Vec<Option<TimeMask>> = candidate
            .iter()
            .map(|(_, p, w, k)| TimeMask::parse(p, *w).ok().map(|m| m.with_skip(*k)))
            .collect();
        let mut conti = vec![0usize; candidate.len()];
        let mut esempi: Vec<String> = vec![String::new(); candidate.len()];
        let mut righe = 0usize;
        self.scansiona_righe(|riga| {
            righe += 1;
            for (i, m) in maschere.iter().enumerate() {
                if let Some((a, b, _)) = m.as_ref().and_then(|m| m.find(riga)) {
                    conti[i] += 1;
                    if esempi[i].is_empty() {
                        esempi[i] = String::from_utf8_lossy(&riga[a..b]).into_owned();
                    }
                }
            }
            righe < 80_000
        });
        self.mask_sources = candidate
            .into_iter()
            .enumerate()
            // il tempo in uso si mostra sempre (a meno che sia una regola che non legge niente); gli altri solo se leggono qualcosa
            .filter(|(i, c)| {
                conti[*i] > 0
                    || attiva.as_ref() == Some(&(c.1.clone(), c.2, c.3)) && c.0 != SourceKind::Rule
            })
            .map(|(i, (kind, pattern, within, skip))| TimeSource {
                kind,
                pattern,
                within,
                skip,
                example: esempi[i].clone(),
                lines: conti[i],
            })
            .collect();
    }

    /// I tempi che il file offre da usare nel dialogo del formato.
    pub fn time_sources(&self) -> &[TimeSource] {
        &self.mask_sources
    }

    /// Quale dei tempi offerti e' quello scritto adesso nel dialogo (pattern, entro e occorrenza uguali); `None` se e' un formato
    /// scritto a mano che non e' nell'elenco.
    pub fn time_source_selected(&self) -> Option<usize> {
        let pattern = self.mask_text[0].trim();
        let within = self.mask_text[1].trim().parse::<usize>().ok()?;
        self.mask_sources
            .iter()
            .position(|s| s.pattern == pattern && s.within == within && s.skip == self.mask_skip)
    }

    /// Sceglie nel dialogo il tempo `i` dell'elenco: il suo pattern, entro e occorrenza vanno nei campi (poi OK lo applica).
    pub fn dialog_time_pick_source(&mut self, i: usize) {
        if !matches!(self.dialog, Some(DialogKind::TimeMask | DialogKind::Time)) {
            return;
        }
        if let Some(s) = self.mask_sources.get(i).cloned() {
            self.mask_text = [s.pattern, s.within.to_string()];
            self.mask_skip = s.skip;
            self.mask_error.clear();
            self.time_error.clear();
        }
    }

    /// Toglie dall'elenco una delle "mie regole" (la voce `i` dei tempi offerti): non e' piu' offerta e sparisce dalle impostazioni.
    /// Una voce che non e' una regola dell'utente non si tocca. Se la regola e' quella in uso resta in uso, ma come un tempo del file.
    pub fn dialog_time_remove_rule(&mut self, i: usize) {
        if !matches!(self.dialog, Some(DialogKind::TimeMask | DialogKind::Time)) {
            return;
        }
        let Some(s) = self.mask_sources.get(i) else {
            return;
        };
        if s.kind != SourceKind::Rule {
            return;
        }
        let regola = (s.pattern.clone(), s.within, s.skip);
        self.time_rules.retain(|r| *r != regola);
        self.calcola_fonti();
        self.save_settings();
    }

    /// Le regole per leggere il tempo scritte dall'utente (pattern, entro, occorrenza).
    pub fn time_rules(&self) -> Vec<(String, usize, usize)> {
        self.time_rules.clone()
    }

    /// Cambia le regole senza salvare (lo usa il `Workspace` per allineare le schede).
    pub fn set_time_rules(&mut self, regole: Vec<(String, usize, usize)>) {
        self.time_rules = regole;
    }

    /// Ricorda una regola scritta a mano (se non c'e' gia'): al piu' otto, la piu' vecchia esce.
    pub(super) fn aggiungi_regola(&mut self, pattern: &str, within: usize, skip: usize) {
        let regola = (pattern.to_string(), within, skip);
        if self.time_rules.contains(&regola) {
            return;
        }
        self.time_rules.push(regola);
        if self.time_rules.len() > 8 {
            self.time_rules.remove(0);
        }
    }

    /// Il suggerimento "c'e' anche un'altra data" (vedi `calcola_suggerimento`), se c'e'.
    pub fn time_suggestion(&self) -> Option<&TimeSuggestion> {
        self.mask_suggestion.as_ref()
    }

    /// Quale occorrenza dell'orario nella riga e' scelta nel dialogo del formato (0: la prima).
    pub fn time_mask_skip_draft(&self) -> usize {
        self.mask_skip
    }

    /// Sceglie nel dialogo del formato quale occorrenza dell'orario nella riga vale (`n`: 0 la prima, 1 la seconda... al piu' 9). Dalla
    /// seconda in poi la data sta piu' avanti nella riga, quindi "entro quanti caratteri" si porta almeno a 200.
    pub fn dialog_time_set_skip(&mut self, n: usize) {
        if !matches!(self.dialog, Some(DialogKind::TimeMask | DialogKind::Time)) {
            return;
        }
        self.mask_skip = n.min(9);
        if self.mask_skip > 0
            && self.mask_text[1].trim().parse::<usize>().unwrap_or(0) < SUGGERIMENTO_ENTRO
        {
            self.mask_text[1] = SUGGERIMENTO_ENTRO.to_string();
        }
        self.mask_error.clear();
        self.time_error.clear();
    }

    /// Usa il suggerimento: il suo pattern, entro `SUGGERIMENTO_ENTRO` caratteri.
    pub fn dialog_time_use_suggestion(&mut self) {
        if !matches!(self.dialog, Some(DialogKind::TimeMask | DialogKind::Time)) {
            return;
        }
        if let Some(s) = self.mask_suggestion.clone() {
            self.mask_text = [s.pattern, SUGGERIMENTO_ENTRO.to_string()];
            self.mask_skip = s.skip;
            self.mask_error.clear();
            self.time_error.clear();
        }
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
            // la finestra "Tempo": 0 pattern, 1 entro quanti caratteri, 2 dal, 3 al (scrivere una data accende il filtro)
            Some(DialogKind::Time) if i < 2 => {
                self.mask_text[i] = testo.to_string();
                self.time_error.clear();
            }
            Some(DialogKind::Time) if i < 4 => {
                self.range_text[i - 2] = testo.to_string();
                self.time_error.clear();
                if !testo.trim().is_empty() {
                    self.time_check = true;
                }
            }
            _ => {}
        }
    }

    /// Porta il fuoco su un campo dei dialoghi del tempo.
    pub(crate) fn time_focus(&mut self, campo: u8) {
        match self.dialog {
            Some(DialogKind::TimeRange) if campo < 2 => self.range_field = campo,
            Some(DialogKind::TimeMask) if campo < 2 => self.mask_field = campo,
            Some(DialogKind::Time) if campo < 4 => self.time_field = campo,
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
        if !matches!(self.dialog, Some(DialogKind::TimeMask | DialogKind::Time)) {
            return;
        }
        // "riconoscilo di nuovo" e' il riconoscimento vero, senza la preferenza del formato standard
        let standard = self.time_default.take();
        self.time_mask_tried = false;
        self.time_mask = None;
        self.assicura_maschera();
        self.time_default = standard;
        let (pattern, within) = self.time_mask.as_ref().map_or((String::new(), 0), |m| {
            (m.pattern().to_string(), m.within())
        });
        self.mask_text = [pattern, within.to_string()];
        self.mask_skip = 0;
        self.mask_error = if self.time_mask.is_none() {
            self.cat.t("time.err.no_mask").to_string()
        } else {
            String::new()
        };
        if self.dialog == Some(DialogKind::Time) {
            self.time_error = std::mem::take(&mut self.mask_error);
            self.calcola_aiuti_formato();
        }
    }

    /// I formati noti (pattern e un esempio) e quale e' evidenziato, per il dialogo del formato.
    pub fn time_presets(&self) -> (Vec<(String, String)>, usize) {
        (
            timemask::PRESETS
                .iter()
                .map(|p| (p.pattern.to_string(), p.example.to_string()))
                .collect(),
            self.mask_preset,
        )
    }

    /// Sceglie il formato noto `i` nel dialogo del formato: il suo pattern e la sua finestra vanno nei campi (poi OK lo applica).
    pub fn dialog_time_preset(&mut self, i: usize) {
        if !matches!(self.dialog, Some(DialogKind::TimeMask | DialogKind::Time)) {
            return;
        }
        if let Some(p) = timemask::PRESETS.get(i) {
            self.mask_preset = i;
            self.mask_skip = 0;
            self.mask_text = [p.pattern.to_string(), p.within.to_string()];
            self.mask_error.clear();
            self.time_error.clear();
        }
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
                    Ok(()) => {
                        // scelta a mano: diventa il formato standard, provato per primo sui file che si apriranno
                        self.time_default = Some((pattern, within));
                        self.save_settings();
                        self.dialog = None;
                    }
                    Err(e) => self.mask_error = e,
                }
            }
            KeyCode::Char('a') | KeyCode::Char('A') if alt && !ctrl => self.dialog_time_auto(),
            // Alt+N / Alt+P: il formato noto dopo / prima
            KeyCode::Char('n') | KeyCode::Char('N') if alt && !ctrl => {
                let n = timemask::PRESETS.len();
                self.dialog_time_preset((self.mask_preset + 1) % n);
            }
            KeyCode::Char('p') | KeyCode::Char('P') if alt && !ctrl => {
                let n = timemask::PRESETS.len();
                self.dialog_time_preset((self.mask_preset + n - 1) % n);
            }
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
