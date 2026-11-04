//! Le impostazioni persistenti del TUI: il tema e la lingua. Si salvano da sole quando l'utente cambia tema o lingua
//! (F6/F7) nel profilo utente: `%APPDATA%\mrp\settings.txt` su Windows, `~/.config/mrp/settings.txt` altrove. Formato
//! ASCII, una riga `chiave=valore` (con `#` per i commenti). Eliminare il file riporta ai valori di fabbrica.
//! Stessa tecnica di Manto.FreeDisk (`mfd-tui/src/settings.rs`): `from_text` e' tollerante, le righe che non capisce
//! diventano default, mai un errore.

use crate::theme::ThemeId;
use mrp_core::FileSource;
use std::path::{Path, PathBuf};

/// Le impostazioni che sopravvivono alla sessione.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Settings {
    pub theme: ThemeId,
    /// Codice della lingua scelta (`it`, `pt-BR`...); `None`: l'inglese incorporato. Lo valida chi lo usa.
    pub lang: Option<String>,
    /// Di quante pagine saltano Alt+PgUp/PgDn, Alt+Maiusc+PgUp/PgDn e Ctrl+Alt+PgUp/PgDn.
    pub jump_pages: [usize; 3],
    /// La barra dei salti (Ctrl+B) sotto la barra delle schede.
    pub nav_bar: bool,
    /// I caratteri speciali (spazi, tabulazioni, fine riga) si vedono (Alt+W).
    pub special_chars: bool,
    /// La colorazione dei log (livelli, orari, indirizzi) e' accesa (Alt+L).
    pub log_colors: bool,
    /// I numeri di riga a sinistra del testo si vedono (di fabbrica si': nel file si scrive solo `line_numbers=0`).
    pub line_numbers: bool,
    /// Il corpo del testo della finestra in punti (8..=48); `None`: quello di fabbrica.
    pub font_size: Option<u16>,
    /// L'indice righe si costruisce (di fabbrica si': nel file si scrive solo `index=0`). Senza, niente numeri di riga, "vai alla
    /// riga N", totale delle righe.
    pub index: bool,
    /// Il formato standard dell'orario scelto a mano (pattern e quanto lontano dall'inizio riga puo' stare): si prova per primo
    /// quando si apre un file. `None`: ci pensa il riconoscimento.
    pub time_format: Option<(String, usize)>,
    /// Le regole per leggere il tempo che l'utente ha scritto a mano (pattern, quanto lontano dall'inizio riga, quale occorrenza):
    /// compaiono fra i tempi da scegliere nei file dove leggono qualcosa. Al piu' otto, le piu' recenti in fondo.
    pub time_rules: Vec<(String, usize, usize)>,
    /// Vero se il file ha un tema valido: l'utente ne ha scelto uno (la finestra, altrimenti, segue il sistema).
    pub theme_set: bool,
    /// La timeline com'era quando si e' chiuso: si riapre uguale (accesa, passo, disposizione, periodi vuoti).
    pub timeline: crate::app::timeline::TimelinePref,
}

/// I salti di pagina di fabbrica: 10, 100 e 1000 pagine.
pub const JUMP_PAGES: [usize; 3] = [10, 100, 1000];
/// I limiti di un salto configurato: da 2 a 10000 pagine (oltre, il salto camminerebbe troppe righe sul thread dell'interfaccia).
pub const JUMP_MIN: usize = 2;
pub const JUMP_MAX: usize = 10_000;

impl Default for Settings {
    fn default() -> Self {
        Settings {
            theme: ThemeId::DEFAULT,
            lang: None,
            jump_pages: JUMP_PAGES,
            nav_bar: false,
            special_chars: false,
            log_colors: false,
            line_numbers: true,
            font_size: None,
            index: true,
            time_format: None,
            time_rules: Vec::new(),
            theme_set: false,
            timeline: crate::app::timeline::TimelinePref::default(),
        }
    }
}

