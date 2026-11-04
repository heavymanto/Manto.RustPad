//! "Insegnare con un esempio": l'utente incolla (o seleziona nel documento) un pezzo di riga che contiene la data che gli interessa e
//! il programma ricava la regola per leggerla: trova la data nell'esempio provando i formati che conosce, prende come marcatore il
//! testo che la precede (il piu' corto che basta), la prova sul file (le prime righe, fino a `SCANSIONE_BYTE`) e dice quante righe
//! legge, da quando a quando e se vanno avanti nel file. Una riga d'esempio con piu' date offre una regola per ciascuna.

use super::tempo::{SourceKind, SCANSIONE_BYTE};
use super::App;
use mrp_core::timemask::{self, TimeMask};

/// I formati con cui si cerca la data in un esempio, dal piu' preciso al meno preciso.
const FORMATI_ESEMPIO: [&str; 10] = [
    "dd/MMM/yyyy:HH:mm:ss",
    "yyyy-MM-dd HH:mm:ss.SSS",
    "yyyy-MM-dd HH:mm:ss,SSS",
    "yyyy-MM-ddTHH:mm:ss",
    "yyyy-MM-dd HH:mm:ss",
    "yyyy/MM/dd HH:mm:ss",
    "dd/MM/yyyy HH:mm:ss",
    "dd/MM/yyyy HH:mm",
    "yyyy-MM-dd HH:mm",
    "yyyy-MM-dd",
];

/// Quanto del testo che precede la data si prova come marcatore, in caratteri (dal piu' corto).
const MARCATORI: [usize; 7] = [1, 2, 3, 4, 6, 8, 12];

/// "Entro quanti caratteri" con cui si legge una data dentro la riga.
const ENTRO: usize = 200;

/// Perche' da un esempio non si e' ricavata nessuna regola.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExampleProblem {
    /// Nell'esempio non c'e' nessuna data in un formato che il programma conosce.
    NoDate,
    /// La data c'e', ma quel testo nel file non si trova (nelle prime righe guardate).
    NotInFile,
}

/// Una regola ricavata da un esempio, con come va sul file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExampleCandidate {
    pub kind: SourceKind,
    pub pattern: String,
    pub within: usize,
    pub skip: usize,
    /// Quante righe la leggono (fra quelle guardate), la prima e l'ultima data che legge (`aaaa-mm-gg hh:mm:ss`) e se vanno avanti
    /// nel file (quasi sempre non decrescenti).
    pub lines: usize,
    pub first: String,
    pub last: String,
    pub ordered: bool,
}

/// Cosa si e' ricavato da un esempio: o il motivo per cui non si e' ricavato niente, o le regole (una per data trovata).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExampleOutcome {
    pub problem: Option<ExampleProblem>,
    pub candidates: Vec<ExampleCandidate>,
}

/// Le date dentro un testo: dove cominciano, dove finiscono e con quale dei `FORMATI_ESEMPIO`. Si cerca da sinistra e ogni data
/// prende il formato piu' preciso che la legge.
fn date_nel_testo(testo: &[u8], maschere: &[TimeMask]) -> Vec<(usize, usize, usize)> {
    let mut v = Vec::new();
    let mut p = 0;
    while p < testo.len() {
        let resto = &testo[p..];
        let trovata = maschere
            .iter()
            .enumerate()
            .find_map(|(i, m)| m.find(resto).filter(|f| f.0 == 0).map(|f| (i, f.1)));
        match trovata {
            Some((i, fine)) => {
                v.push((p, p + fine, i));
                p += fine.max(1);
            }
            None => p += 1,
        }
    }
    v
}

/// Vero se `ago` compare dentro `pagliaio`.
fn contiene(pagliaio: &[u8], ago: &[u8]) -> bool {
    ago.is_empty() || pagliaio.windows(ago.len()).any(|w| w == ago)
}

/// Come va una regola sul file: righe lette, prima e ultima data, e quante coppie di righe vicine vanno avanti.
#[derive(Default)]
struct Prova {
    righe: usize,
    chiavi: Vec<i64>,
    prima: Option<i64>,
    ultima: Option<i64>,
}

