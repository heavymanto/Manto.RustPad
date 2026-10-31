//! Le codifiche multibyte dell'Asia orientale compatibili con l'ASCII: Shift_JIS (Windows-31J), EUC-JP, GBK, GB18030,
//! Big5 ed EUC-KR (UHC). Un byte sotto 0x80 e' sempre un carattere ASCII (anche `\n`: l'a-capo e' il byte 0x0A, che non
//! compare mai dentro un carattere multibyte); un carattere sono 1-4 byte. Le tabelle sono quelle di `encoding_rs`
//! (WHATWG); qui ci sono solo le regole che servono a un editor: dove finisce un carattere, come si torna indietro, come
//! si scrive un carattere digitato.
//!
//! Tornare indietro e' il punto difficile: in Shift_JIS, GBK, Big5 ed EUC-KR il secondo byte di un carattere puo' avere
//! lo stesso valore di un carattere ASCII o del primo byte di un altro (per esempio il secondo byte di "表" e' 0x5C, la
//! barra rovesciata), quindi guardando solo gli ultimi byte non si sa dove comincia l'ultimo carattere. Si riparte dal
//! piu' vicino byte che NON puo' essere un secondo byte (`safe_below`: un byte piu' piccolo e' sempre un carattere da
//! solo, quindi il carattere dopo e' un inizio sicuro) e si va avanti con le regole di `first_char`. Se nei byte
//! guardati non c'e' un byte cosi' (una riga lunghissima di soli ideogrammi) si assume che il primo byte guardato sia
//! un inizio: nel caso peggiore il cursore va indietro di un byte di troppo o di meno su quella riga.
//!
//! Limiti: una sequenza Big5 che `encoding_rs` decodifica in DUE caratteri (le quattro di Big5-HKSCS con la diacritica
//! combinante, 0x8862, 0x8864, 0x88A3, 0x88A5) vale qui il solo primo; la codifica non e' stateful, quindi ISO-2022-JP e
//! HZ non ci sono.

use encoding_rs::{EncoderResult, Encoding as Rs, BIG5, EUC_JP, EUC_KR, GB18030, GBK, SHIFT_JIS};

/// La famiglia di regole di lunghezza di un carattere.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    ShiftJis,
    EucJp,
    Gb,
    Big5,
    EucKr,
}

/// Una codifica multibyte.
pub struct Multi {
    /// Nome canonico (minuscolo).
    pub name: &'static str,
    /// Altri nomi accettati (minuscoli).
    pub aliases: &'static [&'static str],
    rs: &'static Rs,
    kind: Kind,
    /// Un byte piu' piccolo di questo e' sempre un carattere da solo (non puo' essere il secondo byte di un altro).
    safe_below: u8,
}

/// Tutte le codifiche multibyte note.
pub static MULTI: [Multi; 6] = [
    Multi {
        name: "shift-jis",
        aliases: &["shiftjis", "sjis", "windows-31j", "cp932", "932", "ms932"],
        rs: SHIFT_JIS,
        kind: Kind::ShiftJis,
        safe_below: 0x40,
    },
    Multi {
        name: "euc-jp",
        aliases: &["eucjp"],
        rs: EUC_JP,
        kind: Kind::EucJp,
        safe_below: 0x80,
    },
    Multi {
        name: "gbk",
        aliases: &["cp936", "936", "gb2312"],
        rs: GBK,
        kind: Kind::Gb,
        safe_below: 0x30,
    },
    Multi {
        name: "gb18030",
        aliases: &[],
        rs: GB18030,
        kind: Kind::Gb,
        safe_below: 0x30,
    },
    Multi {
        name: "big5",
        aliases: &["big5-hkscs", "cp950", "950"],
        rs: BIG5,
        kind: Kind::Big5,
        safe_below: 0x40,
    },
    Multi {
        name: "euc-kr",
        aliases: &["euckr", "cp949", "949", "uhc", "ks-c-5601-1987"],
        rs: EUC_KR,
        kind: Kind::EucKr,
        safe_below: 0x41,
    },
];

