//! Le schede: piu' documenti aperti insieme, ognuno con la sua `App` (il suo indice righe, la sua ricerca, la sua storia
//! di modifiche). Il `Workspace` sceglie la scheda attiva, le manda i tasti e il mouse, apre e chiude le schede e disegna
//! la barra (nel TUI: una riga in cima, solo con piu' di una scheda; la GUI disegna la sua con egui e spegne questa).
//!
//! Tasti (con un dialogo aperto vanno tutti al dialogo): Ctrl+O apre un file in una scheda nuova (o porta avanti quella
//! che lo ha gia'), Ctrl+W chiude la scheda (con modifiche chiede prima), Ctrl+PgSu/PgGiu e Alt+Sinistra/Destra cambiano
//! scheda, Alt+1..9 va alla scheda col numero; F10 e Ctrl+Q chiudono tutto, chiedendo scheda per scheda per quelle con
//! modifiche.

use crate::app::{App, AppConfig, ReplaceAsk, ReplaceDone, ReplaceSpec, MAX_REPLACE};
use crate::sessionstore;
use crate::theme::{Theme, ThemeId};
use crate::ui;
use mrp_core::pattern::{Mode, Pattern};
use mrp_core::session::TabRecord;
use ratatui::backend::TestBackend;
use ratatui::crossterm::event::{
    KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::{Frame, Terminal};
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Quanti caratteri al massimo per il nome di una scheda nella barra.
const NOME_MAX: usize = 24;

/// Ogni quanto si guarda se la sessione e' cambiata.
const CONTROLLO_SESSIONE: Duration = Duration::from_secs(1);
/// Al piu' una scrittura della sessione ogni tanto.
const SCRITTURA_SESSIONE: Duration = Duration::from_secs(5);

/// Cio' che la sessione ricorda di una scheda, per capire se e' cambiata: percorso, versione del documento, cursore, modalita'.
type ImprontaScheda = (PathBuf, Option<u64>, u64, bool);

/// La sessione su disco di questo `Workspace`: dove sta e quando si e' scritta l'ultima volta.
struct Sessione {
    dir: PathBuf,
    /// Il blocco della cartella: finche' il processo e' vivo (o il file e' aperto) nessun altro `mrp-tui` scrive la stessa
    /// sessione; il sistema lo toglie da solo se il processo muore, quindi non resta mai bloccata.
    _blocco: std::fs::File,
    /// L'impronta dell'ultima scrittura (e quale scheda era avanti).
    scritta: Option<(Vec<ImprontaScheda>, usize)>,
    controllato: Instant,
    scritta_il: Instant,
    controllo: Duration,
    intervallo: Duration,
}

impl Sessione {
    /// La sessione in `dir`, se la cartella si puo' preparare e nessun altro processo la tiene (due `mrp-tui` aperti insieme
    /// non devono scriversi addosso: il secondo lavora senza sessione).
    fn apri(dir: PathBuf) -> Option<Sessione> {
        std::fs::create_dir_all(&dir).ok()?;
        let blocco = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(dir.join("lock"))
            .ok()?;
        blocco.try_lock().ok()?;
        let ora = Instant::now();
        Some(Sessione {
            dir,
            _blocco: blocco,
            scritta: None,
            controllato: ora,
            scritta_il: ora,
            controllo: CONTROLLO_SESSIONE,
            intervallo: SCRITTURA_SESSIONE,
        })
    }
}

/// Una sostituzione in corso su una o piu' schede: l'anteprima (conta) o l'applicazione.
struct ReplaceRun {
    spec: ReplaceSpec,
    apply: bool,
    /// Le schede su cui gira e quante non hanno ancora finito.
    schede: Vec<usize>,
    pendenti: usize,
    /// Le occorrenze contate (o sostituite) e in quante schede ce n'erano.
    totale: u64,
    schede_con: usize,
    /// Schede saltate (per esempio una UTF-16 con una regex).
    saltate: usize,
    troppe: bool,
    annullata: bool,
    errore: Option<String>,
}

/// Piu' schede aperte, una attiva.
pub struct Workspace {
    tabs: Vec<App>,
    active: usize,
    /// La configurazione con cui si aprono le schede nuove (la codifica si riconosce, non si impone).
    cfg: AppConfig,
    size: (u16, u16),
    /// Se questo `Workspace` disegna la barra delle schede (il TUI si', la GUI no: ha la sua).
    bar: bool,
    /// Si sta chiudendo tutto: finita una scheda si passa alla prossima con modifiche, poi si esce.
    closing_all: bool,
    quit: bool,
    /// Dove sta ogni scheda nella barra dopo l'ultimo disegno: colonne da..a (escluso); (0, 0) se non si vede.
    spans: Vec<(u16, u16)>,
    /// La sessione su disco, se e' attiva: le schede (e le modifiche non salvate) sopravvivono alla chiusura.
    session: Option<Sessione>,
    /// La sostituzione in corso (anteprima o applicazione), se c'e'.
    replace: Option<ReplaceRun>,
    /// La barra dei salti (Ctrl+B) e' visibile: una riga sotto la barra delle schede, disegnata da questo `Workspace`
    /// anche nella GUI (e' nella griglia di celle).
    nav: bool,
    /// Dove sta ogni pulsante della barra dei salti dopo l'ultimo disegno: colonne da..a (escluso) e cosa fa.
    nav_spans: Vec<(u16, u16, NavAzione)>,
    /// La barra dei salti si disegna a celle (il TUI si', la finestra no: ha i suoi pulsanti e legge `nav_bar`).
    nav_cells: bool,
}

impl Drop for Workspace {
    fn drop(&mut self) {
        // senza sessione nessuno riprendera' i documenti senza nome: i loro file vuoti di appoggio non servono piu'
        if self.session.is_none() {
            for a in &self.tabs {
                Workspace::pulisci_scheda(a);
            }
        }
    }
}

/// Cosa fa un pulsante della barra dei salti.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NavAzione {
    /// Inizio del file.
    Home,
    /// Fine del file.
    End,
    /// `(avanti, pagine)`: salta di tante pagine.
    Salto(bool, usize),
}

