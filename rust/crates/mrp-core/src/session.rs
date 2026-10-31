//! La sessione su disco: di ogni scheda aperta si ricorda il file, dove stava il cursore e, se ci sono modifiche non
//! salvate, le modifiche stesse (`EditState`: i pezzi della tabella e il buffer delle aggiunte). Il file originale non si
//! copia mai: la sessione pesa quanto le modifiche, non quanto il documento. Il formato e' binario, little endian, con
//! un controllo (FNV-1a a 64 bit) in coda: un file troncato o alterato si riconosce e non si usa.
//!
//! Scheda: `MRPS`, versione (1), percorso, cursore, flag (bit 0: modalita' modifica), codifica (nome, vuoto se non
//! imposta), presenza dello stato; se c'e': lunghezza e data dell'originale, buffer delle aggiunte, pezzi.
//! L'elenco delle schede (`Manifest`) e' un testo: `mrp-session 1`, `active N`, e una riga `tab NOME` per scheda.

use crate::edit::{Buf, Piece};

const MAGIC: &[u8; 4] = b"MRPS";
const VERSIONE: u8 = 1;
const INTESTAZIONE: &str = "mrp-session 1";

/// Le modifiche non salvate di un documento, abbastanza per rifarle sopra il file originale.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditState {
    /// Lunghezza del file originale quando si e' cominciato a modificarlo.
    pub original_len: u64,
    /// Data di modifica dell'originale (secondi e nanosecondi dall'epoca Unix); `None` se non si sapeva.
    pub modified: Option<(i64, u32)>,
    /// Il buffer delle aggiunte.
    pub add: Vec<u8>,
    /// I pezzi in ordine di documento.
    pub pieces: Vec<Piece>,
}

/// Una scheda della sessione.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabRecord {
    pub path: String,
    pub cursor: u64,
    pub edit: bool,
    /// Il nome della codifica scelta (quello di `Encoding::name`); vuoto se non si e' imposta.
    pub encoding: String,
    pub state: Option<EditState>,
}

/// L'elenco delle schede: i nomi dei file di scheda, in ordine, e quale era avanti.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    pub active: usize,
    pub tabs: Vec<String>,
}

fn fnv(dati: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in dati {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

fn u64_in(v: &mut Vec<u8>, x: u64) {
    v.extend_from_slice(&x.to_le_bytes());
}

fn bytes_in(v: &mut Vec<u8>, b: &[u8]) {
    u64_in(v, b.len() as u64);
    v.extend_from_slice(b);
}

/// La scheda come byte.
pub fn encode(t: &TabRecord) -> Vec<u8> {
    let mut v = Vec::new();
    v.extend_from_slice(MAGIC);
    v.push(VERSIONE);
    bytes_in(&mut v, t.path.as_bytes());
    u64_in(&mut v, t.cursor);
    v.push(u8::from(t.edit));
    bytes_in(&mut v, t.encoding.as_bytes());
    match &t.state {
        None => v.push(0),
        Some(s) => {
            v.push(1);
            u64_in(&mut v, s.original_len);
            match s.modified {
                None => v.push(0),
                Some((sec, nano)) => {
                    v.push(1);
                    u64_in(&mut v, sec as u64);
                    v.extend_from_slice(&nano.to_le_bytes());
                }
            }
            bytes_in(&mut v, &s.add);
            u64_in(&mut v, s.pieces.len() as u64);
            for p in &s.pieces {
                v.push(match p.buf {
                    Buf::Original => 0,
                    Buf::Add => 1,
                });
                u64_in(&mut v, p.start);
                u64_in(&mut v, p.len);
            }
        }
    }
    let h = fnv(&v);
    u64_in(&mut v, h);
    v
}

/// Un lettore di byte con i controlli dei limiti: mai un panico, mai un'allocazione fuori misura.
struct Lettore<'a> {
    dati: &'a [u8],
    pos: usize,
}

impl<'a> Lettore<'a> {
    fn prendi(&mut self, n: usize) -> Result<&'a [u8], &'static str> {
        let fine = self.pos.checked_add(n).ok_or("troncato")?;
        let s = self.dati.get(self.pos..fine).ok_or("troncato")?;
        self.pos = fine;
        Ok(s)
    }

    fn u8(&mut self) -> Result<u8, &'static str> {
        Ok(self.prendi(1)?[0])
    }

    fn u32(&mut self) -> Result<u32, &'static str> {
        let b: [u8; 4] = self.prendi(4)?.try_into().map_err(|_| "troncato")?;
        Ok(u32::from_le_bytes(b))
    }

    fn u64(&mut self) -> Result<u64, &'static str> {
        let b: [u8; 8] = self.prendi(8)?.try_into().map_err(|_| "troncato")?;
        Ok(u64::from_le_bytes(b))
    }

    fn blocco(&mut self) -> Result<&'a [u8], &'static str> {
        // una lunghezza falsa non puo' chiedere memoria: `prendi` guarda i byte che ci sono davvero
        let n = self.u64()?;
        self.prendi(usize::try_from(n).map_err(|_| "troppo grande")?)
    }

    fn testo(&mut self) -> Result<String, &'static str> {
        String::from_utf8(self.blocco()?.to_vec()).map_err(|_| "testo non valido")
    }
}

