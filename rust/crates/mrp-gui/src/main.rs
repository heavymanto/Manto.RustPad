//! Il binario `mrpg`: la versione grafica di Manto.RustPad. Stessi argomenti del TUI (`mrp`) per file, tema, lingua e
//! codifica; apre la finestra con l'`App` del TUI disegnata da egui. Codici di uscita: 0 tutto bene, 1 il file non si
//! apre o la finestra non parte, 2 errore d'uso.

use eframe::egui;
use mrp_core::lang::Catalog;
use mrp_gui::gui::Finestra;
use mrp_tui::app::{App, AppConfig};
use mrp_tui::args;

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
        println!("mrpg FILE [--encoding NAME] [--theme NAME] [--lang CODE]");
        return 0;
    }
    let Some(file) = &a.file else {
        return uso(&cat);
    };
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
        settings_path: profilo.path,
        index: true,
        encoding,
    };
    let app = match App::open(file, cfg.clone()) {
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
    // un file trascinato sulla finestra si apre con la codifica riconosciuta, non con quella imposta per il primo
    let nuova = AppConfig {
        encoding: None,
        ..cfg
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
            let finestra = Finestra::new(cc, app, Box::new(move || nuova.clone()));
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

/// Manca il file: l'uso, nella lingua scelta.
fn uso(cat: &Catalog) -> i32 {
    eprintln!("{}", cat.tf("cli.err.no_file", &[]));
    2
}
