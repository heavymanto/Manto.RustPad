//! Salta a un orario (fase 2, 3.4): `parse_target` legge l'orario che l'utente scrive e `find_time` cerca, con una
//! ricerca binaria sui byte di un `Document`, l'inizio della prima riga il cui orario e' maggiore o uguale a
//! quello cercato, in un log con gli orari in ordine non decrescente. Niente indice e nessuna lettura del file
//! per intero: le righe senza orario (un'intestazione, le righe di una traccia) si saltano e il testo di una
//! riga si legge per al piu' `TESTO_MAX` byte, perche' l'orario sta all'inizio.
//!
//! La ricerca divide l'intervallo di byte `[lo, hi)` a meta': dal punto in mezzo si va all'inizio della riga che
//! lo contiene e si guarda la prima riga con orario da lì in avanti (al piu' `RIGHE_MAX`); secondo il confronto
//! con il target l'intervallo si restringe da un lato o dall'altro, e la risposta e' l'ultima candidata trovata.

use crate::document::Document;
use crate::lines::Cancelled;
use crate::loghl::{self, Kind};
use crate::scan::{self, Next};
use std::sync::atomic::{AtomicBool, Ordering};

/// Quanti byte al massimo del testo di una riga si leggono per trovarne l'orario.
const TESTO_MAX: u64 = 512;
/// Quante righe al massimo si guardano, da un punto in avanti, per trovare la prima con orario: cosi' ogni giro
/// della ricerca legge al piu' `RIGHE_MAX` teste di riga da `TESTO_MAX` byte.
const RIGHE_MAX: u32 = 4096;
/// Il passo della scansione in avanti: le righe piu' lunghe si attraversano a pezzi di questo passo (i `Cut` di
/// `scan`), come le righe visuali del resto del progetto.
const PASSO: u64 = 512;
/// Il limite di scansione per trovare gli inizi di riga veri (come altrove nel progetto: 1 MiB): una riga piu'
/// lunga di cosi' resta a pezzi.
const MAX_SCAN: u64 = scan::DEFAULT_MAX_SCAN;

/// Una data: anno, mese e giorno.
type Data = (u16, u8, u8);

/// L'orario cercato, come lo puo' scrivere l'utente: una data (`2026-10-31`), un orario del giorno (`09:00`,
/// `09:00:53`, anche con la frazione) o entrambi, con uno spazio o una `T` in mezzo. Per una data sola l'orario
/// del giorno e' mezzanotte.
#[derive(Debug, Clone, PartialEq)]
pub struct Target {
    /// La data, se il target l'ha.
    data: Option<Data>,
    /// L'orario del giorno in millisecondi.
    ms: u64,
}

/// L'orario scritto dall'utente, o `None` se non e' nessuno dei formati accettati (vedi `Target`). Gli spazi ai
/// lati si tolgono; il fuso non si accetta, perche' e' quello che si scrive a mano, non il testo di un log.
pub fn parse_target(s: &str) -> Option<Target> {
    let b = s.trim().as_bytes();
    if let Some(data) = data(b) {
        // dopo la data: niente, oppure UN connettore e un orario che occupa tutto il resto
        if b.len() == 10 {
            return Some(Target {
                data: Some(data),
                ms: 0,
            });
        }
        if b.get(10) == Some(&b'T') || b.get(10) == Some(&b' ') {
            if let Some(ms) = orario_stretto(&b[11..]) {
                return Some(Target {
                    data: Some(data),
                    ms,
                });
            }
        }
        return None;
    }
    let ms = orario_stretto(b)?;
    Some(Target { data: None, ms })
}