impl Workspace {
    /// Un `Workspace` con una sola scheda, `app`; `cfg` serve ad aprire le altre.
    pub fn new(app: App, cfg: AppConfig) -> Workspace {
        let nav = cfg.nav_bar;
        Workspace {
            tabs: vec![app],
            active: 0,
            cfg: AppConfig {
                encoding: None,
                ..cfg
            },
            size: (0, 0),
            bar: true,
            closing_all: false,
            quit: false,
            spans: Vec::new(),
            session: None,
            replace: None,
            nav,
            nav_spans: Vec::new(),
            nav_cells: true,
        }
    }

    /// Accende la sessione in `dir`: da ora la sessione si riscrive da sola quando cambia (al piu' ogni 5 secondi),
    /// F10 / Ctrl+Q / la chiusura della finestra non chiedono niente (la sessione tiene le modifiche non salvate) e alla
    /// prossima apertura `from_session` / `merge_session` le rimettono.
    /// Se un altro processo tiene gia' la sessione, questo `Workspace` resta senza (`has_session` e' falso).
    pub fn with_session(mut self, dir: PathBuf) -> Workspace {
        self.session = Sessione::apri(dir);
        self
    }

    /// Cambia i tempi della sessione (per i test): ogni quanto si controlla e ogni quanto al piu' si scrive.
    pub fn with_session_timing(mut self, controllo: Duration, intervallo: Duration) -> Workspace {
        if let Some(s) = &mut self.session {
            s.controllo = controllo;
            s.intervallo = intervallo;
        }
        self
    }

    /// Vero se la sessione e' accesa.
    pub fn has_session(&self) -> bool {
        self.session.is_some()
    }

    /// I record delle schede per la sessione (quelle che si possono ricordare) e quale era avanti fra loro.
    fn records(&self) -> (Vec<TabRecord>, usize) {
        let mut v = Vec::new();
        let mut avanti = 0;
        for (i, a) in self.tabs.iter().enumerate() {
            if let Some(r) = a.session_record() {
                if i == self.active {
                    avanti = v.len();
                }
                v.push(r);
            }
        }
        (v, avanti)
    }

    /// L'impronta della sessione adesso: cambia se cambia una modifica, un cursore, una scheda o quella avanti.
    fn fingerprint(&self) -> (Vec<ImprontaScheda>, usize) {
        let v = self
            .tabs
            .iter()
            .map(|a| {
                (
                    a.path().to_path_buf(),
                    a.doc_version(),
                    a.cursor(),
                    a.is_edit(),
                )
            })
            .collect();
        (v, self.active)
    }

    /// Scrive la sessione adesso. Senza sessione non fa nulla.
    pub fn save_session(&mut self) -> io::Result<()> {
        // un documento in mano al salvataggio non ha un record: riscrivere adesso toglierebbe la scheda (e le sue modifiche,
        // gia' al sicuro nella sessione) dal disco; si lascia quella di prima e si riprova al giro dopo
        if self.tabs.iter().any(|a| a.doc_version().is_none()) {
            return Ok(());
        }
        let imp = self.fingerprint();
        let (recs, avanti) = self.records();
        let Some(s) = &mut self.session else {
            return Ok(());
        };
        sessionstore::scrivi(&s.dir, &recs, avanti)?;
        s.scritta = Some(imp);
        s.scritta_il = Instant::now();
        Ok(())
    }

    /// Un giro della sessione: la riscrive se e' cambiata e se e' passato il tempo.
    fn tick_session(&mut self) {
        let Some(s) = &self.session else {
            return;
        };
        if s.controllato.elapsed() < s.controllo {
            return;
        }
        let imp = self.fingerprint();
        let Some(s) = &mut self.session else {
            return;
        };
        s.controllato = Instant::now();
        if s.scritta.as_ref() == Some(&imp) || s.scritta_il.elapsed() < s.intervallo {
            return;
        }
        // un errore di scrittura non ferma l'editor: si riprova al prossimo giro utile
        let _ = self.save_session();
    }

    /// Le schede della sessione in `dir`: le app riaperte, quale era avanti (se e' fra quelle riaperte) e gli avvisi
    /// per quello che non si e' potuto rimettere. Con `solo_modificate` si riaprono solo le schede con modifiche non
    /// salvate. Una scheda le cui modifiche non si rimettono (il file e' cambiato o sparito) mette da parte il suo file
    /// nella cartella `stale` della sessione.
    fn load_session(
        dir: &Path,
        cfg: &AppConfig,
        solo_modificate: bool,
    ) -> (Vec<App>, Option<usize>, Vec<String>) {
        let mut app: Vec<App> = Vec::new();
        let mut avvisi: Vec<String> = Vec::new();
        let mut avanti_aperta = None;
        let Some((avanti, schede)) = sessionstore::leggi(dir) else {
            return (app, None, avvisi);
        };
        let cat = |chiave: &str, argomenti: &[&str]| {
            let c = mrp_core::lang::Catalog::english();
            let c = match &cfg.lang_code {
                Some(code) => cfg
                    .langs
                    .iter()
                    .find(|l| &l.code == code)
                    .map_or(c, |l| mrp_core::lang::Catalog::with(l.clone())),
                None => c,
            };
            c.tf(chiave, argomenti)
        };
        let mut modificate = 0;
        for (i, (nome, r)) in schede.into_iter().enumerate() {
            let rec = match r {
                Ok(rec) => rec,
                Err(_) => {
                    let _ = sessionstore::metti_da_parte(dir, &nome);
                    avvisi.push(cat("session.corrupt", &[]));
                    continue;
                }
            };
            if solo_modificate && rec.state.is_none() {
                continue;
            }
            let file = Path::new(&rec.path)
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            match App::restore_tab(&rec, cfg.clone(), Some(&Workspace::untitled_dir(Some(dir)))) {
                Ok((a, esito)) => {
                    if esito.is_some() {
                        let dove = sessionstore::metti_da_parte(dir, &nome)
                            .map(|p| p.display().to_string())
                            .unwrap_or_default();
                        avvisi.push(cat("session.stale", &[&file, &dove]));
                    } else if rec.state.is_some() {
                        modificate += 1;
                    }
                    if i == avanti {
                        avanti_aperta = Some(app.len());
                    }
                    app.push(a);
                }
                Err(_) => {
                    if rec.state.is_some() {
                        let dove = sessionstore::metti_da_parte(dir, &nome)
                            .map(|p| p.display().to_string())
                            .unwrap_or_default();
                        avvisi.push(cat("session.lost", &[&file, &dove]));
                    }
                }
            }
        }
        if modificate > 0 {
            avvisi.insert(0, cat("session.restored", &[&modificate.to_string()]));
        }
        (app, avanti_aperta, avvisi)
    }

