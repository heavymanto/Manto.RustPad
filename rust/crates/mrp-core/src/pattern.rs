//! Ricerche con regole: testo (anche senza distinguere maiuscole), glob (`*` `?` `[...]`) ed espressioni regolari, e le
//! sostituzioni che ne vengono. Le regole sono **per riga**: un'occorrenza non contiene mai un a-capo (come `grep`),
//! cosi' si legge a blocchi di righe intere, senza caricare il file, e con un tempo che cresce col file (il motore
//! `regex` non fa backtracking: nessuna espressione puo' rallentare all'infinito). Le occorrenze vuote si scartano.
//!
//! Le regole sono sui byte del documento come UTF-8: valgono per i documenti UTF-8 (e per testo semplice con
//! maiuscole esatte anche nelle altre codifiche, che passano da `Snapshot::find_all_literal`). Una riga piu' lunga di
//! `MAX_LINE` si legge a pezzi (un'occorrenza a cavallo del taglio puo' sfuggire).

use crate::docsearch::{SearchError, Snapshot};
use memchr::{memchr, memrchr};
use regex::bytes::{Regex, RegexBuilder};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

/// Il tetto di una riga letta in una volta: oltre, la riga si taglia.
pub const MAX_LINE: usize = 16 * 1024 * 1024;
/// Il tetto della dimensione dell'espressione compilata (byte): una regex enorme si rifiuta.
const SIZE_LIMIT: usize = 8 * 1024 * 1024;

/// Come si legge il testo cercato.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Testo semplice: i caratteri sono quelli che si scrivono.
    Text,
    /// Glob: `*` (qualunque cosa nella riga, il meno possibile), `?` (un carattere), `[abc]`, `[a-z]`, `[!abc]`; `\` fa
    /// letterale il carattere dopo.
    Glob,
    /// Espressione regolare (sintassi del crate `regex`), con `$1`, `${nome}` nella sostituzione e `\n` `\t` `\r` `\\`.
    Regex,
}

/// Perche' un testo cercato non si compila.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatternError(pub String);

impl std::fmt::Display for PatternError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for PatternError {}

/// Un testo cercato pronto all'uso.
#[derive(Debug, Clone)]
pub struct Pattern {
    re: Regex,
    mode: Mode,
}

/// Una sostituzione: cosa mettere al posto di un'occorrenza.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edit {
    /// Dove comincia l'occorrenza nel documento.
    pub start: u64,
    /// Quanti byte occupa.
    pub len: u64,
    /// Cosa va al suo posto.
    pub new: Vec<u8>,
}

/// Il glob come espressione regolare.
pub fn glob_to_regex(glob: &str) -> String {
    let mut out = String::new();
    let mut it = glob.chars().peekable();
    while let Some(c) = it.next() {
        match c {
            '*' => out.push_str("[^\\n]*?"),
            '?' => out.push_str("[^\\n]"),
            '\\' => match it.next() {
                Some(n) => out.push_str(&regex::escape(&n.to_string())),
                None => out.push_str("\\\\"),
            },
            '[' => {
                // una classe: fino alla `]` che la chiude (la prima dopo almeno un carattere); senza chiusura la `[` e' letterale
                let resto: Vec<char> = it.clone().collect();
                let mut i = 0;
                let nega = matches!(resto.first(), Some('!') | Some('^'));
                if nega {
                    i = 1;
                }
                let inizio = i;
                while i < resto.len() && (resto[i] != ']' || i == inizio) {
                    i += 1;
                }
                if i >= resto.len() || i == inizio {
                    out.push_str("\\[");
                    continue;
                }
                out.push('[');
                if nega {
                    out.push('^');
                }
                let dentro = &resto[inizio..i];
                for (k, &ch) in dentro.iter().enumerate() {
                    if ch == '-' && k > 0 && k + 1 < dentro.len() {
                        out.push('-');
                    } else if "\\[]^&~-".contains(ch) {
                        out.push('\\');
                        out.push(ch);
                    } else {
                        out.push(ch);
                    }
                }
                out.push(']');
                for _ in 0..=i {
                    it.next();
                }
            }
            altro => out.push_str(&regex::escape(&altro.to_string())),
        }
    }
    out
}

impl Pattern {
    /// Compila `testo` nel modo `mode`; `case` e' vero se le maiuscole contano.
    pub fn new(testo: &str, mode: Mode, case: bool) -> Result<Pattern, PatternError> {
        let sorgente = match mode {
            Mode::Text => regex::escape(testo),
            Mode::Glob => glob_to_regex(testo),
            Mode::Regex => testo.to_string(),
        };
        if sorgente.is_empty() {
            return Err(PatternError("empty".to_string()));
        }
        let re = RegexBuilder::new(&sorgente)
            .case_insensitive(!case)
            .dot_matches_new_line(false)
            .size_limit(SIZE_LIMIT)
            .build()
            .map_err(|e| PatternError(e.to_string()))?;
        Ok(Pattern { re, mode })
    }

    /// Il modo con cui si e' compilato.
    pub fn mode(&self) -> Mode {
        self.mode
    }

