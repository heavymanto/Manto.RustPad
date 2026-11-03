//! La maschera dell'orario (task H2): come si legge l'orario scritto in una riga di log secondo un
//! pattern (`yyyy-MM-dd HH:mm:ss.SSS`, syslog, Apache, epoch...), come si riconosce il pattern da un
//! campione di righe (`detect`) e come si scrive una chiave per chi legge (`iso`).
//!
//! La chiave e' il numero di millisecondi dal 1970-01-01, senza fuso e col calendario gregoriano
//! scritto a mano (niente chrono): vale anche per le date prima del 1970, quindi le chiavi possono
//! essere negative. Le maschere con la sola ora hanno per chiave i millisecondi del giorno. L'orario
//! deve cominciare entro i primi `within + 1` byte della riga, perche' nei log sta quasi sempre in
//! testa: cosi' basta la testa di una riga, e un file non si legge mai per intero.

use std::fmt;

/// Quanti millisecondi ha un giorno.
const MS_GIORNO: i64 = 86_400_000;

/// Le maschere che `detect` prova, nell'ordine di priorita': vince quella che legge piu' righe del
/// campione, a parita' la prima di quest'ordine (con `within` 0 prima di 40).
const CANDIDATI: [&str; 17] = [
    "yyyy-MM-dd HH:mm:ss.SSS",
    "yyyy-MM-dd HH:mm:ss,SSS",
    "yyyy-MM-ddTHH:mm:ss.SSS",
    "yyyy-MM-ddTHH:mm:ss,SSS",
    "yyyy-MM-dd HH:mm:ss",
    "yyyy-MM-ddTHH:mm:ss",
    "yyyy/MM/dd HH:mm:ss",
    "dd/MM/yyyy HH:mm:ss",
    "dd-MM-yyyy HH:mm:ss",
    "dd/MMM/yyyy:HH:mm:ss",
    "MMM d HH:mm:ss",
    "HH:mm:ss.SSS",
    "HH:mm:ss,SSS",
    "HH:mm:ss",
    "epoch_ms",
    "epoch_s",
    "yyyy-MM-dd",
];

/// Perche' un pattern non va.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MaskError {
    /// Niente data completa (anno, mese e giorno), niente ora completa (`HH` e `mm`), niente `epoch_*`.
    Incomplete,
    /// Una serie di lettere di gettone con una lunghezza che non esiste (per esempio `yyyyy` o `SS`).
    BadToken(String),
}

impl fmt::Display for MaskError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MaskError::Incomplete => {
                write!(f, "maschera incompleta: serve la data, l'ora o un epoch")
            }
            MaskError::BadToken(serie) => write!(f, "gettone non valido: {serie}"),
        }
    }
}

impl std::error::Error for MaskError {}

/// Un pezzo del pattern: un gettone, uno spazio o un letterale.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Pezzo {
    /// `yyyy`: l'anno a quattro cifre.
    Anno4,
    /// `yy`: due cifre, anno 2000+.
    Anno2,
    /// `MM`: il mese a due cifre.
    Mese2,
    /// `MMM`: il mese in inglese a tre lettere, maiuscole indifferenti.
    Mese3,
    /// `dd`: il giorno a due cifre.
    Giorno2,
    /// `d`: il giorno a una o due cifre.
    Giorno12,
    /// `HH`: l'ora a due cifre.
    Ore,
    /// `mm`: i minuti a due cifre.
    Minuti,
    /// `ss`: i secondi a due cifre (il 60 vale 59 e mezzo).
    Secondi,
    /// `SSS`: la frazione, da una a nove cifre.
    Frazione,
    /// `Z`: il fuso, facoltativo, letto e ignorato.
    Fuso,
    /// Uno spazio del pattern: uno o piu' spazi della riga.
    Spazio,
    /// Un testo letterale, anche quello fra apici singoli.
    Letterale(Vec<u8>),
}

/// Che cosa si legge nella riga: il pattern spezzato in pezzi, oppure un epoch.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Corpo {
    /// Gettoni e letterali del pattern.
    Pezzi(Vec<Pezzo>),
    /// `epoch_s` o `epoch_ms`: il numero per intero, con la lunghezza a decidere qual e'.
    Epoch {
        /// `epoch_s` (secondi) invece di `epoch_ms` (millisecondi).
        secondi: bool,
    },
}

