//! Codifiche dei caratteri. Il documento resta sempre una sequenza di BYTE: la codifica dice soltanto come quei byte si
//! leggono come caratteri (vista, cursore, larghezza delle colonne) e come un carattere digitato diventa byte.
//!
//! Le codifiche di questo modulo sono tutte compatibili con l'ASCII (il byte 0x0A e' sempre `\n`, 0x09 sempre la
//! tabulazione), quindi righe, indice delle righe e ricerca per byte non cambiano:
//! - `Utf8`: quella di sempre; un byte che non inizia un carattere valido e' un carattere tutto suo, U+FFFD;
//! - `Single`: una code page a byte singolo (windows-125x, ISO-8859-x, DOS, KOI8, Mac Roman; tabelle in `charsets`):
//!   un byte = un carattere; i byte 0x00-0x7F sono ASCII, gli altri vengono dalla tabella (U+FFFD se la code page
//!   non li definisce).
//!
//! I nomi si confrontano senza distinguere maiuscole e con `_` uguale a `-` (`by_name`).

use crate::charsets::CHARSETS;

/// Una codifica. E' un valore piccolo e copiabile (nel caso `Single`, l'indice in `charsets::CHARSETS`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum Encoding {
    /// UTF-8 (la predefinita).
    #[default]
    Utf8,
    /// Una code page a byte singolo: indice in `charsets::CHARSETS`.
    Single(usize),
}

impl Encoding {
    /// La codifica con questo nome (o alias), `None` se non esiste. Nomi: `utf-8` (anche `utf8`), poi i nomi canonici e
    /// gli alias di `charsets` (per esempio `windows-1252`, `cp1252`, `ansi`, `latin1`, `ibm437`, `437`).
    pub fn by_name(name: &str) -> Option<Encoding> {
        // i nomi canonici e gli alias sono minuscoli e col trattino: l'input si normalizza allo
        // stesso modo prima di confrontarlo
        let nome = name.trim().to_lowercase().replace('_', "-");
        if nome == "utf-8" || nome == "utf8" {
            return Some(Encoding::Utf8);
        }
        for (i, cs) in CHARSETS.iter().enumerate() {
            if nome == cs.name || cs.aliases.contains(&nome.as_str()) {
                return Some(Encoding::Single(i));
            }
        }
        None
    }