/// `a,b,c` con tre numeri fra `JUMP_MIN` e `JUMP_MAX`; qualunque altra cosa: `None` (si resta ai valori di fabbrica).
fn parse_jump(v: &str) -> Option<[usize; 3]> {
    let mut it = v.split(',').map(|p| p.trim().parse::<usize>().ok());
    let a = it.next()??;
    let b = it.next()??;
    let c = it.next()??;
    if it.next().is_some() {
        return None;
    }
    let ok = |n: usize| (JUMP_MIN..=JUMP_MAX).contains(&n);
    (ok(a) && ok(b) && ok(c)).then_some([a, b, c])
}

impl Settings {
    /// Il testo del file: un commento in testa, poi `theme=<codice>` e, se c'e' una lingua scelta, `lang=<codice>`.
    pub fn to_text(&self) -> String {
        let mut out = String::new();
        out.push_str("# mrp-tui: theme and language of the TUI, saved by the program.\n");
        out.push_str("# Delete this file to return to the factory defaults.\n");
        // il tema c'e' nel file solo se l'utente l'ha scelto: senza, la finestra segue il sistema (chiaro o scuro)
        if self.theme_set {
            out.push_str(&format!("theme={}\n", self.theme.code()));
        }
        if let Some(lang) = &self.lang {
            out.push_str(&format!("lang={}\n", lang));
        }
        if self.jump_pages != JUMP_PAGES {
            let [a, b, c] = self.jump_pages;
            out.push_str(&format!("jump_pages={a},{b},{c}\n"));
        }
        if self.nav_bar {
            out.push_str("nav_bar=1\n");
        }
        if self.special_chars {
            out.push_str("special=1\n");
        }
        if self.log_colors {
            out.push_str("log_colors=1\n");
        }
        if !self.line_numbers {
            out.push_str("line_numbers=0\n");
        }
        if let Some(n) = self.font_size {
            out.push_str(&format!("font_size={n}\n"));
        }
        if !self.index {
            out.push_str("index=0\n");
        }
        if let Some((pattern, within)) = &self.time_format {
            out.push_str(&format!("time_format={pattern}\ntime_within={within}\n"));
        }
        if self.timeline.on {
            out.push_str("timeline=1\n");
        }
        if let Some(passo) = self.timeline.step {
            out.push_str(&format!("timeline_step={}\n", passo.code()));
        }
        if self.timeline.bytes {
            out.push_str("timeline_mode=bytes\n");
        }
        if self.timeline.empty {
            out.push_str("timeline_empty=1\n");
        }
        for (pattern, within, skip) in &self.time_rules {
            out.push_str(&format!("time_rule={pattern}|{within}|{skip}\n"));
        }
        out
    }