/// La maschera dell'orario di un log: si costruisce con `parse` e si legge una riga con `read`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimeMask {
    /// Il pattern, come l'ha scritto chi ha chiamato `parse`.
    pattern: String,
    /// Quanti byte di finestra prima dell'orario.
    within: usize,
    /// L'anno delle maschere senza anno (di fabbrica 2000; lo cambia `with_year`).
    anno: i32,
    /// La maschera ha una data: `false` solo per le maschere con la sola ora.
    ha_data: bool,
    /// Che cosa si legge nella riga.
    corpo: Corpo,
}

impl TimeMask {
    /// La maschera di un pattern. I gettoni sono `yyyy`, `yy`, `MM`, `MMM`, `dd`, `d`, `HH`, `mm`,
    /// `ss`, `SSS` e `Z` (il fuso, facoltativo); ogni altro carattere e' un letterale, e il testo fra
    /// apici singoli lo e' sempre. Il pattern intero `epoch_s` o `epoch_ms` e' speciale. `within` e'
    /// quanti byte di finestra prima dell'orario. Serve la data completa (anno, mese e giorno) o
    /// l'ora completa (`HH` e `mm`): altrimenti `Incomplete`; una serie di lettere di gettone con una
    /// lunghezza che non esiste e' `BadToken`.
    pub fn parse(pattern: &str, within: usize) -> Result<TimeMask, MaskError> {
        // gli epoch si riconoscono prima di spezzare in gettoni, e da soli: le loro lettere (la `s`
        // finale, per esempio) lette a gettoni non passerebbero
        if pattern == "epoch_s" || pattern == "epoch_ms" {
            return Ok(TimeMask {
                pattern: pattern.to_string(),
                within,
                anno: 2000,
                ha_data: true,
                corpo: Corpo::Epoch {
                    secondi: pattern == "epoch_s",
                },
            });
        }
        let pezzi = spezza(pattern)?;
        // o la data completa, o l'ora completa: altrimenti non c'e' niente da leggere
        let ha_anno = pezzi
            .iter()
            .any(|p| matches!(p, Pezzo::Anno4 | Pezzo::Anno2));
        let ha_mese = pezzi
            .iter()
            .any(|p| matches!(p, Pezzo::Mese2 | Pezzo::Mese3));
        let ha_giorno = pezzi
            .iter()
            .any(|p| matches!(p, Pezzo::Giorno2 | Pezzo::Giorno12));
        let ha_ora = pezzi.iter().any(|p| matches!(p, Pezzo::Ore))
            && pezzi.iter().any(|p| matches!(p, Pezzo::Minuti));
        if !(ha_anno && ha_mese && ha_giorno) && !ha_ora {
            return Err(MaskError::Incomplete);
        }
        Ok(TimeMask {
            pattern: pattern.to_string(),
            within,
            anno: 2000,
            ha_data: ha_anno || ha_mese || ha_giorno,
            corpo: Corpo::Pezzi(pezzi),
        })
    }

    /// L'anno delle maschere senza anno (syslog), di fabbrica 2000: per le altre non cambia niente.
    pub fn with_year(self, year: i32) -> TimeMask {
        TimeMask { anno: year, ..self }
    }

    /// Il pattern, come l'ha scritto chi ha chiamato `parse`.
    pub fn pattern(&self) -> &str {
        &self.pattern
    }

    /// Quanti byte di finestra prima dell'orario.
    pub fn within(&self) -> usize {
        self.within
    }

    /// La maschera ha una data: `false` solo per le maschere con la sola ora.
    pub fn has_date(&self) -> bool {
        self.ha_data
    }

