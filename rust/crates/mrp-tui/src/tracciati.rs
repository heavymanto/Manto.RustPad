//! Il check massivo dei tracciati (`--check-tracciati`): l'istogramma delle lunghezze delle righe (la passata
//! di Alt+H) su tutti i file di un albero che matchano un glob, senza aprire il terminale. Il report e' un CSV
//! col separatore `;` (lo apre Excel italiano) nella root della ricerca: una tabella sola, una riga per file
//! e lunghezza - percorso, byte, righe, lunghezza, conteggio - cosi' in Excel si filtra per lunghezza e si
//! ordina per conteggio (le anomalie col record solo salgono da sole). Scritto man mano (un flush per file:
//! un'interruzione lascia il fatto fin lì). Nessuna logica di giudizio: i dati grezzi li guarda l'utente.

use mrp_core::docsearch::SearchError;
use mrp_core::encoding::Encoding;
use mrp_core::lang::Catalog;
use mrp_core::pattern::FileGlob;
use mrp_core::timemask;
use mrp_core::Document;
use std::fs;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64};
use std::time::{SystemTime, UNIX_EPOCH};

/// Il risultato del giro, per il riepilogo che stampa il binario.
pub struct EsitoMassivo {
    /// Il report CSV scritto.
    pub report: PathBuf,
    /// Quanti file matchano il glob e si sono letti.
    pub file: u64,
    /// Le righe totali dei file letti.
    pub righe: u64,
    /// Quanti file non si sono potuti leggere (hanno una riga ERRORE nel report).
    pub errori: u64,
}

/// Il giro intero: separa root e glob dall'argomento (`"c:\log\*.txt"`: la cartella e' la root, il resto il
/// glob sul nome file), cammina l'albero e scrive il report nella root. `Err` (nella lingua scelta) solo per i
/// fatti che bloccano tutto: root che non esiste, pattern mancante, report non scrivibile. Un file che non si
/// legge non ferma il giro: diventa una riga ERRORE.
pub fn massivo(cat: &Catalog, argomento: &Path) -> Result<EsitoMassivo, String> {
    // il pattern e' il nome file dell'argomento; la root tutto cio' che viene prima ("*.log" senza cartelle
    // vuol dire la cartella corrente)
    let Some(nome_glob) = argomento.file_name() else {
        return Err(cat.t("cli.err.trk_pattern").to_string());
    };
    let glob = nome_glob.to_string_lossy().into_owned();
    let matcher = FileGlob::new(&glob).map_err(|_| cat.t("cli.err.trk_pattern").to_string())?;
    let root_data = argomento.parent().unwrap_or(Path::new(""));
    let root_data = if root_data.as_os_str().is_empty() {
        Path::new(".")
    } else {
        root_data
    };
    let root = std::path::absolute(root_data)
        .map_err(|_| cat.tf("cli.err.trk_root", &[&root_data.display().to_string()]))?;
    if !root.is_dir() {
        return Err(cat.tf("cli.err.trk_root", &[&root.display().to_string()]));
    }
    // il report: il nome col momento del giro, libero da collisioni (due giri nello stesso secondo non si
    // mischiano), CSV col separatore ; cosi' Excel italiano lo apre a doppio clic
    let report = nome_libero(&root, &format!("mrp-check-tracciati_{}.csv", adesso_utc()));
    let file =
        fs::File::create(&report).map_err(|e| cat.tf("cli.err.trk_report", &[&e.to_string()]))?;
    let mut out = BufWriter::new(file);
    // il BOM: cosi' Excel capisce che e' UTF-8 (i percorsi possono avere accenti)
    out.write_all(b"\xef\xbb\xbf")
        .and_then(|_| out.write_all(TESTATA.as_bytes()))
        .map_err(|e| cat.tf("cli.err.trk_report", &[&e.to_string()]))?;
    let mut stato = Stato {
        cat,
        matcher: &matcher,
        out: &mut out,
        report: &report,
        file: 0,
        righe: 0,
        errori: 0,
    };
    visita(&root, &mut stato)?;
    stato
        .out
        .flush()
        .map_err(|e| cat.tf("cli.err.trk_report", &[&e.to_string()]))?;
    let (file, righe, errori) = (stato.file, stato.righe, stato.errori);
    Ok(EsitoMassivo {
        report,
        file,
        righe,
        errori,
    })
}

/// La riga di testata del report CSV (le colonne, in italiano come il resto della funzione): lunghezza e
/// conteggio in colonne proprie, cosi' in Excel si filtra per lunghezza. Le righe finiscono col CRLF di
/// Windows: e' quello che i fogli a calcolo si aspettano.
const TESTATA: &str = "percorso;byte;righe;lunghezza;conteggio\r\n";