    /// Rilegge il testo, tollerante: commenti, righe vuote, chiavi sconosciute, righe senza `=` e valori non validi
    /// si saltano (restano i default); spazi, maiuscole nel tema, CRLF e BOM si tollerano; se una chiave compare due
    /// volte vale l'ultima valida; una lingua vuota si ignora.
    pub fn from_text(text: &str) -> Settings {
        let mut s = Settings::default();
        // il BOM di chi ha salvato con Notepad non e' un errore
        let text = text.strip_prefix('\u{feff}').unwrap_or(text);
        for line in text.lines() {
            let line = line.trim();
            // commenti e righe vuote si saltano
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            // senza '=' non c'e' niente da capire
            let Some((k, v)) = line.split_once('=') else {
                continue;
            };
            let (k, v) = (k.trim(), v.trim());
            match k {
                "theme" => {
                    // una chiave doppia: vale l'ultima valida
                    if let Some(t) = ThemeId::parse(v) {
                        s.theme = t;
                        s.theme_set = true;
                    }
                }
                "lang" if !v.is_empty() => {
                    // una lingua vuota non si salva: si resta al fabbrica
                    s.lang = Some(v.to_string());
                }
                "jump_pages" => {
                    // tre numeri validi, altrimenti la riga si salta (vale l'ultima buona)
                    if let Some(j) = parse_jump(v) {
                        s.jump_pages = j;
                    }
                }
                "nav_bar" => match v.to_ascii_lowercase().as_str() {
                    "1" | "on" | "true" => s.nav_bar = true,
                    "0" | "off" | "false" => s.nav_bar = false,
                    // un valore che non si capisce si salta
                    _ => {}
                },
                "special" => match v.to_ascii_lowercase().as_str() {
                    "1" | "on" | "true" => s.special_chars = true,
                    "0" | "off" | "false" => s.special_chars = false,
                    _ => {}
                },
                "font_size" => {
                    if let Some(n) = v.parse::<u16>().ok().filter(|n| (8..=48).contains(n)) {
                        s.font_size = Some(n);
                    }
                }
                // il formato dell'orario: il pattern (si prova se e' valido) e quanto lontano dall'inizio riga (da 0 a 200)
                "time_format" => {
                    if mrp_core::timemask::TimeMask::parse(v, 0).is_ok() {
                        let within = s.time_format.as_ref().map_or(0, |(_, w)| *w);
                        s.time_format = Some((v.to_string(), within));
                    }
                }
                // una regola scritta a mano: pattern|entro|occorrenza (il pattern deve andare; al piu' otto)
                "time_rule" => {
                    let mut parti = v.rsplitn(3, '|');
                    if let (Some(skip), Some(within), Some(pattern)) =
                        (parti.next(), parti.next(), parti.next())
                    {
                        if let (Ok(w), Ok(k)) = (within.parse::<usize>(), skip.parse::<usize>()) {
                            let regola = (pattern.to_string(), w.min(200), k.min(9));
                            if mrp_core::timemask::TimeMask::parse(pattern, 0).is_ok()
                                && !s.time_rules.contains(&regola)
                                && s.time_rules.len() < 8
                            {
                                s.time_rules.push(regola);
                            }
                        }
                    }
                }
                "time_within" => {
                    if let (Ok(n), Some(f)) = (v.parse::<usize>(), s.time_format.as_mut()) {
                        f.1 = n.min(200);
                    }
                }
                "index" => match v.to_ascii_lowercase().as_str() {
                    "1" | "on" | "true" => s.index = true,
                    "0" | "off" | "false" => s.index = false,
                    _ => {}
                },
                "line_numbers" => match v.to_ascii_lowercase().as_str() {
                    "1" | "on" | "true" => s.line_numbers = true,
                    "0" | "off" | "false" => s.line_numbers = false,
                    _ => {}
                },
                "timeline" => match v.to_ascii_lowercase().as_str() {
                    "1" | "on" | "true" => s.timeline.on = true,
                    "0" | "off" | "false" => s.timeline.on = false,
                    _ => {}
                },
                "timeline_step" => {
                    if let Some(p) = mrp_core::timeline::Step::parse(v) {
                        s.timeline.step = Some(p);
                    }
                }
                "timeline_mode" => match v.to_ascii_lowercase().as_str() {
                    "bytes" => s.timeline.bytes = true,
                    "time" => s.timeline.bytes = false,
                    _ => {}
                },
                "timeline_empty" => match v.to_ascii_lowercase().as_str() {
                    "1" | "on" | "true" => s.timeline.empty = true,
                    "0" | "off" | "false" => s.timeline.empty = false,
                    _ => {}
                },
                "log_colors" => match v.to_ascii_lowercase().as_str() {
                    "1" | "on" | "true" => s.log_colors = true,
                    "0" | "off" | "false" => s.log_colors = false,
                    _ => {}
                },
                _ => {}
            }
        }
        s
    }

    /// Dove vivono le impostazioni (vedi la testa del modulo); `None` solo senza profilo utente.
    pub fn default_path() -> Option<PathBuf> {
        #[cfg(windows)]
        {
            std::env::var_os("APPDATA").map(|d| PathBuf::from(d).join("mrp").join("settings.txt"))
        }
        #[cfg(not(windows))]
        {
            std::env::var_os("HOME").map(|d| {
                PathBuf::from(d)
                    .join(".config")
                    .join("mrp")
                    .join("settings.txt")
            })
        }
    }

    /// Scrive il file, creando le cartelle del profilo se mancano.
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(path, self.to_text())
    }

    /// Legge il file: `None` se manca o non si legge.
    pub fn load(path: &Path) -> Option<Settings> {
        // niente lettura intera in memoria (regola del progetto): il file si mappa
        // alla maniera di `Lang::from_file`; un file che non e' nemmeno UTF-8 vale
        // come non leggibile
        let src = FileSource::open(path).ok()?;
        let text = std::str::from_utf8(src.bytes()).ok()?;
        Some(Settings::from_text(text))
    }
}
