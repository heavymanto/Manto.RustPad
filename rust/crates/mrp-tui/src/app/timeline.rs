//! La timeline di una scheda (F2): una barra "mappa" accanto alla scrollbar, con una riga per ogni bucket di tempo (minuto, dieci
//! minuti, ora, giorno) lunga quanto il volume di righe di quel periodo. Le tacche (gli offset dei confini) le trova in un thread
//! `mrp_core::timeline::build` su una copia della sorgente del file, senza leggerlo per intero; qui ci sono lo stato (spenta di
//! fabbrica, per scheda), il lavoro in background, e le funzioni che servono a disegnarla e a usarla: le righe (raggruppate se i
//! bucket sono piu' di quelli che stanno nell'altezza), dov'e' la parte che si vede, il salto a un punto e l'orario sotto il mouse.
//! Ogni file e' un mondo a se': niente fusione di piu' file.

use super::{App, FilterConditions};
use mrp_core::pattern::{LineFilter, Mode};
use mrp_core::timeline::{self, BuildError, LevelCounts, Step, Timeline};
use mrp_core::timemask;
use mrp_core::timesearch;
use mrp_core::Document;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
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
    /// Quante righe di ogni livello ci sono nel periodo (tutto zero finche' la passata dei livelli non e' arrivata).
    pub levels: LevelCounts,
    /// I livelli sono noti (la passata e' finita): serve a distinguere "nessun errore" da "non ancora contati".
    pub levels_known: bool,
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

/// La passata che conta i livelli bucket per bucket.
struct LevelsJob {
    rx: Receiver<Result<Vec<LevelCounts>, mrp_core::lines::Cancelled>>,
    cancel: Arc<AtomicBool>,
    /// Vero: la passata non legge dal disco (si sta ancora indicizzando il file o l'utente sta saltando).
    pausa: Arc<AtomicBool>,
    /// Byte della finestra gia' esaminati e quanti sono in tutto (per la percentuale).
    progress: Arc<AtomicU64>,
    total: u64,
}

/// Cosa si ricorda della timeline fra una sessione e l'altra (le impostazioni dell'utente): com'era quando si e' chiusa, per
/// riaprirla uguale. Non e' lo stato di una scheda: e' il punto di partenza di quelle che si aprono.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TimelinePref {
    /// Accesa.
    pub on: bool,
    /// Il passo fisso scelto; `None`: lo sceglie da solo (di fabbrica).
    pub step: Option<Step>,
    /// Le righe seguono i byte invece del tempo.
    pub bytes: bool,
    /// Si vedono anche i periodi senza righe.
    pub empty: bool,
}

/// I livelli che la timeline sa filtrare, nell'ordine in cui si mostrano.
pub const TIMELINE_LEVELS: [&str; 4] = ["ERROR", "WARN", "INFO", "DEBUG"];

/// Tutto cio' che una scheda sa della sua timeline.
pub(super) struct TimelineState {
    pub on: bool,
    /// Il passo fisso scelto dall'utente; con `auto` vale quello che dice `timeline::auto_step` per il file.
    pub step: Step,
    pub auto: bool,
    pub mode: TimelineMode,
    /// Si disegnano anche i periodi senza righe (di fabbrica no: la barra e' fatta solo di quello che c'e').
    pub show_empty: bool,
    data: Option<Arc<Timeline>>,
    job: Option<TimelineJob>,
    /// I livelli per bucket (stessa lunghezza delle tacche) e la passata che li sta contando.
    levels: Option<Arc<Vec<LevelCounts>>>,
    levels_job: Option<LevelsJob>,
    /// Quali livelli sono scelti per il filtro (nell'ordine di `TIMELINE_LEVELS`).
    filter_levels: [bool; 4],
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
            show_empty: false,
            data: None,
            job: None,
            levels: None,
            levels_job: None,
            filter_levels: [false; 4],
            error: None,
            unavailable: false,
            built_len: 0,
            built_window: None,
            built_step: None,
        }
    }
}

