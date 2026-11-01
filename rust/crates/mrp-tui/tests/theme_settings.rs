//! Test di temi e impostazioni del TUI. Scritti da ARCH prima dell'implementazione (tecnica di Manto.FreeDisk:
//! quattro temi nel codice, nomi dalle lingue, impostazioni in un file chiave=valore tollerante).

use mrp_core::lang::Lang;
use mrp_tui::settings::Settings;
use mrp_tui::theme::{Theme, ThemeId};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

struct TestDir(PathBuf);

impl TestDir {
    fn new() -> TestDir {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let p = std::env::temp_dir().join(format!("mrp_ts_test_{}_{}", std::process::id(), n));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        TestDir(p)
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

// ---------- temi ----------

#[test]
fn the_six_themes_in_menu_order_with_their_codes() {
    assert_eq!(
        ThemeId::ALL,
        [
            ThemeId::Night,
            ThemeId::Debian,
            ThemeId::Console,
            ThemeId::Minimal,
            ThemeId::Dark,
            ThemeId::Light
        ]
    );
    assert_eq!(ThemeId::DEFAULT, ThemeId::Console);
    let codici: Vec<&str> = ThemeId::ALL.iter().map(|t| t.code()).collect();
    assert_eq!(
        codici,
        vec!["night", "debian", "console", "minimal", "dark", "light"]
    );
    let chiavi: Vec<&str> = ThemeId::ALL.iter().map(|t| t.key()).collect();
    assert_eq!(
        chiavi,
        vec![
            "theme.night",
            "theme.debian",
            "theme.console",
            "theme.minimal",
            "theme.dark",
            "theme.light"
        ]
    );
    let descr: Vec<&str> = ThemeId::ALL.iter().map(|t| t.blurb_key()).collect();
    assert_eq!(
        descr,
        vec![
            "theme.night.b",
            "theme.debian.b",
            "theme.console.b",
            "theme.minimal.b",
            "theme.dark.b",
            "theme.light.b"
        ]
    );
}

#[test]
fn dark_and_light_are_the_window_themes_with_true_colours() {
    use mrp_tui::theme::Theme;
    use ratatui::style::Color;
    assert!(ThemeId::Dark.is_dark() && !ThemeId::Light.is_dark());
    // gli altri quattro sono scuri (le finestre li disegnano con i controlli scuri)
    for t in [
        ThemeId::Night,
        ThemeId::Debian,
        ThemeId::Console,
        ThemeId::Minimal,
    ] {
        assert!(t.is_dark(), "{t:?}");
    }
    let d = Theme::for_id(ThemeId::Dark);
    assert_eq!(d.screen.bg, Some(Color::Rgb(0x1e, 0x1e, 0x1e)));
    assert_eq!(d.fg.fg, Some(Color::Rgb(0xd4, 0xd4, 0xd4)));
    assert_eq!(d.selection.bg, Some(Color::Rgb(0x26, 0x4f, 0x78)));
    let l = Theme::for_id(ThemeId::Light);
    assert_eq!(l.screen.bg, Some(Color::Rgb(0xff, 0xff, 0xff)));
    assert_eq!(l.fg.fg, Some(Color::Rgb(0x1f, 0x1f, 0x1f)));
    assert_eq!(l.selection.bg, Some(Color::Rgb(0xad, 0xd6, 0xff)));
}

#[test]
fn theme_names_exist_in_the_embedded_english() {
    let en = Lang::english();
    for t in ThemeId::ALL {
        assert!(en.get(t.key()).is_some(), "manca {} in en.json", t.key());
        assert!(
            en.get(t.blurb_key()).is_some(),
            "manca {} in en.json",
            t.blurb_key()
        );
    }
}

#[test]
fn parse_is_tolerant_and_round_trips_the_codes() {
    for t in ThemeId::ALL {
        assert_eq!(ThemeId::parse(t.code()), Some(t));
        assert_eq!(
            ThemeId::parse(&format!("  {}\t", t.code().to_uppercase())),
            Some(t)
        );
    }
    assert_eq!(ThemeId::parse("Night"), Some(ThemeId::Night));
    assert_eq!(
        ThemeId::parse("notturno"),
        None,
        "il codice e' quello inglese"
    );
    assert_eq!(ThemeId::parse(""), None);
    assert_eq!(ThemeId::parse("blu"), None);
}

#[test]
fn every_theme_builds_and_the_roles_are_told_apart() {
    for t in ThemeId::ALL {
        let th = Theme::for_id(t);
        assert_ne!(th.selected, th.fg, "{t:?}: la selezione si deve vedere");
        assert_ne!(
            th.gutter, th.fg,
            "{t:?}: i numeri di riga si distinguono dal testo"
        );
        assert_ne!(th.found, th.fg, "{t:?}: il trovato si distingue dal testo");
        assert_ne!(
            th.mode_edit, th.mode_view,
            "{t:?}: VIEW e EDIT si distinguono"
        );
        assert_ne!(
            th.key, th.fg,
            "{t:?}: i numeri dei tasti funzione si distinguono"
        );
    }
}

#[test]
fn the_themes_differ_where_they_should() {
    let n = Theme::for_id(ThemeId::Night);
    let d = Theme::for_id(ThemeId::Debian);
    let c = Theme::for_id(ThemeId::Console);
    let m = Theme::for_id(ThemeId::Minimal);
    assert_eq!(n.screen, d.screen, "stesso blu: le differenze sono altrove");
    assert_ne!(n.selected, d.selected);
    assert_ne!(d.selected, c.selected);
    assert_ne!(m.screen, n.screen);
    assert_ne!(m.screen, c.screen);
    assert_ne!(d.dialog, n.dialog);
}

// ---------- impostazioni ----------

#[test]
fn defaults() {
    let s = Settings::default();
    assert_eq!(s.theme, ThemeId::Console);
    assert_eq!(s.lang, None);
}

fn dati(text: &str) -> Vec<&str> {
    text.lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
        .collect()
}

#[test]
fn to_text_is_key_value_lines_with_comments() {
    let s = Settings {
        theme: ThemeId::Debian,
        lang: Some("it".to_string()),
        theme_set: true,
        ..Settings::default()
    };
    let t = s.to_text();
    assert!(t.starts_with('#'), "un commento in testa spiega il file");
    assert_eq!(dati(&t), vec!["theme=debian", "lang=it"]);
    // senza un tema scelto non si scrive nemmeno il tema (la finestra segue il sistema), e senza lingua scelta nemmeno quella
    let s = Settings::default();
    assert_eq!(dati(&s.to_text()), Vec::<&str>::new());
    let s = Settings {
        theme_set: true,
        ..Settings::default()
    };
    assert_eq!(dati(&s.to_text()), vec!["theme=console"]);
    assert!(s.to_text().is_ascii());
}

#[test]
fn round_trip_text_to_settings() {
    for t in ThemeId::ALL {
        for lang in [None, Some("it".to_string()), Some("pt-BR".to_string())] {
            let s = Settings {
                theme: t,
                lang,
                theme_set: true,
                ..Settings::default()
            };
            assert_eq!(Settings::from_text(&s.to_text()), s);
        }
    }
}

#[test]
fn from_text_never_fails_on_odd_files() {
    // commenti, righe vuote, chiavi sconosciute, spazi, CRLF, BOM, valori sbagliati
    let s = Settings::from_text(
        "\u{feff}# ciao\r\n\r\n  theme = NIGHT \r\nlang=  de \r\ncolor=red\r\nsenza uguale\r\n",
    );
    assert_eq!(s.theme, ThemeId::Night);
    assert_eq!(s.lang.as_deref(), Some("de"));
    let s = Settings::from_text("theme=arcobaleno\nlang=\n");
    assert_eq!(
        s,
        Settings::default(),
        "tema sconosciuto e lingua vuota: default"
    );
    let s = Settings::from_text("");
    assert_eq!(s, Settings::default());
    let s = Settings::from_text("theme=debian\ntheme=minimal\n");
    assert_eq!(s.theme, ThemeId::Minimal, "l'ultima vince");
    // binario: non panica
    let _ = Settings::from_text("\u{0}\u{1}=\u{2}\n===\n=\n");
}

#[test]
fn save_and_load_through_a_file() {
    let dir = TestDir::new();
    let path = dir.0.join("sotto").join("cartella").join("settings.txt");
    assert!(
        Settings::load(&path).is_none(),
        "file assente: nessuna impostazione"
    );
    let s = Settings {
        theme: ThemeId::Minimal,
        lang: Some("ja".to_string()),
        theme_set: true,
        ..Settings::default()
    };
    s.save(&path).unwrap();
    assert!(path.is_file(), "save crea le cartelle del profilo");
    assert_eq!(Settings::load(&path), Some(s));
}

#[test]
fn default_path_lives_in_the_mrp_profile_folder() {
    // non si legge ne' si scrive niente: si controlla solo la forma del percorso
    if let Some(p) = Settings::default_path() {
        assert_eq!(p.file_name().and_then(|n| n.to_str()), Some("settings.txt"));
        assert_eq!(
            p.parent()
                .and_then(|d| d.file_name())
                .and_then(|n| n.to_str()),
            Some("mrp")
        );
    }
}

#[test]
fn jump_pages_default_to_ten_hundred_thousand_and_are_not_written() {
    let s = Settings::default();
    assert_eq!(s.jump_pages, [10, 100, 1000]);
    assert!(
        !s.to_text().contains("jump_pages"),
        "il valore di fabbrica non si scrive"
    );
}

#[test]
fn jump_pages_round_trip_and_tolerate_spaces() {
    let s = Settings {
        jump_pages: [5, 50, 500],
        theme_set: true,
        ..Settings::default()
    };
    assert_eq!(
        dati(&s.to_text()),
        vec!["theme=console", "jump_pages=5,50,500"]
    );
    assert_eq!(Settings::from_text(&s.to_text()), s);
    let t = Settings::from_text("jump_pages = 3 , 30 ,300 \r\n");
    assert_eq!(t.jump_pages, [3, 30, 300]);
    // i limiti: da 2 a 10000 pagine
    let t = Settings::from_text("jump_pages=2,9999,10000");
    assert_eq!(t.jump_pages, [2, 9999, 10_000]);
}

#[test]
fn bad_jump_pages_fall_back_to_the_defaults_as_a_whole() {
    for brutto in [
        "jump_pages=",
        "jump_pages=10,100",
        "jump_pages=4,40,400,5",
        "jump_pages=1,100,1000",
        "jump_pages=10,100,10001",
        "jump_pages=0,0,0",
        "jump_pages=a,b,c",
        "jump_pages=10;100;1000",
        "jump_pages=-5,100,1000",
    ] {
        assert_eq!(
            Settings::from_text(brutto).jump_pages,
            [10, 100, 1000],
            "{brutto}"
        );
    }
    // una riga sbagliata dopo una buona non la cancella
    let t = Settings::from_text("jump_pages=4,40,400\njump_pages=x\n");
    assert_eq!(t.jump_pages, [4, 40, 400]);
}

#[test]
fn nav_bar_is_off_by_default_written_only_when_on_and_read_back() {
    assert!(!Settings::default().nav_bar);
    assert!(!Settings::default().to_text().contains("nav_bar"));
    let s = Settings {
        nav_bar: true,
        theme_set: true,
        ..Settings::default()
    };
    assert_eq!(dati(&s.to_text()), vec!["theme=console", "nav_bar=1"]);
    assert_eq!(Settings::from_text(&s.to_text()), s);
    for si in ["nav_bar=1", "nav_bar = on", "nav_bar=true"] {
        assert!(Settings::from_text(si).nav_bar, "{si}");
    }
    // la chiave e' minuscola: la maiuscola e' sconosciuta
    assert!(!Settings::from_text("NAV_BAR=1").nav_bar);
    for no in [
        "nav_bar=0",
        "nav_bar=off",
        "nav_bar=false",
        "nav_bar=",
        "nav_bar=2",
        "nav_bar=si",
    ] {
        assert!(!Settings::from_text(no).nav_bar, "{no}");
    }
    // l'ultima valida vince
    assert!(!Settings::from_text("nav_bar=1\nnav_bar=0\n").nav_bar);
    assert!(Settings::from_text("nav_bar=1\nnav_bar=boh\n").nav_bar);
}

#[test]
fn a_theme_in_the_file_means_the_user_chose_one() {
    assert!(!Settings::default().theme_set);
    assert!(!Settings::from_text("").theme_set);
    assert!(!Settings::from_text("lang=it\n").theme_set);
    assert!(
        !Settings::from_text("theme=arcobaleno\n").theme_set,
        "un tema che non esiste non conta"
    );
    assert!(Settings::from_text("theme=light\n").theme_set);
    assert_eq!(Settings::from_text("theme=light\n").theme, ThemeId::Light);
    assert!(
        Settings::from_text("theme=console\n").theme_set,
        "anche il predefinito, se scelto"
    );
    // uscendo da un file scritto dal programma la scelta c'e'
    let s = Settings::from_text("theme=dark\n");
    assert!(Settings::from_text(&s.to_text()).theme_set);
}

#[test]
fn the_theme_is_explicit_with_the_option_or_with_a_saved_choice() {
    use mrp_tui::args::{self, Args};
    let nessuno = Args::default();
    assert!(!args::theme_is_explicit(&nessuno, &None));
    assert!(!args::theme_is_explicit(
        &nessuno,
        &Some(Settings::default())
    ));
    let con_opzione = Args {
        theme: Some("night".to_string()),
        ..Args::default()
    };
    assert!(args::theme_is_explicit(&con_opzione, &None));
    assert!(args::theme_is_explicit(
        &nessuno,
        &Some(Settings::from_text("theme=dark\n"))
    ));
}
