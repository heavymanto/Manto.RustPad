//! Modello di visualizzazione: da un `Document` a righe di celle di terminale, indipendente dall'interfaccia (lo usano
//! TUI e GUI). Tabulazioni, caratteri di controllo, byte non UTF-8, caratteri larghi e scorrimento orizzontale sono
//! risolti qui; l'interfaccia disegna soltanto.
//!
//! Modello di cella (la specifica esatta; i test la verificano con un oracolo ingenuo):
//! - la riga si decodifica a caratteri con la codifica di `ViewOptions.encoding`: in UTF-8 un carattere valido,
//!   oppure UN SINGOLO BYTE non valido = U+FFFD largo 1; in una code page a byte singolo un byte = un carattere
//!   della tabella (U+FFFD se la code page non lo definisce);
//! - larghezza: tabulazione fino al prossimo multiplo di `tab_width` (0 = 1); `\r` come ULTIMO byte di una riga che
//!   termina con `\n` -> larghezza 0, invisibile; ogni altro carattere di controllo (`char::is_control`) -> `U+00B7` largo 1;
//!   il resto `UnicodeWidthChar::width(c).unwrap_or(0)`;
//! - finestra = celle `[hscroll, hscroll + cols)`: i caratteri interi si mostrano; un carattere largo (o una
//!   tabulazione) solo in parte dentro la finestra lascia UNO SPAZIO per ogni sua cella dentro la finestra.

use crate::document::Document;
use crate::encoding::Encoding;
use crate::scan::{next_row, Next};
use unicode_width::UnicodeWidthChar;

/// Dimensione dei blocchi di lettura (come `scan`): 64 KiB.
const CHUNK: u64 = 64 * 1024;

/// Opzioni di visualizzazione.
#[derive(Debug, Clone)]
pub struct ViewOptions {
    /// Larghezza della tabulazione in celle (0 si tratta come 1).
    pub tab_width: usize,
    /// Byte massimi esaminati per trovare la fine di una riga (vedi `scan`).
    pub max_scan: u64,
    /// Se vero e il documento e' vuoto o finisce con `\n`, in fondo alla camminata c'e' una riga vuota in piu' dove sta
    /// il cursore in un editor.
    pub show_eof_row: bool,
    /// Come si leggono i byte come caratteri (vedi `encoding`). Predefinita: UTF-8.
    pub encoding: Encoding,
}

impl Default for ViewOptions {
    fn default() -> Self {
        ViewOptions {
            tab_width: 4,
            max_scan: crate::scan::DEFAULT_MAX_SCAN,
            show_eof_row: false,
            encoding: Encoding::Utf8,
        }
    }
}

/// Una riga visuale pronta da disegnare.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    /// Primo byte della riga.
    pub start: u64,
    /// Subito dopo l'ultimo byte della riga, `\n` ESCLUSO (per una riga tagliata e' il punto di taglio).
    pub end: u64,
    /// Le celle della finestra `[hscroll, hscroll + cols)` come testo (mai piu' di `cols` celle).
    pub text: String,
    /// `hscroll > 0` e la riga ha almeno una cella.
    pub more_left: bool,
    /// La riga ha piu' di `hscroll + cols` celle, oppure e' stata tagliata (continua nella riga successiva).
    pub more_right: bool,
    /// Vero se `start` e' l'inizio di una riga vera (la prima riga della camminata: `top == 0` o l'unita'
    /// prima e' un a-capo).
    pub starts_line: bool,
}

