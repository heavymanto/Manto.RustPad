//! Codifiche dei caratteri. Il documento resta sempre una sequenza di BYTE: la codifica dice soltanto come quei byte si
//! leggono come caratteri (vista, cursore, larghezza delle colonne) e come un carattere digitato diventa byte.
//!
//! Due famiglie. Quelle compatibili con l'ASCII (`Utf8`, `Single`): il byte 0x0A e' sempre `\n`, 0x09 sempre la
//! tabulazione, quindi righe, indice delle righe e ricerca per byte non cambiano. Quelle a 16 bit (`Utf16Le`,
//! `Utf16Be`): un carattere e' una o due unita' da 2 byte e l'a-capo e' una unita' (`newline::Newline`), quindi
//! posizioni e ricerche devono stare allineate a 2 byte.
//! - `Utf8`: quella di sempre; un byte che non inizia un carattere valido e' un carattere tutto suo, U+FFFD;
//! - `Single`: una code page a byte singolo (windows-125x, ISO-8859-x, DOS, KOI8, Mac Roman; tabelle in `charsets`):
//!   un byte = un carattere; i byte 0x00-0x7F sono ASCII, gli altri vengono dalla tabella (U+FFFD se la code page
//!   non li definisce);
//! - `Utf16Le`, `Utf16Be`: UTF-16 little e big endian. Un carattere sono 2 byte (unita'), o 4 se e' una coppia di
//!   surrogati (alto D800-DBFF seguito da basso DC00-DFFF). Un surrogato spaiato e' un carattere U+FFFD di 2 byte;
//!   un byte finale che non completa una unita' e' un carattere U+FFFD di 1 byte. Il BOM (U+FEFF) e' un carattere
//!   come un altro: sta nei byte del documento e si conserva.
//!
//! I nomi si confrontano senza distinguere maiuscole e con `_` uguale a `-` (`by_name`).

use crate::charsets::CHARSETS;
use crate::multibyte::{self, MULTI};
use crate::newline::Newline;