    /// Il nome canonico (minuscolo): `utf-8` oppure quello della code page. Vale `by_name(e.name()) == Some(e)`.
    pub fn name(self) -> &'static str {
        match self {
            Encoding::Utf8 => "utf-8",
            // l'indice e' sempre dentro CHARSETS (esiste solo se costruito da `by_name`/`all`),
            // ma un valore fuori tabella non deve poter andare in panico
            Encoding::Single(i) => CHARSETS.get(i).map_or("", |cs| cs.name),
        }
    }

    /// Tutte le codifiche: prima `Utf8`, poi le code page nell'ordine di `charsets::CHARSETS`. Nomi tutti diversi.
    pub fn all() -> Vec<Encoding> {
        let mut tutte = Vec::with_capacity(CHARSETS.len() + 1);
        tutte.push(Encoding::Utf8);
        for i in 0..CHARSETS.len() {
            tutte.push(Encoding::Single(i));
        }
        tutte
    }

    /// Vero per `Utf8`.
    pub fn is_utf8(self) -> bool {
        matches!(self, Encoding::Utf8)
    }

    /// Il primo carattere di `bytes` e quanti byte occupa, `None` se `bytes` e' vuoto.
    /// - `Utf8`: il piu' corto prefisso che e' un carattere UTF-8 valido; se non c'e' (byte non valido, sequenza
    ///   troncata o sovralunga) `('\u{fffd}', 1)`: un byte non valido e' un carattere tutto suo;
    /// - `Single`: sempre un byte; `b < 0x80` e' il carattere ASCII, altrimenti quello della tabella.
    pub fn first_char(self, bytes: &[u8]) -> Option<(char, usize)> {
        let &b = bytes.first()?;
        match self {
            // code page a byte singolo: un byte = un carattere
            Encoding::Single(i) => Some((Self::char_di_byte(i, b), 1)),
            // il piu' corto prefisso che e' un carattere intero (stessa regola di
            // `primo_carattere` in mrp-tui); se non c'e', un byte non valido e' un carattere
            // tutto suo; non si guardano mai piu' di 4 byte
            Encoding::Utf8 => {
                for n in 1..=bytes.len().min(4) {
                    if let Some(c) = std::str::from_utf8(&bytes[..n])
                        .ok()
                        .and_then(|s| s.chars().next())
                        .filter(|c| c.len_utf8() == n)
                    {
                        return Some((c, n));
                    }
                }
                Some(('\u{fffd}', 1))
            }
        }
    }

    /// Quanti byte occupa l'ULTIMO carattere di `bytes` (0 se e' vuoto): per `Utf8` il piu' corto suffisso che e' un
    /// carattere valido, altrimenti 1 (stessa regola di `first_char`, letta dalla fine); per `Single` 1.
    pub fn last_char_len(self, bytes: &[u8]) -> usize {
        if bytes.is_empty() {
            return 0;
        }
        match self {
            // code page a byte singolo: un carattere e' sempre un byte
            Encoding::Single(_) => 1,
            // il piu' corto suffisso che e' un carattere intero (stessa regola di
            // `ultimo_carattere` in mrp-tui), altrimenti un byte
            Encoding::Utf8 => {
                for n in 1..=bytes.len().min(4) {
                    let intero = std::str::from_utf8(&bytes[bytes.len() - n..])
                        .ok()
                        .and_then(|s| s.chars().next())
                        .is_some_and(|c| c.len_utf8() == n);
                    if intero {
                        return n;
                    }
                }
                1
            }
        }
    }

    /// I byte che scrivono `c` in questa codifica, `None` se non si puo' scrivere (carattere fuori dalla code page;
    /// U+FFFD non si scrive mai in una code page). In `Utf8` ogni carattere si scrive (U+FFFD compreso).
    pub fn encode(self, c: char) -> Option<Vec<u8>> {
        match self {
            // in UTF-8 si scrive tutto, U+FFFD compreso
            Encoding::Utf8 => {
                let mut buf = [0u8; 4];
                Some(c.encode_utf8(&mut buf).as_bytes().to_vec())
            }
            Encoding::Single(i) => {
                // il carattere sostitutivo non si scrive mai in una code page
                if c == '\u{fffd}' {
                    return None;
                }
                // ASCII: il suo byte
                if c.is_ascii() {
                    return Some(vec![c as u8]);
                }
                // ricerca inversa nella tabella: il primo byte che scrive c
                let cs = CHARSETS.get(i)?;
                cs.table
                    .iter()
                    .position(|&t| t == c)
                    .map(|j| vec![(0x80 + j) as u8])
            }
        }
    }

    /// Tutto `s` in questa codifica, oppure `Err(primo carattere che non si puo' scrivere)`.
    pub fn encode_str(self, s: &str) -> Result<Vec<u8>, char> {
        let mut out = Vec::with_capacity(s.len());
        for c in s.chars() {
            match self.encode(c) {
                Some(b) => out.extend_from_slice(&b),
                // si ferma al primo carattere che non si puo' scrivere
                None => return Err(c),
            }
        }
        Ok(out)
    }

    /// `bytes` come testo. `Utf8`: come `String::from_utf8_lossy`; `Single`: carattere per carattere dalla tabella
    /// (un byte non definito diventa U+FFFD).
    pub fn decode_lossy(self, bytes: &[u8]) -> String {
        match self {
            Encoding::Utf8 => String::from_utf8_lossy(bytes).into_owned(),
            Encoding::Single(i) => bytes.iter().map(|&b| Self::char_di_byte(i, b)).collect(),
        }
    }

    /// Indovina la codifica dai primi byte di un file (`sample`, di norma i primi 64 KiB), con queste regole:
    /// 1. se contiene un byte NUL: `Utf8` (file binario o in una codifica a 16 bit: non si indovina);
    /// 2. se e' UTF-8 valido, ammettendo una sequenza multibyte INCOMPLETA in coda (il campione e' tagliato): `Utf8`;
    /// 3. altrimenti la code page `windows-1252`.
    ///
    /// Un campione vuoto o solo ASCII e' `Utf8`.
    pub fn detect(sample: &[u8]) -> Encoding {
        // 1. un byte NUL: file binario o codifica a 16 bit, non si indovina
        if sample.contains(&0) {
            return Encoding::Utf8;
        }
        // 2. UTF-8 valido, ammettendo una sequenza multibyte INCOMPLETA in coda (il campione e'
        //    tagliato): con from_utf8 e' l'errore con error_len() = None, quello che dice che la
        //    sequenza sarebbe continuata oltre la fine del campione
        let valido = match std::str::from_utf8(sample) {
            Ok(_) => true,
            Err(e) => e.error_len().is_none(),
        };
        if valido {
            return Encoding::Utf8;
        }
        // 3. altrimenti la code page di ripiego
        Encoding::by_name("windows-1252").unwrap_or(Encoding::Utf8)
    }

    /// Il carattere del byte `b` nella code page `i`: ASCII sotto 0x80, altrimenti la tabella
    /// (U+FFFD se il byte non e' definito o l'indice non esiste).
    fn char_di_byte(i: usize, b: u8) -> char {
        if b < 0x80 {
            return b as char;
        }
        CHARSETS
            .get(i)
            .and_then(|cs| cs.table.get(usize::from(b - 0x80)))
            .map_or('\u{fffd}', |&c| c)
    }
}
