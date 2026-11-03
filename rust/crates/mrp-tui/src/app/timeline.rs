//! La timeline di una scheda (F2): una barra "mappa" accanto alla scrollbar, con una riga per ogni bucket di tempo (minuto, dieci
//! minuti, ora, giorno) lunga quanto il volume di righe di quel periodo. Le tacche (gli offset dei confini) le trova in un thread
//! `mrp_core::timeline::build` su una copia della sorgente del file, senza leggerlo per intero; qui ci sono lo stato (spenta di
//! fabbrica, per scheda), il lavoro in background, e le funzioni che servono a disegnarla e a usarla: le righe (raggruppate se i
//! bucket sono piu' di quelli che stanno nell'altezza), dov'e' la parte che si vede, il salto a un punto e l'orario sotto il mouse.
//! Ogni file e' un mondo a se': niente fusione di piu' file.

use super::App;
use mrp_core::timeline::{self, BuildError, Step, Timeline};
use mrp_core::timemask;
use mrp_core::timesearch;
use mrp_core::Document;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::Arc;

/// Quante tacche al massimo si cercano (oltre, la barra chiede un passo piu' lungo).
const TACCHE_MAX: usize = 20_000;

/// Come si dispongono le righe nell'altezza della barra.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimelineMode {
    /// Ogni bucket alto uguale: l'altezza e' il tempo, i buchi si vedono (la lunghezza della riga e' il volume).
    Time,
    /// Ogni bucket alto quanto i suoi byte: la barra segue la scrollbar (la lunghezza della riga e' la densita').
    Bytes,
}

/// Lo stato della timeline, a uso della barra.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TimelineStatus {
    /// Spenta.
    Off,
    /// Si stanno cercando le tacche: quante se ne sono trovate finora.
    Building(usize),
    /// Pronta.
    Ready,
    /// Nessuna riga con l'orario (la maschera non legge niente).
    NoTimes,
    /// I bucket sarebbero troppi per questo passo: quanti.
    TooMany(usize),
    /// Non si puo' calcolare (documento modificato o senza file).
    Unavailable,
}

/// Una riga della barra: uno o piu' bucket vicini, con dove sta nell'altezza e quanto e' pesante.
#[derive(Debug, Clone, PartialEq)]
pub struct TimelineRow {
    /// La chiave (ms dal 1970) dell'inizio e quanto tempo copre.
    pub start: i64,
    pub span_ms: i64,
    /// L'offset della prima riga del periodo e quanti byte occupa.
    pub offset: u64,
    pub bytes: u64,
    /// Dove sta nell'altezza della barra: frazioni da 0 a 1 (in modo `Bytes` una riga senza byte e' alta 0).
    pub y0: f32,
    pub y1: f32,
    /// Quanto e' lunga la riga, da 0 a 1 (volume in modo `Time`, densita' in modo `Bytes`, rispetto alla piu' pesante).
    pub weight: f32,
    /// La data (e l'ora, se il periodo e' piu' corto di un giorno) da scrivere accanto.
    pub label: String,
}

/// Il lavoro in background che cerca le tacche.
struct TimelineJob {
    rx: Receiver<Result<Timeline, BuildError>>,
    progress: Arc<AtomicUsize>,
    cancel: Arc<AtomicBool>,
    /// Quante tacche si aspettano (note solo a lavoro finito: finche' non lo e' vale 0 e si mostra "in corso").
    len: u64,
    window: Option<(u64, u64)>,
    /// Il passo chiesto (`None`: automatico).
    step: Option<Step>,
}

/// Tutto cio' che una scheda sa della sua timeline.
pub(super) struct TimelineState {
    pub on: bool,
    /// Il passo fisso scelto dall'utente; con `auto` vale quello che dice `timeline::auto_step` per il file.
    pub step: Step,
    pub auto: bool,
    pub mode: TimelineMode,
    data: Option<Arc<Timeline>>,
    job: Option<TimelineJob>,
    error: Option<BuildError>,
    /// Il documento e' modificato o senza file: non si puo' calcolare.
    unavailable: bool,
    /// Per quale lunghezza del file, finestra e passo sono le tacche che ci sono (se cambiano, si ricalcolano).
    built_len: u64,
    built_window: Option<(u64, u64)>,
    built_step: Option<Option<Step>>,
}