/// Al piu' `count` righe a partire da `top` (un inizio di riga visuale), camminando con `scan::next_row`. `top >= len`:
/// nessuna riga, salvo la riga vuota di `show_eof_row` (vedi `ViewOptions`). Per ogni riga si leggono solo i byte che
/// servono a coprire `hscroll + cols` celle (con un limite di sicurezza), mai una riga lunga intera.
pub fn rows_from(
    doc: &Document,
    top: u64,
    count: usize,
    hscroll: usize,
    cols: usize,
    opts: &ViewOptions,
) -> Vec<Row> {
    let len = doc.len();
    let mut rows = Vec::new();
    if count == 0 {
        return rows;
    }
    let tab = opts.tab_width.max(1);
    let mut start = top;
    // l'a-capo del documento e' un'unita' (1 byte in UTF-8 e code page, 2 in UTF-16): le righe vere
    // e la riga vuota di fine file si decidono sull'unita', non sul singolo byte
    let nl = doc.newline();
    let unit = nl.unit() as u64;
    // la prima riga della camminata parte da top: e' una riga vera se e' l'inizio del documento
    // oppure se l'unita' prima e' un a-capo
    let mut starts_line =
        start == 0 || (start >= unit && nl.is_newline(&doc.read_range(start - unit, unit)));
    while rows.len() < count && start < len {
        let (end, prossima, a_capo, tagliata) = fine_riga(doc, start, opts.max_scan);
        // per riga si leggono solo i byte che possono coprire la finestra (limite di sicurezza),
        // mai una riga lunga intera; la fine resta quella vera, dalla scansione
        let disponibili = end - start;
        let limite = hscroll
            .saturating_add(cols)
            .saturating_mul(4)
            .saturating_add(8);
        let leggi = (limite as u64).min(disponibili);
        let troncato = leggi < disponibili;
        let mut testo = String::new();
        let mut colonna = 0usize;
        let mut caratteri = Caratteri::new(doc, start, start + leggi, opts.encoding);
        while let Some((p, c, n)) = caratteri.prossimo() {
            let nascosto = c == '\r' && p + n as u64 == end && a_capo;
            let (w, glifo) = cella(c, colonna, tab, nascosto);
            let (a, b) = (colonna, colonna + w);
            colonna = b;
            if w == 0 {
                // nessuna cella: invisibile (il \r nascosto)
                testo.push_str(&glifo);
            } else if a >= hscroll && b <= hscroll + cols {
                // carattere intero dentro la finestra
                testo.push_str(&glifo);
            } else {
                // dentro solo in parte: uno spazio per ogni cella dentro la finestra
                let da = a.max(hscroll);
                let fino = b.min(hscroll + cols);
                if fino > da {
                    testo.push_str(&" ".repeat(fino - da));
                }
            }
            // lettura troncata: oltre la finestra non serve piu' nulla (more_right e' gia' vero)
            if troncato && colonna > 0 && colonna >= hscroll + cols {
                break;
            }
        }
        rows.push(Row {
            start,
            end,
            text: testo,
            more_left: hscroll > 0 && colonna > 0,
            more_right: tagliata || troncato || colonna > hscroll + cols,
            starts_line,
        });
        match prossima {
            Some(n) => {
                start = n;
                // la riga dopo un a-capo e' una riga vera; quella dopo un taglio no
                starts_line = !tagliata;
            }
            None => break,
        }
    }
    // la riga vuota di fine file, dove l'editor mette il cursore: c'e' se il documento e' vuoto o
    // finisce con un a-capo (lunghezza multipla dell'unita' e ultima unita' = a-capo)
    if opts.show_eof_row
        && rows.len() < count
        && (len == 0
            || (len.is_multiple_of(unit)
                && nl.ends_with_newline(&doc.read_range(len - unit, unit))))
    {
        rows.push(Row {
            start: len,
            end: len,
            text: String::new(),
            more_left: false,
            more_right: false,
            starts_line: true,
        });
    }
    rows
}

/// Colonna di terminale (da 0) del byte `off` nella riga che inizia a `row_start`: somma delle larghezze dei caratteri
/// prima di `off`; `off` a fine riga da' la larghezza totale.
pub fn col_of(doc: &Document, row_start: u64, off: u64, opts: &ViewOptions) -> usize {
    let (end, _, a_capo, _) = fine_riga(doc, row_start, opts.max_scan);
    let off = off.min(end);
    let tab = opts.tab_width.max(1);
    // contano i caratteri che iniziano prima di `off`: l'ultimo puo' estendersi oltre di 3 byte,
    // leggere piu' in la' non serve
    let fine = end.min(off.saturating_add(3));
    let mut caratteri = Caratteri::new(doc, row_start, fine, opts.encoding);
    let mut colonna = 0usize;
    while let Some((p, c, n)) = caratteri.prossimo() {
        if p >= off {
            break;
        }
        let nascosto = c == '\r' && p + n as u64 == end && a_capo;
        let (w, _) = cella(c, colonna, tab, nascosto);
        colonna += w;
    }
    colonna
}

