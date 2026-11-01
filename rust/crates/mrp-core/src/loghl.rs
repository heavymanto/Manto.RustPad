//! La colorazione delle righe di log: in UNA riga si trovano i livelli (ERROR, WARN, INFO, DEBUG e parenti), le date
//! con gli orari e gli indirizzi IPv4, come posizioni in byte (`Span`). I colori li decide chi disegna: qui c'e' solo
//! la geometria dei pezzi, niente I/O, niente dipendenze, niente stato.
//!
//! Il riconoscimento e' a mano, senza espressioni regolari: una passata sola sui byte della riga, dall'inizio alla
//! fine. Quando un token viene riconosciuto la scansione riprende dal suo `end`, quando non lo e' avanza di un byte:
//! non si ricomincia mai prima, cosi' il costo resta lineare e gli span escono gia' in ordine, senza sovrapporsi. I
//! token sono tutti ASCII e i byte >= 0x80 non sono mai alfanumerici ne' parte di un token: ogni span comincia e
//! finisce su un byte ASCII, che e' sempre un confine di carattere.

/// Che cos'e' un pezzo di riga trovato da `highlight`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Un errore: `ERROR`, `ERR`, `FATAL`, `CRITICAL`, `SEVERE`, `PANIC`.
    Error,
    /// Un avviso: `WARN`, `WARNING`.
    Warn,
    /// Una notizia: `INFO`, `NOTICE`.
    Info,
    /// Il dettaglio: `DEBUG`, `TRACE`.
    Debug,
    /// Una data, un orario o una data con l'orario che segue (frazione e fuso compresi).
    Time,
    /// Un indirizzo IPv4.
    Ip,
}

/// Un pezzo di riga da colorare: i byte `[start, end)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    /// Il primo byte del pezzo.
    pub start: usize,
    /// Il byte dopo l'ultimo del pezzo.
    pub end: usize,
    /// Che cos'e' il pezzo.
    pub kind: Kind,
}

/// I pezzi di `line`, in ordine di `start` e mai sovrapposti: una passata sola, si riprende dalla fine di ogni token
/// riconosciuto (vedi il modulo).
pub fn highlight(line: &str) -> Vec<Span> {
    let b = line.as_bytes();
    let mut spans = Vec::new();
    let mut i = 0;
    while i < b.len() {
        // un token comincia con una maiuscola (livello) o con una cifra (data, orario, indirizzo), se il "prima" va bene
        let token = if b[i].is_ascii_uppercase() && prima(b, i) {
            livello(b, i)
        } else if b[i].is_ascii_digit() && prima_punto(b, i) {
            // nell'ordine: la data con l'orario che la segue, l'orario da solo, l'indirizzo
            data_oraria(b, i)
                .or_else(|| orario(b, i))
                .map(|fine| (fine, Kind::Time))
                .or_else(|| indirizzo(b, i))
        } else {
            None
        };
        if let Some((fine, tipo)) = token {
            spans.push(Span {
                start: i,
                end: fine,
                kind: tipo,
            });
            i = fine;
        } else {
            // niente token: un byte alla volta, mai da capo
            i += 1;
        }
    }
    spans
}

/// Il livello della riga: il tipo del primo span di livello (`Error`, `Warn`, `Info` o `Debug`) che `highlight`
/// trova, o `None` se la riga non ne ha. Niente secondo riconoscimento: si riusa `highlight`.
pub fn line_level(line: &str) -> Option<Kind> {
    highlight(line)
        .into_iter()
        .find(|sp| matches!(sp.kind, Kind::Error | Kind::Warn | Kind::Info | Kind::Debug))
        .map(|sp| sp.kind)
}

/// Il "prima" di un livello: il byte prima non c'e' (inizio riga) o non e' una lettera o cifra ASCII (i byte >= 0x80
/// non sono mai alfanumerici, quindi vanno bene come delimitatori).
fn prima(b: &[u8], i: usize) -> bool {
    i == 0 || !b[i - 1].is_ascii_alphanumeric()
}