impl Default for TimelineState {
    fn default() -> Self {
        TimelineState {
            on: false,
            step: Step::Day,
            auto: true,
            mode: TimelineMode::Time,
            data: None,
            job: None,
            error: None,
            unavailable: false,
            built_len: 0,
            built_window: None,
            built_step: None,
        }
    }
}

/// Gli estremi del raggruppamento: quante tacche per riga perche' `n` tacche stiano in `max_righe` righe.
fn gruppo(n: usize, max_righe: usize) -> usize {
    n.div_ceil(max_righe.max(1)).max(1)
}

impl App {
    /// La timeline e' accesa su questa scheda.
    pub fn timeline_on(&self) -> bool {
        self.timeline.on
    }

    /// Accende o spegne la timeline di questa scheda (spegnendola il lavoro in corso si ferma, le tacche si buttano).
    pub fn set_timeline(&mut self, acceso: bool) {
        if self.timeline.on == acceso {
            return;
        }
        self.timeline.on = acceso;
        if !acceso {
            self.timeline_stop();
            self.timeline.data = None;
            self.timeline.error = None;
            self.timeline.unavailable = false;
        }
    }

    /// Accende la timeline se e' spenta, la spegne se e' accesa.
    pub fn toggle_timeline(&mut self) {
        self.set_timeline(!self.timeline.on);
    }

    /// Il passo della timeline in uso: quello delle tacche che ci sono (con il passo automatico, quello scelto per il file) o,
    /// prima che ci siano, il passo fisso (di fabbrica un giorno).
    pub fn timeline_step(&self) -> Step {
        self.timeline
            .data
            .as_ref()
            .map_or(self.timeline.step, |t| t.step)
    }

    /// Il passo e' scelto da solo (di fabbrica): il piu' lungo che da' una barra leggibile per il file.
    pub fn timeline_auto(&self) -> bool {
        self.timeline.auto
    }

    /// Sceglie un passo fisso: le tacche si ricalcolano.
    pub fn set_timeline_step(&mut self, passo: Step) {
        self.timeline.step = passo;
        self.timeline.auto = false;
    }

    /// Torna al passo scelto da solo: le tacche si ricalcolano.
    pub fn set_timeline_auto(&mut self) {
        self.timeline.auto = true;
    }

    /// Come si dispongono le righe (di fabbrica: l'altezza e' il tempo).
    pub fn timeline_mode(&self) -> TimelineMode {
        self.timeline.mode
    }

    /// Cambia la disposizione delle righe.
    pub fn set_timeline_mode(&mut self, modo: TimelineMode) {
        self.timeline.mode = modo;
    }

    /// Ferma il lavoro in corso (il thread si accorge dell'annullamento e muore da solo).
    fn timeline_stop(&mut self) {
        if let Some(job) = self.timeline.job.take() {
            job.cancel.store(true, Ordering::Relaxed);
        }
    }