    /// Un `Workspace` con le schede dell'ultima sessione in `dir` (nessun file nella riga di comando); `None` se non c'e'
    /// nessuna scheda da riaprire. La sessione resta accesa su `dir`. Gli avvisi sono nella riga di stato.
    pub fn from_session(dir: PathBuf, cfg: AppConfig) -> Option<Workspace> {
        // prima si prende il blocco: se un altro `mrp-tui` ha la sessione, le sue modifiche non si ripetono qui
        let sessione = Sessione::apri(dir)?;
        let (mut app, avanti, avvisi) = Workspace::load_session(&sessione.dir, &cfg, false);
        if app.is_empty() {
            return None;
        }
        let primo = app.remove(0);
        let mut ws = Workspace::new(primo, cfg);
        ws.session = Some(sessione);
        // le altre schede in ordine; `avanti` contava anche la prima
        ws.tabs.extend(app);
        ws.active = avanti.unwrap_or(0).min(ws.tabs.len() - 1);
        ws.apply_size();
        ws.avvisa(avvisi);
        Some(ws)
    }

    /// Aggiunge a questo `Workspace` le schede con modifiche non salvate dell'ultima sessione (si usa quando ci sono file
    /// nella riga di comando: le modifiche di prima non si perdono). Una scheda dello stesso file gia' aperta senza
    /// modifiche si sostituisce con quella della sessione. Resta avanti la scheda che era avanti.
    pub fn merge_session(&mut self) {
        let Some(s) = &self.session else {
            return;
        };
        let (app, _, avvisi) = Workspace::load_session(&s.dir, &self.cfg, true);
        for a in app {
            match self.tabs.iter().position(|t| t.path() == a.path()) {
                Some(i) if !self.tabs[i].is_modified() => self.tabs[i] = a,
                Some(_) => {}
                None => self.tabs.push(a),
            }
        }
        self.apply_size();
        self.avvisa(avvisi);
    }

    /// Mostra il primo avviso sulla riga di stato della scheda attiva.
    fn avvisa(&mut self, avvisi: Vec<String>) {
        if let Some(a) = avvisi.into_iter().next() {
            self.tabs[self.active].set_flash(a);
        }
    }

    /// Senza la barra delle schede nel disegno (la GUI la fa da se').
    pub fn senza_barra(mut self) -> Workspace {
        self.bar = false;
        self
    }

    /// La scheda attiva.
    pub fn active(&self) -> &App {
        &self.tabs[self.active]
    }

    /// La scheda attiva, modificabile.
    pub fn active_mut(&mut self) -> &mut App {
        &mut self.tabs[self.active]
    }

    /// L'indice della scheda attiva.
    pub fn active_index(&self) -> usize {
        self.active
    }

    /// Le schede, in ordine.
    pub fn tabs(&self) -> &[App] {
        &self.tabs
    }

    /// Quante schede ci sono: almeno una, anche dopo `should_quit` (l'ultima scheda chiusa resta com'e', cosi' nessun
    /// metodo trova il `Workspace` vuoto).
    pub fn len(&self) -> usize {
        self.tabs.len()
    }

    /// Mai vuoto (vedi `len`).
    pub fn is_empty(&self) -> bool {
        false
    }

    /// Il nome di ogni scheda e se ha modifiche non salvate.
    pub fn titles(&self) -> Vec<(String, bool)> {
        self.tabs
            .iter()
            .map(|a| (a.file_name().to_string(), a.is_modified()))
            .collect()
    }

    /// Vero quando e' stata chiusa l'ultima scheda o tutto: l'ospite deve uscire.
    pub fn should_quit(&self) -> bool {
        self.quit
    }

    /// Quante righe prende la barra: una con piu' schede (e se la disegna questo `Workspace`), altrimenti nessuna.
    fn bar_height(&self) -> u16 {
        u16::from(self.bar && self.tabs.len() > 1)
    }

    /// Quante righe prende la barra dei salti: una se e' visibile, altrimenti nessuna.
    fn nav_height(&self) -> u16 {
        u16::from(self.nav && self.nav_cells)
    }

    /// Le righe sopra la scheda attiva: la barra delle schede e quella dei salti.
    fn top_height(&self) -> u16 {
        self.bar_height() + self.nav_height()
    }

    /// La barra dei salti e' visibile.
    pub fn nav_bar(&self) -> bool {
        self.nav
    }

