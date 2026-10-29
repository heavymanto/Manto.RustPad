//! Le lingue del prodotto (stessa tecnica di Manto.FreeDisk, vedi `docs/DESIGN-I18N.md` li'): l'inglese e' incorporato
//! nel binario, ogni altra lingua e' un file JSON (un file per lingua, il nome del file senza estensione e' il codice)
//! che la community puo' aggiungere senza toccare codice. Niente dipendenze: il lettore e' interno.
//!
//! Formato: un oggetto JSON piatto chiave -> valore, tutte e due stringhe. Le chiavi che iniziano con `_` sono meta:
//! `_name` e' il nome della lingua nel menu, `_ai` dichiara una traduzione generata da AI. La degradazione e' per
//! chiave: attiva -> inglese incorporato -> la chiave stessa (un buco si vede subito).
//!
//! Segnaposti: `{}` nelle stringhe, riempiti in ordine da `Catalog::tf`; mai `format!` sulle traduzioni.

use crate::source::FileSource;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Il vocabolario inglese, incorporato: il fallback garantito e la referenza dei traduttori.
const EN_JSON: &str = include_str!("../lang/en.json");

/// Una lingua: codice, nome nel menu (`_name`, il codice se manca), vero se la traduzione e' generata da AI (`_ai`)
/// e il dizionario.
#[derive(Clone, Debug)]
pub struct Lang {
    pub code: String,
    pub name: String,
    pub ai: bool,
    dict: HashMap<String, String>,
}

impl Lang {
    /// L'inglese incorporato. Se `en.json` non si leggesse (non puo': lo prova un test) restituisce una lingua vuota
    /// con codice `en`: mai un panico.
    pub fn english() -> Lang {
        Lang::from_json_str("en", EN_JSON).unwrap_or_else(|_| Lang {
            code: "en".to_string(),
            name: "en".to_string(),
            ai: false,
            dict: HashMap::new(),
        })
    }

    /// Una lingua dal suo testo JSON; il codice lo dichiara chi chiama. `Err` con un messaggio inglese che dice la
    /// riga (`line N: ...`) se il testo non e' un oggetto piatto di stringhe.
    pub fn from_json_str(code: &str, text: &str) -> Result<Lang, String> {
        let dict = parse_flat(text)?;
        let name = dict
            .get("_name")
            .cloned()
            .unwrap_or_else(|| code.to_string());
        let ai = dict.contains_key("_ai");
        Ok(Lang {
            code: code.to_string(),
            name,
            ai,
            dict,
        })
    }

    /// Una lingua dal file: il codice e' il nome del file senza estensione.
    pub fn from_file(path: &Path) -> Result<Lang, String> {
        let code = path.file_stem().and_then(|s| s.to_str()).unwrap_or("?");
        // il file si mappa in memoria, non si copia (regola del progetto: mai un file letto intero)
        let src = FileSource::open(path).map_err(|e| format!("{}: {}", path.display(), e))?;
        let text =
            std::str::from_utf8(src.bytes()).map_err(|e| format!("{}: {}", path.display(), e))?;
        Lang::from_json_str(code, text)
    }

    /// Il valore di una chiave, meta comprese.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.dict.get(key).map(|s| s.as_str())
    }

    /// Le chiavi del vocabolario (meta escluse), ordinate: serve al gate di parita' con l'inglese.
    pub fn keys(&self) -> Vec<&str> {
        let mut v: Vec<&str> = self
            .dict
            .keys()
            .filter(|k| !k.starts_with('_'))
            .map(|k| k.as_str())
            .collect();
        v.sort();
        v
    }
}

/// La lingua attiva con il fallback inglese incorporato.
#[derive(Clone, Debug)]
pub struct Catalog {
    pub active: Lang,
    pub fallback: Lang,
}

impl Catalog {
    /// Inglese attivo, inglese di riserva.
    pub fn english() -> Catalog {
        let en = Lang::english();
        Catalog {
            fallback: en.clone(),
            active: en,
        }
    }

    /// Una lingua attiva qualsiasi con l'inglese incorporato di riserva.
    pub fn with(active: Lang) -> Catalog {
        Catalog {
            fallback: Lang::english(),
            active,
        }
    }

    /// Il codice della lingua attiva.
    pub fn code(&self) -> &str {
        &self.active.code
    }

