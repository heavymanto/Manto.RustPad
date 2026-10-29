//! Il binario `mrp`: legge gli argomenti e il profilo, decide il tema e la lingua di partenza (opzione >
//! impostazioni > predefinito) e apre il file; poi il terminale vero o, con `--dump`, lo schermo come testo
//! (con `--keys` si prova anche una sequenza di tasti). I messaggi vengono dal `Catalog` nella lingua scelta
//! (inglese se non se ne conosce ancora una); i codici di uscita: 0 tutto bene, 1 il file non si apre o il
//! terminale fallisce, 2 errore d'uso.

use mrp_core::lang::Catalog;
use mrp_tui::app::{App, AppConfig};
use mrp_tui::args;
use mrp_tui::keys;
use mrp_tui::term;
use mrp_tui::ui;
use ratatui::crossterm::event::KeyEvent;
use std::ffi::OsString;
use std::io::{self, Write};
use std::time::Duration;

/// Quanto `--dump` aspetta i lavori in background (indicizzazione, ricerca, salvataggio).
const ATTESA_DUMP: Duration = Duration::from_secs(30);

fn main() {
    std::process::exit(run(std::env::args_os().skip(1)));
}

/// Tutto il binario, dal primo argomento al codice di uscita.
fn run<I: IntoIterator<Item = OsString>>(argv: I) -> i32 {
    // gli argomenti: un'opzione sconosciuta o senza valore e' un errore d'uso (la lingua non e' ancora
    // nota: inglese)
    let a = match args::parse(argv) {
        Ok(a) => a,
        Err(_) => return usage_error(&Catalog::english(), "cli.err.no_file", &[]),
    };
    // il profilo: le lingue, le impostazioni lette e dove salvarle
    let profilo = args::profile(&a);
    // la lingua dei messaggi: quella scelta se c'e' e vale, altrimenti l'inglese (la validazione di
    // `--lang` arriva piu' avanti, nell'ordine degli errori d'uso)
    let lingua = args::resolve_lang(&a, &profilo.langs, &profilo.settings);
    let cat = match &lingua {
        Ok(Some(l)) => Catalog::with(l.clone()),
        Ok(None) | Err(_) => Catalog::english(),
    };
    // l'aiuto viene prima di ogni controllo: il testo e' nella lingua scelta
    if a.help {
        for chiave in [
            "cli.help.l1",
            "cli.help.l2",
            "cli.help.l3",
            "cli.help.l4",
            "cli.help.l5",
        ] {
            println!("{}", cat.t(chiave));
        }
        return 0;
    }
    // gli errori d'uso, nell'ordine: il file, il tema, la lingua, la dimensione del dump, i tasti
    let Some(file) = &a.file else {
        return usage_error(&cat, "cli.err.no_file", &[]);
    };
    let theme = match args::resolve_theme(&a, &profilo.settings) {
        Ok(t) => t,
        Err(nome) => return usage_error(&cat, "cli.err.theme", &[&nome]),
    };
    let lang_code = match lingua {
        Ok(Some(l)) => Some(l.code),
        Ok(None) => None,
        Err(code) => return usage_error(&cat, "cli.err.lang", &[&code]),
    };
    let dump = match &a.dump {
        Some(testo) => match args::parse_dump(testo) {
            Some((w, h)) => Some((w, h)),
            None => return usage_error(&cat, "cli.err.dump", &[testo]),
        },
        None => None,
    };
    let tasti = match &a.keys {
        Some(lista) => match keys::parse(lista) {
            Ok(t) => t,
            Err(voce) => return usage_error(&cat, "cli.err.key", &[&voce]),
        },
        None => Vec::new(),
    };
    // l'apertura: il percorso cosi' come e' stato dato, l'errore di sistema
    let cfg = AppConfig {
        theme,
        lang_code,
        langs: profilo.langs,
        settings_path: profilo.path,
        index: true,
    };
    let mut app = match App::open(file, cfg) {
        Ok(app) => app,
        Err(e) => {
            eprintln!(
                "{}",
                cat.tf(
                    "cli.err.open",
                    &[&file.display().to_string(), &e.to_string()]
                )
            );
            return 1;
        }
    };
    match dump {
        // --dump: niente terminale, lo schermo come testo
        Some((w, h)) => match dump_run(&mut app, w, h, &tasti) {
            Ok(()) => 0,
            Err(e) => {
                eprintln!("{}", e);
                1
            }
        },
        None => match term::run(app) {
            Ok(()) => 0,
            Err(e) => {
                eprintln!("{}", e);
                1
            }
        },
    }
}

/// La modalita' `--dump`: ridimensiona l'app a W x H, applica i tasti (un `tick` dopo ognuno), aspetta che i
/// lavori in background finiscano e stampa lo schermo, una riga per riga.
fn dump_run(app: &mut App, w: u16, h: u16, tasti: &[KeyEvent]) -> io::Result<()> {
    app.resize(w, h);
    for tasto in tasti {
        app.handle_key(*tasto);
        app.tick();
        // i lavori in background devono stare al passo coi tasti, come nella sessione vera (i tasti di un
        // utente sono distanziati): prima del tasto successivo si aspetta che finiscano, altrimenti un
        // tasto puo' trovare un lavoro ancora in corso (per esempio il "vai a" di una riga numerica, che
        // senza l'indice pronto non si muove)
        let _ = app.wait_idle(ATTESA_DUMP);
    }
    let _ = app.wait_idle(ATTESA_DUMP);
    let mut out = io::stdout().lock();
    for riga in ui::render_lines(app, w, h) {
        writeln!(out, "{}", riga)?;
    }
    out.flush()
}

/// Un errore d'uso: il messaggio (dal `Catalog` nella lingua scelta) su stderr e il codice 2.
fn usage_error(cat: &Catalog, chiave: &str, argomenti: &[&str]) -> i32 {
    eprintln!("{}", cat.tf(chiave, argomenti));
    2
}
