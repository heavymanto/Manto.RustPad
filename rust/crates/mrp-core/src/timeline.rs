//! La timeline della barra laterale (fase 2, F2): i confini dei bucket di tempo di un log (minuto,
//! dieci minuti, ora, giorno) come offset nel file, anche per un file enorme. Ogni bucket e' un tratto
//! della barra, la sua lunghezza e' il volume in byte di quel periodo, un clic salta all'offset della
//! tacca. Il file non si legge mai per intero: `build` trova la prima e l'ultima riga con orario della
//! finestra (`first_key_in` / `last_key_in` di `timesearch`), conta i bucket e cerca ogni tacca
//! partendo dall'offset della precedente, con salti in avanti che raddoppiano (galloping) e poi la
//! ricerca binaria di `find_key_in` sul solo intervallo cosi' trovato: mai una ricerca su tutto il file
//! per ogni tacca, mai un ciclo sul numero di righe del file.
//! `count_levels` (F3) ci mette sopra i colori dei log: una passata annullabile e con pausa sulla
//! finestra, letta a pezzi, conta quante righe di ogni livello cominciano in ogni bucket.

use crate::document::Document;
use crate::lines::Cancelled;
use crate::loghl::{first_level, Kind};
use crate::timemask::TimeMask;
use crate::timesearch::{find_key_in, first_key_in, key_of_line, last_key_in};
use memchr::memchr;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::time::Duration;

/// Il salto minimo del galloping: sotto i 4 KiB non vale la pena di raffinare il salto, si cerca
/// direttamente nell'intorno del bucket.
const SALTO_MIN: u64 = 4096;

/// Il pezzo di lettura di `count_levels`: la finestra della timeline si legge a pezzi, mai tutta
/// insieme.
const PEZZO_LETTURA: u64 = 1024 * 1024;

/// Oltre questa lunghezza una riga non si guarda piu' per il livello: conta una volta sola, nel
/// bucket del suo inizio, ma il livello si cerca solo nei primi 64 KiB.
const MAX_RIGA: usize = 64 * 1024;

/// La larghezza dei bucket della timeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// Un minuto.
    Minute,
    /// Dieci minuti.
    TenMinutes,
    /// Un'ora.
    Hour,
    /// Un giorno.
    Day,
}

impl Step {
    /// Il nome nel file delle impostazioni.
    pub fn code(self) -> &'static str {
        match self {
            Step::Minute => "minute",
            Step::TenMinutes => "10min",
            Step::Hour => "hour",
            Step::Day => "day",
        }
    }

    /// Il passo dal suo nome (`code`); `None` se non lo e'.
    pub fn parse(testo: &str) -> Option<Step> {
        [Step::Minute, Step::TenMinutes, Step::Hour, Step::Day]
            .into_iter()
            .find(|s| s.code().eq_ignore_ascii_case(testo))
    }

    /// La larghezza del bucket in millisecondi.
    pub fn ms(self) -> i64 {
        match self {
            Step::Minute => 60_000,
            Step::TenMinutes => 600_000,
            Step::Hour => 3_600_000,
            Step::Day => 86_400_000,
        }
    }

    /// L'inizio del bucket che contiene `key`: la chiave tagliata al multiplo del passo, verso il
    /// basso (anche sotto zero, come la mezzanotte per il giorno).
    pub fn floor(self, key: i64) -> i64 {
        let ms = self.ms();
        key - key.rem_euclid(ms)
    }
}

/// Quanti bucket almeno deve avere la barra perche' sia utile: sotto, si sceglie un passo piu' corto.
pub const BUCKET_MIN: i64 = 12;

/// Il passo che da' una barra leggibile per un log che va da `first` a `last` (chiavi in millisecondi): il piu' lungo fra giorno, ora
/// e dieci minuti che fa almeno `BUCKET_MIN` bucket, altrimenti il minuto. Un log di due settimane va a giorni, uno di un giorno a
/// ore, uno di sette minuti a minuti.
pub fn auto_step(first: i64, last: i64) -> Step {
    [Step::Day, Step::Hour, Step::TenMinutes]
        .into_iter()
        .find(|s| (s.floor(last) - s.floor(first)) / s.ms() + 1 >= BUCKET_MIN)
        .unwrap_or(Step::Minute)
}

/// Una tacca della timeline: l'inizio del bucket (`start`, la chiave) e l'inizio della prima riga con
/// chiave >= `start` (`offset`); le righe senza orario prima di quella riga appartengono al bucket.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tick {
    /// L'inizio del bucket, in millisecondi dal 1970.
    pub start: i64,
    /// L'inizio della prima riga del bucket, in byte nel documento.
    pub offset: u64,
}