/// Una codifica. E' un valore piccolo e copiabile (nel caso `Single`, l'indice in `charsets::CHARSETS`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum Encoding {
    /// UTF-8 (la predefinita).
    #[default]
    Utf8,
    /// Una code page a byte singolo: indice in `charsets::CHARSETS`.
    Single(usize),
    /// UTF-16 little endian.
    Utf16Le,
    /// UTF-16 big endian.
    Utf16Be,
    /// UTF-32 little endian.
    Utf32Le,
    /// UTF-32 big endian.
    Utf32Be,
    /// Una codifica multibyte dell'Asia orientale: indice in `multibyte::MULTI`.
    Multi(usize),
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
        // UTF-16: `utf-16le`, `utf16le`, `utf-16-le` e gli stessi per `be`; `utf-16` da solo e' ambiguo: non esiste
        match nome.as_str() {
            "utf-16le" | "utf16le" | "utf-16-le" => return Some(Encoding::Utf16Le),
            "utf-16be" | "utf16be" | "utf-16-be" => return Some(Encoding::Utf16Be),
            "utf-32le" | "utf32le" | "utf-32-le" => return Some(Encoding::Utf32Le),
            "utf-32be" | "utf32be" | "utf-32-be" => return Some(Encoding::Utf32Be),
            _ => {}
        }
        for (i, cs) in CHARSETS.iter().enumerate() {
            if nome == cs.name || cs.aliases.contains(&nome.as_str()) {
                return Some(Encoding::Single(i));
            }
        }
        for (i, m) in MULTI.iter().enumerate() {
            if nome == m.name || m.aliases.contains(&nome.as_str()) {
                return Some(Encoding::Multi(i));
            }
        }
        None
    }

    /// Il nome canonico (minuscolo): `utf-8`, `utf-16le`, `utf-16be` oppure quello della code page. Vale `by_name(e.name()) == Some(e)`.
    pub fn name(self) -> &'static str {
        match self {
            Encoding::Utf8 => "utf-8",
            Encoding::Utf16Le => "utf-16le",
            Encoding::Utf16Be => "utf-16be",
            Encoding::Utf32Le => "utf-32le",
            Encoding::Utf32Be => "utf-32be",
            // l'indice e' sempre dentro CHARSETS (esiste solo se costruito da `by_name`/`all`),
            // ma un valore fuori tabella non deve poter andare in panico
            Encoding::Single(i) => CHARSETS.get(i).map_or("", |cs| cs.name),
            Encoding::Multi(i) => MULTI.get(i).map_or("", |m| m.name),
        }
    }

    /// Tutte le codifiche: `Utf8`, `Utf16Le`, `Utf16Be`, `Utf32Le`, `Utf32Be`, poi le code page nell'ordine di
    /// `charsets::CHARSETS`, poi le multibyte nell'ordine di `multibyte::MULTI`. Nomi tutti diversi.
    pub fn all() -> Vec<Encoding> {
        let mut tutte = Vec::with_capacity(CHARSETS.len() + MULTI.len() + 5);
        tutte.push(Encoding::Utf8);
        tutte.push(Encoding::Utf16Le);
        tutte.push(Encoding::Utf16Be);
        tutte.push(Encoding::Utf32Le);
        tutte.push(Encoding::Utf32Be);
        for i in 0..CHARSETS.len() {
            tutte.push(Encoding::Single(i));
        }
        for i in 0..MULTI.len() {
            tutte.push(Encoding::Multi(i));
        }
        tutte
    }

    /// Vero per `Utf8`.
    pub fn is_utf8(self) -> bool {
        matches!(self, Encoding::Utf8)
    }

    /// Vero per `Utf16Le` e `Utf16Be`.
    pub fn is_utf16(self) -> bool {
        matches!(self, Encoding::Utf16Le | Encoding::Utf16Be)
    }

    /// Vero per `Utf32Le` e `Utf32Be`.
    pub fn is_utf32(self) -> bool {
        matches!(self, Encoding::Utf32Le | Encoding::Utf32Be)
    }

    /// Vero per le codifiche multibyte dell'Asia orientale (`Multi`): un carattere sono 1-4 byte e, per tornare
    /// indietro, serve guardare piu' di 4 byte (`back_window`).
    pub fn is_multibyte(self) -> bool {
        matches!(self, Encoding::Multi(_))
    }

    /// Quanti byte prima di una posizione bisogna leggere per trovare dove comincia il carattere precedente
    /// (`last_char_len`) o quello che la contiene (`char_start`): 4 dove bastano (UTF-8, UTF-16, UTF-32, code page),
    /// `multibyte::BACK_WINDOW` per le multibyte, dove il secondo byte di un carattere puo' sembrare un altro carattere.
    pub fn back_window(self) -> usize {
        if self.is_multibyte() {
            multibyte::BACK_WINDOW
        } else {
            4
        }
    }

    /// Dove comincia il carattere che contiene `ctx[i]` (`i < ctx.len()`), leggendo da `ctx` che finisce DOPO `i`
    /// (servono i byte seguenti per sapere quanto e' lungo): vale per le sole multibyte, per le altre vale `i`.
    pub fn char_start(self, ctx: &[u8], i: usize) -> usize {
        match self {
            Encoding::Multi(k) => MULTI.get(k).map_or(i, |m| multibyte::char_start(m, ctx, i)),
            _ => i,
        }
    }

    /// Quanti byte ha l'unita' di questa codifica, cioe' l'allineamento di posizioni e tagli: 4 per UTF-32, 2 per UTF-16, 1 per le altre.
    pub fn unit(self) -> usize {
        self.newline().unit()
    }

    /// La forma dell'a-capo nei byte di questa codifica (vedi `newline::Newline`).
    pub fn newline(self) -> Newline {
        match self {
            Encoding::Utf16Le => Newline::Le16,
            Encoding::Utf16Be => Newline::Be16,
            Encoding::Utf32Le => Newline::Le32,
            Encoding::Utf32Be => Newline::Be32,
            Encoding::Utf8 | Encoding::Single(_) | Encoding::Multi(_) => Newline::Byte,
        }
    }

    /// Il primo carattere di `bytes` e quanti byte occupa, `None` se `bytes` e' vuoto.
    /// - `Utf8`: il piu' corto prefisso che e' un carattere UTF-8 valido; se non c'e' (byte non valido, sequenza
    ///   troncata o sovralunga) `('\u{fffd}', 1)`: un byte non valido e' un carattere tutto suo;
    /// - `Single`: sempre un byte; `b < 0x80` e' il carattere ASCII, altrimenti quello della tabella;
    /// - `Utf16Le`, `Utf16Be`: una unita' (2 byte), o una coppia di surrogati (4 byte, alto D800-DBFF seguito
    ///   da basso DC00-DFFF); un surrogato spaiato e' `('\u{fffd}', 2)`, un byte finale che non completa una
    ///   unita' e' `('\u{fffd}', 1)`;
    /// - `Utf32Le`, `Utf32Be`: una unita' di 4 byte; un valore sopra 0x10FFFF o un surrogato (D800-DFFF) e'
    ///   `('\u{fffd}', 4)`; una coda di 1-3 byte che non completa una unita' e' UN SOLO `('\u{fffd}', n)`.
    pub fn first_char(self, bytes: &[u8]) -> Option<(char, usize)> {
        let &b = bytes.first()?;
        match self {
            // code page a byte singolo: un byte = un carattere
            Encoding::Single(i) => Some((Self::char_di_byte(i, b), 1)),
            // multibyte: le regole sono in `multibyte`
            Encoding::Multi(i) => MULTI.get(i).and_then(|m| multibyte::first_char(m, bytes)),
            // UTF-16: le regole nel commento del metodo, lette su una finestra di fino a 4 byte
            Encoding::Utf16Le | Encoding::Utf16Be => {
                // meno di 2 byte (il vuoto e' gia' escluso): resta un byte spaiato
                let Some(prima) = self.unita(bytes, 0) else {
                    return Some(('\u{fffd}', 1));
                };
                // alto seguito da basso: la coppia e' un carattere di 4 byte
                if let Some(c) = self.coppia(bytes, 0) {
                    return Some((c, 4));
                }
                // ogni altra unita' e' un carattere di 2 byte; un surrogato spaiato (un alto senza
                // il basso dopo, o un basso da solo) non e' un carattere e vale U+FFFD
                let c = char::from_u32(u32::from(prima)).unwrap_or('\u{fffd}');
                Some((c, 2))
            }
            // UTF-32: una unita' da 4 byte; la coda che non la completa e' un carattere solo
            Encoding::Utf32Le | Encoding::Utf32Be => {
                let Some(v) = self.unita32(bytes) else {
                    return Some(('\u{fffd}', bytes.len()));
                };
                Some((char::from_u32(v).unwrap_or('\u{fffd}'), 4))
            }
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
    /// carattere valido, altrimenti 1 (stessa regola di `first_char`, letta dalla fine); per `Single` 1; per UTF-16
    /// 4 se l'ultima unita' e' un basso preceduto da un alto, altrimenti 2 (e 1 se la lunghezza e' dispari); per
    /// UTF-32 4, o il resto della divisione per 4 se la lunghezza non e' multipla di 4 (la coda).
    pub fn last_char_len(self, bytes: &[u8]) -> usize {
        if bytes.is_empty() {
            return 0;
        }
        match self {
            // code page a byte singolo: un carattere e' sempre un byte
            Encoding::Single(_) => 1,
            // multibyte: si riparte da un byte sicuro e si va avanti (`multibyte`)
            Encoding::Multi(i) => MULTI
                .get(i)
                .map_or(1, |m| multibyte::last_char_len(m, bytes)),
            Encoding::Utf16Le | Encoding::Utf16Be => {
                // lunghezza dispari: l'ultimo byte non completa una unita', e' un carattere spaiato
                if !bytes.len().is_multiple_of(2) {
                    return 1;
                }
                // l'ultima unita' e' un basso preceduto da un alto: il carattere e' la coppia (4 byte)
                let n = bytes.len();
                if n >= 4 && self.coppia(bytes, n - 4).is_some() {
                    return 4;
                }
                2
            }
            // UTF-32: la coda che non completa una unita' e' un carattere solo, altrimenti l'ultima unita'
            Encoding::Utf32Le | Encoding::Utf32Be => match bytes.len() % 4 {
                0 => 4,
                resto => resto,
            },
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
    /// U+FFFD non si scrive mai in una code page). In `Utf8` e in UTF-16 ogni carattere si scrive (U+FFFD compreso).
    pub fn encode(self, c: char) -> Option<Vec<u8>> {
        match self {
            // in UTF-8 si scrive tutto, U+FFFD compreso
            Encoding::Utf8 => {
                let mut buf = [0u8; 4];
                Some(c.encode_utf8(&mut buf).as_bytes().to_vec())
            }
            Encoding::Utf16Le | Encoding::Utf16Be => {
                // in UTF-16 si scrive tutto, U+FFFD compreso: un'unita' (2 byte), o una coppia (4
                // byte) sopra U+FFFF, nell'endian di questa codifica
                let le = self == Encoding::Utf16Le;
                let mut out = Vec::with_capacity(4);
                let mut buf = [0u16; 2];
                for u in c.encode_utf16(&mut buf) {
                    out.extend_from_slice(&if le { u.to_le_bytes() } else { u.to_be_bytes() });
                }
                Some(out)
            }
            // UTF-32: il valore del carattere in 4 byte, nell'endian di questa codifica (si scrive tutto)
            Encoding::Utf32Le => Some((c as u32).to_le_bytes().to_vec()),
            Encoding::Utf32Be => Some((c as u32).to_be_bytes().to_vec()),
            Encoding::Multi(i) => MULTI.get(i).and_then(|m| multibyte::encode(m, c)),
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
    /// (un byte non definito diventa U+FFFD); UTF-16: le regole di `first_char` carattere per carattere.
    pub fn decode_lossy(self, bytes: &[u8]) -> String {
        match self {
            Encoding::Utf8 => String::from_utf8_lossy(bytes).into_owned(),
            Encoding::Single(i) => bytes.iter().map(|&b| Self::char_di_byte(i, b)).collect(),
            Encoding::Utf16Le
            | Encoding::Utf16Be
            | Encoding::Utf32Le
            | Encoding::Utf32Be
            | Encoding::Multi(_) => {
                // carattere per carattere: le coppie si uniscono, ogni surrogato spaiato e un
                // byte finale spaiato valgono un U+FFFD
                let mut s = String::with_capacity(bytes.len() / 2);
                let mut i = 0;
                while let Some((c, n)) = self.first_char(&bytes[i..]) {
                    s.push(c);
                    i += n;
                }
                s
            }
        }
    }

    /// Indovina la codifica dai primi byte di un file (`sample`, di norma i primi 64 KiB), con queste regole:
    /// 0. BOM UTF-32: `FF FE 00 00` -> `Utf32Le`; `00 00 FE FF` -> `Utf32Be` (senza BOM l'UTF-32 non si riconosce);
    /// 1. BOM UTF-16: il campione comincia con `FF FE` (e non e' un BOM UTF-32) -> `Utf16Le`; comincia con `FE FF`
    ///    -> `Utf16Be`;
    /// 2. senza BOM, se il campione ha almeno 8 byte: i byte zero nelle unita' intere dicono l'endian (in un
    ///    testo a 16 bit circa meta' dei byte e' zero, e sta tutto nella meta' giusta dell'unita');
    /// 3. se contiene un byte NUL: `Utf8` (file binario: non si indovina);
    /// 4. se e' UTF-8 valido, ammettendo una sequenza multibyte INCOMPLETA in coda (il campione e' tagliato): `Utf8`;
    /// 5. altrimenti la code page `windows-1252`.
    ///
    /// Un campione vuoto o solo ASCII e' `Utf8`.
    pub fn detect(sample: &[u8]) -> Encoding {
        // 0. il BOM UTF-32 (`FF FE 00 00` e' anche un BOM UTF-16 LE seguito da un NUL: vince UTF-32, come in Python)
        if sample.starts_with(&[0xFF, 0xFE, 0x00, 0x00]) {
            return Encoding::Utf32Le;
        }
        if sample.starts_with(&[0x00, 0x00, 0xFE, 0xFF]) {
            return Encoding::Utf32Be;
        }
        // 1. il BOM UTF-16 all'inizio del campione
        if sample.starts_with(&[0xFF, 0xFE]) {
            return Encoding::Utf16Le;
        }
        if sample.starts_with(&[0xFE, 0xFF]) {
            return Encoding::Utf16Be;
        }
        // 2. senza BOM, con almeno 8 byte: n = len / 2 unita' intere; z_pari = quanti byte 0 stanno
        //    in posizione pari fra i primi 2n byte, z_dispari = in posizione dispari. In little endian
        //    lo zero e' il byte ALTO dell'unita' (posizione dispari), in big endian il BASSO (posizione
        //    pari); le due condizioni non possono valere insieme
        if sample.len() >= 8 {
            let n = sample.len() / 2;
            let (mut z_pari, mut z_dispari) = (0usize, 0usize);
            for (i, &b) in sample.iter().take(2 * n).enumerate() {
                if b == 0 {
                    if i.is_multiple_of(2) {
                        z_pari += 1;
                    } else {
                        z_dispari += 1;
                    }
                }
            }
            if 2 * z_dispari >= n && 20 * z_pari <= z_dispari {
                return Encoding::Utf16Le;
            }
            if 2 * z_pari >= n && 20 * z_dispari <= z_pari {
                return Encoding::Utf16Be;
            }
        }
        // 3. un byte NUL: file binario o codifica a 16 bit non riconosciuta sopra, non si indovina
        if sample.contains(&0) {
            return Encoding::Utf8;
        }
        // 4. UTF-8 valido, ammettendo una sequenza multibyte INCOMPLETA in coda (il campione e'
        //    tagliato): con from_utf8 e' l'errore con error_len() = None, quello che dice che la
        //    sequenza sarebbe continuata oltre la fine del campione
        let valido = match std::str::from_utf8(sample) {
            Ok(_) => true,
            Err(e) => e.error_len().is_none(),
        };
        if valido {
            return Encoding::Utf8;
        }
        // 5. altrimenti la code page di ripiego
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

    /// L'unita' a 16 bit che comincia in `i` di `bytes`, nell'endian di questa codifica: `None` se i
    /// byte non bastano o la codifica non e' a 16 bit.
    fn unita(self, bytes: &[u8], i: usize) -> Option<u16> {
        let b = bytes.get(i..i + 2)?;
        Some(match self {
            Encoding::Utf16Le => u16::from_le_bytes([b[0], b[1]]),
            Encoding::Utf16Be => u16::from_be_bytes([b[0], b[1]]),
            _ => return None,
        })
    }

    /// Il valore dell'unita' a 32 bit che comincia all'inizio di `bytes`, nell'endian di questa codifica: `None` se
    /// i byte non bastano (meno di 4) o la codifica non e' a 32 bit.
    fn unita32(self, bytes: &[u8]) -> Option<u32> {
        let b = bytes.get(..4)?;
        let b = [b[0], b[1], b[2], b[3]];
        match self {
            Encoding::Utf32Le => Some(u32::from_le_bytes(b)),
            Encoding::Utf32Be => Some(u32::from_be_bytes(b)),
            _ => None,
        }
    }

    /// Il carattere della coppia di surrogati che comincia in `i` di `bytes` (alto D800-DBFF seguito da
    /// basso DC00-DFFF): `None` se in `i` non comincia una coppia. Vale solo per `Utf16Le` e `Utf16Be`.
    fn coppia(self, bytes: &[u8], i: usize) -> Option<char> {
        let prima = self.unita(bytes, i)?;
        let seconda = self.unita(bytes, i + 2)?;
        if (0xd800..0xdc00).contains(&prima) && (0xdc00..0xe000).contains(&seconda) {
            // la stessa aritmetica della coppia `\u{...}\u{...}` in `lang`
            let n = 0x10000 + (u32::from(prima - 0xd800) << 10) + u32::from(seconda - 0xdc00);
            Some(char::from_u32(n).unwrap_or('\u{fffd}'))
        } else {
            None
        }
    }
}