    /// Ctrl+B: mostra o nasconde la barra dei salti; la scelta si salva nelle impostazioni e vale per le schede nuove.
    pub fn toggle_nav(&mut self) {
        self.nav = !self.nav;
        self.cfg.nav_bar = self.nav;
        let (nav, attiva) = (self.nav, self.active);
        for (i, a) in self.tabs.iter_mut().enumerate() {
            // una scheda sola scrive il file
            a.set_nav_bar(nav, i == attiva);
        }
        self.apply_size();
    }

    /// I caratteri speciali (spazi, tabulazioni, fine riga) si vedono.
    pub fn special_chars(&self) -> bool {
        self.cfg.special_chars
    }

    /// Alt+W: mostra o nasconde i caratteri speciali in tutte le schede; la scelta si salva nelle impostazioni e vale per le
    /// schede nuove.
    pub fn toggle_special(&mut self) {
        self.cfg.special_chars = !self.cfg.special_chars;
        let (visibili, attiva) = (self.cfg.special_chars, self.active);
        for (i, a) in self.tabs.iter_mut().enumerate() {
            // una scheda sola scrive il file
            a.set_special_chars(visibili, i == attiva);
        }
    }

    /// La colorazione dei log e' accesa.
    pub fn log_colors(&self) -> bool {
        self.cfg.log_colors
    }

    /// Alt+L: accende o spegne la colorazione dei log in tutte le schede; la scelta si salva nelle impostazioni e vale per le
    /// schede nuove.
    pub fn toggle_log_colors(&mut self) {
        self.cfg.log_colors = !self.cfg.log_colors;
        let (accesa, attiva) = (self.cfg.log_colors, self.active);
        for (i, a) in self.tabs.iter_mut().enumerate() {
            // una scheda sola scrive il file
            a.set_log_colors(accesa, i == attiva);
        }
    }

    /// Adatta la scheda attiva allo spazio (meno le barre).
    fn apply_size(&mut self) {
        let (w, h) = self.size;
        let barra = self.top_height();
        let h = if h > barra { h - barra } else { h };
        self.tabs[self.active].resize(w, h);
    }

    /// Lo spazio di tutto lo schermo; la scheda attiva prende quello che resta sotto la barra.
    pub fn resize(&mut self, w: u16, h: u16) {
        self.size = (w, h);
        self.apply_size();
    }

    /// Porta avanti la scheda `i` (se c'e').
    pub fn select(&mut self, i: usize) {
        if i < self.tabs.len() {
            self.active = i;
            self.apply_size();
        }
    }

    /// La scheda dopo l'attiva (dall'ultima si torna alla prima).
    pub fn next(&mut self) {
        self.select((self.active + 1) % self.tabs.len());
    }

    /// La scheda prima dell'attiva (dalla prima si va all'ultima).
    pub fn prev(&mut self) {
        self.select((self.active + self.tabs.len() - 1) % self.tabs.len());
    }

    /// Apre `path` in una scheda nuova e la porta avanti; se un'altra scheda ha gia' quel file porta avanti quella.
    /// La scheda nuova prende il tema e la lingua di quella attiva. Errore se il file non si apre (le schede restano).
    pub fn open(&mut self, path: &Path) -> io::Result<()> {
        // una sostituzione in corso conosce le schede per posizione: finche' gira non se ne aprono ne' se ne tolgono
        if self.replace.is_some() {
            return Ok(());
        }
        let canon = path.canonicalize()?;
        if let Some(i) = self.tabs.iter().position(|a| a.path() == canon) {
            self.select(i);
            return Ok(());
        }
        let attiva = &self.tabs[self.active];
        let cfg = AppConfig {
            theme: attiva.theme_id(),
            lang_code: Some(attiva.lang_code().to_string()),
            ..self.cfg.clone()
        };
        let app = App::open(&canon, cfg)?;
        // una scheda senza nome ancora vuota e intatta lascia il posto al file (come negli altri editor)
        let vuota = {
            let a = &self.tabs[self.active];
            a.is_untitled() && !a.is_modified() && a.document_len() == 0
        };
        let vecchia = self.active;
        self.tabs.push(app);
        if vuota {
            let tolta = self.tabs.remove(vecchia);
            Workspace::pulisci_scheda(&tolta);
        }
        self.select(self.tabs.len() - 1);
        Ok(())
    }

    /// La cartella dei file vuoti che reggono i documenti senza nome: `untitled` dentro la cartella della sessione (cosi'
    /// tornano con la sessione) oppure, senza sessione, una cartella fra i temporanei.
    pub fn untitled_dir(sessione: Option<&Path>) -> PathBuf {
        match sessione {
            Some(d) => d.join("untitled"),
            None => std::env::temp_dir().join(format!("mrp-untitled-{}", std::process::id())),
        }
    }

    /// Ctrl+N: apre un documento nuovo, vuoto e in modifica, in una scheda nuova (la porta avanti). Il numero e' il piu' piccolo
    /// libero fra le schede aperte ("Untitled 1", "Untitled 2"...). Prende il tema e la lingua della scheda attiva.
    pub fn new_tab(&mut self) -> io::Result<()> {
        let usati: Vec<u32> = self.tabs.iter().filter_map(App::untitled_number).collect();
        let n = (1u32..).find(|n| !usati.contains(n)).unwrap_or(1);
        let dir = Workspace::untitled_dir(self.session.as_ref().map(|s| s.dir.as_path()));
        let attiva = &self.tabs[self.active];
        let cfg = AppConfig {
            theme: attiva.theme_id(),
            lang_code: Some(attiva.lang_code().to_string()),
            ..self.cfg.clone()
        };
        let app = App::new_untitled(&dir, n, cfg)?;
        self.tabs.push(app);
        self.select(self.tabs.len() - 1);
        Ok(())
    }