/// Il "prima" di date, orari e indirizzi: come `prima`, ma il byte prima non deve nemmeno essere un punto (un numero
/// dopo un punto e' la continuazione di un altro numero, non l'inizio di un token).
fn prima_punto(b: &[u8], i: usize) -> bool {
    i == 0 || (!b[i - 1].is_ascii_alphanumeric() && b[i - 1] != b'.')
}

/// Il livello che comincia in `i` (maiuscola con `prima` gia' valido): la sequenza massimale di maiuscole e' una
/// delle parole dei livelli e il byte dopo non e' una lettera, una cifra o `_`. La fine del livello e il suo tipo.
fn livello(b: &[u8], i: usize) -> Option<(usize, Kind)> {
    let mut fine = i;
    while b.get(fine).is_some_and(|&c| c.is_ascii_uppercase()) {
        fine += 1;
    }
    let tipo = match &b[i..fine] {
        b"ERROR" | b"ERR" | b"FATAL" | b"CRITICAL" | b"SEVERE" | b"PANIC" => Kind::Error,
        b"WARN" | b"WARNING" => Kind::Warn,
        b"INFO" | b"NOTICE" => Kind::Info,
        b"DEBUG" | b"TRACE" => Kind::Debug,
        _ => return None,
    };
    // per un livello il "dopo" non va bene nemmeno con `_`: ERROR_CODE non e' un errore
    if b.get(fine)
        .is_some_and(|&c| c.is_ascii_alphanumeric() || c == b'_')
    {
        return None;
    }
    Some((fine, tipo))
}

/// La data che comincia in `i` (cifra con `prima_punto` gia' valido) e l'orario che la segue se c'e' il connettore:
/// la fine dello span. La data e' `dddd-dd-dd` con mese 01-12 e giorno 01-31; il connettore e' UN `T` o UNO spazio
/// seguito subito da un orario valido; senza connettore la data sola vuole il "dopo" valido.
fn data_oraria(b: &[u8], i: usize) -> Option<usize> {
    // l'anno di quattro cifre, i trattini in mezzo, il mese e il giorno di due cifre
    if !b.get(i..i + 4)?.iter().all(|&c| c.is_ascii_digit()) {
        return None;
    }
    if *b.get(i + 4)? != b'-' || *b.get(i + 7)? != b'-' {
        return None;
    }
    let mese = due_cifre(b, i + 5)?;
    let giorno = due_cifre(b, i + 8)?;
    if !(1..=12).contains(&mese) || !(1..=31).contains(&giorno) {
        return None;
    }
    let fine = i + 10;
    // il connettore: UN `T` o UNO spazio, seguito subito da un orario valido
    match b.get(fine) {
        Some(&b'T') | Some(&b' ') => {
            if let Some(fine_orario) = orario(b, fine + 1) {
                return Some(fine_orario);
            }
        }
        _ => {}
    }
    // data sola: anche il "dopo" deve essere valido
    if b.get(fine).is_some_and(|&c| c.is_ascii_alphanumeric()) {
        return None;
    }
    Some(fine)
}