/// Quanti byte occupa, per la struttura, il carattere che comincia con `b[0]` (`b` non vuoto, `b[0] >= 0x80`): 1 se il
/// byte non apre una sequenza. Puo' essere piu' di `b.len()` (sequenza troncata).
fn struct_len(kind: Kind, b: &[u8]) -> usize {
    let b0 = b[0];
    match kind {
        Kind::ShiftJis => match b0 {
            0x81..=0x9F | 0xE0..=0xFC => 2,
            _ => 1,
        },
        Kind::EucJp => match b0 {
            0x8E | 0xA1..=0xFE => 2,
            0x8F => 3,
            _ => 1,
        },
        Kind::Gb => match b0 {
            // il secondo byte 0x30-0x39 apre la forma di 4 byte
            0x81..=0xFE if b.get(1).is_some_and(|x| (0x30..=0x39).contains(x)) => 4,
            0x81..=0xFE => 2,
            _ => 1,
        },
        Kind::Big5 | Kind::EucKr => match b0 {
            0x81..=0xFE => 2,
            _ => 1,
        },
    }
}

/// Il primo carattere di `bytes` e quanti byte occupa, `None` se `bytes` e' vuoto. Un byte sotto 0x80 e' ASCII; una
/// sequenza che la codifica non definisce, o troncata, e' `('\u{fffd}', 1)`: un byte non valido e' un carattere tutto
/// suo (il byte dopo si rilegge da capo).
pub fn first_char(m: &Multi, bytes: &[u8]) -> Option<(char, usize)> {
    let &b0 = bytes.first()?;
    if b0 < 0x80 {
        return Some((b0 as char, 1));
    }
    let n = struct_len(m.kind, bytes);
    if n > bytes.len() {
        return Some(('\u{fffd}', 1));
    }
    let mut dec = m.rs.new_decoder_without_bom_handling();
    let mut out = [0u8; 16];
    let (res, _, scritti) = dec.decode_to_utf8_without_replacement(&bytes[..n], &mut out, true);
    if !matches!(res, encoding_rs::DecoderResult::InputEmpty) {
        return Some(('\u{fffd}', 1));
    }
    // i caratteri sono UTF-8 valido: il primo e' quello che serve
    let c = std::str::from_utf8(&out[..scritti])
        .ok()
        .and_then(|s| s.chars().next());
    Some((c.unwrap_or('\u{fffd}'), if c.is_some() { n } else { 1 }))
}

/// I byte che scrivono `c`, `None` se la codifica non lo rappresenta (U+FFFD non si scrive mai). L'ASCII e' il suo byte.
pub fn encode(m: &Multi, c: char) -> Option<Vec<u8>> {
    if c == '\u{fffd}' {
        return None;
    }
    if c.is_ascii() {
        return Some(vec![c as u8]);
    }
    let mut buf = [0u8; 4];
    let s: &str = c.encode_utf8(&mut buf);
    let mut enc = m.rs.new_encoder();
    let mut out = [0u8; 16];
    let (res, _, scritti) = enc.encode_from_utf8_without_replacement(s, &mut out, true);
    match res {
        EncoderResult::InputEmpty => Some(out[..scritti].to_vec()),
        _ => None,
    }
}

/// Dove comincia il carattere che contiene `ctx[i]` (`i < ctx.len()`), dove `ctx` parte da un punto che si assume essere
/// un inizio di carattere SE nei byte non c'e' un byte sicuro prima di `i` (vedi il commento del modulo).
pub fn char_start(m: &Multi, ctx: &[u8], i: usize) -> usize {
    if ctx[i] < m.safe_below {
        return i;
    }
    // il primo inizio sicuro: subito dopo l'ultimo byte sicuro prima di `i`, oppure l'inizio di `ctx`
    let mut pos = ctx[..i]
        .iter()
        .rposition(|&b| b < m.safe_below)
        .map_or(0, |p| p + 1);
    loop {
        let n = first_char(m, &ctx[pos..]).map_or(1, |(_, n)| n);
        if pos + n > i {
            return pos;
        }
        pos += n;
    }
}

/// Quanti byte occupa l'ULTIMO carattere di `bytes` (0 se e' vuoto); `bytes` finisce a un confine di carattere.
pub fn last_char_len(m: &Multi, bytes: &[u8]) -> usize {
    if bytes.is_empty() {
        return 0;
    }
    bytes.len() - char_start(m, bytes, bytes.len() - 1)
}

/// Quanti byte guardare indietro per trovare un inizio sicuro (vedi il commento del modulo).
pub const BACK_WINDOW: usize = 1024;
