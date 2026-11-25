//! Il check massivo dei tracciati da riga di comando (`--check-tracciati`, FL58): l'istogramma delle lunghezze
//! (la passata di Alt+H) su tutti i file di un albero che matchano il glob, col report CSV nella root. Scritti
//! da ARCH: l'oracolo e' l'albero costruito qui a mano - byte, righe e lunghezze contati a mano nel
//! PLAN_FL58c, mai presi dall'output del programma. I percorsi attesi si costruiscono con `Path::join`
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

/// Il CSV del report senza BOM (il confronto e' sulle righe).
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
    // f.log: "xxxxx\nxxxxx\ny\n" = 14 byte, 3 righe {5:2, 1:1}: la lunghezza LUNGA e' la dominante, cosi'
    // l'ordinamento per conteggio decrescente si distingue da quello per lunghezza crescente
    fs::write(r.join("sub").join("d").join("f.log"), "xxxxx\nxxxxx\ny\n").unwrap();
    // vuoto.log: 0 byte, 0 righe: una riga sua comunque (il colpo d'occhio lo vuole)
    fs::write(r.join("sub").join("vuoto.log"), "").unwrap();

    let esito = massivo(&Catalog::english(), &r.join("*.log")).unwrap();
    // 5 file matchano (b.txt no), 11 righe in tutto (3+2+3+3+0), nessun errore
    assert_eq!(esito.file, 5);
    assert_eq!(esito.righe, 11);
    assert_eq!(esito.errori, 0);
    // il report sta nella root, col nome che si aspetta (CSV ormai: il ; lo apre Excel italiano)
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
        nome.starts_with("mrp-check-tracciati_") && nome.ends_with("Z.csv"),
        "{}",
        nome
    );

    // l'ordine di visita: le entrate di ogni cartella in ordine di nome, in profondita' (a.log, poi in sub:
    // c.log, d/e.log, d/f.log, vuoto.log); le lunghezze per conteggio decrescente, a parita' la piu' corta
    // (f.log e' la prova che conta l'ordine dei CONTEGGI: la 5 sta prima della 1).
    // UNA riga per file (il report e' una tabella CSV sola, senza righe vuote ne' dettaglio), le lunghezze
    // compatte col tetto di dieci. Le percentuali a mano: 2/3 = 66.67, 1/3 = 33.33, 1/2 = 50.00,
    // 3/3 = 100.00 — col PUNTO decimale: nel file non ci devono essere virgole, cosi' nessun foglio a
    // calcolo puo' spezzare una riga. Le righe finiscono col CRLF di Windows.
    let atteso = format!(
        "percorso;byte;righe;n.lunghezze;lunghezze\r\n\
         {a};13;3;2;2x2 (66.67%) | 3x1 (33.33%)\r\n\
         {c};10;2;2;4x1 (50.00%) | 5x1 (50.00%)\r\n\
         {e};6;3;1;1x3 (100.00%)\r\n\
         {f};14;3;2;5x2 (66.67%) | 1x1 (33.33%)\r\n\
         {v};0;0;0;\r\n",
        a = r.join("a.log").display(),
        c = r.join("sub").join("c.log").display(),
        e = r.join("sub").join("d").join("e.log").display(),
        f = r.join("sub").join("d").join("f.log").display(),
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
    let csv = leggi(&esito.report);
    let righe: Vec<&str> = csv.lines().collect();
    // intestazione + 2 file: la tabella e' una sola, niente righe vuote
    assert_eq!(righe.len(), 3, "{}", csv);
    assert!(
        righe[1].starts_with(&format!("{}", r.join("a.log").display())),
        "{}",
        csv
    );
    assert!(
        righe[2].starts_with(&format!("{}", r.join("sub").join("alfa.log").display())),
        "{}",
        csv
    );
    assert!(!csv.contains("b.log;"), "{}", csv);
}

#[test]
fn tracciati_il_report_non_si_analizza() {
    let r = dir();
    // col glob *.csv l'unico csv della cartella e' il report che sta nascendo: non deve analizzare se'
    // stesso (una riga sua nel report sarebbe un finto file vuoto)
    let esito = massivo(&Catalog::english(), &r.join("*.csv")).unwrap();
    assert_eq!(esito.file, 0);
    assert_eq!(esito.errori, 0);
    let csv = leggi(&esito.report);
    assert_eq!(csv.lines().count(), 1, "{}", csv); // solo la testata
}