    /// Senza file nella riga di comando: l'ultima sessione in `sessione` se c'e' qualcosa da riaprire, altrimenti un
    /// documento nuovo senza nome. Se c'e' una cartella di sessione resta accesa (anche per il documento nuovo).
    pub fn start(cfg: AppConfig, sessione: Option<PathBuf>) -> io::Result<Workspace> {
        if let Some(dir) = &sessione {
            if let Some(ws) = Workspace::from_session(dir.clone(), cfg.clone()) {
                return Ok(ws);
            }
        }
        let dir = Workspace::untitled_dir(sessione.as_deref());
        let app = App::new_untitled(&dir, 1, cfg.clone())?;
        let ws = Workspace::new(app, cfg);
        Ok(match sessione {
            Some(d) => ws.with_session(d),
            None => ws,
        })
    }

    /// Una scheda che si toglie davvero (chiusa a mano, non l'uscita con la sessione): il file vuoto che reggeva un
    /// documento senza nome non serve piu'.
    fn pulisci_scheda(a: &App) {
        if let Some(p) = a.scratch_path() {
            let _ = std::fs::remove_file(p);
        }
    }

    /// Chiede di chiudere la scheda `i`: senza modifiche si chiude subito, con modifiche la porta avanti e apre il
    /// suo dialogo (salva, scarta, annulla).
    pub fn close(&mut self, i: usize) {
        // come per `open`: durante una sostituzione le schede restano dove sono
        if i >= self.tabs.len() || self.replace.is_some() {
            return;
        }
        self.select(i);
        self.tabs[i].request_close();
        self.settle();
    }

    /// Chiude tutto: chiede, una scheda alla volta, per quelle con modifiche; poi `should_quit`.
    pub fn close_all(&mut self) {
        // con la sessione le modifiche non salvate restano nella sessione: si scrive e si esce senza chiedere
        if self.session.is_some() && self.save_session().is_ok() {
            self.quit = true;
            return;
        }
        self.closing_all = true;
        self.ask_next();
        self.settle();
    }

    /// Alla prossima scheda con modifiche: la porta avanti e chiede; se non ce ne sono si esce.
    fn ask_next(&mut self) {
        match self.tabs.iter().position(|a| a.is_modified()) {
            Some(j) => {
                self.select(j);
                self.tabs[j].request_close();
            }
            None => self.quit = true,
        }
    }

    /// Dopo ogni tasto o `tick`: toglie le schede che hanno deciso di chiudersi (finita la conferma, o un salvataggio
    /// in uscita), prende la richiesta di apertura di un file, porta il tema e la lingua scelti nella scheda attiva a
    /// tutte, e capisce se una chiusura generale e' stata annullata.
    fn settle(&mut self) {
        while let Some(i) = self.tabs.iter().position(|a| a.should_quit()) {
            if self.tabs.len() == 1 {
                // l'ultima scheda: non si toglie (il `Workspace` non resta mai vuoto), si esce; chiusa a mano, non deve
                // tornare alla prossima apertura
                Workspace::pulisci_scheda(&self.tabs[0]);
                if let Some(s) = &self.session {
                    let _ = sessionstore::azzera(&s.dir);
                }
                self.quit = true;
                return;
            }
            let tolta = self.tabs.remove(i);
            Workspace::pulisci_scheda(&tolta);
            if i < self.active {
                self.active -= 1;
            }
            self.active = self.active.min(self.tabs.len() - 1);
            self.apply_size();
            if self.closing_all {
                self.ask_next();
            }
        }
        self.poll_replace();
        if let Some(ask) = self.tabs[self.active].take_replace_request() {
            self.start_replace(ask);
        }
        if let Some(percorso) = self.tabs[self.active].take_open_request() {
            if let Err(e) = self.open(&percorso) {
                let a = &mut self.tabs[self.active];
                let testo = a.text_with(
                    "cli.err.open",
                    &[&percorso.display().to_string(), &e.to_string()],
                );
                a.set_flash(testo);
            }
        }
        // chiusura generale annullata (nessun dialogo aperto sulla scheda attiva e nessuno che sta per uscire)
        if self.closing_all && !self.quit && self.tabs[self.active].dialog_kind().is_none() {
            self.closing_all = false;
        }
        let (tema, lingua) = self.look();
        for a in &mut self.tabs {
            if a.theme_id() != tema || a.lang_code() != lingua {
                a.adopt_look(tema, &lingua);
            }
        }
    }

    /// Vero mentre una sostituzione (anteprima o applicazione) gira.
    pub fn replace_busy(&self) -> bool {
        self.replace.is_some()
    }

    /// Ferma la sostituzione in corso: le schede che stanno ancora lavorando la lasciano a meta' (niente si applica).
    fn cancel_replace(&mut self) {
        if let Some(run) = &self.replace {
            for &i in &run.schede {
                if let Some(a) = self.tabs.get(i) {
                    a.replace_cancel();
                }
            }
        }
    }

    /// Fa partire l'anteprima o l'applicazione di `ask` sulle schede dell'ambito (la attiva, o tutte). Un testo cercato non
    /// valido (regex sbagliata) si dice nello stato del dialogo e non parte niente; una scheda che non puo' farlo (una
    /// codifica che non e' UTF-8 con una regex) si salta e si conta.
    fn start_replace(&mut self, ask: ReplaceAsk) {
        let attiva = self.active;
        // il testo cercato si controlla una volta sola, qui, cosi' l'errore e' uno e le schede non lo ripetono
        if !(ask.spec.mode == Mode::Text && ask.spec.case) {
            if let Err(e) = Pattern::new(&ask.spec.find, ask.spec.mode, ask.spec.case) {
                let testo = self.tabs[attiva].text_with("replace.bad", &[&e.to_string()]);
                self.tabs[attiva].set_replace_status(testo, false);
                return;
            }
        }
        let indici: Vec<usize> = if ask.all_tabs {
            (0..self.tabs.len()).collect()
        } else {
            vec![attiva]
        };
        let mut run = ReplaceRun {
            spec: ask.spec.clone(),
            apply: ask.apply,
            schede: Vec::new(),
            pendenti: 0,
            totale: 0,
            schede_con: 0,
            saltate: 0,
            troppe: false,
            annullata: false,
            errore: None,
        };
        let mut primo_errore: Option<String> = None;
        for i in indici {
            match self.tabs[i].replace_start(&ask.spec, ask.apply) {
                Ok(()) => {
                    run.schede.push(i);
                    run.pendenti += 1;
                }
                Err(e) => {
                    run.saltate += 1;
                    primo_errore.get_or_insert(e);
                }
            }
        }
        if run.pendenti == 0 {
            // nessuna scheda ha potuto: si dice il perche' della prima
            let testo = primo_errore.unwrap_or_default();
            self.tabs[attiva].set_replace_status(testo, false);
            return;
        }
        let chiave = if ask.apply {
            "replace.applying"
        } else {
            "replace.counting"
        };
        let testo = self.tabs[attiva].text(chiave).to_string();
        self.tabs[attiva].set_replace_status(testo, false);
        self.replace = Some(run);
    }