    /// L'orario della riga, se c'e': la chiave in millisecondi dal 1970 (senza fuso) oppure, per le
    /// maschere con la sola ora, i millisecondi del giorno. L'orario deve cominciare entro i primi
    /// `within + 1` byte: si prova in ogni posizione da 0 a `within` e vale la prima che corrisponde;
    /// dopo l'orario il testo puo' continuare. Una riga troppo corta, non UTF-8, con valori
    /// impossibili o che non corrisponde da' `None`, mai un panic.
    pub fn read(&self, line: &[u8]) -> Option<i64> {
        if std::str::from_utf8(line).is_err() {
            return None;
        }
        // oltre la fine della riga non c'e' niente da provare
        let fine = self.within.min(line.len().saturating_sub(1));
        for da in 0..=fine {
            let chiave = match &self.corpo {
                Corpo::Pezzi(_) => self.leggi_pezzi(line, da),
                Corpo::Epoch { secondi } => leggi_epoch(line, da, *secondi),
            };
            if chiave.is_some() {
                return chiave;
            }
        }
        None
    }

    /// L'orario che comincia in `da`, secondo i pezzi del pattern. Ogni accesso passa da `get` o da
    /// `cifre`, cosi' una riga corta non fa mai uscire un indice fuori misura.
    fn leggi_pezzi(&self, line: &[u8], da: usize) -> Option<i64> {
        let Corpo::Pezzi(pezzi) = &self.corpo else {
            return None;
        };
        let mut i = da;
        let mut anno: Option<i32> = None;
        let mut mese: Option<u32> = None;
        let mut giorno: Option<u32> = None;
        let mut ore: Option<u32> = None;
        let mut minuti: Option<u32> = None;
        let mut secondi: Option<u32> = None;
        let mut frazione: Option<i64> = None;
        for pezzo in pezzi {
            match pezzo {
                Pezzo::Anno4 => {
                    anno = Some(cifre(line, i, 4)? as i32);
                    i += 4;
                }
                Pezzo::Anno2 => {
                    anno = Some(2000 + cifre(line, i, 2)? as i32);
                    i += 2;
                }
                Pezzo::Mese2 => {
                    let m = cifre(line, i, 2)?;
                    if !(1..=12).contains(&m) {
                        return None;
                    }
                    mese = Some(m);
                    i += 2;
                }
                Pezzo::Mese3 => {
                    mese = Some(mese_nome(line, i)?);
                    i += 3;
                }
                Pezzo::Giorno2 => {
                    let g = cifre(line, i, 2)?;
                    if !(1..=31).contains(&g) {
                        return None;
                    }
                    giorno = Some(g);
                    i += 2;
                }
                Pezzo::Giorno12 => {
                    // una cifra, due se anche la seconda e' una cifra
                    let mut g = cifre(line, i, 1)?;
                    i += 1;
                    if let Some(due) = cifra(line, i) {
                        g = g * 10 + u32::from(due);
                        i += 1;
                    }
                    if !(1..=31).contains(&g) {
                        return None;
                    }
                    giorno = Some(g);
                }
                Pezzo::Ore => {
                    let h = cifre(line, i, 2)?;
                    if h > 23 {
                        return None;
                    }
                    ore = Some(h);
                    i += 2;
                }
                Pezzo::Minuti => {
                    let m = cifre(line, i, 2)?;
                    if m > 59 {
                        return None;
                    }
                    minuti = Some(m);
                    i += 2;
                }
                Pezzo::Secondi => {
                    let s = cifre(line, i, 2)?;
                    if s > 60 {
                        return None;
                    }
                    secondi = Some(s);
                    i += 2;
                }
                Pezzo::Frazione => {
                    let (fra, n) = frazione_cifre(line, i)?;
                    frazione = Some(fra);
                    i += n;
                }
                Pezzo::Fuso => {
                    i += lungo_fuso(line, i)?;
                }
                Pezzo::Spazio => {
                    // uno spazio del pattern: uno o piu' spazi della riga
                    if line.get(i) != Some(&b' ') {
                        return None;
                    }
                    while line.get(i) == Some(&b' ') {
                        i += 1;
                    }
                }
                Pezzo::Letterale(testo) => {
                    if line.get(i..i + testo.len()) != Some(&testo[..]) {
                        return None;
                    }
                    i += testo.len();
                }
            }
        }
        // l'orario del giorno, con quello che manca a zero: i secondi 60 valgono 59 e mezzo
        let ha_ora = ore.is_some() || minuti.is_some() || secondi.is_some() || frazione.is_some();
        let ms_giorno = if ha_ora {
            let s = secondi.unwrap_or(0);
            let mezzo = if s == 60 { 500 } else { 0 };
            (i64::from(ore.unwrap_or(0)) * 3600
                + i64::from(minuti.unwrap_or(0)) * 60
                + i64::from(s.min(59)))
                * 1000
                + frazione.unwrap_or(0)
                + i64::from(mezzo)
        } else {
            0
        };
        if !self.ha_data {
            // la sola ora: i millisecondi del giorno
            return Some(ms_giorno);
        }
        // la data: l'anno che manca viene da with_year, il mese e il giorno che mancano da gennaio
        let anno = anno.unwrap_or(self.anno);
        let mese = mese.unwrap_or(1);
        let giorno = giorno.unwrap_or(1);
        if giorno > giorni_del_mese(anno, mese) {
            return None;
        }
        Some(giorni_dal_1970(anno, mese, giorno) * MS_GIORNO + ms_giorno)
    }
}