/// La timeline di una finestra del documento: `first` e `last` sono le chiavi della prima e dell'ultima
/// riga con orario della finestra, i bucket sono pezzi contigui di `[lo, hi)` (la somma dei `bytes` e'
/// `hi - lo`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Timeline {
    /// Il passo dei bucket.
    pub step: Step,
    /// Le tacche, una per bucket: `start` cresce di `step.ms()` alla volta, `offset` non decresce.
    pub ticks: Vec<Tick>,
    /// L'inizio della finestra coperta, in byte.
    pub lo: u64,
    /// La fine della finestra coperta, in byte.
    pub hi: u64,
    /// La chiave della prima riga con orario della finestra.
    pub first: i64,
    /// La chiave dell'ultima riga con orario della finestra.
    pub last: i64,
}

impl Timeline {
    /// Quanti byte copre il bucket `i`: dalla sua tacca alla successiva; l'ultimo arriva fino a `hi`.
    /// Un bucket senza righe (la stessa tacca del successivo) copre zero byte, come un `i` fuori
    /// range.
    pub fn bytes(&self, i: usize) -> u64 {
        match (self.ticks.get(i), self.ticks.get(i + 1)) {
            (Some(a), Some(b)) => b.offset - a.offset,
            (Some(a), None) => self.hi - a.offset,
            (None, _) => 0,
        }
    }

    /// Il bucket non vuoto che contiene `offset`: il bucket `i` copre `[ticks[i].offset,
    /// ticks[i].offset + bytes(i))`. `None` fuori da `[lo, hi)`; un bucket vuoto non contiene nessun
    /// offset (i bucket vuoti prima di uno non vuoto condividono la sua tacca, e quel punto appartiene
    /// a lui).
    pub fn bucket_at(&self, offset: u64) -> Option<usize> {
        if offset >= self.hi {
            return None;
        }
        // l'ultima tacca con offset <= `offset`: i bucket vuoti condividono l'offset con il successivo,
        // quindi quella trovata e' sempre di un bucket non vuoto
        self.ticks
            .partition_point(|t| t.offset <= offset)
            .checked_sub(1)
    }
}

/// Perche' `build` fallisce.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildError {
    /// Nessuna riga con orario nella finestra: non c'e' timeline da costruire.
    NoTimes,
    /// Servono piu' bucket del tetto dato: quanti ne servirebbero.
    TooManyBuckets(usize),
    /// La costruzione e' stata interrotta dal chiamante.
    Cancelled,
}

/// L'offset della prima riga con chiave >= `start` in `[da, hi)`: lo stesso risultato di
/// `find_key_in(doc, mask, start, da, hi, cancel)`, senza pero' rifare la ricerca binaria su tutto
/// l'intervallo. Si parte da `da` e si va in avanti a salti (galloping): prove a `da + salto`, col
/// salto che raddoppia a ogni prova, finche' la riga raggiunta ha gia' una chiave >= `start` (allora
/// la risposta sta su quella riga o prima) o si arriva a `hi`; poi la ricerca binaria nel solo
/// intervallo cosi' trovato. `salto` e' il salto iniziale (almeno `SALTO_MIN`: la dimensione del
/// bucket precedente, cosi' su un log regolare la prima prova e' quasi sempre quella buona).
fn tacca(
    doc: &Document,
    mask: &TimeMask,
    start: i64,
    da: u64,
    hi: u64,
    cancel: &AtomicBool,
    salto: u64,
) -> Result<Option<u64>, Cancelled> {
    let mut salto = salto.max(SALTO_MIN);
    let fine;
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err(Cancelled);
        }
        let prova = da.saturating_add(salto);
        if prova >= hi {
            // oltre la finestra: vale l'intervallo intero, come `find_key_in` su `[da, hi)`
            fine = hi;
            break;
        }
        // la chiave della riga che contiene il punto di prova, letta dall'inizio della riga (come in
        // `find_key_in`): un orario scritto in mezzo a una riga non conta
        if key_of_line(doc, mask, prova).is_some_and(|k| k >= start) {
            // la risposta sta su questa riga o prima: la ricerca binaria si ferma qui
            fine = prova + 1;
            break;
        }
        salto = salto.saturating_mul(2);
    }
    find_key_in(doc, mask, start, da, fine, cancel)
}