    /// Le occorrenze non vuote in `riga` (senza a-capo), come (inizio, fine) relativi alla riga.
    fn in_riga<'a>(&'a self, riga: &'a [u8]) -> impl Iterator<Item = (usize, usize)> + 'a {
        self.re
            .find_iter(riga)
            .filter(|m| m.end() > m.start())
            .map(|m| (m.start(), m.end()))
    }
}

/// La sostituzione: il modello (con `$1` e le sequenze con barra, solo in modo `Regex`) gia' interpretato.
#[derive(Debug, Clone)]
pub struct Replacement {
    modello: Vec<u8>,
    espandi: bool,
}

impl Replacement {
    /// La sostituzione per `testo` nel modo `mode`: in modo `Regex` `\n`, `\t`, `\r` e `\\` diventano i caratteri e `$1`,
    /// `${nome}` i gruppi (`$$` un dollaro); negli altri modi `testo` e' letterale.
    pub fn new(testo: &str, mode: Mode) -> Replacement {
        if mode != Mode::Regex {
            return Replacement {
                modello: testo.as_bytes().to_vec(),
                espandi: false,
            };
        }
        let mut m = String::new();
        let mut it = testo.chars();
        while let Some(c) = it.next() {
            if c == '\\' {
                match it.next() {
                    Some('n') => m.push('\n'),
                    Some('t') => m.push('\t'),
                    Some('r') => m.push('\r'),
                    Some('\\') => m.push('\\'),
                    Some(altro) => {
                        m.push('\\');
                        m.push(altro);
                    }
                    None => m.push('\\'),
                }
            } else {
                m.push(c);
            }
        }
        Replacement {
            modello: m.into_bytes(),
            espandi: true,
        }
    }
}

/// Dove comincia la riga che contiene la posizione `pos` (il primo byte dopo l'a-capo prima di `pos`), guardando
/// indietro al piu' `MAX_LINE` byte.
fn inizio_riga(s: &Snapshot, pos: u64) -> u64 {
    let mut fine = pos;
    let passo = 64 * 1024u64;
    let limite = pos.saturating_sub(MAX_LINE as u64);
    while fine > limite {
        let da = fine.saturating_sub(passo).max(limite);
        let blocco = s.read_range(da, fine - da);
        if let Some(i) = memrchr(b'\n', &blocco) {
            return da + i as u64 + 1;
        }
        fine = da;
    }
    limite
}

/// Dove finisce la riga che contiene `pos` (la posizione del suo a-capo, o la fine), guardando avanti al piu' `MAX_LINE`.
fn fine_riga(s: &Snapshot, pos: u64) -> u64 {
    let len = s.len();
    let mut da = pos;
    let passo = 64 * 1024u64;
    let limite = pos.saturating_add(MAX_LINE as u64).min(len);
    while da < limite {
        let fine = da.saturating_add(passo).min(limite);
        let blocco = s.read_range(da, fine - da);
        if let Some(i) = memchr(b'\n', &blocco) {
            return da + i as u64;
        }
        da = fine;
    }
    limite
}

impl Snapshot {
    /// Il primo inizio di un'occorrenza di `p` in `[from, fine)` e la sua lunghezza (`None` se non c'e'). Per riga: vedi
    /// il modulo.
    pub fn find_pattern_next(
        &self,
        p: &Pattern,
        from: u64,
        cancel: &AtomicBool,
        progress: &AtomicU64,
    ) -> Result<Option<(u64, u64)>, SearchError> {
        let len = self.len();
        if cancel.load(Ordering::Relaxed) {
            return Err(SearchError::Cancelled);
        }
        if from >= len {
            return Ok(None);
        }
        let mut pos = inizio_riga(self, from);
        let blocco_max = self.chunk_size() as u64;
        while pos < len {
            if cancel.load(Ordering::Relaxed) {
                return Err(SearchError::Cancelled);
            }
            // un blocco di righe intere: fino a `blocco_max` byte e fino alla fine della riga
            let tentativo = pos.saturating_add(blocco_max).min(len);
            let fine = if tentativo >= len {
                len
            } else {
                (fine_riga(self, tentativo) + 1).min(len)
            };
            let dati = self.read_range(pos, fine - pos);
            let mut riga_da = 0usize;
            while riga_da < dati.len() {
                let riga_fine = memchr(b'\n', &dati[riga_da..]).map_or(dati.len(), |i| riga_da + i);
                for (a, b) in p.in_riga(&dati[riga_da..riga_fine]) {
                    let inizio = pos + (riga_da + a) as u64;
                    if inizio >= from {
                        return Ok(Some((inizio, (b - a) as u64)));
                    }
                }
                riga_da = riga_fine + 1;
            }
            progress.fetch_add(fine - pos, Ordering::Relaxed);
            pos = fine;
        }
        Ok(None)
    }