/// Il riconoscimento di `detect`: la maschera, quante righe legge e quante sono quelle non vuote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Detected {
    /// La maschera vincitrice, con la finestra con cui ha vinto.
    pub mask: TimeMask,
    /// Le righe del campione che la maschera legge.
    pub matched: usize,
    /// Le righe non vuote del campione.
    pub total: usize,
}

/// La maschera che legge piu' righe di un campione (vedi `CANDIDATI`): `None` se nessuna ne legge
/// almeno il 30% (e almeno una). Le righe vuote non contano nel totale.
pub fn detect(lines: &[&[u8]]) -> Option<Detected> {
    let piene: Vec<&[u8]> = lines.iter().copied().filter(|l| !l.is_empty()).collect();
    let total = piene.len();
    let mut mask: Option<TimeMask> = None;
    let mut massimo = 0;
    for pattern in CANDIDATI {
        for within in [0, 40] {
            let Ok(maschera) = TimeMask::parse(pattern, within) else {
                continue;
            };
            let matched = piene.iter().filter(|l| maschera.read(l).is_some()).count();
            if matched > massimo {
                massimo = matched;
                mask = Some(maschera);
            }
        }
    }
    // serve che le righe lette siano almeno il 30% e almeno una
    let mask = mask?;
    if massimo * 10 >= total * 3 {
        Some(Detected {
            mask,
            matched: massimo,
            total,
        })
    } else {
        None
    }
}

/// La chiave scritta per chi legge: `yyyy-MM-dd HH:mm:ss` con la data, o il solo `HH:mm:ss` senza; i
/// millisecondi si scrivono solo quando non sono zero. Le chiavi negative (le date prima del 1970)
/// si scrivono giuste.
pub fn iso(key: i64, with_date: bool) -> String {
    let ms = key.rem_euclid(MS_GIORNO);
    let (h, m, s, fra) = (
        ms / 3_600_000,
        (ms / 60_000) % 60,
        (ms / 1000) % 60,
        ms % 1000,
    );
    let fra_str = if fra == 0 {
        String::new()
    } else {
        format!(".{fra:03}")
    };
    if with_date {
        let (anno, mese, giorno) = data_dai_giorni(key.div_euclid(MS_GIORNO));
        format!("{anno:04}-{mese:02}-{giorno:02} {h:02}:{m:02}:{s:02}{fra_str}")
    } else {
        format!("{h:02}:{m:02}:{s:02}{fra_str}")
    }
}

