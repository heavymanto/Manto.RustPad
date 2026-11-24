//! Il check massivo dei tracciati da riga di comando (`--check-tracciati`, FL58): l'istogramma delle lunghezze
//! (la passata di Alt+H) su tutti i file di un albero che matchano il glob, col report TSV nella root. Scritti
//! da ARCH: l'oracolo e' l'albero costruito qui a mano - byte, righe e lunghezze contati a mano nel
//! PLAN_FL58, mai presi dall'output del programma. I percorsi attesi si costruiscono con `Path::join`
//! (il separatore e' quello del sistema).

use mrp_core::lang::Catalog;
use mrp_tui::tracciati::massivo;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

/// Una cartella vuota per l'albero di prova.
fn dir() -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let d = std::env::temp_dir().join(format!("mrp_tracciati_{}_{}", std::process::id(), n));
    fs::create_dir_all(&d).unwrap();
    d
}

/// Il TSV del report senza BOM (il confronto e' sulle righe).
fn leggi(p: &PathBuf) -> String {
    let mut s = fs::read_to_string(p).unwrap();
    if s.starts_with('\u{feff}') {
        s.remove(0);
    }
    s
}

#[test]
fn tracciati_report_completo() {
    let r = dir();
    // a.log: "aa\r\nbbb\r\ncc\r\n" = 13 byte, 3 righe, lunghezze {2:2, 3:1} (il \r non e' del record)
    fs::write(r.join("a.log"), "aa\r\nbbb\r\ncc\r\n").unwrap();
    // b.txt non matcha *.log: nel report non deve comparire
    fs::write(r.join("b.txt"), "xx\n").unwrap();
    fs::create_dir_all(r.join("sub").join("d")).unwrap();
    // c.log: "abcd\nabcde" = 10 byte, 2 righe {4:1, 5:1} (l'ultima senza a-capo conta lo stesso)
    fs::write(r.join("sub").join("c.log"), "abcd\nabcde").unwrap();
    // e.log: "z\nz\nz\n" = 6 byte, 3 righe {1:3}
    fs::write(r.join("sub").join("d").join("e.log"), "z\nz\nz\n").unwrap();
    // vuoto.log: 0 byte, 0 righe: una riga sua comunque (il colpo d'occhio lo vuole)
    fs::write(r.join("sub").join("vuoto.log"), "").unwrap();

    let esito = massivo(&Catalog::english(), &r.join("*.log")).unwrap();
    // 4 file matchano (b.txt no), 8 righe in tutto (3+2+3+0), nessun errore
    assert_eq!(esito.file, 4);
    assert_eq!(esito.righe, 8);
    assert_eq!(esito.errori, 0);
    // il report sta nella root, col nome che si aspetta
    assert_eq!(
        esito.report.parent(),
        Some(r.as_path()),
        "{}",
        esito.report.display()
    );
    let nome = esito
        .report
        .file_name()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    assert!(
        nome.starts_with("mrp-check-tracciati_") && nome.ends_with("Z.tsv"),
        "{}",
        nome
    );

    // l'ordine di visita: le entrate di ogni cartella in ordine di nome, in profondita' (a.log, poi in sub:
    // c.log, d/e.log, vuoto.log); le lunghezze per conteggio decrescente, a parita' la piu' corta
    let atteso = format!(
        "percorso\tbyte\trighe\tlunghezza\tconteggio\tpercentuale\tnota\n\
         {a}\t13\t3\t2\t2\t66,67\t\n\
         {a}\t13\t3\t3\t1\t33,33\t\n\
         {c}\t10\t2\t4\t1\t50,00\t\n\
         {c}\t10\t2\t5\t1\t50,00\t\n\
         {e}\t6\t3\t1\t3\t100,00\t\n\
         {v}\t0\t0\t\t\t\t\n",
        a = r.join("a.log").display(),
        c = r.join("sub").join("c.log").display(),
        e = r.join("sub").join("d").join("e.log").display(),
        v = r.join("sub").join("vuoto.log").display(),
    );
    assert_eq!(leggi(&esito.report), atteso);
}

#[test]
fn tracciati_glob_sul_nome_non_sul_percorso() {
    let r = dir();
    // il glob guarda il SOLO nome del file: "a*.log" matcha a.log e sub/alfa.log, non b.log; e non matcha
    // mai il percorso intero (che comincia con la lettera del disco, non con 'a')
    fs::write(r.join("a.log"), "pippo\n").unwrap();
    fs::write(r.join("b.log"), "pluto\n").unwrap();
    fs::create_dir_all(r.join("sub")).unwrap();
    fs::write(r.join("sub").join("alfa.log"), "paperino\n").unwrap();

    let esito = massivo(&Catalog::english(), &r.join("a*.log")).unwrap();
    assert_eq!(esito.file, 2, "solo a.log e sub/alfa.log");
    let tsv = leggi(&esito.report);
    let righe: Vec<&str> = tsv.lines().collect();
    assert_eq!(righe.len(), 3, "{}", tsv); // testata + 2
    assert!(
        righe[1].starts_with(&format!("{}", r.join("a.log").display())),
        "{}",
        tsv
    );
    assert!(
        righe[2].starts_with(&format!("{}", r.join("sub").join("alfa.log").display())),
        "{}",
        tsv
    );
    assert!(!tsv.contains("b.log\t"), "{}", tsv);
}