/// L'inizio della prima riga, in ordine di file, il cui orario e' maggiore o uguale a quello del `target`
/// (documento con gli orari non decrescenti): `Ok(None)` se non c'e' (documento vuoto, nessuna riga con orario
/// o tutte precedenti). Il `target` senza data prende la data della prima riga con orario del documento; se non
/// ce n'e' nessuna non c'e' niente da trovare. Ogni giro controlla `cancel` (vero -> `Err(Cancelled)`).
pub fn find_time(
    doc: &Document,
    target: &Target,
    cancel: &AtomicBool,
) -> Result<Option<u64>, Cancelled> {
    let len = doc.len();
    if len == 0 {
        return Ok(None);
    }
    if cancel.load(Ordering::Relaxed) {
        return Err(Cancelled);
    }
    // il target senza data prende la data della prima riga con orario, cercata una volta sola dall'inizio
    let data = match target.data {
        Some(d) => Some(d),
        None => match prima_con_orario_del_file(doc, cancel)? {
            Some(chiave) => chiave.data,
            None => return Ok(None),
        },
    };
    let t = Chiave {
        data,
        ms: Some(target.ms),
    };
    // la ricerca binaria: in `[lo, hi)` c'e' sempre la prima riga che va bene (o la candidata trovata)
    let mut lo: u64 = 0;
    let mut hi: u64 = len;
    let mut candidata: Option<u64> = None;
    while lo < hi {
        if cancel.load(Ordering::Relaxed) {
            return Err(Cancelled);
        }
        let mid = lo + (hi - lo) / 2;
        let inizio = inizio_riga(doc, mid);
        match prima_con_orario(doc, inizio, hi) {
            // niente orari da qui in avanti, entro l'intervallo: la risposta non puo' stare oltre
            None => hi = inizio,
            Some((r, chiave)) => {
                if non_prima(&chiave, &t) {
                    // questa riga va bene: se ne cerca una piu' precoce prima di lei
                    candidata = Some(r);
                    hi = r;
                } else {
                    // la prima riga con orario da qui e' ancora prima del target: si riparte dalla successiva
                    lo = riga_dopo(doc, r);
                }
            }
        }
    }
    Ok(candidata)
}

/// Che cosa si confronta di una riga: la data se c'e' e l'orario del giorno in millisecondi se c'e' (una riga
/// con la data sola non ha orario; una con l'orario solo non ha data).
struct Chiave {
    data: Option<Data>,
    ms: Option<u64>,
}

/// `true` se la riga non precede il target: con entrambe le date si confrontano prima quelle e a parita' gli
/// orari; se manca una delle due date si confrontano solo gli orari (quello che manca vale mezzanotte).
fn non_prima(riga: &Chiave, t: &Chiave) -> bool {
    match (t.data, riga.data) {
        (Some(dt), Some(dr)) if dr != dt => dr > dt,
        _ => riga.ms.unwrap_or(0) >= t.ms.unwrap_or(0),
    }
}

/// La prima riga con orario da `da` in avanti, guardando al piu' `RIGHE_MAX` righe e solo quelle che cominciano
/// prima di `limite`: `None` se non se ne trova nessuna.
fn prima_con_orario(doc: &Document, da: u64, limite: u64) -> Option<(u64, Chiave)> {
    let mut r = da;
    for _ in 0..RIGHE_MAX {
        if r >= limite {
            return None;
        }
        if let Some(chiave) = chiave_riga(doc, r) {
            return Some((r, chiave));
        }
        r = match scan::next_row(doc, r, PASSO) {
            Next::Line(p) | Next::Cut(p) => p,
            Next::End => return None,
        };
    }
    None
}

/// La prima riga con orario del documento, dall'inizio in avanti: le righe senza orario si saltano una alla
/// volta. Serve la data di quella riga a un target senza data. `Ok(None)` se nessuna riga ha orario.
fn prima_con_orario_del_file(
    doc: &Document,
    cancel: &AtomicBool,
) -> Result<Option<Chiave>, Cancelled> {
    let mut r = 0;
    while r < doc.len() {
        if cancel.load(Ordering::Relaxed) {
            return Err(Cancelled);
        }
        if let Some(chiave) = chiave_riga(doc, r) {
            return Ok(Some(chiave));
        }
        r = match scan::next_row(doc, r, PASSO) {
            Next::Line(p) | Next::Cut(p) => p,
            Next::End => return Ok(None),
        };
    }
    Ok(None)
}

/// L'inizio della riga che contiene `pos`: prima con una scansione breve (basta per le righe normali e costa
/// poco), poi col limite lungo per le righe piu' grosse, che cosi' partono davvero dal loro inizio.
fn inizio_riga(doc: &Document, pos: u64) -> u64 {
    let (p, vera) = scan::row_start(doc, pos, PASSO);
    if vera {
        p
    } else {
        scan::row_start(doc, pos, MAX_SCAN).0
    }
}

