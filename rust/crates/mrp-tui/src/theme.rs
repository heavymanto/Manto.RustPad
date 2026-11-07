//! I quattro temi del TUI (F6): **Night** (blu Norton/Total Commander, selezione gialla), **Debian** (sfondo blu
//! dell'installer, finestre grigie, selezione rossa), **Console** (il predefinito: bianco su nero, accenti gialli,
//! selezione verde) e **Minimal** (nessun colore imposto: quelli del terminale). 16 colori ANSI, niente truecolor.
//! Stessa tecnica di Manto.FreeDisk (`mfd-tui/src/theme.rs`, da cui si porta): i temi sono codice, i nomi nel menu
//! vengono dalle lingue (`key`/`blurb_key`).

use ratatui::style::{Color, Modifier, Style};

/// L'identita' di un tema: quello che si sceglie nel menu e che l'app ricorda. L'ordine delle varianti e' l'ordine
/// del menu.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ThemeId {
    Night,
    Debian,
    Console,
    Minimal,
    /// Scuro: grigio scuro morbido, colori veri (RGB); il tema della finestra.
    Dark,
    /// Chiaro: pagina bianca, testo scuro, colori veri (RGB); il tema della finestra.
    Light,
}

impl ThemeId {
    /// I sei temi, nell'ordine del menu (i quattro del terminale, poi Scuro e Chiaro).
    pub const ALL: [ThemeId; 6] = [
        ThemeId::Night,
        ThemeId::Debian,
        ThemeId::Console,
        ThemeId::Minimal,
        ThemeId::Dark,
        ThemeId::Light,
    ];

    /// Il tema e' scuro? Decide se i controlli della finestra (menu, pulsanti, finestre) sono scuri o chiari: solo Chiaro e'
    /// chiaro.
    pub fn is_dark(self) -> bool {
        self != ThemeId::Light
    }

    /// Il tema di partenza e di fabbrica: Console.
    pub const DEFAULT: ThemeId = ThemeId::Console;

    /// Chiave della lingua per il nome nel menu: `theme.night`, `theme.debian`, `theme.console`, `theme.minimal`.
    pub fn key(self) -> &'static str {
        match self {
            ThemeId::Night => "theme.night",
            ThemeId::Debian => "theme.debian",
            ThemeId::Console => "theme.console",
            ThemeId::Minimal => "theme.minimal",
            ThemeId::Dark => "theme.dark",
            ThemeId::Light => "theme.light",
        }
    }

    /// Chiave della lingua per la riga descrittiva: la chiave del nome piu' `.b`.
    pub fn blurb_key(self) -> &'static str {
        match self {
            ThemeId::Night => "theme.night.b",
            ThemeId::Debian => "theme.debian.b",
            ThemeId::Console => "theme.console.b",
            ThemeId::Minimal => "theme.minimal.b",
            ThemeId::Dark => "theme.dark.b",
            ThemeId::Light => "theme.light.b",
        }
    }

    /// Codice ASCII nel file delle impostazioni e in `--theme`: `night`, `debian`, `console`, `minimal`.
    pub fn code(self) -> &'static str {
        match self {
            ThemeId::Night => "night",
            ThemeId::Debian => "debian",
            ThemeId::Console => "console",
            ThemeId::Minimal => "minimal",
            ThemeId::Dark => "dark",
            ThemeId::Light => "light",
        }
    }

    /// Il tema dal suo codice, tollerante a maiuscole e spazi; `None` se non e' uno dei quattro.
    pub fn parse(s: &str) -> Option<ThemeId> {
        match s.trim().to_ascii_lowercase().as_str() {
            "night" => Some(ThemeId::Night),
            "debian" => Some(ThemeId::Debian),
            "console" => Some(ThemeId::Console),
            "minimal" => Some(ThemeId::Minimal),
            "dark" => Some(ThemeId::Dark),
            "light" => Some(ThemeId::Light),
            _ => None,
        }
    }
}

/// Gli stili della colorazione dei log (Alt+L): il livello e il colore del resto della riga per gli errori e gli avvisi,
/// l'ora e l'indirizzo.
#[derive(Clone, Copy, Debug)]
pub struct LogStyles {
    pub error: Style,
    pub warn: Style,
    pub info: Style,
    pub debug: Style,
    pub time: Style,
    pub ip: Style,
}

