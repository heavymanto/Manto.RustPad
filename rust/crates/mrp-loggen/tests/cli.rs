//! Il programma `mrp-loggen` da riga di comando: opzioni, errori e la modalita' `--all`. Scritti da ARCH.

use mrp_loggen::{generate, Config, Scenario};
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn cartella() -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let d = std::env::temp_dir().join(format!("mrp_loggen_cli_{}_{}", std::process::id(), n));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn esegui(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_mrp-loggen"))
        .args(args)
        .output()
        .unwrap()
}

fn codice(o: &Output) -> i32 {
    o.status.code().unwrap()
}

fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

fn genera(cfg: &Config) -> Vec<u8> {
    let mut v = Vec::new();
    generate(cfg, &mut v, &mut |_| {}).unwrap();
    v
}

#[test]
fn a_file_is_written_and_is_what_the_library_gives() {
    let d = cartella();
    let f = d.join("a.log");
    let o = esegui(&[
        "--size",
        "64K",
        "--scenario",
        "syslog",
        "--seed",
        "5",
        "--rate",
        "3",
        "--start",
        "2030-02-03",
        f.to_str().unwrap(),
    ]);
    assert_eq!(codice(&o), 0, "{}", stderr(&o));
    let mut cfg = Config::new(Scenario::Syslog, 64 << 10);
    cfg.seed = 5;
    cfg.rate = 3.0;
    cfg.start_day = mrp_loggen::parse_day("2030-02-03").unwrap();
    assert_eq!(std::fs::read(&f).unwrap(), genera(&cfg));
    assert!(stderr(&o).contains("bytes"), "{}", stderr(&o));
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn the_standard_output_gives_the_same_bytes_as_a_file() {
    let d = cartella();
    let f = d.join("b.log");
    assert_eq!(codice(&esegui(&["--size", "50K", f.to_str().unwrap()])), 0);
    let o = esegui(&["--size", "50K", "-"]);
    assert_eq!(codice(&o), 0);
    assert_eq!(o.stdout, std::fs::read(&f).unwrap());
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn an_existing_file_is_refused_unless_forced() {
    let d = cartella();
    let f = d.join("c.log");
    std::fs::write(&f, b"mio").unwrap();
    let o = esegui(&["--size", "10K", f.to_str().unwrap()]);
    assert_eq!(codice(&o), 2);
    assert!(stderr(&o).contains("already exists"), "{}", stderr(&o));
    assert_eq!(std::fs::read(&f).unwrap(), b"mio", "il file non si tocca");
    let o = esegui(&["--size", "10K", "--force", f.to_str().unwrap()]);
    assert_eq!(codice(&o), 0, "{}", stderr(&o));
    assert!(std::fs::metadata(&f).unwrap().len() >= 10 << 10);
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn wrong_options_are_refused_with_code_2_and_a_message() {
    let d = cartella();
    let f = d.join("x.log");
    let f = f.to_str().unwrap();
    let casi: [(&[&str], &str); 11] = [
        (&[f], "--size is required"),
        (&["--size", "10K"], "output file is missing"),
        (&["--size", "banana", f], "not a size"),
        (
            &["--size", "10K", "--scenario", "nope", f],
            "unknown scenario",
        ),
        (&["--size", "10K", "--seed", "x", f], "not a number"),
        (&["--size", "10K", "--start", "2026-02-30", f], "not a day"),
        (&["--size", "10K", "--rate", "0", f], "not a rate"),
        (&["--size", "10K", "--rate", "nan", f], "not a rate"),
        (&["--size", "10K", "--wat", f], "unknown option"),
        (&["--size", "10K", f, f], "only one output file"),
        (&["--size"], "needs a value"),
    ];
    for (args, frase) in casi {
        let o = esegui(args);
        assert_eq!(codice(&o), 2, "{args:?}");
        assert!(stderr(&o).contains(frase), "{args:?}: {}", stderr(&o));
    }
    assert!(
        !std::path::Path::new(f).exists(),
        "nessun file per un errore"
    );
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn help_and_list_print_and_succeed() {
    let o = esegui(&["--help"]);
    assert_eq!(codice(&o), 0);
    let t = String::from_utf8_lossy(&o.stdout);
    for parola in [
        "--size",
        "--scenario",
        "--seed",
        "--start",
        "--rate",
        "--force",
        "--all",
        "--list",
    ] {
        assert!(t.contains(parola), "manca {parola}");
    }
    let o = esegui(&["--list"]);
    assert_eq!(codice(&o), 0);
    let t = String::from_utf8_lossy(&o.stdout);
    assert_eq!(t.lines().count(), Scenario::ALL.len());
    for s in Scenario::ALL {
        assert!(t.lines().any(|l| l.starts_with(s.name())), "{}", s.name());
    }
}

#[test]
fn all_writes_every_scenario_small_and_slow_in_one_go() {
    let d = cartella();
    let dir = d.join("nuova").join("sotto");
    let o = esegui(&["--all", dir.to_str().unwrap()]);
    assert_eq!(codice(&o), 0, "{}", stderr(&o));
    for s in Scenario::ALL {
        let f = dir.join(format!("{}.log", s.name()));
        let dati = std::fs::read(&f).unwrap_or_else(|_| panic!("manca {}", f.display()));
        // 1 MiB di default e il ritmo lento del campione
        let mut cfg = Config::new(s, 1 << 20);
        cfg.rate = s.sample_rate();
        assert_eq!(dati, genera(&cfg), "{}", s.name());
        assert!(
            String::from_utf8_lossy(&o.stdout).contains(&format!("{}.log", s.name())),
            "{}",
            s.name()
        );
    }
    assert!(String::from_utf8_lossy(&o.stdout).contains("12 files"));
    // la dimensione si puo' cambiare, e il ritmo
    let dir2 = d.join("piccoli");
    let o = esegui(&[
        "--all",
        "--size",
        "20K",
        "--rate",
        "7",
        "--seed",
        "9",
        dir2.to_str().unwrap(),
    ]);
    assert_eq!(codice(&o), 0, "{}", stderr(&o));
    let mut cfg = Config::new(Scenario::Json, 20 << 10);
    cfg.rate = 7.0;
    cfg.seed = 9;
    assert_eq!(std::fs::read(dir2.join("json.log")).unwrap(), genera(&cfg));
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn all_refuses_to_overwrite_and_a_wrong_use() {
    let d = cartella();
    let dir = d.join("campioni");
    assert_eq!(
        codice(&esegui(&["--all", "--size", "10K", dir.to_str().unwrap()])),
        0
    );
    // il secondo giro: i file ci sono gia'
    std::fs::write(dir.join("service.log"), b"mio").unwrap();
    let o = esegui(&["--all", "--size", "10K", dir.to_str().unwrap()]);
    assert_eq!(codice(&o), 2);
    assert!(stderr(&o).contains("already exists"), "{}", stderr(&o));
    assert_eq!(std::fs::read(dir.join("service.log")).unwrap(), b"mio");
    let o = esegui(&["--all", "--force", "--size", "10K", dir.to_str().unwrap()]);
    assert_eq!(codice(&o), 0, "{}", stderr(&o));
    assert_ne!(std::fs::read(dir.join("service.log")).unwrap(), b"mio");
    // un solo scenario e --all insieme non hanno senso; la standard output nemmeno; e serve la cartella
    for (args, frase) in [
        (
            vec!["--all", "--scenario", "json", dir.to_str().unwrap()],
            "do not give --scenario",
        ),
        (vec!["--all", "-"], "not the standard output"),
        (vec!["--all"], "directory is missing"),
    ] {
        let o = esegui(&args);
        assert_eq!(codice(&o), 2, "{args:?}");
        assert!(stderr(&o).contains(frase), "{args:?}: {}", stderr(&o));
    }
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn the_sample_rates_are_slow_so_that_one_mib_spans_days() {
    // valori fissati: i campioni pubblicati dipendono da questi numeri
    for s in Scenario::ALL {
        let atteso = if s == Scenario::Bursts { 0.02 } else { 0.05 };
        assert_eq!(s.sample_rate(), atteso, "{}", s.name());
    }
    // a quel ritmo 1 MiB di service copre piu' di due giorni, e uno di bursts quasi due (le tempeste lo riempiono di errori con la pila di chiamate: meno voci per MiB)
    let giorni = |s: Scenario| {
        let mut cfg = Config::new(s, 1 << 20);
        cfg.rate = s.sample_rate();
        let mut v = Vec::new();
        let st = generate(&cfg, &mut v, &mut |_| {}).unwrap();
        (st.last_ms - st.first_ms) as f64 / 86_400_000.0
    };
    assert!(giorni(Scenario::Service) > 2.0);
    let b = giorni(Scenario::Bursts);
    assert!(b > 1.5 && b < 3.0, "{b}");
}