#[test]
fn tracciati_glob_case_insensitive() {
    let r = dir();
    // il caso vero dell'utente (25 nov 2026): i file si chiamano .TXT maiuscolo e il glob e' *.txt minuscolo
    // (o viceversa) - il glob non deve badare alle maiuscole, come il filesystem di Windows
    fs::create_dir_all(r.join("2026-11-10")).unwrap();
    fs::write(r.join("2026-11-10").join("datiepc_01166.TXT"), "aa\nbb\n").unwrap();
    let esito = massivo(&Catalog::english(), &r.join("*.txt")).unwrap();
    assert_eq!(esito.file, 1, "il glob prende anche il .TXT maiuscolo");
    assert_eq!(esito.righe, 2);
    // e anche al contrario: il glob maiuscolo prende il file minuscolo
    let r2 = dir();
    fs::write(r2.join("basso.txt"), "xx\n").unwrap();
    let esito2 = massivo(&Catalog::english(), &r2.join("*.TXT")).unwrap();
    assert_eq!(esito2.file, 1, "il glob prende anche il .txt minuscolo");
}

#[test]
fn tracciati_colonna_compatta_tetto_dieci() {
    let r = dir();
    // 12 righe di lunghezza crescente 1..12: 12 lunghezze da 1 riga ciascuna (78 + 12 a-capo = 90 byte).
    // La compatta si ferma a dieci e dice quante mancano; 1/12 = (10000+6)/12 = 833 -> 8.33.
    let mut testo = String::new();
    for lunghezza in 1..=12u64 {
        for _ in 0..lunghezza {
            testo.push('a');
        }
        testo.push('\n');
    }
    fs::write(r.join("tante.log"), &testo).unwrap();
    let esito = massivo(&Catalog::english(), &r.join("*.log")).unwrap();
    assert_eq!(esito.file, 1);
    assert_eq!(esito.righe, 12);
    let csv = leggi(&esito.report);
    let p = r.join("tante.log").display().to_string();
    let attesa_sintesi = format!(
        "{p};90;12;12;{}",
        (1..=10)
            .map(|l| format!("{l}x1 (8.33%)"))
            .collect::<Vec<_>>()
            .join(" | ")
    ) + " | e altre 2";
    assert_eq!(csv.lines().nth(1), Some(attesa_sintesi.as_str()), "{}", csv);
    // la tabella e' una riga per file: due righe in tutto (testata + questa)
    assert_eq!(csv.lines().count(), 2, "{}", csv);
}

#[test]
fn tracciati_percorso_col_punto_e_virgola() {
    let r = dir();
    // un nome file col punto e virgola: il campo si avvolge fra virgolette CSV (le virgolette interne
    // si raddoppiano), senno' Excel spezzerebbe la riga
    fs::write(r.join("con;ptv.log"), "z\nz\nz\n").unwrap();
    let esito = massivo(&Catalog::english(), &r.join("*.log")).unwrap();
    assert_eq!(esito.file, 1);
    let csv = leggi(&esito.report);
    let p = format!("\"{}\"", r.join("con;ptv.log").display());
    assert_eq!(
        csv.lines().nth(1),
        Some(format!("{p};6;3;1;1x3 (100.00%)").as_str()),
        "{}",
        csv
    );
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
    let csv = leggi(&report);
    // la seconda riga e' x.log: 6 byte, 2 righe, una lunghezza (2) per 2 righe, 100.00
    assert_eq!(
        csv.lines().nth(1),
        Some(format!("{};6;2;1;2x2 (100.00%)", r.join("x.log").display()).as_str()),
        "{csv}"
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
    // due righe: testata e la riga del file (una lunghezza sola: 6 per 50_000, 100.00)
    let csv = leggi(&esito.report);
    assert_eq!(csv.lines().count(), 2, "{}", csv);
    assert_eq!(
        csv.lines().nth(1),
        Some(format!("{};350000;50000;1;6x50000 (100.00%)", p.display()).as_str()),
        "{}",
        csv
    );
}
