//! Gli argomenti della riga di comando del binario `mrp-tui`: il parser (le opzioni dell'aiuto piu' quelle da
//! sviluppatore, che nell'aiuto non compaiono), la dimensione di `--dump` e la risoluzione di tema e lingua
//! (opzione > impostazioni > predefinito). I file sono gli argomenti che non cominciano con `-` (ognuno
//! una scheda, in ordine) e si tengono cosi' come sono dati: un percorso non deve essere UTF-8 per aprirsi. Anche il
//! profilo (le lingue disponibili e le impostazioni) sta qui, perche' e' il mondo dentro cui gli argomenti
//! si interpretano. I test del binario (`tests/cli.rs`) provano tutto da fuori: qui non servono test interni.

use crate::settings::Settings;
use crate::theme::ThemeId;
use mrp_core::encoding::Encoding;
use mrp_core::lang::{self, Lang};
use mrp_core::lineending::LineEnding;
use std::ffi::OsString;
use std::path::PathBuf;

/// Gli argomenti letti: i valori delle opzioni restano testo, li valida chi li usa.
#[derive(Debug, Default)]
pub struct Args {
    /// `--lang CODICE`: la lingua dell'interfaccia.
    pub lang: Option<String>,
    /// `--theme NOME`: night, debian, console o minimal.
    pub theme: Option<String>,
    /// `--encoding NOME`: la codifica del file (`auto`: la riconosce l'apertura).
    pub encoding: Option<String>,
    /// `--eol STILE`: lf, crlf o cr; lo stile di fine riga che scrive Invio (le righe che ci sono non cambiano).
    pub eol: Option<String>,
    /// `--lang-dir DIR` (sviluppatore): le lingue si cercano in DIR invece che nel posto solito.
    pub lang_dir: Option<PathBuf>,
    /// `--settings PERCORSO` (sviluppatore): le impostazioni stanno in PERCORSO.
    pub settings: Option<PathBuf>,
    /// `--dump WxH` (sviluppatore): lo schermo come testo, senza terminale.
    pub dump: Option<String>,
    /// `--dump-colori` (sviluppatore): con `--dump`, ogni cella come `colonna riga fg bg carattere`, per disegnare i colori.
    pub dump_colori: bool,
    /// `--keys LISTA` (sviluppatore): i tasti che `--dump` applica uno alla volta.
    pub keys: Option<String>,
    /// `-h` / `--help`.
    pub help: bool,
    /// `--list-encodings`: stampa i nomi delle codifiche, uno per riga, ed esce.
    pub list_encodings: bool,
    /// I file da aprire, uno per scheda, nell'ordine dato.
    pub files: Vec<PathBuf>,
}

/// Legge la riga di comando: `Err` con l'argomento che non va (opzione sconosciuta o senza valore). Un
/// argomento che comincia con `-` e non e' un'opzione nota non puo' essere il file.
pub fn parse<I: IntoIterator<Item = OsString>>(argv: I) -> Result<Args, String> {
    let mut args = Args::default();
    let mut resto = argv.into_iter();
    while let Some(arg) = resto.next() {
        // quello che non comincia con '-' e' un file, cosi' com'e'
        if !arg.to_string_lossy().starts_with('-') {
            args.files.push(PathBuf::from(arg));
            continue;
        }
        // le opzioni sono testo: un byte strano non puo' matchare nessuna opzione nota
        match arg.to_string_lossy().as_ref() {
            "-h" | "--help" => args.help = true,
            "--lang" => args.lang = Some(testo(&mut resto, "--lang")?),
            "--theme" => args.theme = Some(testo(&mut resto, "--theme")?),
            "--encoding" => args.encoding = Some(testo(&mut resto, "--encoding")?),
            "--eol" => args.eol = Some(testo(&mut resto, "--eol")?),
            "--list-encodings" => args.list_encodings = true,
            "--lang-dir" => args.lang_dir = Some(percorso(&mut resto, "--lang-dir")?),
            "--settings" => args.settings = Some(percorso(&mut resto, "--settings")?),
            "--dump" => args.dump = Some(testo(&mut resto, "--dump")?),
            "--dump-colori" => args.dump_colori = true,
            "--keys" => args.keys = Some(testo(&mut resto, "--keys")?),
            _ => return Err(arg.to_string_lossy().into_owned()),
        }
    }
    Ok(args)
}

/// Il valore di testo dell'opzione appena letta: `Err` se la riga finisce prima (il valore manca).
fn testo(resto: &mut impl Iterator<Item = OsString>, opzione: &str) -> Result<String, String> {
    match resto.next() {
        Some(v) => Ok(v.to_string_lossy().into_owned()),
        None => Err(opzione.to_string()),
    }
}

/// Come `testo`, per i valori che sono percorsi (che restano `OsString`).
fn percorso(resto: &mut impl Iterator<Item = OsString>, opzione: &str) -> Result<PathBuf, String> {
    match resto.next() {
        Some(v) => Ok(PathBuf::from(v)),
        None => Err(opzione.to_string()),
    }
}

/// La dimensione di `--dump`: `WxH` (per esempio `80x24`), W e H cifre ASCII che stanno in un `u16` e sono
/// almeno 1.
pub fn parse_dump(s: &str) -> Option<(u16, u16)> {
    let (w, h) = s.split_once('x')?;
    let (w, h) = (cifra_u16(w)?, cifra_u16(h)?);
    (w > 0 && h > 0).then_some((w, h))
}