/// Il mondo del giro mentre cammina: il matcher del glob, il report aperto e i fatti contati.
struct Stato<'a> {
    cat: &'a Catalog,
    matcher: &'a FileGlob,
    out: &'a mut BufWriter<fs::File>,
    report: &'a Path,
    file: u64,
    righe: u64,
    errori: u64,
}

/// Visita una cartella dell'albero: le entrate in ordine di nome, in profondita'. Solo i file regolari col nome
/// che matcha si analizzano; le sottocartelle si scendono, i symlink non si seguono, il report stesso si salta.
/// Una cartella che non si legge si salta (lo dice su stderr): il giro non muore.
fn visita(dir: &Path, stato: &mut Stato) -> Result<(), String> {
    let entrate = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => {
            eprintln!(
                "{}",
                stato
                    .cat
                    .tf("cli.trk.saltata", &[&dir.display().to_string()])
            );
            return Ok(());
        }
    };
    let mut v: Vec<fs::DirEntry> = entrate.flatten().collect();
    v.sort_by_key(|e| e.file_name());
    for e in v {
        let path = e.path();
        if path == *stato.report {
            continue;
        }
        // il tipo non segue i symlink: un link a una cartella non si scende, uno a un file non si legge
        let Ok(tipo) = e.file_type() else { continue };
        if tipo.is_dir() {
            visita(&path, stato)?;
        } else if tipo.is_file() && stato.matcher.is_match(&e.file_name().to_string_lossy()) {
            analizza_e_scrivi(&path, &e, stato)?;
        }
    }
    Ok(())
}

/// Analizza un file che matcha e ne scrive le righe (una per lunghezza, i formati dominanti prima); il file
/// vuoto ha la sua riga con le ultime due celle vuote. L'avanzamento va su stdout. Un errore di apertura o
/// lettura diventa la riga ERRORE e il giro continua.
fn analizza_e_scrivi(path: &Path, entry: &fs::DirEntry, stato: &mut Stato) -> Result<(), String> {
    let byte_hint = entry.metadata().ok().map(|m| m.len());
    let percorso = campo(&path.display().to_string());
    let scritto: std::io::Result<()> = match analizza(path) {
        Ok(a) => {
            stato.file += 1;
            stato.righe += a.righe;
            println!(
                "{}",
                stato.cat.tf(
                    "cli.trk.riga",
                    &[
                        &path.display().to_string(),
                        &a.righe.to_string(),
                        &a.lunghezze.len().to_string()
                    ]
                )
            );
            let mut esito_io = Ok(());
            if a.lunghezze.is_empty() {
                // il file vuoto compare comunque: il colpo d'occhio lo vuole
                esito_io = write!(stato.out, "{};{};0;;\r\n", percorso, a.byte);
            } else {
                for (lunghezza, conteggio) in &a.lunghezze {
                    esito_io = write!(
                        stato.out,
                        "{};{};{};{};{}\r\n",
                        percorso, a.byte, a.righe, lunghezza, conteggio
                    );
                    if esito_io.is_err() {
                        break;
                    }
                }
            }
            esito_io
        }
        Err(msg) => {
            stato.errori += 1;
            println!(
                "{}",
                stato
                    .cat
                    .tf("cli.trk.errore", &[&path.display().to_string(), &msg])
            );
            write!(stato.out, "{}", riga_errore(&percorso, byte_hint, &msg))
        }
    };
    scritto.map_err(|e| stato.cat.tf("cli.err.trk_report", &[&e.to_string()]))?;
    stato
        .out
        .flush()
        .map_err(|e| stato.cat.tf("cli.err.trk_report", &[&e.to_string()]))?;
    Ok(())
}

/// L'analisi di un file: i fatti che vanno nel report.
struct Analisi {
    /// I byte del file.
    byte: u64,
    /// Le righe in tutto (la somma dei conteggi).
    righe: u64,
    /// Le lunghezze col loro conteggio, per conteggio decrescente (a parita' la piu' corta).
    lunghezze: Vec<(u64, u64)>,
}

/// L'analisi di un file, con la stessa apertura dell'interfaccia: la codifica la suggeriscono i primi 64 KiB.
/// `Err` se il file non si apre (perche' manca o non e' un file regolare).
fn analizza(path: &Path) -> Result<Analisi, String> {
    let doc = Document::open(path).map_err(|e| e.to_string())?;
    let byte = doc.len();
    let campione = doc.read_range(0, byte.min(65536));
    let enc = Encoding::detect(&campione);
    let snap = doc
        .snapshot()
        .ok_or_else(|| "documento chiuso".to_string())?;
    let istogramma = snap
        .with_boundary(enc)
        .count_lengths(&AtomicBool::new(false), &AtomicU64::new(0))
        .map_err(|e| match e {
            SearchError::Io(e) => e.to_string(),
            SearchError::Cancelled => "annullata".to_string(),
        })?;
    let righe = istogramma.values().sum();
    let mut lunghezze: Vec<(u64, u64)> = istogramma.into_iter().collect();
    lunghezze.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    Ok(Analisi {
        byte,
        righe,
        lunghezze,
    })
}