/// Le tacche dei bucket di `step` nella finestra di byte `[lo, hi)` del documento, con le chiavi non
/// decrescenti (come in `timesearch`): la prima tacca sta in `lo` (le righe senza orario prima della
/// prima con orario sono del primo bucket), ogni successiva e' l'inizio della prima riga con chiave
/// non minore dell'inizio del suo bucket (un bucket senza righe ha la stessa tacca del successivo)
/// e l'ultima arriva fino a `hi`. `max_ticks` e' il tetto al numero di bucket: oltre,
/// `Err(TooManyBuckets)` con quanti ne servirebbero, prima di cercare qualsiasi tacca. `progress`,
/// se c'e', vale il numero di tacche gia' trovate (alla fine, quante sono). `cancel` viene
/// controllato subito, a ogni tacca e dentro le ricerche.
// la firma a otto parametri e' quella imposta dal PLAN: uno in piu' del limite di clippy
#[allow(clippy::too_many_arguments)]
pub fn build(
    doc: &Document,
    mask: &TimeMask,
    step: Step,
    lo: u64,
    hi: u64,
    cancel: &AtomicBool,
    max_ticks: usize,
    progress: Option<&AtomicUsize>,
) -> Result<Timeline, BuildError> {
    if cancel.load(Ordering::Relaxed) {
        return Err(BuildError::Cancelled);
    }
    // la finestra non esce dal documento
    let hi = hi.min(doc.len());
    // la prima e l'ultima chiave della finestra: senza orari non c'e' timeline
    let (_, first) = first_key_in(doc, mask, lo, hi, cancel)
        .map_err(|_| BuildError::Cancelled)?
        .ok_or(BuildError::NoTimes)?;
    let (_, last) = last_key_in(doc, mask, lo, hi, cancel)
        .map_err(|_| BuildError::Cancelled)?
        .ok_or(BuildError::NoTimes)?;
    // quanti bucket servono, prima di cercare qualsiasi tacca
    let ms = step.ms();
    let inizio = step.floor(first);
    let n = ((step.floor(last) - inizio) / ms + 1) as usize;
    if n > max_ticks {
        return Err(BuildError::TooManyBuckets(n));
    }
    let mut ticks = Vec::with_capacity(n);
    // le righe senza orario prima della prima con orario sono del primo bucket
    ticks.push(Tick {
        start: inizio,
        offset: lo,
    });
    if let Some(p) = progress {
        p.store(1, Ordering::Relaxed);
    }
    for i in 1..n {
        if cancel.load(Ordering::Relaxed) {
            return Err(BuildError::Cancelled);
        }
        let start = inizio + i as i64 * ms;
        let prev = ticks[i - 1].offset;
        // il salto iniziale del galloping: la dimensione dell'ultimo bucket gia' finito (per la
        // prima tacca cercata e' zero, e vale `SALTO_MIN`)
        let base = if i >= 2 { ticks[i - 2].offset } else { lo };
        let offset = tacca(doc, mask, start, prev, hi, cancel, prev - base)
            .map_err(|_| BuildError::Cancelled)?
            .unwrap_or(hi);
        ticks.push(Tick { start, offset });
        if let Some(p) = progress {
            p.store(ticks.len(), Ordering::Relaxed);
        }
    }
    Ok(Timeline {
        step,
        ticks,
        lo,
        hi,
        first,
        last,
    })
}

/// Quante righe di ogni livello cominciano in un bucket della timeline: i colori dei log sulla barra.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LevelCounts {
    /// Le righe il cui primo livello e' un errore.
    pub error: u32,
    /// Le righe il cui primo livello e' un avviso.
    pub warn: u32,
    /// Le righe il cui primo livello e' una notizia.
    pub info: u32,
    /// Le righe il cui primo livello e' il dettaglio.
    pub debug: u32,
}