    /// Da chiamare a ogni giro del disegno: raccoglie il risultato del thread e, se serve, ne fa partire uno nuovo (la timeline e'
    /// accesa e non ci sono tacche per questa lunghezza del file, finestra e passo). Ritorna vero se la barra e' in attesa di
    /// qualcosa (chi disegna deve ridisegnare fra poco).
    pub fn timeline_tick(&mut self) -> bool {
        if !self.timeline.on {
            return false;
        }
        // il risultato del thread
        if let Some(job) = self.timeline.job.as_ref() {
            match job.rx.try_recv() {
                Ok(esito) => {
                    let job = self.timeline.job.take().expect("lavoro in corso");
                    self.timeline.built_len = job.len;
                    self.timeline.built_window = job.window;
                    self.timeline.built_step = Some(job.step);
                    match esito {
                        Ok(t) => {
                            self.timeline.data = Some(Arc::new(t));
                            self.timeline.error = None;
                        }
                        Err(BuildError::Cancelled) => {}
                        Err(e) => {
                            self.timeline.data = None;
                            self.timeline.error = Some(e);
                        }
                    }
                }
                Err(TryRecvError::Empty) => return true,
                Err(TryRecvError::Disconnected) => {
                    self.timeline.job = None;
                }
            }
        }
        // serve un calcolo nuovo?
        self.assicura_maschera();
        let len = self.document_len();
        let finestra = self.window;
        let passo = (!self.timeline.auto).then_some(self.timeline.step);
        let aggiornata = self.timeline.built_step == Some(passo)
            && self.timeline.built_window == finestra
            && self.timeline.built_len == len;
        if aggiornata {
            return false;
        }
        // un lavoro per un'altra lunghezza / finestra / passo non serve piu'
        self.timeline_stop();
        let documento_ok = self.doc.as_ref().is_some_and(|d| !d.is_modified());
        if !documento_ok || self.path.as_os_str().is_empty() {
            self.timeline.unavailable = true;
            self.timeline.data = None;
            self.timeline.built_len = len;
            self.timeline.built_window = finestra;
            self.timeline.built_step = Some(passo);
            return false;
        }
        self.timeline.unavailable = false;
        let Some(mask) = self.time_mask.clone() else {
            self.timeline.data = None;
            self.timeline.error = Some(BuildError::NoTimes);
            self.timeline.built_len = len;
            self.timeline.built_window = finestra;
            self.timeline.built_step = Some(passo);
            return false;
        };
        let (lo, hi) = (self.lo(), self.hi());
        let path = self.path.clone();
        let (tx, rx) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let progress = Arc::new(AtomicUsize::new(0));
        let cancel_thread = Arc::clone(&cancel);
        let progress_thread = Arc::clone(&progress);
        std::thread::spawn(move || {
            // il thread apre la SUA copia del file (come l'indicizzazione): il documento non si condivide
            let esito = match Document::open(&path) {
                Ok(doc) => {
                    // il passo automatico dipende da quanto tempo copre il file: si guardano la prima e l'ultima riga
                    let passo = match passo {
                        Some(p) => Ok(p),
                        None => {
                            let primo =
                                timesearch::first_key_in(&doc, &mask, lo, hi, &cancel_thread);
                            let ultimo =
                                timesearch::last_key_in(&doc, &mask, lo, hi, &cancel_thread);
                            match (primo, ultimo) {
                                (Ok(Some((_, a))), Ok(Some((_, b)))) => {
                                    Ok(timeline::auto_step(a, b))
                                }
                                (Ok(_), Ok(_)) => Err(BuildError::NoTimes),
                                _ => Err(BuildError::Cancelled),
                            }
                        }
                    };
                    passo.and_then(|passo| {
                        timeline::build(
                            &doc,
                            &mask,
                            passo,
                            lo,
                            hi,
                            &cancel_thread,
                            TACCHE_MAX,
                            Some(&progress_thread),
                        )
                    })
                }
                Err(_) => Err(BuildError::NoTimes),
            };
            let _ = tx.send(esito);
        });
        self.timeline.job = Some(TimelineJob {
            rx,
            progress,
            cancel,
            len,
            window: finestra,
            step: passo,
        });
        true
    }

    /// Lo stato della timeline, per scrivere cosa sta succedendo.
    pub fn timeline_status(&self) -> TimelineStatus {
        let t = &self.timeline;
        if !t.on {
            return TimelineStatus::Off;
        }
        if t.unavailable {
            return TimelineStatus::Unavailable;
        }
        if let Some(job) = t.job.as_ref() {
            return TimelineStatus::Building(job.progress.load(Ordering::Relaxed));
        }
        match (&t.data, &t.error) {
            (Some(_), _) => TimelineStatus::Ready,
            (None, Some(BuildError::TooManyBuckets(n))) => TimelineStatus::TooMany(*n),
            (None, Some(_)) => TimelineStatus::NoTimes,
            (None, None) => TimelineStatus::Building(0),
        }
    }

