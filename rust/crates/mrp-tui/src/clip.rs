//! Appunti verso il terminale: base64 (RFC 4648) e la sequenza OSC 52 che chiede al terminale di mettere il testo
//! negli appunti di sistema. Funzioni pure, senza dipendenze.

/// Byte massimi che si mandano al terminale con OSC 52 (in base64 diventano circa 100.000 caratteri: oltre, molti
/// terminali tagliano o scartano la sequenza).
pub const OSC52_LIMIT: usize = 75_000;

/// Il carattere di una cifra base64 (6 bit), nell'alfabeto dell'RFC 4648: `A..Z`, `a..z`, `0..9`, `+` e `/`.
fn cifra(v: u8) -> char {
    match v {
        0..=25 => char::from(b'A' + v),
        26..=51 => char::from(b'a' + (v - 26)),
        52..=61 => char::from(b'0' + (v - 52)),
        62 => '+',
        _ => '/',
    }
}

/// Base64 standard dell'RFC 4648 (alfabeto `A-Za-z0-9+/`, riempimento con `=`): ogni gruppo di tre byte
/// diventa quattro cifre, con uno o due `=` quando i byte del gruppo erano meno di tre.
pub fn base64(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for blocco in data.chunks(3) {
        // Il gruppo come 24 bit: i byte che mancano valgono zero, ma le loro cifre non escono (diventano `=`).
        let b = [
            *blocco.first().unwrap_or(&0),
            *blocco.get(1).unwrap_or(&0),
            *blocco.get(2).unwrap_or(&0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        out.push(cifra(((n >> 18) & 0x3f) as u8));
        out.push(cifra(((n >> 12) & 0x3f) as u8));
        out.push(if blocco.len() > 1 {
            cifra(((n >> 6) & 0x3f) as u8)
        } else {
            '='
        });
        out.push(if blocco.len() > 2 {
            cifra((n & 0x3f) as u8)
        } else {
            '='
        });
    }
    out
}

/// La sequenza OSC 52 per gli appunti: `ESC ] 52 ; c ; <base64> BEL`. `None` se `data` supera `OSC52_LIMIT`
/// byte (i terminali la taglierebbero: gli appunti interni continuano comunque a funzionare).
pub fn osc52(data: &[u8]) -> Option<String> {
    if data.len() > OSC52_LIMIT {
        return None;
    }
    Some(format!("\x1b]52;c;{}\x07", base64(data)))
}