/// Un tema completo: uno stile per ruolo.
#[derive(Clone, Copy, Debug)]
pub struct Theme {
    /// La colorazione dei log.
    pub log: LogStyles,
    /// Sfondo di tutto il TUI e testo di default.
    pub screen: Style,
    /// Sfondo delle finestre di dialogo (puo' differire da `screen`).
    pub dialog: Style,
    /// Testo del documento.
    pub fg: Style,
    /// Titoli delle finestre e barra del titolo.
    pub title: Style,
    /// Suggerimenti e note a margine.
    pub hint: Style,
    /// I numeri dei tasti funzione nella barra in fondo.
    pub key: Style,
    /// Riga di stato e messaggi transitori.
    pub status: Style,
    /// La voce selezionata nei menu.
    pub selected: Style,
    /// Bordo delle finestre.
    pub border: Style,
    /// Numeri di riga.
    pub gutter: Style,
    /// Testo trovato dalla ricerca.
    pub found: Style,
    /// Le celle del testo selezionato (Maiusc+frecce, Ctrl+A): vince sullo stile del trovato.
    pub selection: Style,
    /// I segni `«` e `»` di scorrimento orizzontale.
    pub more: Style,
    /// Il marcatore VIEW nella barra del titolo.
    pub mode_view: Style,
    /// Il marcatore EDIT (e il segno di modificato) nella barra del titolo.
    pub mode_edit: Style,
    /// Primo piano del velo che scurisce cio' che sta dietro le finestre.
    pub shadow_fg: Color,
    /// Fondo del velo: `None` per Minimal (lascia i colori del terminale).
    pub shadow_bg: Option<Color>,
}

impl Theme {
    pub fn for_id(id: ThemeId) -> Theme {
        match id {
            ThemeId::Night => Theme::night(),
            ThemeId::Debian => Theme::debian(),
            ThemeId::Console => Theme::console(),
            ThemeId::Minimal => Theme::minimal(),
            ThemeId::Dark => Theme::dark(),
            ThemeId::Light => Theme::light(),
        }
    }

    /// Minimal: nessun colore imposto, restano quelli del terminale (su un terminale nero e' il tema nero).
    /// Selezione e trovato a video inverso, il marcatore EDIT aggiunge il grassetto. L'ombra tocca solo il primo
    /// piano: il fondo resta del terminale.
    fn minimal() -> Theme {
        Theme {
            screen: Style::new(),
            dialog: Style::new(),
            fg: Style::new(),
            log: LogStyles {
                error: Style::new().add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
                warn: Style::new().add_modifier(Modifier::BOLD),
                info: Style::new(),
                debug: Style::new().add_modifier(Modifier::DIM),
                time: Style::new().add_modifier(Modifier::DIM),
                ip: Style::new().add_modifier(Modifier::UNDERLINED),
            },
            title: Style::new().add_modifier(Modifier::BOLD),
            hint: Style::new().add_modifier(Modifier::DIM),
            key: Style::new().add_modifier(Modifier::BOLD),
            status: Style::new().add_modifier(Modifier::BOLD),
            selected: Style::new().add_modifier(Modifier::REVERSED),
            border: Style::new().add_modifier(Modifier::BOLD),
            gutter: Style::new().add_modifier(Modifier::DIM),
            found: Style::new().add_modifier(Modifier::REVERSED),
            selection: Style::new().add_modifier(Modifier::REVERSED),
            more: Style::new().add_modifier(Modifier::BOLD),
            mode_view: Style::new().add_modifier(Modifier::BOLD),
            mode_edit: Style::new().add_modifier(Modifier::REVERSED | Modifier::BOLD),
            shadow_fg: Color::DarkGray,
            shadow_bg: None,
        }
    }

    /// Night: il blu Norton/Total Commander. Sfondo blu, testo bianco, ciano per titoli, suggerimenti e numeri di
    /// riga, giallo per gli accenti; selezione, trovato e marcatore EDIT nero su giallo. Ombra nera a velo.
    fn night() -> Theme {
        // il blu vero di Norton / Total Commander (VGA #0000AA), non quello della tavolozza del terminale
        let blu = Color::Rgb(0x00, 0x00, 0xaa);
        Theme {
            screen: Style::new().bg(blu).fg(Color::White),
            dialog: Style::new().bg(blu).fg(Color::White),
            fg: Style::new().fg(Color::White),
            log: LogStyles {
                error: Style::new().fg(Color::LightRed).bold(),
                warn: Style::new().fg(Color::Yellow),
                info: Style::new().fg(Color::LightGreen),
                debug: Style::new().fg(Color::Gray),
                time: Style::new().fg(Color::Cyan),
                ip: Style::new().fg(Color::LightMagenta),
            },
            title: Style::new().fg(Color::Cyan).bold(),
            hint: Style::new().fg(Color::Cyan),
            key: Style::new().fg(Color::Yellow).bold(),
            status: Style::new().fg(Color::Yellow),
            selected: Style::new().fg(Color::Black).bg(Color::Yellow),
            border: Style::new().fg(Color::Yellow),
            gutter: Style::new().fg(Color::Cyan),
            found: Style::new().fg(Color::Black).bg(Color::Yellow),
            selection: Style::new().fg(Color::Black).bg(Color::Cyan),
            more: Style::new().fg(Color::Yellow).bold(),
            mode_view: Style::new().fg(Color::Yellow),
            mode_edit: Style::new().fg(Color::Black).bg(Color::Yellow),
            shadow_fg: Color::DarkGray,
            shadow_bg: Some(Color::Black),
        }
    }