/// L'orario che comincia in `i` (cifra): `dd:dd:dd` con ore 00-23, minuti 00-59 e secondi 00-60, piu' le estensioni
/// facoltative: la frazione (`.` o `,` con una o piu' cifre) e il fuso (`Z`, oppure `+`/`-` con `hh:mm` o `hhmm`;
/// incompleto non si prende). Il "dopo" valido e' quello di fine token, dopo le estensioni. Il "prima" lo controlla
/// chi chiama: da solo serve, come continuazione di una data no.
fn orario(b: &[u8], i: usize) -> Option<usize> {
    if *b.get(i + 2)? != b':' || *b.get(i + 5)? != b':' {
        return None;
    }
    let ore = due_cifre(b, i)?;
    let minuti = due_cifre(b, i + 3)?;
    let secondi = due_cifre(b, i + 6)?;
    if ore > 23 || minuti > 59 || secondi > 60 {
        return None;
    }
    let mut fine = i + 8;
    // la frazione: un punto o una virgola, e una o piu' cifre
    if b.get(fine).is_some_and(|&c| c == b'.' || c == b',') {
        let mut j = fine + 1;
        while b.get(j).is_some_and(|&c| c.is_ascii_digit()) {
            j += 1;
        }
        if j > fine + 1 {
            fine = j;
        }
    }
    // il fuso: Z, oppure un segno con hh:mm o hhmm (incompleto non si prende)
    match b.get(fine) {
        Some(&b'Z') => fine += 1,
        Some(&b'+') | Some(&b'-') => {
            if let Some(fine_fuso) = fuso(b, fine + 1) {
                fine = fine_fuso;
            }
        }
        _ => {}
    }
    // il "dopo" valido si guarda qui, a fine token: `Z` e i segni del fuso possono seguire i secondi
    if b.get(fine).is_some_and(|&c| c.is_ascii_alphanumeric()) {
        return None;
    }
    Some(fine)
}

/// Il fuso che comincia in `i`, subito dopo il segno: `hh:mm` oppure `hhmm`, con ore 00-23 e minuti 00-59. La fine.
fn fuso(b: &[u8], i: usize) -> Option<usize> {
    if b.get(i + 2) == Some(&b':') {
        let ore = due_cifre(b, i)?;
        let minuti = due_cifre(b, i + 3)?;
        return (ore <= 23 && minuti <= 59).then_some(i + 5);
    }
    let ore = due_cifre(b, i)?;
    let minuti = due_cifre(b, i + 2)?;
    (ore <= 23 && minuti <= 59).then_some(i + 4)
}

/// L'indirizzo IPv4 che comincia in `i` (cifra con `prima_punto` gia' valido): quattro gruppi di 1-3 cifre da 0 a
/// 255 separati da punti. La fine col suo tipo: il "dopo" non deve essere una lettera o una cifra, ne' un punto
/// seguito da una cifra (cosi' `1.2.3.4.5` non e' un indirizzo); la porta dietro i due punti resta fuori.
fn indirizzo(b: &[u8], i: usize) -> Option<(usize, Kind)> {
    let mut fine = i;
    for gruppo in 0..4 {
        // il gruppo: da una a tre cifre, di valore al piu' 255
        let mut valore: u16 = 0;
        let mut cifre = 0;
        while cifre < 3 {
            let Some(d) = cifra(b, fine) else {
                break;
            };
            valore = valore * 10 + u16::from(d);
            fine += 1;
            cifre += 1;
        }
        if cifre == 0 || valore > 255 {
            return None;
        }
        // i primi tre gruppi sono seguiti da un punto
        if gruppo < 3 {
            if b.get(fine) != Some(&b'.') {
                return None;
            }
            fine += 1;
        }
    }
    match b.get(fine) {
        // il "dopo": non alfanumerico, e non un punto con una cifra dietro
        Some(&c) if c.is_ascii_alphanumeric() => return None,
        Some(&b'.') if b.get(fine + 1).is_some_and(|&d| d.is_ascii_digit()) => return None,
        _ => {}
    }
    Some((fine, Kind::Ip))
}

/// La cifra in `i`: il valore del byte, se c'e' ed e' una cifra ASCII.
fn cifra(b: &[u8], i: usize) -> Option<u8> {
    let c = *b.get(i)?;
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        _ => None,
    }
}

/// Le due cifre in `i` e `i+1` come numero (anni, mesi, giorni, ore, minuti, secondi, fusi).
fn due_cifre(b: &[u8], i: usize) -> Option<u8> {
    Some(cifra(b, i)? * 10 + cifra(b, i + 1)?)
}