    /// Raccoglie gli esiti delle schede e, a lavoro finito, scrive l'anteprima nel dialogo o chiude con il riepilogo.
    fn poll_replace(&mut self) {
        let Some(mut run) = self.replace.take() else {
            return;
        };
        for &i in &run.schede {
            let Some(esito) = self.tabs.get_mut(i).and_then(|a| a.take_replace_done()) else {
                continue;
            };
            run.pendenti = run.pendenti.saturating_sub(1);
            match esito {
                ReplaceDone::Counted(n) => {
                    run.totale += n;
                    run.schede_con += usize::from(n > 0);
                }
                ReplaceDone::Applied(n) => {
                    run.totale += n as u64;
                    run.schede_con += usize::from(n > 0);
                }
                ReplaceDone::TooMany => run.troppe = true,
                ReplaceDone::Cancelled => run.annullata = true,
                ReplaceDone::Failed(e) => run.errore = Some(e),
            }
        }
        if run.pendenti > 0 {
            self.replace = Some(run);
            return;
        }
        let a = self.active;
        let cat = |ws: &Workspace, chiave: &str, args: &[&str]| ws.tabs[a].text_with(chiave, args);
        // le schede che non hanno potuto lavorare (una codifica che non regge la regex) si dicono, in coda al riepilogo
        let saltate = if run.saltate > 0 {
            format!(
                " {}",
                cat(self, "replace.skipped", &[&run.saltate.to_string()])
            )
        } else {
            String::new()
        };
        if run.annullata {
            let t = cat(self, "replace.cancelled", &[]);
            self.tabs[a].set_replace_status(t, false);
        } else if let Some(e) = &run.errore {
            let t = cat(self, "replace.fail", &[e]);
            self.tabs[a].set_replace_status(t, false);
        } else if run.troppe {
            let t = cat(self, "replace.too_many", &[&MAX_REPLACE.to_string()]);
            self.tabs[a].set_replace_status(t, false);
        } else if !run.apply {
            if run.totale == 0 {
                let t = cat(self, "replace.none", &[]);
                self.tabs[a].set_replace_status(t, false);
            } else {
                let t = cat(
                    self,
                    "replace.preview",
                    &[&run.totale.to_string(), &run.schede_con.to_string()],
                ) + &saltate;
                self.tabs[a].set_replace_status(t, true);
            }
        } else {
            let t = cat(
                self,
                "replace.done",
                &[&run.totale.to_string(), &run.schede_con.to_string()],
            ) + &saltate;
            self.tabs[a].remember_find(&run.spec.find);
            self.tabs[a].finish_replace(t);
        }
    }

    /// Il tema e la lingua della scheda attiva (quelli che le altre adottano).
    fn look(&self) -> (ThemeId, String) {
        let a = &self.tabs[self.active];
        (a.theme_id(), a.lang_code().to_string())
    }

    /// Esegue un comando del registro (`commands`): lo stesso del suo tasto. Non fa niente (e dice `false`) se il comando non
    /// e' attivo adesso, se c'e' un dialogo aperto, o se e' della sola finestra (lo zoom).
    pub fn exec(&mut self, cmd: crate::commands::Cmd) -> bool {
        if self.tabs[self.active].dialog_kind().is_some() || !crate::commands::enabled(self, cmd) {
            return false;
        }
        let Some(k) = cmd.info().key else {
            return false;
        };
        self.handle_key(k);
        true
    }

    /// Un tasto: quelli delle schede si tengono qui (se non c'e' un dialogo aperto), gli altri vanno alla scheda attiva.
    pub fn handle_key(&mut self, k: KeyEvent) {
        // una sostituzione in corso tiene tutti i tasti: Esc la ferma
        if self.replace.is_some() {
            if k.code == KeyCode::Esc {
                self.cancel_replace();
            }
            return;
        }
        if self.tabs[self.active].dialog_kind().is_none() {
            let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
            let alt = k.modifiers.contains(KeyModifiers::ALT);
            match k.code {
                KeyCode::PageDown if ctrl && !alt => return self.next(),
                KeyCode::PageUp if ctrl && !alt => return self.prev(),
                // Ctrl+Tab e Ctrl+Maiusc+Tab, come nella GUI (non tutti i terminali li lasciano arrivare)
                KeyCode::Tab if ctrl && k.modifiers.contains(KeyModifiers::SHIFT) => {
                    return self.prev();
                }
                KeyCode::Tab if ctrl => return self.next(),
                KeyCode::BackTab if ctrl => return self.prev(),
                KeyCode::Right if alt => return self.next(),
                KeyCode::Left if alt => return self.prev(),
                KeyCode::Char(c @ '1'..='9') if alt => {
                    return self.select(usize::from(c as u8 - b'1'));
                }
                KeyCode::Char('b') | KeyCode::Char('B') if ctrl => return self.toggle_nav(),
                KeyCode::Char('w') | KeyCode::Char('W') if alt && !ctrl => {
                    return self.toggle_special()
                }
                KeyCode::Char('l') | KeyCode::Char('L') if alt && !ctrl => {
                    return self.toggle_log_colors()
                }
                // Ctrl+N: un documento nuovo e vuoto in una scheda nuova
                KeyCode::Char('n') | KeyCode::Char('N') if ctrl => {
                    if let Err(e) = self.new_tab() {
                        let a = &mut self.tabs[self.active];
                        let testo = a.text_with("cli.err.open", &["", &e.to_string()]);
                        a.set_flash(testo);
                    }
                    return;
                }
                KeyCode::Char('w') | KeyCode::Char('W') if ctrl => return self.close(self.active),
                KeyCode::F(10) => return self.close_all(),
                KeyCode::Char('q') | KeyCode::Char('Q') if ctrl => return self.close_all(),
                _ => {}
            }
        }
        self.tabs[self.active].handle_key(k);
        self.settle();
    }