/// Offset del carattere che copre la colonna `col` della riga che inizia a `row_start` (una colonna dentro un
/// carattere largo o una tabulazione e' quel carattere); oltre la fine della riga: la fine della riga (prima del `\n`).
pub fn offset_at_col(doc: &Document, row_start: u64, col: u64, opts: &ViewOptions) -> u64 {
    let (end, _, a_capo, _) = fine_riga(doc, row_start, opts.max_scan);
    let col = usize::try_from(col).unwrap_or(usize::MAX);
    let tab = opts.tab_width.max(1);
    let mut caratteri = Caratteri::new(doc, row_start, end, opts.encoding);
    let mut colonna = 0usize;
    while let Some((p, c, n)) = caratteri.prossimo() {
        let nascosto = c == '\r' && p + n as u64 == end && a_capo;
        let (w, _) = cella(c, colonna, tab, nascosto);
        if w > 0 {
            if col < colonna + w {
                // la colonna sta dentro questo carattere (nel mezzo di uno largo o di una tabulazione)
                return p;
            }
            colonna += w;
        }
    }
    // oltre la fine della riga: la fine della riga, prima dell'a-capo
    end
}

/// Fine della riga che inizia in `start` e come prosegue la camminata:
/// `(fine, prossima, a_capo, tagliata)`. `fine` esclude l'a-capo di chiusura (per una riga tagliata
/// e' il punto di taglio, per l'ultima senza a-capo la fine del documento); `prossima` e' l'inizio
/// della riga visuale successiva (`None`: non c'e'); `a_capo` dice se la riga termina con l'a-capo
/// del documento (serve per il `\r` nascosto); `tagliata` se `max_scan` ha tagliato la riga.
pub(crate) fn fine_riga(
    doc: &Document,
    start: u64,
    max_scan: u64,
) -> (u64, Option<u64>, bool, bool) {
    // l'a-capo del documento e' un'unita' (1 byte in UTF-8 e code page, 2 in UTF-16)
    let nl = doc.newline();
    let unit = nl.unit() as u64;
    match next_row(doc, start, max_scan) {
        // l'a-capo chiude la riga: la fine sta prima dell'unita', la prossima e' una riga vera
        Next::Line(n) => (n - unit, Some(n), true, false),
        // riga tagliata: la fine e' il punto di taglio, la prossima comincia da li'
        Next::Cut(n) => (n, Some(n), false, true),
        // ultima riga: se il documento finisce con un a-capo, la riga finisce prima di quell'unita'
        Next::End => {
            let len = doc.len();
            if len > 0
                && len.is_multiple_of(unit)
                && nl.ends_with_newline(&doc.read_range(len - unit, unit))
            {
                (len - unit, None, true, false)
            } else {
                (len, None, false, false)
            }
        }
    }
}

/// Larghezza in celle e glifo di un carattere: tabulazione fino al prossimo multiplo di `tab`;
/// `\r` nascosto (ultimo byte di una riga che termina con a-capo) largo 0 e invisibile; ogni altro
/// carattere di controllo -> `U+00B7` largo 1; il resto la larghezza Unicode (0 se non definita).
pub(crate) fn cella(c: char, colonna: usize, tab: usize, nascosto: bool) -> (usize, String) {
    if c == '\t' {
        let w = tab - colonna % tab;
        (w, " ".repeat(w))
    } else if nascosto {
        (0, String::new())
    } else if c.is_control() {
        (1, "\u{b7}".to_string())
    } else {
        (c.width().unwrap_or(0), c.to_string())
    }
}

/// I caratteri di un pezzo di riga, letti a blocchi dal documento: in UTF-8 un carattere valido,
/// oppure un singolo byte non valido che vale un carattere `U+FFFD` (i byte non validi non si
/// uniscono); in UTF-16 una o due unita' da 2 byte (una coppia di surrogati e' un carattere solo;
/// surrogati spaiati e byte finale spaiato valgono `U+FFFD`); in una code page a byte singolo un
/// byte = un carattere della tabella. Un carattere multibyte tagliato dal confine del blocco si
/// rilegge col blocco che parte da lui; se invece a tagliarlo e' il limite di lettura (vedi
/// `rows_from`) la lettura si ferma: oltre quel punto non si sa niente e non si spaccia per non
/// valido cio' che non si e' letto.
pub(crate) struct Caratteri<'a> {
    doc: &'a Document,
    /// fino a qui si legge (fine riga o limite del chiamante)
    fine: u64,
    /// blocco corrente: byte del documento in `[blocco, blocco + buf.len())`
    buf: Vec<u8>,
    blocco: u64,
    /// posizione del prossimo carattere, nel documento
    pos: u64,
    /// come si leggono i byte (UTF-8 o una code page a byte singolo)
    encoding: Encoding,
}