impl Prova {
    fn in_ordine(&self) -> bool {
        let coppie = self.chiavi.len().saturating_sub(1);
        let giuste = self.chiavi.windows(2).filter(|w| w[0] <= w[1]).count();
        giuste * 100 >= coppie * 95
    }

    /// Una regola utile per navigare: almeno cinque righe, almeno cinque date diverse e (quasi) sempre in ordine.
    fn utile(&self) -> bool {
        let mut diversi = self.chiavi.clone();
        diversi.sort_unstable();
        diversi.dedup();
        self.righe >= 5 && diversi.len() >= 5 && self.in_ordine()
    }
}

/// Una regola da provare: da quale data dell'esempio viene, il testo che precede (il marcatore), il pattern completo e la maschera.
struct Tentativo {
    data: usize,
    marcatore: String,
    kind: SourceKind,
    pattern: String,
    within: usize,
    maschera: TimeMask,
}

impl App {
    /// Cambia l'esempio scritto nella finestra "Tempo" (non ricava niente: lo fa `dialog_time_find_rule`).
    pub fn dialog_time_set_example(&mut self, testo: &str) {
        if self.dialog == Some(super::DialogKind::Time) {
            self.example_text = testo.to_string();
            self.example_result = None;
        }
    }

    /// L'esempio scritto adesso nella finestra "Tempo".
    pub fn time_example(&self) -> &str {
        &self.example_text
    }

    /// Cosa si e' ricavato dall'esempio, se si e' cercato.
    pub fn time_example_result(&self) -> Option<&ExampleOutcome> {
        self.example_result.as_ref()
    }

    /// Il testo selezionato nel documento (la sua prima riga, al piu' 400 byte), per precompilare l'esempio.
    pub(super) fn esempio_dalla_selezione(&self) -> String {
        let (Some((a, b)), Some(doc)) = (self.selection(), self.doc.as_ref()) else {
            return String::new();
        };
        let dati = doc.read_range(a, (b - a + 1).min(400));
        let testo = String::from_utf8_lossy(&dati);
        testo.lines().next().unwrap_or("").trim_end().to_string()
    }

    /// Ricava le regole dall'esempio (il bottone "Trova la regola"): vedi il modulo.
    pub fn dialog_time_find_rule(&mut self) {
        if self.dialog != Some(super::DialogKind::Time) {
            return;
        }
        let esito = self.ricava_regole(&self.example_text.clone());
        self.example_result = Some(esito);
    }

    /// Mette nel dialogo la regola `i` ricavata dall'esempio (poi OK la applica e la ricorda).
    pub fn dialog_time_use_example(&mut self, i: usize) {
        if self.dialog != Some(super::DialogKind::Time) {
            return;
        }
        let Some(c) = self
            .example_result
            .as_ref()
            .and_then(|r| r.candidates.get(i))
            .cloned()
        else {
            return;
        };
        self.mask_text = [c.pattern, c.within.to_string()];
        self.mask_skip = c.skip;
        self.mask_error.clear();
        self.time_error.clear();
    }

    /// Quale delle regole ricavate e' quella scritta adesso nel dialogo (pattern, entro e occorrenza uguali).
    pub fn time_example_selected(&self) -> Option<usize> {
        let pattern = self.mask_text[0].trim();
        let within = self.mask_text[1].trim().parse::<usize>().ok()?;
        self.example_result
            .as_ref()?
            .candidates
            .iter()
            .position(|c| c.pattern == pattern && c.within == within && c.skip == self.mask_skip)
    }