    /// Le righe della barra per un'altezza che ne contiene al piu' `max_righe`: i bucket si raggruppano (gli stessi periodi vicini
    /// in una riga) se sono di piu'. Vuoto se la timeline non e' pronta.
    pub fn timeline_rows(&self, max_righe: usize) -> Vec<TimelineRow> {
        let Some(t) = self.timeline.data.as_ref() else {
            return Vec::new();
        };
        let n = t.ticks.len();
        let g = gruppo(n, max_righe);
        let passo = t.step.ms();
        let lunghezza = t.hi.saturating_sub(t.lo).max(1) as f32;
        let mut righe: Vec<TimelineRow> = Vec::new();
        let mut i = 0;
        while i < n {
            let fine = (i + g).min(n);
            let bytes: u64 = (i..fine).map(|k| t.bytes(k)).sum();
            let span_ms = passo * (fine - i) as i64;
            let offset = t.ticks[i].offset;
            let (y0, y1) = match self.timeline.mode {
                TimelineMode::Time => (i as f32 / n as f32, fine as f32 / n as f32),
                TimelineMode::Bytes => (
                    offset.saturating_sub(t.lo) as f32 / lunghezza,
                    (offset + bytes).saturating_sub(t.lo) as f32 / lunghezza,
                ),
            };
            righe.push(TimelineRow {
                start: t.ticks[i].start,
                span_ms,
                offset,
                bytes,
                y0,
                y1,
                weight: 0.0,
                label: etichetta(t.ticks[i].start, span_ms),
            });
            i = fine;
        }
        // il peso: il volume (modo tempo) o la densita' (modo byte), rispetto alla riga piu' pesante
        let valore = |r: &TimelineRow| match self.timeline.mode {
            TimelineMode::Time => r.bytes as f64,
            TimelineMode::Bytes => r.bytes as f64 / r.span_ms.max(1) as f64,
        };
        let massimo = righe.iter().map(valore).fold(0.0_f64, f64::max);
        if massimo > 0.0 {
            for r in &mut righe {
                r.weight = (valore(r) / massimo) as f32;
            }
        }
        righe
    }

    /// Dove sta la parte del file che si vede, come frazioni dell'altezza della barra (`None` se non e' pronta).
    pub fn timeline_viewport(&self, max_righe: usize) -> Option<(f32, f32)> {
        let t = self.timeline.data.as_ref()?;
        let info = self.scroll_info();
        let lunghezza = t.hi.saturating_sub(t.lo);
        if lunghezza == 0 {
            return None;
        }
        let da = t.lo + (info.start * lunghezza as f64) as u64;
        let a = t.lo + ((info.start + info.size).min(1.0) * lunghezza as f64) as u64;
        let righe = self.timeline_rows(max_righe);
        Some((
            offset_a_y(&righe, da, t.lo, t.hi, self.timeline.mode),
            offset_a_y(&righe, a, t.lo, t.hi, self.timeline.mode),
        ))
    }

    /// L'offset a cui si arriva col punto `y` (frazione da 0 a 1 dell'altezza) della barra.
    pub fn timeline_offset_at(&self, y: f32, max_righe: usize) -> Option<u64> {
        let t = self.timeline.data.as_ref()?;
        let y = y.clamp(0.0, 1.0);
        match self.timeline.mode {
            TimelineMode::Bytes => {
                let lunghezza = t.hi.saturating_sub(t.lo);
                Some(t.lo + (y as f64 * lunghezza as f64) as u64)
            }
            TimelineMode::Time => {
                let righe = self.timeline_rows(max_righe);
                let r = riga_a(&righe, y)?;
                let frazione = if r.y1 > r.y0 {
                    f64::from(y - r.y0) / f64::from(r.y1 - r.y0)
                } else {
                    0.0
                };
                Some(r.offset + (frazione * r.bytes as f64) as u64)
            }
        }
    }