    /// Un evento del mouse: un clic sulla barra sceglie la scheda, il resto va alla scheda attiva (con la riga
    /// contata da sotto la barra).
    pub fn handle_mouse(&mut self, mut m: MouseEvent) {
        if self.replace.is_some() {
            return;
        }
        let sopra = self.top_height();
        if sopra > 0 && m.row < sopra {
            // un clic sulle barre; gli altri eventi li' arrivano alla scheda come sono
            if let MouseEventKind::Down(MouseButton::Left) = m.kind {
                if self.bar_height() == 1 && m.row == 0 {
                    if let Some(i) = self
                        .spans
                        .iter()
                        .position(|&(da, a)| a > da && m.column >= da && m.column < a)
                    {
                        self.select(i);
                    }
                } else {
                    self.nav_click(m.column);
                }
                return;
            }
        } else if sopra > 0 {
            m.row -= sopra;
        }
        self.tabs[self.active].handle_mouse(m);
    }

    /// Senza la barra dei salti a celle: la finestra la disegna con i suoi pulsanti (`nav_bar` dice se e' accesa).
    pub fn senza_nav_celle(mut self) -> Workspace {
        self.nav_cells = false;
        self
    }

    /// Accende o spegne la barra dei salti a celle (la finestra la spegne quando disegna i suoi pulsanti); lo spazio
    /// del testo si adatta.
    pub fn set_nav_cells(&mut self, accesa: bool) {
        if self.nav_cells != accesa {
            self.nav_cells = accesa;
            self.apply_size();
        }
    }

    /// Salta di `pagine` pagine avanti o indietro nella scheda attiva (come i pulsanti della barra dei salti).
    pub fn jump(&mut self, avanti: bool, pagine: usize) {
        self.tabs[self.active].page_jump(avanti, pagine);
        self.settle();
    }

    /// Porta la scheda attiva all'inizio del file (Ctrl+Home).
    pub fn go_start(&mut self) {
        self.tabs[self.active].handle_key(KeyEvent::new(KeyCode::Home, KeyModifiers::CONTROL));
        self.settle();
    }

    /// Porta la scheda attiva alla fine del file (Ctrl+End).
    pub fn go_end(&mut self) {
        self.tabs[self.active].handle_key(KeyEvent::new(KeyCode::End, KeyModifiers::CONTROL));
        self.settle();
    }

    /// I moltiplicatori dei salti di pagina della scheda attiva (le impostazioni `jump_pages`).
    pub fn jump_pages(&self) -> [usize; 3] {
        self.tabs[self.active].jump_pages()
    }

    /// Cambia il tema di tutte le schede (senza salvare: e' il tema del sistema, non una scelta dell'utente).
    pub fn set_theme(&mut self, tema: crate::theme::ThemeId) {
        let lingua = self.tabs[self.active].lang_code().to_string();
        for a in &mut self.tabs {
            a.adopt_look(tema, &lingua);
        }
    }

    /// Un clic sulla colonna `col` della barra dei salti: il pulsante che la copre agisce sulla scheda attiva.
    fn nav_click(&mut self, col: u16) {
        let Some(&(_, _, azione)) = self
            .nav_spans
            .iter()
            .find(|&&(da, a, _)| col >= da && col < a)
        else {
            return;
        };
        match azione {
            NavAzione::Home => self.go_start(),
            NavAzione::End => self.go_end(),
            NavAzione::Salto(avanti, pagine) => self.jump(avanti, pagine),
        }
    }

    /// Un testo incollato dal terminale.
    pub fn paste_text(&mut self, testo: &str) {
        if self.replace.is_some() {
            return;
        }
        self.tabs[self.active].paste_text(testo);
    }

    /// La copia in sospeso della scheda attiva (per gli appunti di sistema).
    pub fn take_copied(&mut self) -> Option<Vec<u8>> {
        self.tabs[self.active].take_copied()
    }

    /// Un giro dei lavori in background di tutte le schede (indice, ricerca, salvataggio).
    pub fn tick(&mut self) {
        for a in &mut self.tabs {
            a.tick();
        }
        self.settle();
        self.tick_session();
    }

    /// Aspetta che i lavori in background di tutte le schede finiscano, al piu' `t` per scheda; vero se sono finiti.
    pub fn wait_idle(&mut self, t: Duration) -> bool {
        let mut tutti = true;
        for a in &mut self.tabs {
            tutti &= a.wait_idle(t);
        }
        self.settle();
        tutti
    }