/// La riga di un file che non si e' potuto leggere: `ERRORE` al posto della lunghezza, il messaggio nel
/// campo in coda (senza intestazione: compare solo nelle rigere di errore).
fn riga_errore(percorso: &str, byte: Option<u64>, messaggio: &str) -> String {
    let byte = byte.map_or_else(String::new, |b| b.to_string());
    format!("{};{};;ERRORE;;{}\r\n", percorso, byte, campo(messaggio))
}

/// Il campo CSV protetto: se contiene `;`, virgolette o a-capo si avvolge fra virgolette doppie, con le
/// virgolette interne raddoppiate (lo standard che capisce Excel); altrimenti resta nudo.
fn campo(testo: &str) -> String {
    if testo.contains(';') || testo.contains('"') || testo.contains('\n') || testo.contains('\r') {
        format!("\"{}\"", testo.replace('"', "\"\""))
    } else {
        testo.to_string()
    }
}

/// Il momento del giro come nome di file, in UTC (niente dipendenze nuove: il calendario e' quello della
/// timeline): `AAAAMMGG_HHMMSSZ`.
fn adesso_utc() -> String {
    let ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    let (a, m, g) = timemask::civil(ms);
    let giorno = ms.rem_euclid(86_400_000);
    let (h, min, s) = (
        giorno / 3_600_000,
        (giorno / 60_000) % 60,
        (giorno / 1000) % 60,
    );
    format!("{a:04}{m:02}{g:02}_{h:02}{min:02}{s:02}Z")
}

/// Il nome del report libero da collisioni: se `dir/base` esiste gia', un suffisso `_1`, `_2`, ... prima
/// dell'estensione.
fn nome_libero(dir: &Path, base: &str) -> PathBuf {
    let p = dir.join(base);
    if !p.exists() {
        return p;
    }
    let (radice, est) = match base.rsplit_once('.') {
        Some((r, e)) => (r.to_string(), format!(".{e}")),
        None => (base.to_string(), String::new()),
    };
    let mut n = 1u64;
    loop {
        let candidato = dir.join(format!("{radice}_{n}{est}"));
        if !candidato.exists() {
            return candidato;
        }
        n += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    static COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

    /// Una cartella vuota per le prove.
    fn dir() -> PathBuf {
        let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let d =
            std::env::temp_dir().join(format!("mrp_tracciati_unit_{}_{}", std::process::id(), n));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn tracciati_nome_libero() {
        let d = dir();
        let base = "mrp-check-tracciati_prova.csv";
        std::fs::write(d.join(base), "").unwrap();
        let p1 = nome_libero(&d, base);
        assert_eq!(p1, d.join("mrp-check-tracciati_prova_1.csv"));
        std::fs::write(&p1, "").unwrap();
        assert_eq!(
            nome_libero(&d, base),
            d.join("mrp-check-tracciati_prova_2.csv")
        );
    }

    #[test]
    fn tracciati_errore_cartella() {
        let d = dir();
        // una cartella non e' un file regolare: l'apertura deve fallire (la riga ERRORE del report la scrive
        // chi cammina, su questo errore)
        assert!(analizza(&d).is_err());
        // la riga ERRORE, calcolata a mano: ERRORE al posto della lunghezza, il messaggio in coda
        assert_eq!(
            riga_errore("a/b.log", Some(10), "boom"),
            "a/b.log;10;;ERRORE;;boom\r\n"
        );
        // senza i byte noti (i metadati mancano) la cella resta vuota, ma il campo c'e' sempre
        assert_eq!(riga_errore("c.log", None, "x"), "c.log;;;ERRORE;;x\r\n");
    }

    #[test]
    fn tracciati_campo_csv() {
        // il campo innocente resta nudo
        assert_eq!(campo("abc"), "abc");
        // il punto e virgola, le virgolette e gli a-capo si proteggono; le virgolette interne si raddoppiano
        assert_eq!(campo("a;b"), "\"a;b\"");
        assert_eq!(campo("un \"quote\""), "\"un \"\"quote\"\"\"");
        assert_eq!(campo("riga\nspezzata"), "\"riga\nspezzata\"");
        assert_eq!(campo("vecchio\r\n"), "\"vecchio\r\n\"");
    }
}