/// Una scheda dai byte; `Err` col motivo se il file non e' una scheda intera e integra.
pub fn decode(dati: &[u8]) -> Result<TabRecord, &'static str> {
    if dati.len() < MAGIC.len() + 1 + 8 {
        return Err("troppo corto");
    }
    let (corpo, coda) = dati.split_at(dati.len() - 8);
    let atteso = u64::from_le_bytes(coda.try_into().map_err(|_| "troncato")?);
    if fnv(corpo) != atteso {
        return Err("controllo non valido");
    }
    let mut l = Lettore {
        dati: corpo,
        pos: 0,
    };
    if l.prendi(4)? != MAGIC {
        return Err("non e' una scheda");
    }
    if l.u8()? != VERSIONE {
        return Err("versione sconosciuta");
    }
    let path = l.testo()?;
    let cursor = l.u64()?;
    let edit = l.u8()? != 0;
    let encoding = l.testo()?;
    let state = match l.u8()? {
        0 => None,
        1 => {
            let original_len = l.u64()?;
            let modified = match l.u8()? {
                0 => None,
                1 => {
                    let sec = l.u64()? as i64;
                    Some((sec, l.u32()?))
                }
                _ => return Err("data non valida"),
            };
            let add = l.blocco()?.to_vec();
            let n = l.u64()?;
            // ogni pezzo pesa 17 byte: un conteggio che non ci sta nel resto e' falso
            if n > ((corpo.len() - l.pos) / 17) as u64 {
                return Err("troppi pezzi");
            }
            let mut pieces = Vec::with_capacity(n as usize);
            for _ in 0..n {
                let buf = match l.u8()? {
                    0 => Buf::Original,
                    1 => Buf::Add,
                    _ => return Err("pezzo non valido"),
                };
                let start = l.u64()?;
                let len = l.u64()?;
                pieces.push(Piece { buf, start, len });
            }
            Some(EditState {
                original_len,
                modified,
                add,
                pieces,
            })
        }
        _ => return Err("stato non valido"),
    };
    if l.pos != corpo.len() {
        return Err("byte in piu'");
    }
    Ok(TabRecord {
        path,
        cursor,
        edit,
        encoding,
        state,
    })
}

/// L'elenco come testo.
pub fn encode_manifest(m: &Manifest) -> String {
    let mut s = format!("{INTESTAZIONE}\nactive {}\n", m.active);
    for t in &m.tabs {
        s.push_str("tab ");
        s.push_str(t);
        s.push('\n');
    }
    s
}

/// L'elenco dal testo; `Err` se non lo e'. I nomi di scheda non possono contenere separatori di percorso (l'elenco
/// non deve poter puntare fuori dalla cartella della sessione).
pub fn decode_manifest(testo: &str) -> Result<Manifest, &'static str> {
    let mut righe = testo.lines();
    if righe.next() != Some(INTESTAZIONE) {
        return Err("non e' una sessione");
    }
    let active = righe
        .next()
        .and_then(|r| r.strip_prefix("active "))
        .and_then(|n| n.parse::<usize>().ok())
        .ok_or("avanti mancante")?;
    let mut tabs = Vec::new();
    for r in righe {
        let nome = r.strip_prefix("tab ").ok_or("riga sconosciuta")?;
        if nome.is_empty() || nome.contains(['/', '\\', ':']) || nome.starts_with('.') {
            return Err("nome di scheda non valido");
        }
        tabs.push(nome.to_string());
    }
    Ok(Manifest { active, tabs })
}