/// Il pattern spezzato in pezzi: gettoni, spazi e letterali. Un testo fra apici singoli e' letterale
/// anche se ha lettere dei gettoni (senza apice di chiusura, fino in fondo); una serie di lettere di
/// gettone con una lunghezza che non esiste e' `BadToken`.
fn spezza(pattern: &str) -> Result<Vec<Pezzo>, MaskError> {
    let b = pattern.as_bytes();
    let mut pezzi = Vec::new();
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        if c == b'\'' {
            // il testo fra apici: letterale, gli apici si tolgono
            let chiuso = b[i + 1..]
                .iter()
                .position(|&x| x == b'\'')
                .map(|p| i + 1 + p);
            let fine = chiuso.unwrap_or(b.len());
            accorpa(&mut pezzi, &b[i + 1..fine]);
            i = fine + 1;
        } else if e_lettera_gettone(c) {
            // una serie di lettere uguali: un gettone, se la lunghezza esiste
            let mut n = 1;
            while i + n < b.len() && b[i + n] == c {
                n += 1;
            }
            pezzi.push(match (c, n) {
                (b'y', 2) => Pezzo::Anno2,
                (b'y', 4) => Pezzo::Anno4,
                (b'M', 2) => Pezzo::Mese2,
                (b'M', 3) => Pezzo::Mese3,
                (b'd', 1) => Pezzo::Giorno12,
                (b'd', 2) => Pezzo::Giorno2,
                (b'H', 2) => Pezzo::Ore,
                (b'm', 2) => Pezzo::Minuti,
                (b's', 2) => Pezzo::Secondi,
                (b'S', 3) => Pezzo::Frazione,
                (b'Z', 1) => Pezzo::Fuso,
                _ => {
                    return Err(MaskError::BadToken(
                        String::from_utf8_lossy(&b[i..i + n]).into_owned(),
                    ))
                }
            });
            i += n;
        } else if c == b' ' {
            // uno spazio del pattern: uno o piu' spazi della riga
            pezzi.push(Pezzo::Spazio);
            i += 1;
        } else {
            // un letterale: si accumula fino al prossimo carattere speciale
            let da = i;
            while i < b.len() && !e_speciale(b[i]) {
                i += 1;
            }
            accorpa(&mut pezzi, &b[da..i]);
        }
    }
    Ok(pezzi)
}

/// Aggiunge un letterale ai pezzi, unito a quello che c'e' gia' prima: due letterali vicini valgono
/// un confronto solo, piu' grosso.
fn accorpa(pezzi: &mut Vec<Pezzo>, testo: &[u8]) {
    if testo.is_empty() {
        return;
    }
    if let Some(Pezzo::Letterale(ultimo)) = pezzi.last_mut() {
        ultimo.extend_from_slice(testo);
    } else {
        pezzi.push(Pezzo::Letterale(testo.to_vec()));
    }
}

/// Le lettere dei gettoni: `y`, `M`, `d`, `H`, `m`, `s`, `S` e `Z`. Le altre lettere (la `T`, per
/// esempio) sono letterali come tutto il resto.
fn e_lettera_gettone(c: u8) -> bool {
    matches!(c, b'y' | b'M' | b'd' | b'H' | b'm' | b's' | b'S' | b'Z')
}

/// Se il byte ferma un letterale: apice, spazio o lettera di gettone.
fn e_speciale(c: u8) -> bool {
    c == b'\'' || c == b' ' || e_lettera_gettone(c)
}

/// L'epoch che comincia in `da`: 9-10 cifre per i secondi (con frazione facoltativa), 12-13 per i
/// millisecondi; dopo il numero il testo puo' continuare.
fn leggi_epoch(line: &[u8], da: usize, secondi: bool) -> Option<i64> {
    let mut n = 0;
    while line.get(da + n).is_some_and(|c| c.is_ascii_digit()) {
        n += 1;
    }
    let ammesse = if secondi { 9..=10 } else { 12..=13 };
    if !ammesse.contains(&n) {
        return None;
    }
    let mut valore: i64 = 0;
    for j in 0..n {
        valore = valore * 10 + i64::from(line[da + j] - b'0');
    }
    if secondi {
        valore *= 1000;
        // la frazione e' facoltativa: se non c'e', quello che segue resta testo che continua
        if line.get(da + n) == Some(&b'.') {
            if let Some((fra, _)) = frazione_cifre(line, da + n + 1) {
                valore += fra;
            }
        }
    }
    Some(valore)
}

/// La frazione che comincia in `i`: i millisecondi delle prime tre cifre (con meno di tre cifre le
/// altre sono zeri) e quante cifre occupa. `None` se non c'e' nessuna cifra, o se sono piu' di nove.
fn frazione_cifre(line: &[u8], i: usize) -> Option<(i64, usize)> {
    let mut n = 0;
    while line.get(i + n).is_some_and(|c| c.is_ascii_digit()) {
        n += 1;
    }
    if !(1..=9).contains(&n) {
        return None;
    }
    let mut ms: i64 = 0;
    for j in 0..n.min(3) {
        ms = ms * 10 + i64::from(line[i + j] - b'0');
    }
    for _ in n..3 {
        ms *= 10;
    }
    Some((ms, n))
}