    /// Una stringa: attiva, poi inglese, poi la chiave stessa.
    pub fn t<'a>(&'a self, key: &'a str) -> &'a str {
        self.active
            .get(key)
            .or_else(|| self.fallback.get(key))
            .unwrap_or(key)
    }

    /// La stringa con gli argomenti al posto dei `{}`, in ordine. Un segnaposto senza argomento resta vuoto, un
    /// argomento in piu' si ignora; non puo' andare in panico.
    pub fn tf(&self, key: &str, args: &[&str]) -> String {
        let pattern = self.t(key);
        let mut out = String::with_capacity(pattern.len());
        let mut i = 0usize;
        for (n, part) in pattern.split("{}").enumerate() {
            if n > 0 {
                if let Some(a) = args.get(i) {
                    out.push_str(a);
                }
                i += 1;
            }
            out.push_str(part);
        }
        out
    }
}

/// Cartella delle lingue del profilo utente: `%APPDATA%\mrp\lang` su Windows, `~/.config/mrp/lang` altrove.
pub fn profile_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        std::env::var_os("APPDATA").map(|d| PathBuf::from(d).join("mrp").join("lang"))
    }
    #[cfg(not(windows))]
    {
        std::env::var_os("HOME").map(|d| PathBuf::from(d).join(".config").join("mrp").join("lang"))
    }
}

/// Le lingue disponibili: l'inglese incorporato in testa, piu' i file `*.json` della cartella (ordinati per codice).
/// Un file con il codice di uno incorporato lo sostituisce. Un file illeggibile non e' un errore: la lingua non
/// compare e il nome del file va nell'elenco restituito per secondo.
pub fn registry_in(dir: Option<&Path>) -> (Vec<Lang>, Vec<String>) {
    let mut langs = vec![Lang::english()];
    let mut unreadable = Vec::new();
    if let Some(dir) = dir {
        if let Ok(entries) = std::fs::read_dir(dir) {
            let mut found = Vec::new();
            for e in entries.flatten() {
                let path = e.path();
                if path.extension().and_then(|x| x.to_str()) != Some("json") {
                    continue;
                }
                match Lang::from_file(&path) {
                    Ok(l) => found.push(l),
                    Err(_) => unreadable.push(
                        path.file_name()
                            .map(|n| n.to_string_lossy().into_owned())
                            .unwrap_or_default(),
                    ),
                }
            }
            found.sort_by(|a, b| a.code.cmp(&b.code));
            for l in found {
                if let Some(slot) = langs.iter_mut().find(|x| x.code == l.code) {
                    *slot = l;
                } else {
                    langs.push(l);
                }
            }
        }
    }
    (langs, unreadable)
}

/// La cartella delle lingue accanto all'eseguibile in esecuzione (modo portatile): se c'e', vince sul profilo.
fn local_dir() -> Option<PathBuf> {
    std::env::current_exe().ok().and_then(|p| {
        let dir = p.parent()?.join("lang");
        if dir.is_dir() {
            Some(dir)
        } else {
            None
        }
    })
}

/// Come `registry_in`, scegliendo la cartella: `lang/` accanto all'eseguibile se esiste (modo portatile), altrimenti
/// il profilo.
pub fn registry() -> (Vec<Lang>, Vec<String>) {
    if let Some(dir) = local_dir() {
        return registry_in(Some(&dir));
    }
    registry_in(profile_dir().as_deref())
}

// ---------------------------------------------------------------------------
// Il lettore: un oggetto JSON piatto di stringhe, niente di piu'. Severo dove serve (cio' che non e' una stringa si
// rifiuta, la roba dopo l'oggetto pure) e tollerante dove il mondo lo e' (BOM, a-capo CRLF, caratteri non ASCII
// scritti raw o come escape \uXXXX).
// ---------------------------------------------------------------------------