    /// L'ultima occorrenza di `p` che comincia prima di `before`.
    pub fn find_pattern_prev(
        &self,
        p: &Pattern,
        before: u64,
        cancel: &AtomicBool,
        progress: &AtomicU64,
    ) -> Result<Option<(u64, u64)>, SearchError> {
        let len = self.len();
        if cancel.load(Ordering::Relaxed) {
            return Err(SearchError::Cancelled);
        }
        let before = before.min(len);
        if before == 0 {
            return Ok(None);
        }
        // si parte dalla fine della riga che contiene l'ultimo byte prima di `before`
        let mut fine = (fine_riga(self, before - 1) + 1).min(len);
        let blocco_max = self.chunk_size() as u64;
        while fine > 0 {
            if cancel.load(Ordering::Relaxed) {
                return Err(SearchError::Cancelled);
            }
            let tentativo = fine.saturating_sub(blocco_max);
            let inizio = if tentativo == 0 {
                0
            } else {
                inizio_riga(self, tentativo)
            };
            let dati = self.read_range(inizio, fine - inizio);
            // le righe del blocco (inizio, fine senza a-capo), poi dall'ultima alla prima
            let mut righe: Vec<(usize, usize)> = Vec::new();
            let mut da = 0usize;
            while da < dati.len() {
                let a = memchr(b'\n', &dati[da..]).map_or(dati.len(), |i| da + i);
                righe.push((da, a));
                da = a + 1;
            }
            for &(da, a) in righe.iter().rev() {
                let mut ultima = None;
                for (x, y) in p.in_riga(&dati[da..a]) {
                    let assoluto = inizio + (da + x) as u64;
                    if assoluto < before {
                        ultima = Some((assoluto, (y - x) as u64));
                    }
                }
                if ultima.is_some() {
                    return Ok(ultima);
                }
            }
            progress.fetch_add(fine - inizio, Ordering::Relaxed);
            fine = inizio;
        }
        Ok(None)
    }

    /// Tutte le sostituzioni di `p` con `r`, in ordine di documento e senza sovrapposizioni. Si ferma con
    /// `Ok(Err(n))` se ce ne sono piu' di `max` (n = `max + 1`: il conteggio smette li').
    pub fn replace_edits(
        &self,
        p: &Pattern,
        r: &Replacement,
        max: usize,
        cancel: &AtomicBool,
        progress: &AtomicU64,
    ) -> Result<Result<Vec<Edit>, usize>, SearchError> {
        let len = self.len();
        let mut edits: Vec<Edit> = Vec::new();
        let mut pos = 0u64;
        let blocco_max = self.chunk_size() as u64;
        while pos < len {
            if cancel.load(Ordering::Relaxed) {
                return Err(SearchError::Cancelled);
            }
            let tentativo = pos.saturating_add(blocco_max).min(len);
            let fine = if tentativo >= len {
                len
            } else {
                (fine_riga(self, tentativo) + 1).min(len)
            };
            let dati = self.read_range(pos, fine - pos);
            let mut riga_da = 0usize;
            while riga_da < dati.len() {
                let riga_fine = memchr(b'\n', &dati[riga_da..]).map_or(dati.len(), |i| riga_da + i);
                let riga = &dati[riga_da..riga_fine];
                if r.espandi {
                    for caps in p.re.captures_iter(riga) {
                        let m = caps.get(0).map_or(0..0, |m| m.range());
                        if m.is_empty() {
                            continue;
                        }
                        let mut nuovo = Vec::new();
                        caps.expand(&r.modello, &mut nuovo);
                        edits.push(Edit {
                            start: pos + (riga_da + m.start) as u64,
                            len: m.len() as u64,
                            new: nuovo,
                        });
                    }
                } else {
                    for (a, b) in p.in_riga(riga) {
                        edits.push(Edit {
                            start: pos + (riga_da + a) as u64,
                            len: (b - a) as u64,
                            new: r.modello.clone(),
                        });
                    }
                }
                if edits.len() > max {
                    return Ok(Err(edits.len()));
                }
                riga_da = riga_fine + 1;
            }
            progress.fetch_add(fine - pos, Ordering::Relaxed);
            pos = fine;
        }
        Ok(Ok(edits))
    }

    /// Quante occorrenze di `p` ci sono, al piu' `max` (oltre, restituisce `max + 1`).
    pub fn count_pattern(
        &self,
        p: &Pattern,
        max: u64,
        cancel: &AtomicBool,
        progress: &AtomicU64,
    ) -> Result<u64, SearchError> {
        let len = self.len();
        let mut totale = 0u64;
        let mut pos = 0u64;
        let blocco_max = self.chunk_size() as u64;
        while pos < len {
            if cancel.load(Ordering::Relaxed) {
                return Err(SearchError::Cancelled);
            }
            let tentativo = pos.saturating_add(blocco_max).min(len);
            let fine = if tentativo >= len {
                len
            } else {
                (fine_riga(self, tentativo) + 1).min(len)
            };
            let dati = self.read_range(pos, fine - pos);
            for riga in dati.split(|&b| b == b'\n') {
                totale += p.in_riga(riga).count() as u64;
                if totale > max {
                    return Ok(max + 1);
                }
            }
            progress.fetch_add(fine - pos, Ordering::Relaxed);
            pos = fine;
        }
        Ok(totale)
    }
}