    /// Debian: lo sfondo blu dell'installer, con i colori dei dialoghi debconf: finestre grigio chiaro (il testo
    /// nero dentro le finestre e' lo stile `dialog`), accenti e selezione rossi. Il documento resta bianco sul blu
    /// dello schermo; trovato e marcatore EDIT bianco su rosso. Ombra nera a velo.
    fn debian() -> Theme {
        // lo stesso blu dell'installer (VGA #0000AA)
        Theme {
            screen: Style::new()
                .bg(Color::Rgb(0x00, 0x00, 0xaa))
                .fg(Color::White),
            dialog: Style::new().bg(Color::Gray).fg(Color::Black),
            fg: Style::new().fg(Color::White),
            log: LogStyles {
                error: Style::new().fg(Color::LightRed).bold(),
                warn: Style::new().fg(Color::Yellow),
                info: Style::new().fg(Color::LightGreen),
                debug: Style::new().fg(Color::Gray),
                time: Style::new().fg(Color::Cyan),
                ip: Style::new().fg(Color::LightMagenta),
            },
            title: Style::new().fg(Color::Red).bold(),
            hint: Style::new().fg(Color::DarkGray),
            key: Style::new().fg(Color::Red).bold(),
            status: Style::new().fg(Color::Red),
            selected: Style::new().fg(Color::White).bg(Color::Red),
            border: Style::new().fg(Color::Red),
            gutter: Style::new().fg(Color::DarkGray),
            found: Style::new().fg(Color::White).bg(Color::Red),
            selection: Style::new().fg(Color::Black).bg(Color::Gray),
            more: Style::new().fg(Color::Red).bold(),
            mode_view: Style::new().fg(Color::Red),
            mode_edit: Style::new().fg(Color::White).bg(Color::Red),
            shadow_fg: Color::DarkGray,
            shadow_bg: Some(Color::Black),
        }
    }

    /// Scuro: grigio scuro morbido (#1e1e1e), testo grigio chiaro, accenti azzurri, selezione blu scuro, trovato giallo.
    fn dark() -> Theme {
        let rgb = |r, g, b| Color::Rgb(r, g, b);
        Theme {
            screen: Style::new()
                .bg(rgb(0x1e, 0x1e, 0x1e))
                .fg(rgb(0xd4, 0xd4, 0xd4)),
            dialog: Style::new()
                .bg(rgb(0x25, 0x25, 0x26))
                .fg(rgb(0xd4, 0xd4, 0xd4)),
            fg: Style::new().fg(rgb(0xd4, 0xd4, 0xd4)),
            log: LogStyles {
                error: Style::new().fg(rgb(0xf4, 0x87, 0x71)).bold(),
                warn: Style::new().fg(rgb(0xe5, 0xc0, 0x7b)),
                info: Style::new().fg(rgb(0x89, 0xd1, 0x85)),
                debug: Style::new().fg(rgb(0x56, 0x9c, 0xd6)),
                time: Style::new().fg(rgb(0x4e, 0xc9, 0xb0)),
                ip: Style::new().fg(rgb(0xc5, 0x86, 0xc0)),
            },
            title: Style::new().fg(rgb(0x9c, 0xdc, 0xfe)).bold(),
            hint: Style::new().fg(rgb(0x80, 0x80, 0x80)),
            key: Style::new().fg(rgb(0xdc, 0xdc, 0xaa)).bold(),
            status: Style::new().fg(rgb(0xcc, 0xcc, 0xcc)),
            selected: Style::new()
                .fg(rgb(0xff, 0xff, 0xff))
                .bg(rgb(0x09, 0x47, 0x71)),
            border: Style::new().fg(rgb(0x56, 0x9c, 0xd6)),
            gutter: Style::new().fg(rgb(0x85, 0x85, 0x85)),
            found: Style::new()
                .fg(rgb(0x00, 0x00, 0x00))
                .bg(rgb(0xff, 0xd8, 0x66)),
            selection: Style::new()
                .fg(rgb(0xff, 0xff, 0xff))
                .bg(rgb(0x26, 0x4f, 0x78)),
            more: Style::new().fg(rgb(0x56, 0x9c, 0xd6)).bold(),
            mode_view: Style::new().fg(rgb(0x9c, 0xdc, 0xfe)),
            mode_edit: Style::new()
                .fg(rgb(0x1e, 0x1e, 0x1e))
                .bg(rgb(0xdc, 0xdc, 0xaa)),
            shadow_fg: rgb(0x55, 0x55, 0x55),
            shadow_bg: Some(rgb(0x10, 0x10, 0x10)),
        }
    }