    /// L'orario (`aaaa-mm-gg hh:mm:ss`) al punto `y` della barra, per il suggerimento mentre si trascina.
    pub fn timeline_time_at(&self, y: f32, max_righe: usize) -> Option<String> {
        self.timeline.data.as_ref()?;
        let righe = self.timeline_rows(max_righe);
        let y = y.clamp(0.0, 1.0);
        let r = riga_a(&righe, y)?;
        let frazione = if r.y1 > r.y0 {
            f64::from(y - r.y0) / f64::from(r.y1 - r.y0)
        } else {
            0.0
        };
        let chiave = r.start + (frazione * r.span_ms as f64) as i64;
        Some(timemask::iso(chiave, true))
    }

    /// Salta al punto `y` della barra: il cursore va alla riga che comincia li' (la prima del periodo, se il punto cade in un
    /// periodo senza righe). Con un filtro per data resta dentro la finestra.
    pub fn timeline_jump(&mut self, y: f32, max_righe: usize) {
        if let Some(offset) = self.timeline_offset_at(y, max_righe) {
            let (lo, hi) = (self.lo(), self.hi());
            let offset = offset.clamp(lo, hi.saturating_sub(1).max(lo));
            // sempre all'inizio di una riga: un punto a meta' di una riga porta alla sua testa
            let inizio = self.doc.as_ref().map_or(offset, |doc| {
                mrp_core::scan::row_start(doc, offset, mrp_core::scan::DEFAULT_MAX_SCAN).0
            });
            self.goto_offset(inizio.max(lo));
        }
    }
}

/// La riga della barra che contiene l'altezza `y` (l'ultima se `y` e' proprio in fondo); le righe alte 0 non contano.
fn riga_a(righe: &[TimelineRow], y: f32) -> Option<&TimelineRow> {
    righe
        .iter()
        .filter(|r| r.y1 > r.y0)
        .find(|r| y >= r.y0 && y < r.y1)
        .or_else(|| righe.iter().rev().find(|r| r.y1 > r.y0))
}

/// L'altezza (da 0 a 1) a cui sta l'offset `pos` nella barra.
fn offset_a_y(righe: &[TimelineRow], pos: u64, lo: u64, hi: u64, modo: TimelineMode) -> f32 {
    match modo {
        TimelineMode::Bytes => pos.saturating_sub(lo) as f32 / hi.saturating_sub(lo).max(1) as f32,
        TimelineMode::Time => {
            // la riga col periodo che contiene `pos` (l'ultima che comincia prima), e dentro di lei in proporzione ai byte
            let Some(r) = righe
                .iter()
                .rev()
                .find(|r| r.offset <= pos)
                .or(righe.first())
            else {
                return 0.0;
            };
            let dentro = if r.bytes > 0 {
                (pos.saturating_sub(r.offset) as f32 / r.bytes as f32).min(1.0)
            } else {
                0.0
            };
            r.y0 + dentro * (r.y1 - r.y0)
        }
    }
}

/// La scritta accanto a una riga: la data (`aaaa-mm-gg`) se copre un giorno o piu', altrimenti giorno e ora (`mm-gg hh:mm`).
fn etichetta(start: i64, span_ms: i64) -> String {
    const GIORNO: i64 = 86_400_000;
    let (anno, mese, giorno) = timemask::civil(start);
    if span_ms >= GIORNO {
        format!("{anno:04}-{mese:02}-{giorno:02}")
    } else {
        let ms = start.rem_euclid(GIORNO);
        format!(
            "{mese:02}-{giorno:02} {:02}:{:02}",
            ms / 3_600_000,
            (ms / 60_000) % 60
        )
    }
}
