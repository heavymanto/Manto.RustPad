//! Lo stile di fine riga di un file: `\n` (LF, Unix), `\r\n` (CRLF, Windows) o `\r` da solo (CR, i vecchi Mac). Il
//! documento e' sempre una sequenza di byte e la fine riga, per le righe, resta l'a-capo della codifica (`Newline`):
//! in LF e CRLF la riga finisce al `\n` (il `\r` prima e' l'ultimo carattere della riga e la vista lo nasconde), in CR
//! la riga finisce al `\r` (`Newline::Cr`). Qui si sceglie lo stile da un campione dell'inizio del file e si dice cosa
//! scrive il tasto Invio, cosi' un file CRLF non diventa un file misto.
//!
//! Il CR da solo c'e' solo nelle codifiche a unita' di un byte (UTF-8, code page, multibyte asiatiche): in UTF-16 e
//! UTF-32 vale come LF.

use crate::encoding::Encoding;
use crate::newline::Newline;

/// Lo stile di fine riga.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum LineEnding {
    /// `\n`.
    #[default]
    Lf,
    /// `\r\n`.
    CrLf,
    /// `\r` da solo.
    Cr,
}

impl LineEnding {
    /// Lo stile del file di cui `sample` e' l'inizio (di norma i primi 64 KiB), letto con la codifica `enc`:
    /// - nessun `\n` e almeno un `\r` (in una codifica a un byte): `Cr`;
    /// - piu' `\r\n` che `\n` da soli: `CrLf`;
    /// - altrimenti `Lf` (anche senza nessuna fine riga).
    ///
    /// Un `\r` all'ultimo byte del campione non si conta (potrebbe essere il primo di un `\r\n` tagliato).
    pub fn detect(sample: &[u8], enc: Encoding) -> LineEnding {
        let u = enc.unit();
        let lf = enc.newline().bytes().to_vec();
        let Some(cr) = enc.encode('\r') else {
            return LineEnding::Lf;
        };
        let (mut crlf, mut lf_soli, mut cr_soli) = (0u64, 0u64, 0u64);
        let mut i = 0;
        // le unita' intere, allineate; un `\r` nell'ultima unita' del campione non si conta
        while i + u <= sample.len() {
            let unita = &sample[i..i + u];
            if unita == cr {
                if i + 2 * u <= sample.len() {
                    if sample[i + u..i + 2 * u] == lf[..] {
                        crlf += 1;
                        i += 2 * u;
                        continue;
                    }
                    cr_soli += 1;
                }
            } else if unita == lf {
                lf_soli += 1;
            }
            i += u;
        }
        if u == 1 && cr_soli > 0 && lf_soli == 0 && crlf == 0 {
            LineEnding::Cr
        } else if crlf > lf_soli {
            LineEnding::CrLf
        } else {
            LineEnding::Lf
        }
    }

    /// Lo stesso stile, se la codifica lo ammette: il CR da solo non c'e' in UTF-16 e UTF-32 (diventa LF).
    pub fn for_encoding(self, enc: Encoding) -> LineEnding {
        if self == LineEnding::Cr && enc.unit() != 1 {
            LineEnding::Lf
        } else {
            self
        }
    }

    /// La forma dell'a-capo del documento con questa codifica: `Cr` per il CR da solo, altrimenti quella della codifica.
    pub fn newline_for(self, enc: Encoding) -> Newline {
        if self == LineEnding::Cr && enc.unit() == 1 {
            Newline::Cr
        } else {
            enc.newline()
        }
    }

    /// I byte che scrive il tasto Invio: `\n`, `\r\n` o `\r`, nella codifica `enc` (un'unita' per carattere in
    /// UTF-16 e UTF-32).
    pub fn bytes(self, enc: Encoding) -> Vec<u8> {
        let lf = enc.newline().bytes().to_vec();
        let cr = enc.encode('\r').unwrap_or_default();
        match self.for_encoding(enc) {
            LineEnding::Lf => lf,
            LineEnding::CrLf => [cr, lf].concat(),
            LineEnding::Cr => cr,
        }
    }

    /// Il nome breve: `LF`, `CRLF` o `CR`.
    pub fn label(self) -> &'static str {
        match self {
            LineEnding::Lf => "LF",
            LineEnding::CrLf => "CRLF",
            LineEnding::Cr => "CR",
        }
    }
}