    /// Disegna: con una sola scheda quella e basta; con piu' schede la barra in cima e la scheda attiva sotto.
    pub fn draw(&mut self, f: &mut Frame) {
        let area = f.area();
        let (w, h) = (area.width, area.height);
        let sopra = self.top_height();
        if sopra == 0 || w == 0 || h < 4 + sopra {
            self.spans.clear();
            self.nav_spans.clear();
            ui::draw(f, &mut self.tabs[self.active]);
            return;
        }
        // la scheda attiva su un terminale di prova piu' basso delle barre, poi copiato sotto di loro
        let Ok(mut prova) = Terminal::new(TestBackend::new(w, h - sopra));
        let attiva = &mut self.tabs[self.active];
        if prova.draw(|g| ui::draw(g, attiva)).is_err() {
            return;
        }
        let sorgente = prova.backend().buffer();
        let buf = f.buffer_mut();
        for y in 0..h - sopra {
            for x in 0..w {
                if let (Some(da), Some(a)) = (
                    sorgente.cell((x, y)),
                    buf.cell_mut((area.x + x, area.y + y + sopra)),
                ) {
                    *a = da.clone();
                }
            }
        }
        if let Some((x, y)) = attiva.cursor_xy() {
            f.set_cursor_position((area.x + x, area.y + y + sopra));
            attiva.set_cursor_xy(Some((x, y + sopra)));
        }
        let schede = self.bar_height();
        if schede == 1 {
            self.draw_bar(f, Rect::new(area.x, area.y, w, 1));
        } else {
            self.spans.clear();
        }
        if self.nav && self.nav_cells {
            self.draw_nav(f, Rect::new(area.x, area.y + schede, w, 1));
        } else {
            self.nav_spans.clear();
        }
    }

    /// La barra dei salti: `[Home] [-1000] [-100] [-10] [-1] [+1] [+10] [+100] [+1000] [End]`, con i numeri delle
    /// impostazioni (le pagine); i pulsanti che non entrano interi nella larghezza non si disegnano.
    fn draw_nav(&mut self, f: &mut Frame, area: Rect) {
        let attiva = &self.tabs[self.active];
        let t = Theme::for_id(attiva.theme_id());
        let [a, b, c] = attiva.jump_pages();
        let voci = [
            ("Home".to_string(), NavAzione::Home),
            (format!("-{c}"), NavAzione::Salto(false, c)),
            (format!("-{b}"), NavAzione::Salto(false, b)),
            (format!("-{a}"), NavAzione::Salto(false, a)),
            ("-1".to_string(), NavAzione::Salto(false, 1)),
            ("+1".to_string(), NavAzione::Salto(true, 1)),
            (format!("+{a}"), NavAzione::Salto(true, a)),
            (format!("+{b}"), NavAzione::Salto(true, b)),
            (format!("+{c}"), NavAzione::Salto(true, c)),
            ("End".to_string(), NavAzione::End),
        ];
        self.nav_spans.clear();
        let larghezza = usize::from(area.width);
        let mut spans: Vec<Span> = Vec::new();
        let mut x = 0usize;
        for (etichetta, azione) in voci {
            let testo = format!("[{etichetta}]");
            let n = testo.chars().count();
            let sep = usize::from(x > 0);
            if x + sep + n > larghezza {
                break;
            }
            if sep == 1 {
                spans.push(Span::styled(" ", t.status));
                x += 1;
            }
            self.nav_spans
                .push((area.x + x as u16, area.x + (x + n) as u16, azione));
            spans.push(Span::styled(testo, t.key));
            x += n;
        }
        f.render_widget(Paragraph::new(Line::from(spans)).style(t.status), area);
    }

    /// La barra: un riquadro " nome* " per scheda, quella attiva evidenziata; se non ci stanno tutte parte dalla piu'
    /// a sinistra che lascia vedere l'attiva.
    fn draw_bar(&mut self, f: &mut Frame, area: Rect) {
        let t = Theme::for_id(self.tabs[self.active].theme_id());
        let etichette: Vec<String> = self
            .tabs
            .iter()
            .map(|a| {
                let nome: String = a.file_name().chars().take(NOME_MAX).collect();
                let segno = if a.is_modified() { "*" } else { "" };
                format!(" {nome}{segno} ")
            })
            .collect();
        let larghezze: Vec<usize> = etichette.iter().map(|e| e.chars().count()).collect();
        let w = usize::from(area.width);
        let mut prima = 0;
        while prima < self.active && larghezze[prima..=self.active].iter().sum::<usize>() > w {
            prima += 1;
        }
        self.spans = vec![(0, 0); self.tabs.len()];
        let mut spans: Vec<Span> = Vec::new();
        let mut x = 0usize;
        for i in prima..self.tabs.len() {
            if x >= w {
                break;
            }
            let visibile = larghezze[i].min(w - x);
            let testo: String = etichette[i].chars().take(visibile).collect();
            let stile: Style = if i == self.active {
                t.selected
            } else {
                t.status
            };
            self.spans[i] = (area.x + x as u16, area.x + (x + visibile) as u16);
            spans.push(Span::styled(testo, stile));
            x += visibile;
        }
        f.render_widget(Paragraph::new(Line::from(spans)).style(t.status), area);
    }

    /// Lo schermo come testo, una stringa per riga (come `ui::render_lines`, ma con la barra delle schede).
    pub fn render_lines(&mut self, w: u16, h: u16) -> Vec<String> {
        if self.top_height() == 0 {
            return ui::render_lines(&mut self.tabs[self.active], w, h);
        }
        let Ok(mut prova) = Terminal::new(TestBackend::new(w, h));
        if prova.draw(|f| self.draw(f)).is_err() {
            return vec![String::new(); h as usize];
        }
        prova
            .backend()
            .buffer()
            .content()
            .chunks(usize::from(w).max(1))
            .map(|riga| {
                let s: String = riga
                    .iter()
                    .map(|c| match c.symbol() {
                        "" => " ",
                        s => s,
                    })
                    .collect();
                s.trim_end().to_string()
            })
            .collect()
    }
}