impl<'a> Caratteri<'a> {
    pub(crate) fn new(doc: &'a Document, da: u64, fine: u64, encoding: Encoding) -> Caratteri<'a> {
        Caratteri {
            doc,
            fine,
            buf: Vec::new(),
            blocco: da,
            pos: da,
            encoding,
        }
    }

    /// Il prossimo carattere: `(posizione del primo byte, carattere, byte che occupa)`.
    pub(crate) fn prossimo(&mut self) -> Option<(u64, char, usize)> {
        if self.pos >= self.fine {
            return None;
        }
        loop {
            let fine_blocco = self.blocco + self.buf.len() as u64;
            let copre = self.pos >= self.blocco && self.pos < fine_blocco;
            if !copre {
                // il blocco non copre piu' pos (o e' il primo giro): se ne legge uno che parte da li'
                if self.pos >= self.fine {
                    return None;
                }
                let n = CHUNK.min(self.fine - self.pos);
                self.buf = self.doc.read_range(self.pos, n);
                self.blocco = self.pos;
                if self.buf.is_empty() {
                    // difensivo: documento chiuso o fine inattesa
                    return None;
                }
                continue;
            }
            // offset del carattere dentro il blocco: e' un offset di blocco, sempre piccolo
            let i = usize::try_from(self.pos - self.blocco).ok()?;
            let disponibili = self.buf.len() - i;
            // UTF-16 e UTF-32: il carattere sono una o due unita' da 2 byte, o una da 4, decodificate da
            // `first_char` su una finestra di fino a 4 byte. Se i byte disponibili nel blocco sono meno di 4 e
            // il blocco non arriva a fine, il carattere puo' stare a cavallo del confine e si
            // rilegge il blocco che parte da lui (stessa idea dell'UTF-8); il blocco comunque non
            // passa mai fine, quindi una finestra corta vuol dire che e' fine a tagliare il
            // carattere, che vale U+FFFD per i byte che restano
            if self.encoding.unit() > 1 {
                if disponibili < 4 && fine_blocco < self.fine {
                    self.buf.clear();
                    self.blocco = self.pos;
                    continue;
                }
                let (c, n) = self
                    .encoding
                    .first_char(&self.buf[i..i + 4.min(disponibili)])
                    .unwrap_or(('\u{fffd}', 1));
                self.pos += n as u64;
                return Some((self.pos - n as u64, c, n));
            }
            // code page a byte singolo: un byte = un carattere, sempre lungo 1 (nessun confine di
            // blocco da rispettare); il carattere e' quello del byte nella codifica
            if !self.encoding.is_utf8() {
                let c = self
                    .encoding
                    .first_char(&self.buf[i..i + 1])
                    .map_or('\u{fffd}', |(c, _)| c);
                self.pos += 1;
                return Some((self.pos - 1, c, 1));
            }
            // il primo prefisso valido e' il carattere (stessa regola dei test: un carattere
            // valido, oppure un singolo byte non valido)
            for n in 1..=4.min(disponibili) {
                if let Ok(s) = std::str::from_utf8(&self.buf[i..i + n]) {
                    if let Some(c) = s.chars().next() {
                        self.pos += n as u64;
                        return Some((self.pos - n as u64, c, n));
                    }
                }
            }
            // nessun prefisso valido: o il byte e' davvero non valido, o il carattere multibyte
            // cade a meta' sul confine del blocco e si riprova col blocco che parte da lui
            if disponibili < 4 && fine_blocco < self.fine {
                self.buf.clear();
                self.blocco = self.pos;
                continue;
            }
            // davvero non valido (o oltre non si puo' leggere): un byte = un carattere U+FFFD
            self.pos += 1;
            return Some((self.pos - 1, '\u{fffd}', 1));
        }
    }
}
