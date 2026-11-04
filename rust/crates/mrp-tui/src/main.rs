//! Il binario `mrp-tui`: legge gli argomenti e il profilo, decide il tema e la lingua di partenza (opzione >
//! impostazioni > predefinito) e apre il file; poi il terminale vero o, con `--dump`, lo schermo come testo
//! (con `--keys` si prova anche una sequenza di tasti). I messaggi vengono dal `Catalog` nella lingua scelta
//! (inglese se non se ne conosce ancora una); i codici di uscita: 0 tutto bene, 1 il file non si apre o il
//! terminale fallisce, 2 errore d'uso.

use mrp_core::encoding::Encoding;
use mrp_core::lang::Catalog;
use mrp_tui::app::{App, AppConfig};
use mrp_tui::args;
use mrp_tui::keys;
use mrp_tui::term;
use mrp_tui::workspace::Workspace;
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
            "cli.help.enc",
            "cli.help.eol",
            "cli.help.l5",
        ] {
            println!("{}", cat.t(chiave));
        }
        return 0;
    }
    // anche i nomi delle codifiche vengono prima di ogni controllo (come l'aiuto): i nomi canonici,
    // uno per riga, e non serve nemmeno il file
    if a.list_encodings {
        for e in Encoding::all() {
            println!("{}", e.name());
        }
        return 0;
    }
    // gli errori d'uso, nell'ordine: il file, il tema, la lingua, la codifica, la dimensione del
    // dump, i tasti
    // la sessione (le schede e le modifiche non salvate dell'ultima volta) sta accanto alle impostazioni; con --dump
    // (uso da sviluppatore) non c'e'. Senza file nella riga di comando si riapre l'ultima sessione, se c'e', altrimenti si apre un documento nuovo.
    let sessione = match (&profilo.path, &a.dump) {
        (Some(p), None) => p.parent().map(|d| d.join("session")),
        _ => None,
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
    let encoding = match args::resolve_encoding(&a) {
        Ok(e) => e,
        Err(nome) => return usage_error(&cat, "cli.err.encoding", &[&nome]),
    };
    let eol = match args::resolve_eol(&a) {
        Ok(e) => e,
        Err(nome) => return usage_error(&cat, "cli.err.eol", &[&nome]),
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
        theme_chosen: args::theme_is_explicit(&a, &profilo.settings),
        lang_code,
        langs: profilo.langs,
        jump_pages: profilo
            .settings
            .as_ref()
            .map_or(mrp_tui::settings::JUMP_PAGES, |s| s.jump_pages),
        nav_bar: profilo.settings.as_ref().is_some_and(|s| s.nav_bar),
        special_chars: profilo.settings.as_ref().is_some_and(|s| s.special_chars),
        log_colors: profilo.settings.as_ref().is_some_and(|s| s.log_colors),
        line_numbers: profilo.settings.as_ref().is_none_or(|s| s.line_numbers),
        font_size: profilo.settings.as_ref().and_then(|s| s.font_size),
        time_format: profilo
            .settings
            .as_ref()
            .and_then(|s| s.time_format.clone()),
        time_rules: profilo
            .settings
            .as_ref()
            .map(|s| s.time_rules.clone())
            .unwrap_or_default(),
        chrome: true,
        eol,
        settings_path: profilo.path,
        index: profilo.settings.as_ref().is_none_or(|s| s.index),
        encoding,
    };
    let errore_apertura = |file: &std::path::Path, e: &io::Error| {
        eprintln!(
            "{}",
            cat.tf(
                "cli.err.open",
                &[&file.display().to_string(), &e.to_string()]
            )
        );
        1
    };
    let mut ws = match a.files.first() {
        // senza file: l'ultima sessione, altrimenti un documento nuovo senza nome
        None => match Workspace::start(cfg, sessione.clone()) {
            Ok(ws) => ws,
            Err(e) => {
                eprintln!("{e}");
                return 1;
            }
        },
        Some(file) => {
            let app = match App::open(file, cfg.clone()) {
                Ok(app) => app,
                Err(e) => return errore_apertura(file, &e),
            };
            // un file per scheda, nell'ordine dato; resta avanti la prima
            let mut ws = Workspace::new(app, cfg);
            for altro in &a.files[1..] {
                if let Err(e) = ws.open(altro) {
                    return errore_apertura(altro, &e);
                }
            }
            // le schede con modifiche non salvate dell'ultima volta tornano comunque: nulla si perde
            if let Some(dir) = &sessione {
                ws = ws.with_session(dir.clone());
                ws.merge_session();
            }
            ws.select(0);
            ws
        }
    };
    match dump {
        // --dump: niente terminale, lo schermo come testo
        Some((w, h)) => match dump_run(&mut ws, w, h, &tasti) {
            Ok(()) => 0,
            Err(e) => {
                eprintln!("{}", e);
                1
            }
        },
        None => match term::run(ws) {
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
fn dump_run(app: &mut Workspace, w: u16, h: u16, tasti: &[KeyEvent]) -> io::Result<()> {
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
    for riga in app.render_lines(w, h) {
        writeln!(out, "{}", riga)?;
    }
    out.flush()
}

/// Un errore d'uso: il messaggio (dal `Catalog` nella lingua scelta) su stderr e il codice 2.
fn usage_error(cat: &Catalog, chiave: &str, argomenti: &[&str]) -> i32 {
    eprintln!("{}", cat.tf(chiave, argomenti));
    2
}