/// L'inizio della riga dopo quella che inizia in `r`: il salto e' della riga vera, perche' `lo` deve superare
/// l'a-capo (altrimenti la stessa riga si rivedrebbe a ogni giro). L'ultima riga da' la fine del documento.
fn riga_dopo(doc: &Document, r: u64) -> u64 {
    match scan::next_row(doc, r, PASSO) {
        // il passo breve basta quando la riga ci sta dentro
        Next::Line(p) => p,
        // riga lunga: si riprova col limite lungo; oltre il limite si resta a meta', come le righe visuali
        Next::Cut(_) => match scan::next_row(doc, r, MAX_SCAN) {
            Next::Line(p) | Next::Cut(p) => p,
            Next::End => doc.len(),
        },
        Next::End => doc.len(),
    }
}

/// L'orario di una riga: il testo fra l'inizio e la fine della riga (senza a-capo), letto per al piu'
/// `TESTO_MAX` byte con una decodifica lossy (gli orari sono ASCII). Vale il primo span `Time` di `loghl`, con
/// la data e l'orario uniti se nella riga sono separati da uno o due spazi o da una `T`. `None` se la riga non
/// ha orario.
fn chiave_riga(doc: &Document, r: u64) -> Option<Chiave> {
    let fine = scan::row_end(doc, r, TESTO_MAX);
    let n = fine.saturating_sub(r);
    let riga = doc.read_range(r, n);
    let testo = String::from_utf8_lossy(&riga);
    let spans = loghl::highlight(&testo);
    let mut times = spans.iter().filter(|s| s.kind == Kind::Time);
    let primo = times.next()?;
    let (data, mut ms) = interpreta_span(&testo[primo.start..primo.end])?;
    if data.is_some() && ms.is_none() {
        // data sola: se subito dopo c'e' un altro span `Time` che e' un orario, i due si uniscono
        if let Some(m) = times.next().and_then(|s| unione(&testo, primo.end, s)) {
            ms = Some(m);
        }
    }
    Some(Chiave { data, ms })
}

/// L'orario del secondo span, se sta subito dopo la data (che finisce in `a`), separato da uno o due spazi o da
/// una `T`, ed e' un orario (non un'altra data): la data della riga e quest'orario si uniscono.
fn unione(testo: &str, a: usize, span: &loghl::Span) -> Option<u64> {
    if !matches!(&testo[a..span.start], " " | "  " | "T") {
        return None;
    }
    match interpreta_span(&testo[span.start..span.end]) {
        Some((None, Some(m))) => Some(m),
        _ => None,
    }
}

/// Che cosa vale lo span `Time` di una riga: la data e/o l'orario del giorno, interpretando le cifre. Lo span
/// di `loghl` porta gia' con se' la frazione e il fuso (che si ignora) e ammette i secondi 60.
fn interpreta_span(testo: &str) -> Option<(Option<Data>, Option<u64>)> {
    let b = testo.as_bytes();
    if let Some(data) = data(b) {
        if b.len() == 10 {
            // la data sola
            return Some((Some(data), None));
        }
        // `loghl` lega la data all'orario con una `T` o un solo spazio
        if b.get(10) != Some(&b'T') && b.get(10) != Some(&b' ') {
            return None;
        }
        let ms = orario_riga(&b[11..])?;
        return Some((Some(data), Some(ms)));
    }
    let ms = orario_riga(b)?;
    Some((None, Some(ms)))
}

/// La data nelle prime dieci posizioni di `b` (`AAAA-MM-GG`, con mese 01-12 e giorno 01-31), se c'e'.
fn data(b: &[u8]) -> Option<Data> {
    if b.len() < 10 || b[4] != b'-' || b[7] != b'-' {
        return None;
    }
    let anno = u16::from(cifra(b, 0)?)
        + u16::from(cifra(b, 1)?) * 10
        + u16::from(cifra(b, 2)?) * 100
        + u16::from(cifra(b, 3)?) * 1000;
    let mese = due_cifre(b, 5)?;
    let giorno = due_cifre(b, 8)?;
    if !(1..=12).contains(&mese) || !(1..=31).contains(&giorno) {
        return None;
    }
    Some((anno, mese, giorno))
}