/// Toglie il BOM che gli editor di Windows a volte mettono: il file della lingua si deve poter salvare con
/// qualunque editor ragionevole. Gli errori dicono la riga, in inglese (`line N: ...`).
fn parse_flat(text: &str) -> Result<HashMap<String, String>, String> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut p = P {
        s: text,
        i: 0,
        line: 1,
    };
    p.ws();
    p.expect('{')?;
    let mut dict = HashMap::new();
    loop {
        p.ws();
        if p.peek() == Some('}') {
            p.bump();
            break;
        }
        let key = p.string()?;
        p.ws();
        p.expect(':')?;
        p.ws();
        let val = p.string()?;
        dict.insert(key, val);
        p.ws();
        match p.bump() {
            Some(',') => {
                // la virgola finale non si perdona: la coppia dopo manca
                p.ws();
                if p.peek() == Some('}') {
                    return Err(p.err("trailing comma: the pair after it is missing"));
                }
            }
            Some('}') => break,
            Some(c) => {
                return Err(p.err(&format!("expected ',' or '}}', found '{}'", c)));
            }
            None => return Err(p.err("the object ends before '}'")),
        }
    }
    p.ws();
    if p.i != p.s.len() {
        return Err(p.err("garbage after the closing brace"));
    }
    Ok(dict)
}

/// Il cursore sul testo: posizione (byte) e riga, per gli errori che indicano dove il file smette di farsi capire.
struct P<'a> {
    s: &'a str,
    i: usize,
    line: usize,
}

impl<'a> P<'a> {
    fn peek(&self) -> Option<char> {
        self.s[self.i..].chars().next()
    }

    fn bump(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.i += c.len_utf8();
        if c == '\n' {
            self.line += 1;
        }
        Some(c)
    }

    /// Gli spazi bianchi fra i pezzi (a-capo compreso, LF e CRLF).
    fn ws(&mut self) {
        while matches!(self.peek(), Some(' ' | '\t' | '\r' | '\n')) {
            self.bump();
        }
    }

    fn err(&self, msg: &str) -> String {
        format!("line {}: {}", self.line, msg)
    }

    fn expect(&mut self, c: char) -> Result<(), String> {
        match self.bump() {
            Some(x) if x == c => Ok(()),
            Some(x) => Err(self.err(&format!("expected '{}', found '{}'", c, x))),
            None => Err(self.err(&format!("expected '{}', the file ends", c))),
        }
    }

    /// Una stringa JSON fra doppi apici, con tutti gli escape standard (le coppie surrogate \uXXXX\uXXXX incluse:
    /// servono per i caratteri fuori dal BMP).
    fn string(&mut self) -> Result<String, String> {
        self.expect('"')?;
        let mut out = String::new();
        loop {
            match self.bump() {
                None => return Err(self.err("unterminated string")),
                Some('"') => return Ok(out),
                Some('\\') => match self.bump() {
                    Some('"') => out.push('"'),
                    Some('\\') => out.push('\\'),
                    Some('/') => out.push('/'),
                    Some('b') => out.push('\u{8}'),
                    Some('f') => out.push('\u{c}'),
                    Some('n') => out.push('\n'),
                    Some('r') => out.push('\r'),
                    Some('t') => out.push('\t'),
                    Some('u') => {
                        let cp = self.hex4()?;
                        let c = if (0xd800..0xdc00).contains(&cp) {
                            // alta senza la bassa non e' un carattere: la coppia si completa o si sostituisce
                            if self.peek() == Some('\\') {
                                self.bump();
                                if self.peek() == Some('u') {
                                    self.bump();
                                    let lo = self.hex4()?;
                                    if (0xdc00..0xe000).contains(&lo) {
                                        let n = 0x10000 + ((cp - 0xd800) << 10) + (lo - 0xdc00);
                                        char::from_u32(n).unwrap_or('\u{fffd}')
                                    } else {
                                        '\u{fffd}'
                                    }
                                } else {
                                    '\u{fffd}'
                                }
                            } else {
                                '\u{fffd}'
                            }
                        } else {
                            char::from_u32(cp).unwrap_or('\u{fffd}')
                        };
                        out.push(c);
                    }
                    Some(c) => {
                        return Err(self.err(&format!("unknown escape '\\{}'", c)));
                    }
                    None => return Err(self.err("escape at end of file")),
                },
                Some(c) if (c as u32) < 0x20 => {
                    return Err(self.err("control character inside a string"));
                }
                Some(c) => out.push(c),
            }
        }
    }

    /// Le quattro cifre di \uXXXX.
    fn hex4(&mut self) -> Result<u32, String> {
        let mut v = 0u32;
        for _ in 0..4 {
            let c = self
                .bump()
                .ok_or_else(|| self.err("\\u needs four digits, the file ends"))?;
            let d = c
                .to_digit(16)
                .ok_or_else(|| self.err(&format!("'{}' is not hexadecimal", c)))?;
            v = v * 16 + d;
        }
        Ok(v)
    }
}
