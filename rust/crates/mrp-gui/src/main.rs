//! Il binario `mrpg`: la versione grafica di Manto.RustPad. Stessi argomenti del TUI (`mrp`) per file, tema, lingua e
//! codifica; apre la finestra con l'`App` del TUI disegnata da egui. Codici di uscita: 0 tutto bene, 1 il file non si
//! apre o la finestra non parte, 2 errore d'uso.

use eframe::egui;
use mrp_core::lang::Catalog;
use mrp_gui::gui::Finestra;
use mrp_tui::app::{App, AppConfig};
use mrp_tui::args;
use mrp_tui::workspace::Workspace;

fn main() {
    std::process::exit(run());
}

fn run() -> i32 {
    let a = match args::parse(std::env::args_os().skip(1)) {
        Ok(a) => a,
        Err(_) => return uso(&Catalog::english()),
    };
    let profilo = args::profile(&a);
    let lingua = args::resolve_lang(&a, &profilo.langs, &profilo.settings);
    let cat = match &lingua {
        Ok(Some(l)) => Catalog::with(l.clone()),
        Ok(None) | Err(_) => Catalog::english(),
    };
    if a.help {
        println!("mrpg [FILE...] [--encoding NAME] [--theme NAME] [--lang CODE]");
        return 0;
    }
    let theme = match args::resolve_theme(&a, &profilo.settings) {
        Ok(t) => t,
        Err(nome) => {
            eprintln!("{}", cat.tf("cli.err.theme", &[&nome]));
            return 2;
        }
    };
    let lang_code = match lingua {
        Ok(Some(l)) => Some(l.code),
        Ok(None) => None,
        Err(code) => {
            eprintln!("{}", cat.tf("cli.err.lang", &[&code]));
            return 2;
        }
    };
    let encoding = match args::resolve_encoding(&a) {
        Ok(e) => e,
        Err(nome) => {
            eprintln!("{}", cat.tf("cli.err.encoding", &[&nome]));
            return 2;
        }
    };
    let cfg = AppConfig {
        theme,
        lang_code,
        langs: profilo.langs,
        jump_pages: profilo
            .settings
            .as_ref()
            .map_or(mrp_tui::settings::JUMP_PAGES, |s| s.jump_pages),
        settings_path: profilo.path,
        index: true,
        encoding,
    };
    // la sessione (schede e modifiche non salvate dell'ultima volta) sta accanto alle impostazioni; con
    // MRPG_SCREENSHOT (uso da sviluppatore) non c'e'
    let sessione = match (&cfg.settings_path, std::env::var_os("MRPG_SCREENSHOT")) {
        (Some(p), None) => p.parent().map(|d| d.join("session")),
        _ => None,
    };
    let errore_apertura = |file: &std::path::Path, e: &std::io::Error| {
        eprintln!(
            "{}",
            cat.tf(
                "cli.err.open",
                &[&file.display().to_string(), &e.to_string()]
            )
        );
        1
    };
    // le schede: un file ciascuno (la prima avanti); senza file si riapre l'ultima sessione e, se non c'e', si sceglie
    // con il dialogo (annullare non e' un errore)
    let ws = match a.files.first() {
        Some(file) => {
            let app = match App::open(file, cfg.clone()) {
                Ok(app) => app,
                Err(e) => return errore_apertura(file, &e),
            };
            let mut ws = Workspace::new(app, cfg);
            for altro in a.files.iter().skip(1) {
                if let Err(e) = ws.open(altro) {
                    errore_apertura(altro, &e);
                }
            }
            if let Some(dir) = &sessione {
                ws = ws.with_session(dir.clone());
                // le schede con modifiche non salvate dell'ultima volta tornano comunque
                ws.merge_session();
            }
            ws.select(0);
            ws
        }
        None => {
            let ripresa = sessione
                .as_ref()
                .and_then(|d| Workspace::from_session(d.clone(), cfg.clone()));
            match ripresa {
                Some(ws) => ws,
                None => {
                    let Some(file) = scegli(cat.t("gui.open_title")) else {
                        return 0;
                    };
                    let app = match App::open(&file, cfg.clone()) {
                        Ok(app) => app,
                        Err(e) => return errore_apertura(&file, &e),
                    };
                    let ws = Workspace::new(app, cfg);
                    match &sessione {
                        Some(dir) => ws.with_session(dir.clone()),
                        None => ws,
                    }
                }
            }
        }
    };
    let opzioni = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1000.0, 700.0])
            .with_min_inner_size([320.0, 200.0])
            .with_drag_and_drop(true)
            .with_title("Manto.RustPad"),
        ..Default::default()
    };
    let esito = eframe::run_native(
        "Manto.RustPad",
        opzioni,
        Box::new(move |cc| {
            let finestra = Finestra::new_ws(cc, ws).con_selettore(Box::new(|t: &str| scegli(t)));
            // uso da sviluppatore: MRPG_SCREENSHOT=file.png salva la schermata ed esce
            Ok(Box::new(match std::env::var_os("MRPG_SCREENSHOT") {
                Some(p) => finestra.con_scatto(p.into()),
                None => finestra,
            }))
        }),
    );
    match esito {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("{e}");
            1
        }
    }
}

/// Il dialogo nativo di apertura file: il percorso scelto, o `None` se si annulla.
fn scegli(titolo: &str) -> Option<std::path::PathBuf> {
    rfd::FileDialog::new().set_title(titolo).pick_file()
}

/// Gli argomenti non sono validi: l'uso, nella lingua scelta.
fn uso(cat: &Catalog) -> i32 {
    eprintln!("{}", cat.tf("cli.err.no_file", &[]));
    2
}