/// result[i] = quante righe, con inizio in [ticks[i].offset, ticks[i].offset + t.bytes(i)), hanno
/// quel livello (il primo livello della riga: `first_level`).
///
/// La finestra [t.lo, t.hi) si legge UNA volta, a pezzi di `PEZZO_LETTURA` byte con
/// `Document::read_range`, tenendo il resto di una riga spezzata fra un pezzo e l'altro: mai il file
/// intero in memoria. Ogni riga completa (fino al `\n`, tolto un `\r` finale) si assegna al bucket
/// che contiene il suo INIZIO (i bucket vuoti restano a zero) e si conta col suo livello; anche
/// l'ultima riga, senza `\n`, conta. Una riga non UTF-8 non ha livello; una piu' lunga di `MAX_RIGA`
/// si guarda solo nei primi `MAX_RIGA` byte, ma conta una volta sola e nel bucket giusto. I bucket
/// si percorrono in avanti con un indice che non torna indietro: niente ricerca del bucket da capo
/// per ogni riga. `progress`, se c'e', vale i byte della finestra gia' esaminati (alla fine
/// `t.hi - t.lo`). `cancel` si controlla subito e a ogni pezzo; con `pausa` accesa la passata aspetta
/// dormendo 20 ms finche' non si spegne, ma se arriva `cancel` si esce subito con `Err(Cancelled)`,
/// anche in pausa.
pub fn count_levels(
    doc: &Document,
    t: &Timeline,
    cancel: &AtomicBool,
    pausa: Option<&AtomicBool>,
    progress: Option<&AtomicU64>,
) -> Result<Vec<LevelCounts>, Cancelled> {
    // annullato in partenza, come `build`
    if cancel.load(Ordering::Relaxed) {
        return Err(Cancelled);
    }
    let mut conteggi = vec![LevelCounts::default(); t.ticks.len()];
    // la riga in corso: dove comincia nel file e i suoi primi byte (una riga piu' lunga di
    // `MAX_RIGA` si guarda solo nei primi `MAX_RIGA`)
    let mut inizio = t.lo;
    let mut riga: Vec<u8> = Vec::new();
    // il bucket corrente: le righe escono in ordine di inizio, l'indice avanza e non torna indietro
    let mut bucket = 0usize;
    let mut pos = t.lo;
    while pos < t.hi {
        // il cancel si controlla a ogni pezzo
        if cancel.load(Ordering::Relaxed) {
            return Err(Cancelled);
        }
        // in pausa si aspetta dormendo, ma il cancel vince anche in pausa
        if let Some(pausa) = pausa {
            while pausa.load(Ordering::Relaxed) {
                if cancel.load(Ordering::Relaxed) {
                    return Err(Cancelled);
                }
                std::thread::sleep(Duration::from_millis(20));
            }
        }
        // il pezzo: [pos, fine), al piu' `PEZZO_LETTURA` byte
        let fine = pos.saturating_add(PEZZO_LETTURA).min(t.hi);
        let pezzo = doc.read_range(pos, fine - pos);
        let mut j = 0usize;
        while j < pezzo.len() {
            // il prossimo a-capo: la riga chiusa finisce subito prima
            let a_capo = memchr(b'\n', &pezzo[j..]).map_or(pezzo.len(), |k| j + k);
            accumula(&mut riga, &pezzo[j..a_capo]);
            if a_capo < pezzo.len() {
                chiudi_riga(t, &mut conteggi, &mut bucket, inizio, &riga);
                riga.clear();
                inizio = pos + a_capo as u64 + 1;
            }
            // oltre l'a-capo comincia la riga dopo; senza a-capo il resto resta nella riga in corso
            j = a_capo + 1;
        }
        pos = fine;
        if let Some(p) = progress {
            p.store(pos - t.lo, Ordering::Relaxed);
        }
    }
    // l'ultima riga, senza a-capo, conta lo stesso (vuota se la finestra finisce con l'a-capo)
    chiudi_riga(t, &mut conteggi, &mut bucket, inizio, &riga);
    if let Some(p) = progress {
        p.store(t.hi - t.lo, Ordering::Relaxed);
    }
    Ok(conteggi)
}

/// Chiude la riga cominciata in `inizio`, con i byte gia' accumulati in `riga`: toglie un `\r`
/// finale, cerca il livello (se la riga e' UTF-8) e lo conta nel bucket che contiene `inizio`.
/// `bucket` e' il bucket corrente della passata: le righe escono in ordine di inizio, quindi si
/// avanza in avanti e non si torna mai indietro.
fn chiudi_riga(
    t: &Timeline,
    conteggi: &mut [LevelCounts],
    bucket: &mut usize,
    inizio: u64,
    riga: &[u8],
) {
    // senza bucket non c'e' dove contare (una timeline di `build` ne ha sempre almeno uno)
    if conteggi.is_empty() {
        return;
    }
    // il bucket che contiene l'inizio della riga: si avanza finche' quello corrente e' finito prima
    // (un bucket vuoto copre zero byte: si supera appena l'inizio gli arriva addosso)
    while *bucket + 1 < conteggi.len() && inizio >= t.ticks[*bucket].offset + t.bytes(*bucket) {
        *bucket += 1;
    }
    // il `\r` finale non e' parte della riga
    let fine = riga.len() - usize::from(riga.last() == Some(&b'\r'));
    // una riga non UTF-8 non ha livello
    let Some(tipo) = std::str::from_utf8(&riga[..fine])
        .ok()
        .and_then(first_level)
    else {
        return;
    };
    let c = &mut conteggi[*bucket];
    match tipo {
        Kind::Error => c.error += 1,
        Kind::Warn => c.warn += 1,
        Kind::Info => c.info += 1,
        Kind::Debug => c.debug += 1,
        // Time e Ip non sono livelli: `first_level` non li restituisce mai
        _ => {}
    }
}

/// Allunga la riga in corso con `pezzo`, ma solo finche' resta sotto `MAX_RIGA`: oltre, il livello
/// non si guarda piu' (la riga conta comunque una volta sola, nel bucket del suo inizio).
fn accumula(riga: &mut Vec<u8>, pezzo: &[u8]) {
    let posto = MAX_RIGA.saturating_sub(riga.len()).min(pezzo.len());
    riga.extend_from_slice(&pezzo[..posto]);
}