/// L'orario del giorno in millisecondi di un testo `HH:MM`, `HH:MM:SS` o `HH:MM:SS` con frazione: consuma
/// tutto `b`, i secondi sono al piu' 59 e dopo l'orario non c'e' niente.
fn orario_stretto(b: &[u8]) -> Option<u64> {
    if b.len() == 5 {
        if b[2] != b':' {
            return None;
        }
        let (ore, minuti) = (due_cifre(b, 0)?, due_cifre(b, 3)?);
        if ore > 23 || minuti > 59 {
            return None;
        }
        return Some((u64::from(ore) * 3600 + u64::from(minuti) * 60) * 1000);
    }
    if b.len() < 8 || b[2] != b':' || b[5] != b':' {
        return None;
    }
    let (ore, minuti, secondi) = (due_cifre(b, 0)?, due_cifre(b, 3)?, due_cifre(b, 6)?);
    if ore > 23 || minuti > 59 || secondi > 59 {
        return None;
    }
    // senza frazione l'orario finisce ai secondi
    let (fra, fine) = frazione(b, 8).unwrap_or((0, 8));
    if fine != b.len() {
        return None;
    }
    Some((u64::from(ore) * 3600 + u64::from(minuti) * 60 + u64::from(secondi)) * 1000 + fra)
}

/// L'orario del giorno in millisecondi del testo di uno span di `loghl` (`HH:MM:SS`, con frazione e fuso
/// facoltativi): consuma tutto `b`; i secondi 60 valgono 59 e mezzo e il fuso si ignora.
fn orario_riga(b: &[u8]) -> Option<u64> {
    if b.len() < 8 || b[2] != b':' || b[5] != b':' {
        return None;
    }
    let (ore, minuti, secondi) = (due_cifre(b, 0)?, due_cifre(b, 3)?, due_cifre(b, 6)?);
    if ore > 23 || minuti > 59 || secondi > 60 {
        return None;
    }
    let (fra, fine_fraz) = frazione(b, 8).unwrap_or((0, 8));
    let fine = salta_fuso(b, fine_fraz)?;
    if fine != b.len() {
        return None;
    }
    // i secondi 60 valgono 59 e mezzo
    let mezzo = if secondi == 60 { 500 } else { 0 };
    let secondi = u64::from(secondi.min(59));
    Some((u64::from(ore) * 3600 + u64::from(minuti) * 60 + secondi) * 1000 + fra + mezzo)
}

/// La frazione che comincia in `i` (il separatore, punto o virgola): i millisecondi delle cifre (al massimo tre
/// contate, le altre si lasciano) e la posizione dopo l'ultima cifra. `None` se il separatore non c'e' o non e'
/// seguito da nessuna cifra.
fn frazione(b: &[u8], i: usize) -> Option<(u64, usize)> {
    if !matches!(b.get(i), Some(&b'.') | Some(&b',')) {
        return None;
    }
    let mut ms: u64 = 0;
    let mut cifre = 0;
    let mut j = i + 1;
    while let Some(&c) = b.get(j) {
        if !c.is_ascii_digit() {
            break;
        }
        if cifre < 3 {
            ms = ms * 10 + u64::from(c - b'0');
        }
        cifre += 1;
        j += 1;
    }
    if cifre == 0 {
        return None;
    }
    // con meno di tre cifre le altre sono zeri (".5" e' mezzo secondo)
    for _ in cifre..3 {
        ms *= 10;
    }
    Some((ms, j))
}

/// La posizione dopo il fuso che comincia in `i`, se c'e' (`Z`, oppure un segno con `hh:mm` o `hhmm`): il fuso
/// non cambia l'orario, si salta soltanto.
fn salta_fuso(b: &[u8], i: usize) -> Option<usize> {
    match b.get(i) {
        Some(&b'Z') => Some(i + 1),
        Some(&c) if c == b'+' || c == b'-' => {
            if b.get(i + 3) == Some(&b':') {
                due_cifre(b, i + 1)?;
                due_cifre(b, i + 4)?;
                Some(i + 6)
            } else {
                due_cifre(b, i + 1)?;
                due_cifre(b, i + 3)?;
                Some(i + 5)
            }
        }
        _ => Some(i),
    }
}

/// La cifra in `i`, se c'e' ed e' ASCII.
fn cifra(b: &[u8], i: usize) -> Option<u8> {
    match b.get(i) {
        Some(&c @ b'0'..=b'9') => Some(c - b'0'),
        _ => None,
    }
}

/// Le due cifre in `i` e `i+1` come numero.
fn due_cifre(b: &[u8], i: usize) -> Option<u8> {
    Some(cifra(b, i)? * 10 + cifra(b, i + 1)?)
}