#[test]
fn tracciati_il_report_non_si_analizza() {
    let r = dir();
    // col glob *.tsv l'unico tsv della cartella e' il report che sta nascendo: non deve analizzare se'
    // stesso (una riga sua nel report sarebbe un finto file vuoto)
    let esito = massivo(&Catalog::english(), &r.join("*.tsv")).unwrap();
    assert_eq!(esito.file, 0);
    assert_eq!(esito.errori, 0);
    let tsv = leggi(&esito.report);
    assert_eq!(tsv.lines().count(), 1, "{}", tsv); // solo la testata
}

/// Il binario vero: `--check-tracciati` gira senza aprire il terminale, esce 0 col riepilogo e 1 se la root
/// non esiste; la riga dell'aiuto c'e' (il catalogo e' l'inglese incorporato, con MRP_NO_PROFILE).
#[test]
fn tracciati_dal_binario() {
    use std::process::Command;
    let r = dir();
    fs::write(r.join("x.log"), "aa\nbb\n").unwrap();
    let ok = Command::new(env!("CARGO_BIN_EXE_mrp-tui"))
        .env("MRP_NO_PROFILE", "1")
        .arg("--check-tracciati")
        .arg(r.join("*.log").to_string_lossy().as_ref())
        .output()
        .unwrap();
    assert!(
        ok.status.success(),
        "{}",
        String::from_utf8_lossy(&ok.stderr)
    );
    let stdout = String::from_utf8_lossy(&ok.stdout);
    assert!(
        stdout.contains("1 files, 2 rows, 0 errors - report: "),
        "{stdout}"
    );
    // il report e' nella root
    let report = fs::read_dir(&r)
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| {
            p.file_name()
                .map(|n| n.to_string_lossy().starts_with("mrp-check-tracciati_"))
                .unwrap_or(false)
        })
        .expect("il report nella root");
    let tsv = leggi(&report);
    assert_eq!(
        tsv.lines().nth(1),
        Some(format!("{}\t6\t2\t2\t2\t100,00\t", r.join("x.log").display()).as_str()),
        "{tsv}"
    );
    // la root che non esiste: uscita 1 e il messaggio dice quale
    let male = Command::new(env!("CARGO_BIN_EXE_mrp-tui"))
        .env("MRP_NO_PROFILE", "1")
        .arg("--check-tracciati")
        .arg(
            r.join("inesistente")
                .join("*.log")
                .to_string_lossy()
                .as_ref(),
        )
        .output()
        .unwrap();
    assert_eq!(male.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&male.stderr);
    assert!(
        stderr.contains("the --check-tracciati folder does not exist"),
        "{stderr}"
    );
    // l'aiuto nomina il flag nuovo
    let aiuto = Command::new(env!("CARGO_BIN_EXE_mrp-tui"))
        .env("MRP_NO_PROFILE", "1")
        .arg("--help")
        .output()
        .unwrap();
    let aiuto = String::from_utf8_lossy(&aiuto.stdout);
    assert!(aiuto.contains("--check-tracciati PATH"), "{aiuto}");
}

#[test]
fn tracciati_file_generato_a_flusso() {
    let r = dir();
    let p = r.join("grande.log");
    // 50_000 righe da 7 byte scritte a flusso (mai tutto in memoria): 350_000 byte, una lunghezza sola
    {
        let mut f = fs::File::create(&p).unwrap();
        let mut w = std::io::BufWriter::new(&mut f);
        for _ in 0..50_000 {
            w.write_all(b"aaaaaa\n").unwrap();
        }
        w.flush().unwrap();
    }
    let esito = massivo(&Catalog::english(), &r.join("*.log")).unwrap();
    assert_eq!(esito.file, 1);
    assert_eq!(esito.righe, 50_000);
    assert_eq!(esito.errori, 0);
    // la testata e una riga sola: lunghezza 6, conteggio 50_000, 100,00 per cento
    let tsv = leggi(&esito.report);
    assert_eq!(tsv.lines().count(), 2, "{}", tsv);
    let riga = format!("{}\t350000\t50000\t6\t50000\t100,00\t", p.display());
    assert_eq!(tsv.lines().nth(1), Some(riga.as_str()), "{}", tsv);
}