    /// Chiaro: pagina bianca, testo quasi nero (#1f1f1f), accenti blu, selezione azzurra, trovato giallo chiaro.
    fn light() -> Theme {
        let rgb = |r, g, b| Color::Rgb(r, g, b);
        Theme {
            screen: Style::new()
                .bg(rgb(0xff, 0xff, 0xff))
                .fg(rgb(0x1f, 0x1f, 0x1f)),
            dialog: Style::new()
                .bg(rgb(0xf3, 0xf3, 0xf3))
                .fg(rgb(0x1f, 0x1f, 0x1f)),
            fg: Style::new().fg(rgb(0x1f, 0x1f, 0x1f)),
            log: LogStyles {
                error: Style::new().fg(rgb(0xa3, 0x15, 0x15)).bold(),
                warn: Style::new().fg(rgb(0x98, 0x68, 0x01)),
                info: Style::new().fg(rgb(0x09, 0x86, 0x58)),
                debug: Style::new().fg(rgb(0x00, 0x66, 0xbf)),
                time: Style::new().fg(rgb(0x26, 0x7f, 0x99)),
                ip: Style::new().fg(rgb(0xaf, 0x00, 0xdb)),
            },
            title: Style::new().fg(rgb(0x04, 0x51, 0xa5)).bold(),
            hint: Style::new().fg(rgb(0x6e, 0x6e, 0x6e)),
            key: Style::new().fg(rgb(0x79, 0x5e, 0x26)).bold(),
            status: Style::new().fg(rgb(0x33, 0x33, 0x33)),
            selected: Style::new()
                .fg(rgb(0xff, 0xff, 0xff))
                .bg(rgb(0x00, 0x60, 0xc0)),
            border: Style::new().fg(rgb(0x04, 0x51, 0xa5)),
            gutter: Style::new().fg(rgb(0x8a, 0x8a, 0x8a)),
            found: Style::new()
                .fg(rgb(0x00, 0x00, 0x00))
                .bg(rgb(0xff, 0xe5, 0x8f)),
            selection: Style::new()
                .fg(rgb(0x00, 0x00, 0x00))
                .bg(rgb(0xad, 0xd6, 0xff)),
            more: Style::new().fg(rgb(0x04, 0x51, 0xa5)).bold(),
            mode_view: Style::new().fg(rgb(0x04, 0x51, 0xa5)),
            mode_edit: Style::new()
                .fg(rgb(0xff, 0xff, 0xff))
                .bg(rgb(0xa3, 0x15, 0x15)),
            shadow_fg: rgb(0xb0, 0xb0, 0xb0),
            shadow_bg: Some(rgb(0xe8, 0xe8, 0xe8)),
        }
    }

    /// Console: il tema di fabbrica. Nero, testo bianco, accenti gialli, grigio per il secondario, numeri di riga
    /// in grigio scuro; il selettore e' il verde chiaro delle console vere, trovato e marcatore EDIT nero su
    /// giallo. L'ombra scolorisce in grigio scuro, a velo: l'effetto popup, non la lastra piena.
    fn console() -> Theme {
        Theme {
            screen: Style::new().bg(Color::Black).fg(Color::White),
            dialog: Style::new().bg(Color::Black).fg(Color::White),
            fg: Style::new().fg(Color::White),
            log: LogStyles {
                error: Style::new().fg(Color::LightRed).bold(),
                warn: Style::new().fg(Color::Yellow),
                info: Style::new().fg(Color::Green),
                debug: Style::new().fg(Color::LightBlue),
                time: Style::new().fg(Color::Cyan),
                ip: Style::new().fg(Color::Magenta),
            },
            title: Style::new().fg(Color::Yellow).bold(),
            hint: Style::new().fg(Color::Gray),
            key: Style::new().fg(Color::Yellow).bold(),
            status: Style::new().fg(Color::Yellow),
            selected: Style::new().fg(Color::Black).bg(Color::LightGreen),
            border: Style::new().fg(Color::Yellow),
            gutter: Style::new().fg(Color::DarkGray),
            found: Style::new().fg(Color::Black).bg(Color::Yellow),
            selection: Style::new().fg(Color::White).bg(Color::Blue),
            more: Style::new().fg(Color::Yellow).bold(),
            mode_view: Style::new().fg(Color::Yellow),
            mode_edit: Style::new().fg(Color::Black).bg(Color::Yellow),
            shadow_fg: Color::DarkGray,
            shadow_bg: Some(Color::Black),
        }
    }
}