    fn ricava_regole(&self, esempio: &str) -> ExampleOutcome {
        let esempio = esempio.trim_end();
        let maschere: Vec<TimeMask> = FORMATI_ESEMPIO
            .iter()
            .filter_map(|p| TimeMask::parse(p, 0).ok())
            .collect();
        let testo = esempio.as_bytes();
        let date = date_nel_testo(testo, &maschere);
        if date.is_empty() {
            return ExampleOutcome {
                problem: Some(ExampleProblem::NoDate),
                candidates: Vec::new(),
            };
        }
        // le regole da provare: per ogni data dell'esempio, o l'orario all'inizio della riga (se l'esempio comincia con la data), o un
        // marcatore sempre piu' lungo (il testo che la precede nell'esempio)
        let mut tentativi: Vec<Tentativo> = Vec::new();
        for (n, &(inizio, _, f)) in date.iter().enumerate() {
            let formato = FORMATI_ESEMPIO[f];
            if inizio == 0 && n == 0 {
                if let Ok(m) = TimeMask::parse(formato, 0) {
                    tentativi.push(Tentativo {
                        data: n,
                        marcatore: String::new(),
                        kind: SourceKind::Start,
                        pattern: formato.to_string(),
                        within: 0,
                        maschera: m,
                    });
                }
                continue;
            }
            // il testo che precede la data: i suoi ultimi `k` caratteri, senza apici (romperebbero il pattern) e, se si puo', con
            // qualcosa che non sia solo spazio
            let prima: Vec<char> = esempio[..inizio].chars().collect();
            for &k in &MARCATORI {
                if k > prima.len() {
                    break;
                }
                let marcatore: String = prima[prima.len() - k..].iter().collect();
                if marcatore.contains('\'') || marcatore.trim().is_empty() && prima.len() > k {
                    continue;
                }
                let pattern = format!("'{marcatore}'{formato}");
                if let Ok(m) = TimeMask::parse(&pattern, ENTRO) {
                    tentativi.push(Tentativo {
                        data: n,
                        marcatore,
                        kind: SourceKind::Rule,
                        pattern,
                        within: ENTRO,
                        maschera: m,
                    });
                }
            }
            // senza testo prima (la data e' la seconda dell'esempio, subito dopo la prima): la N-esima data della riga
            if prima.is_empty() {
                if let Ok(m) = TimeMask::parse(formato, ENTRO) {
                    tentativi.push(Tentativo {
                        data: n,
                        marcatore: String::new(),
                        kind: SourceKind::Number(n + 1),
                        pattern: formato.to_string(),
                        within: ENTRO,
                        maschera: m.with_skip(n),
                    });
                }
            }
        }
        // una passata sola sul file per tutte le regole (una riga si prova solo con i marcatori che contiene)
        let mut prove: Vec<Prova> = tentativi.iter().map(|_| Prova::default()).collect();
        let mut righe = 0usize;
        self.scansiona_righe_per(SCANSIONE_BYTE, |riga| {
            righe += 1;
            for (t, p) in tentativi.iter().zip(prove.iter_mut()) {
                if !contiene(riga, t.marcatore.as_bytes()) {
                    continue;
                }
                if let Some((_, _, k)) = t.maschera.find(riga) {
                    p.righe += 1;
                    if p.chiavi.len() < 20_000 {
                        p.chiavi.push(k);
                    }
                    p.prima.get_or_insert(k);
                    p.ultima = Some(k);
                }
            }
            righe < 80_000
        });
        // per ogni data dell'esempio: il marcatore piu' corto che da' una regola utile, altrimenti quello che legge piu' righe
        let mut candidates: Vec<ExampleCandidate> = Vec::new();
        for n in 0..date.len() {
            let mut scelto: Option<usize> = None;
            for (i, _) in tentativi.iter().enumerate().filter(|(_, t)| t.data == n) {
                if prove[i].righe == 0 {
                    continue;
                }
                if prove[i].utile() {
                    scelto = Some(i);
                    break;
                }
                if scelto.is_none_or(|s| prove[i].righe > prove[s].righe) {
                    scelto = Some(i);
                }
            }
            let Some(i) = scelto else {
                continue;
            };
            let (t, p) = (&tentativi[i], &prove[i]);
            let c = ExampleCandidate {
                kind: match &t.kind {
                    SourceKind::Rule => SourceKind::After(t.marcatore.clone()),
                    altro => altro.clone(),
                },
                pattern: t.pattern.clone(),
                within: t.within,
                skip: t.maschera.skip(),
                lines: p.righe,
                first: p.prima.map(|k| timemask::iso(k, true)).unwrap_or_default(),
                last: p.ultima.map(|k| timemask::iso(k, true)).unwrap_or_default(),
                ordered: p.in_ordine(),
            };
            if !candidates
                .iter()
                .any(|x| x.pattern == c.pattern && x.within == c.within && x.skip == c.skip)
            {
                candidates.push(c);
            }
        }
        ExampleOutcome {
            problem: candidates.is_empty().then_some(ExampleProblem::NotInFile),
            candidates,
        }
    }
}