/// Il mese delle tre lettere in `i`, in inglese e maiuscole indifferenti: 1-12, o `None` se non e'
/// un mese.
fn mese_nome(line: &[u8], i: usize) -> Option<u32> {
    const NOMI: [&[u8]; 12] = [
        b"Jan", b"Feb", b"Mar", b"Apr", b"May", b"Jun", b"Jul", b"Aug", b"Sep", b"Oct", b"Nov",
        b"Dec",
    ];
    let tre = line.get(i..i + 3)?;
    NOMI.iter()
        .position(|nome| {
            tre.iter()
                .zip(nome.iter())
                .all(|(c, n)| c.eq_ignore_ascii_case(n))
        })
        .map(|n| n as u32 + 1)
}

/// Quanto e' lungo il fuso in `i`: `Z`, `+hh:mm`, `-hh:mm`, `+hhmm` o `-hhmm` (si legge e si
/// ignora), 0 se non c'e'. `None` se c'e' il segno ma le cifre non bastano: un fuso incompleto non
/// e' testo qualsiasi.
fn lungo_fuso(line: &[u8], i: usize) -> Option<usize> {
    match line.get(i) {
        Some(&b'Z') => Some(1),
        Some(&c) if c == b'+' || c == b'-' => {
            if line.get(i + 3) == Some(&b':') {
                cifre(line, i + 1, 2)?;
                cifre(line, i + 4, 2)?;
                Some(6)
            } else {
                cifre(line, i + 1, 2)?;
                cifre(line, i + 3, 2)?;
                Some(5)
            }
        }
        _ => Some(0),
    }
}

/// La cifra in `i`, se c'e' ed e' ASCII.
fn cifra(line: &[u8], i: usize) -> Option<u8> {
    match line.get(i) {
        Some(&c @ b'0'..=b'9') => Some(c - b'0'),
        _ => None,
    }
}

/// Le `n` cifre in `i` come numero, se ci sono tutte.
fn cifre(line: &[u8], i: usize, n: usize) -> Option<u32> {
    let mut v: u32 = 0;
    for j in 0..n {
        v = v * 10 + u32::from(cifra(line, i + j)?);
    }
    Some(v)
}

/// Quanti giorni ha il mese (serve l'anno per febbraio).
fn giorni_del_mese(anno: i32, mese: u32) -> u32 {
    match mese {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ => {
            // bisestile: divisibile per 4, non per 100 salvo per 400
            if anno % 4 == 0 && (anno % 100 != 0 || anno % 400 == 0) {
                29
            } else {
                28
            }
        }
    }
}

/// I giorni fra il 1970-01-01 e una data del calendario gregoriano (negativi per le date prima):
/// l'algoritmo di Howard Hinnant, che vale per tutto il calendario, non solo dal 1970.
fn giorni_dal_1970(anno: i32, mese: u32, giorno: u32) -> i64 {
    let y = i64::from(anno) - i64::from(mese <= 2);
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400; // [0, 399]
                             // marzo e' il mese zero: cosi' il 29 febbraio sta nell'anno a cui serve il bisestile
    let mp = if mese > 2 {
        i64::from(mese) - 3
    } else {
        i64::from(mese) + 9
    };
    let doy = (153 * mp + 2) / 5 + i64::from(giorno) - 1; // [0, 365]
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy; // [0, 145095]
    era * 146097 + doe - 719468
}

/// La data di un numero di giorni dal 1970-01-01 (l'inverso di `giorni_dal_1970`): anno, mese e
/// giorno.
fn data_dai_giorni(giorni: i64) -> (i64, u32, u32) {
    let z = giorni + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = doy - (153 * mp + 2) / 5 + 1; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 }; // [1, 12]
    (if m <= 2 { y + 1 } else { y }, m as u32, d as u32)
}