/// Un numero fatto di cifre ASCII che sta in un `u16`: la grammatica di `--dump` (come `parse_cifre`
/// dell'app, ma piu' piccolo).
fn cifra_u16(s: &str) -> Option<u16> {
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    s.parse().ok()
}

/// Il profilo utente visto dal binario: le lingue disponibili, le impostazioni lette e dove salvarle.
/// `--lang-dir` e `--settings` vincono sul profilo; con `MRP_NO_PROFILE=1` il profilo non si tocca (lingue:
/// solo l'inglese incorporato; impostazioni: niente da leggere e niente da scrivere). Se accanto all'eseguibile
/// c'e' una cartella `lang` (modo portatile) lingue e impostazioni stanno accanto all'eseguibile.
pub struct Profile {
    /// Le lingue disponibili: il menu di F7 e la validazione di `--lang`.
    pub langs: Vec<Lang>,
    /// Le impostazioni lette (niente se il file manca o non si legge).
    pub settings: Option<Settings>,
    /// Dove l'app salva le impostazioni quando si cambia tema o lingua (niente: non si salva).
    pub path: Option<PathBuf>,
}

/// Costruisce il profilo: vedi `Profile`.
pub fn profile(args: &Args) -> Profile {
    let senza_profilo = std::env::var("MRP_NO_PROFILE").ok().as_deref() == Some("1");
    let langs = match &args.lang_dir {
        Some(dir) => lang::registry_in(Some(dir.as_path())).0,
        None if senza_profilo => vec![Lang::english()],
        None => lang::registry().0,
    };
    let path = match &args.settings {
        Some(p) => Some(p.clone()),
        None if senza_profilo => None,
        // Modo portatile: con una cartella `lang` accanto all'eseguibile anche le impostazioni vivono li'
        // (`settings.txt`), cosi' la cartella si porta dove si vuole senza lasciare niente nel profilo.
        None => lang::portable_dir()
            .map(|d| d.join("settings.txt"))
            .or_else(Settings::default_path),
    };
    let settings = path.as_ref().and_then(|p| Settings::load(p));
    Profile {
        langs,
        settings,
        path,
    }
}

/// La lingua di partenza: l'opzione `--lang` se il codice esiste fra le disponibili, altrimenti quella delle
/// impostazioni se esiste, altrimenti `None` (l'inglese incorporato). `Err` col codice quando `--lang` nomina
/// una lingua che non c'e' (un errore d'uso); nelle impostazioni un codice sconosciuto si ignora.
pub fn resolve_lang(
    args: &Args,
    langs: &[Lang],
    settings: &Option<Settings>,
) -> Result<Option<Lang>, String> {
    if let Some(code) = &args.lang {
        return match langs.iter().find(|l| l.code == *code) {
            Some(l) => Ok(Some(l.clone())),
            None => Err(code.clone()),
        };
    }
    let Some(code) = settings.as_ref().and_then(|s| s.lang.as_deref()) else {
        return Ok(None);
    };
    Ok(langs.iter().find(|l| l.code == code).cloned())
}

/// Il tema di partenza: l'opzione `--theme` se il nome e' uno dei quattro, altrimenti quello delle
/// impostazioni, altrimenti il predefinito. `Err` col nome quando l'opzione non si riconosce (nelle
/// impostazioni un tema sconosciuto non puo' esserci: `Settings` lo scarta gia' leggendolo).
pub fn resolve_theme(args: &Args, settings: &Option<Settings>) -> Result<ThemeId, String> {
    if let Some(nome) = &args.theme {
        return match ThemeId::parse(nome) {
            Some(t) => Ok(t),
            None => Err(nome.clone()),
        };
    }
    Ok(settings
        .as_ref()
        .map(|s| s.theme)
        .unwrap_or(ThemeId::DEFAULT))
}

/// L'utente ha scelto un tema? Con `--theme`, o con un tema salvato nelle impostazioni. Altrimenti la finestra segue il tema
/// chiaro o scuro del sistema.
pub fn theme_is_explicit(args: &Args, settings: &Option<Settings>) -> bool {
    args.theme.is_some() || settings.as_ref().is_some_and(|s| s.theme_set)
}

/// Lo stile di fine riga di `--eol` (`lf`, `crlf`, `cr`, in qualunque maiuscola); nessuna opzione vale `None` (lo
/// stile lo decide il file, o la piattaforma per un documento nuovo). `Err` col testo quando non si riconosce.
pub fn resolve_eol(args: &Args) -> Result<Option<LineEnding>, String> {
    match args.eol.as_deref().map(str::to_ascii_lowercase).as_deref() {
        None => Ok(None),
        Some("lf") => Ok(Some(LineEnding::Lf)),
        Some("crlf") => Ok(Some(LineEnding::CrLf)),
        Some("cr") => Ok(Some(LineEnding::Cr)),
        Some(_) => Err(args.eol.clone().unwrap_or_default()),
    }
}

/// La codifica di partenza: `--encoding NOME` con un nome noto (anche alias, come lo legge
/// `Encoding::by_name`); `auto`, o nessuna opzione, vale `None` (la riconosce l'apertura sui primi
/// byte del file). `Err` col nome quando non si riconosce (un errore d'uso).
pub fn resolve_encoding(args: &Args) -> Result<Option<Encoding>, String> {
    match &args.encoding {
        None => Ok(None),
        Some(nome) if nome == "auto" => Ok(None),
        Some(nome) => Encoding::by_name(nome)
            .map(Some)
            .ok_or_else(|| nome.clone()),
    }
}