impl TimelineState {
    /// Lo stato di una scheda che si apre: come lo lascia la preferenza.
    pub(super) fn from_pref(p: TimelinePref) -> Self {
        TimelineState {
            on: p.on,
            step: p.step.unwrap_or(Step::Day),
            auto: p.step.is_none(),
            mode: if p.bytes {
                TimelineMode::Bytes
            } else {
                TimelineMode::Time
            },
            show_empty: p.empty,
            ..TimelineState::default()
        }
    }

    /// Lo stato com'e' adesso, come preferenza.
    fn pref(&self) -> TimelinePref {
        TimelinePref {
            on: self.on,
            step: (!self.auto).then_some(self.step),
            bytes: self.mode == TimelineMode::Bytes,
            empty: self.show_empty,
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
        self.remember_timeline();
        if !acceso {
            self.timeline_stop();
            self.levels_stop();
            self.timeline.levels = None;
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
        self.remember_timeline();
    }

    /// Torna al passo scelto da solo: le tacche si ricalcolano.
    pub fn set_timeline_auto(&mut self) {
        self.timeline.auto = true;
        self.remember_timeline();
    }

    /// Come si dispongono le righe (di fabbrica: l'altezza e' il tempo).
    pub fn timeline_mode(&self) -> TimelineMode {
        self.timeline.mode
    }

    /// Cambia la disposizione delle righe.
    pub fn set_timeline_mode(&mut self, modo: TimelineMode) {
        self.timeline.mode = modo;
        self.remember_timeline();
    }

    /// Si disegnano anche i periodi senza righe.
    pub fn timeline_show_empty(&self) -> bool {
        self.timeline.show_empty
    }

    /// Mostra o nasconde i periodi senza righe.
    pub fn set_timeline_show_empty(&mut self, si: bool) {
        self.timeline.show_empty = si;
        self.remember_timeline();
    }

    /// Cosa si ricorda della timeline (vedi `TimelinePref`): lo prende il `Workspace` per darlo alle altre schede.
    pub fn timeline_pref(&self) -> TimelinePref {
        self.timeline_pref
    }

    /// Ricorda la preferenza senza scrivere le impostazioni (chi l'ha scelta le ha gia' scritte).
    pub fn set_timeline_pref(&mut self, p: TimelinePref) {
        self.timeline_pref = p;
    }

    /// La scelta dell'utente sulla timeline di questa scheda diventa la preferenza e si scrive nelle impostazioni.
    fn remember_timeline(&mut self) {
        let p = self.timeline.pref();
        if p != self.timeline_pref {
            self.timeline_pref = p;
            self.save_settings();
        }
    }

    /// Ferma il lavoro in corso (il thread si accorge dell'annullamento e muore da solo).
    fn timeline_stop(&mut self) {
        if let Some(job) = self.timeline.job.take() {
            job.cancel.store(true, Ordering::Relaxed);
        }
    }

    /// Una scheda che continua un'altra (la vista filtrata): prende il formato dell'orario, la timeline (se e' accesa, col passo e la
    /// disposizione) e i livelli scelti; le tacche e i conteggi si ricalcolano sulle sue righe.
    pub fn inherit_time_view(&mut self, da: &App) {
        self.time_mask = da.time_mask.clone();
        self.time_mask_tried = da.time_mask_tried;
        self.timeline.on = da.timeline.on;
        self.timeline.step = da.timeline.step;
        self.timeline.auto = da.timeline.auto;
        self.timeline.mode = da.timeline.mode;
        self.timeline.filter_levels = da.timeline.filter_levels;
    }

    /// Ferma la passata dei livelli (il thread si accorge dell'annullamento e muore da solo).
    fn levels_stop(&mut self) {
        if let Some(job) = self.timeline.levels_job.take() {
            job.cancel.store(true, Ordering::Relaxed);
        }
    }

    /// La passata che conta i livelli per bucket: raccoglie il risultato e, appena ci sono le tacche, la fa partire (su una copia del
    /// file, con pausa finche' si sta ancora indicizzando). Ritorna vero se sta lavorando.
    fn levels_tick(&mut self) -> bool {
        if let Some(job) = self.timeline.levels_job.as_ref() {
            job.pausa.store(self.index_job.is_some(), Ordering::Relaxed);
            return match job.rx.try_recv() {
                Ok(esito) => {
                    self.timeline.levels_job = None;
                    if let Ok(v) = esito {
                        self.timeline.levels = Some(Arc::new(v));
                    }
                    false
                }
                Err(TryRecvError::Empty) => true,
                Err(TryRecvError::Disconnected) => {
                    self.timeline.levels_job = None;
                    false
                }
            };
        }
        if self.timeline.levels.is_some() {
            return false;
        }
        let Some(t) = self.timeline.data.clone() else {
            return false;
        };
        let path = self.path.clone();
        let (tx, rx) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let pausa = Arc::new(AtomicBool::new(self.index_job.is_some()));
        let progress = Arc::new(AtomicU64::new(0));
        let (cancel_t, pausa_t, progress_t) = (
            Arc::clone(&cancel),
            Arc::clone(&pausa),
            Arc::clone(&progress),
        );
        let totale = t.hi.saturating_sub(t.lo);
        std::thread::spawn(move || {
            // la sua copia del file, come per le tacche
            let esito = match Document::open(&path) {
                Ok(doc) => {
                    timeline::count_levels(&doc, &t, &cancel_t, Some(&pausa_t), Some(&progress_t))
                }
                Err(_) => Err(mrp_core::lines::Cancelled),
            };
            let _ = tx.send(esito);
        });
        self.timeline.levels_job = Some(LevelsJob {
            rx,
            cancel,
            pausa,
            progress,
            total: totale,
        });
        true
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
                            // tacche nuove: i livelli di prima non valgono piu'
                            self.levels_stop();
                            self.timeline.levels = None;
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
            return self.levels_tick();
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

    /// Quali livelli sono scelti per il filtro (nell'ordine di `TIMELINE_LEVELS`: ERROR, WARN, INFO, DEBUG).
    pub fn timeline_filter_levels(&self) -> [bool; 4] {
        self.timeline.filter_levels
    }

    /// Sceglie o toglie il livello `i` (0 ERROR, 1 WARN, 2 INFO, 3 DEBUG) dal filtro per livelli.
    pub fn set_timeline_filter_level(&mut self, i: usize, scelto: bool) {
        if let Some(l) = self.timeline.filter_levels.get_mut(i) {
            *l = scelto;
        }
    }

    /// Toglie tutti i livelli dal filtro.
    pub fn clear_timeline_filter_levels(&mut self) {
        self.timeline.filter_levels = [false; 4];
    }

    /// Quanto e' avanzata la passata che conta i livelli, da 0 a 1 (`None`: non sta lavorando).
    pub fn timeline_levels_progress(&self) -> Option<f32> {
        let job = self.timeline.levels_job.as_ref()?;
        let fatti = job.progress.load(Ordering::Relaxed);
        Some((fatti as f32 / job.total.max(1) as f32).clamp(0.0, 1.0))
    }

    /// I livelli per bucket sono pronti.
    pub fn timeline_levels_ready(&self) -> bool {
        self.timeline.levels.is_some()
    }

    /// Apre una vista con le sole righe dei livelli scelti (la scheda filtrata di Alt+G): un'espressione regolare con le parole dei
    /// livelli (`ERROR`, `ERR`, `FATAL`... `WARN`, `WRN`... `INFO`, `INF`... `DEBUG`, `DBG`...), maiuscole esatte, parola intera.
    /// Senza nessun livello scelto lo dice e non fa niente.
    pub fn timeline_apply_level_filter(&mut self) {
        let livelli = self.timeline.filter_levels;
        if !livelli.iter().any(|&x| x) {
            self.flash = Some(self.cat.t("timeline.pick_level").to_string());
            return;
        }
        if !self.enc.is_utf8() {
            self.flash = Some(self.cat.t("flash.regex_utf8").to_string());
            return;
        }
        // la scheda si chiama coi livelli scelti ("levels: ERROR, WARN")
        let nomi: Vec<&str> = (0..4)
            .filter(|&i| livelli[i])
            .map(|i| TIMELINE_LEVELS[i])
            .collect();
        let nome = self.cat.tf("timeline.filter_name", &[&nomi.join(", ")]);
        let filtro = LineFilter {
            pattern: None,
            invert: false,
            levels: livelli,
        };
        let con_date = self.window.is_some();
        let condizioni = FilterConditions {
            levels: livelli,
            text: String::new(),
            mode: Mode::Text,
            case: false,
            invert: false,
            time: con_date,
            from: if con_date {
                self.time_range_text(0).to_string()
            } else {
                String::new()
            },
            to: if con_date {
                self.time_range_text(1).to_string()
            } else {
                String::new()
            },
        };
        // in una vista filtrata i livelli si cambiano alle condizioni che l'hanno fatta, e li rifa' il file originale
        if let Some(v) = &self.filter_view {
            if let Some(mut c) = v.conditions.clone() {
                c.levels = livelli;
                self.refilter = Some(c);
                return;
            }
        }
        // dentro la finestra di tempo, se c'e'
        self.filter_start(filtro, Some((self.lo(), self.hi())), &nome, condizioni);
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
        // i bucket che si disegnano: tutti, oppure (di fabbrica) solo quelli con delle righe, cosi' un periodo vuoto non fa una
        // distesa di righe uguali
        let usati: Vec<usize> = (0..t.ticks.len())
            .filter(|&k| self.timeline.show_empty || t.bytes(k) > 0)
            .collect();
        let n = usati.len();
        let g = gruppo(n, max_righe);
        let passo = t.step.ms();
        let lunghezza = t.hi.saturating_sub(t.lo).max(1) as f32;
        let mut righe: Vec<TimelineRow> = Vec::new();
        let mut i = 0;
        while i < n {
            let fine = (i + g).min(n);
            let gruppo_k = &usati[i..fine];
            let bytes: u64 = gruppo_k.iter().map(|&k| t.bytes(k)).sum();
            // dal primo bucket al fondo dell'ultimo (con i vuoti nascosti in mezzo non sono tanti quanti i bucket del gruppo)
            let (primo, ultimo) = (gruppo_k[0], gruppo_k[gruppo_k.len() - 1]);
            let span_ms = t.ticks[ultimo].start + passo - t.ticks[primo].start;
            let offset = t.ticks[primo].offset;
            let (y0, y1) = match self.timeline.mode {
                TimelineMode::Time => (i as f32 / n as f32, fine as f32 / n as f32),
                TimelineMode::Bytes => (
                    offset.saturating_sub(t.lo) as f32 / lunghezza,
                    (offset + bytes).saturating_sub(t.lo) as f32 / lunghezza,
                ),
            };
            righe.push(TimelineRow {
                start: t.ticks[primo].start,
                span_ms,
                offset,
                bytes,
                y0,
                y1,
                weight: 0.0,
                label: etichetta(t.ticks[primo].start, span_ms),
                levels: self
                    .timeline
                    .levels
                    .as_ref()
                    .map_or_else(LevelCounts::default, |lv| {
                        gruppo_k
                            .iter()
                            .fold(LevelCounts::default(), |a, &k| LevelCounts {
                                error: a.error + lv.get(k).map_or(0, |c| c.error),
                                warn: a.warn + lv.get(k).map_or(0, |c| c.warn),
                                info: a.info + lv.get(k).map_or(0, |c| c.info),
                                debug: a.debug + lv.get(k).map_or(0, |c| c.debug),
                            })
                    }),
                levels_known: self.timeline.levels.is_some(),
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
