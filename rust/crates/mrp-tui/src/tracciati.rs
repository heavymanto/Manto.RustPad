//! Il check massivo dei tracciati (`--check-tracciati`): l'istogramma delle lunghezze delle righe (la passata
//! di Alt+H) su tutti i file di un albero che matchano un glob, senza aprire il terminale. Il report e' un TSV
//! nella root della ricerca - una riga per ogni file e lunghezza: percorso, byte, righe, lunghezza, conteggio,
//! percentuale - scritto man mano (un flush per file: un'interruzione lascia il fatto fin lì). Nessuna logica
//! di giudizio: i dati grezzi li guarda l'utente.

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
    /// Il report TSV scritto.
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
    // mischiano), scritto man mano
    let report = nome_libero(&root, &format!("mrp-check-tracciati_{}.tsv", adesso_utc()));
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

/// La riga di testata del report TSV (le colonne, in italiano come il resto della funzione).
const TESTATA: &str = "percorso\tbyte\trighe\tlunghezza\tconteggio\tpercentuale\tnota\n";

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

/// Analizza un file che matcha e lo scrive nel report (una riga per lunghezza, i formati dominanti prima);
/// l'avanzamento va su stdout. Un errore di apertura o lettura diventa la riga ERRORE e il giro continua.
fn analizza_e_scrivi(path: &Path, entry: &fs::DirEntry, stato: &mut Stato) -> Result<(), String> {
    let byte_hint = entry.metadata().ok().map(|m| m.len());
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
            if a.lunghezze.is_empty() {
                // il file vuoto compare comunque: il colpo d'occhio lo vuole
                writeln!(stato.out, "{}\t{}\t0\t\t\t\t", path.display(), a.byte)
            } else {
                let mut esito_io = Ok(());
                for (lunghezza, conteggio) in a.lunghezze {
                    esito_io = writeln!(
                        stato.out,
                        "{}\t{}\t{}\t{}\t{}\t{}\t",
                        path.display(),
                        a.byte,
                        a.righe,
                        lunghezza,
                        conteggio,
                        percentuale(conteggio, a.righe)
                    );
                    if esito_io.is_err() {
                        break;
                    }
                }
                esito_io
            }
        }
        Err(msg) => {
            stato.errori += 1;
            println!(
                "{}",
                stato
                    .cat
                    .tf("cli.trk.errore", &[&path.display().to_string(), &msg])
            );
            write!(stato.out, "{}", riga_errore(path, byte_hint, &msg))
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

/// La riga di un file che non si e' potuto leggere: `ERRORE` al posto della lunghezza, il messaggio in coda.
fn riga_errore(percorso: &Path, byte: Option<u64>, messaggio: &str) -> String {
    let byte = byte.map_or_else(String::new, |b| b.to_string());
    format!(
        "{}\t{}\t\tERRORE\t\t\t{}\n",
        percorso.display(),
        byte,
        messaggio
    )
}

/// La percentuale del report: virgola decimale, due cifre, arrotondata a meta' con la sola aritmetica intera
/// (`(conteggio * 10000 + righe / 2) / righe`); vuota senza righe.
fn percentuale(conteggio: u64, righe: u64) -> String {
    if righe == 0 {
        return String::new();
    }
    let centesimi = (conteggio * 10000 + righe / 2) / righe;
    format!("{},{:02}", centesimi / 100, centesimi % 100)
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
        let base = "mrp-check-tracciati_prova.tsv";
        std::fs::write(d.join(base), "").unwrap();
        let p1 = nome_libero(&d, base);
        assert_eq!(p1, d.join("mrp-check-tracciati_prova_1.tsv"));
        std::fs::write(&p1, "").unwrap();
        assert_eq!(
            nome_libero(&d, base),
            d.join("mrp-check-tracciati_prova_2.tsv")
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
            riga_errore(Path::new("a/b.log"), Some(10), "boom"),
            "a/b.log\t10\t\tERRORE\t\t\tboom\n"
        );
        // senza i byte noti (i metadati mancano) la cella resta vuota
        assert_eq!(
            riga_errore(Path::new("c.log"), None, "x"),
            "c.log\t\t\tERRORE\t\t\tx\n"
        );
    }
}
