//! L'app del TUI: una macchina a stati testabile senza terminale (il contratto sta in `docs/TEAM/DESIGN_TUI.md`).
//! Riceve tasti, mouse e dimensioni dello schermo; non disegna (lo fa `ui.rs`); tutta la logica di testo e' nel core.

use crate::settings::Settings;
use crate::theme::ThemeId;
use mrp_core::lang::{Catalog, Lang};
use mrp_core::scan::{self, Next};
use mrp_core::view::{self, ViewOptions};
use mrp_core::{Document, FileSource, LineIndex, SaveError};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Quanto si guarda avanti e indietro per gli a-capo: il limite di scansione del core.
const MAX_SCAN: u64 = scan::DEFAULT_MAX_SCAN;

/// Le righe della rotella per scatto.
const WHEEL_ROWS: usize = 3;

/// Quante cifre servono per scrivere `n` in decimale (almeno 1): la larghezza dei numeri di riga.
fn cifre(n: u64) -> usize {
    let mut conto = 1;
    let mut v = n;
    while v >= 10 {
        v /= 10;
        conto += 1;
    }
    conto
}

/// Un numero intero non negativo scritto in cifre ASCII: `None` se e' vuoto, contiene altro o non ci sta in un
/// `u64` (la grammatica del "vai a").
fn parse_cifre(s: &str) -> Option<u64> {
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    s.parse().ok()
}

/// Quanti byte occupa il primo carattere di `bytes`: il piu' corto prefisso UTF-8 valido, altrimenti un solo byte
/// (un byte non valido e' un carattere tutto suo, come nel modello di `mrp_core::view`).
fn primo_carattere(bytes: &[u8]) -> usize {
    for n in 1..=bytes.len().min(4) {
        let intero = std::str::from_utf8(&bytes[..n])
            .ok()
            .and_then(|s| s.chars().next())
            .is_some_and(|c| c.len_utf8() == n);
        if intero {
            return n;
        }
    }
    1
}

/// Quanti byte occupa l'ultimo carattere di `bytes`: il piu' corto suffisso UTF-8 valido, altrimenti un solo byte.
fn ultimo_carattere(bytes: &[u8]) -> usize {
    for n in 1..=bytes.len().min(4) {
        let intero = std::str::from_utf8(&bytes[bytes.len() - n..])
            .ok()
            .and_then(|s| s.chars().next())
            .is_some_and(|c| c.len_utf8() == n);
        if intero {
            return n;
        }
    }
    1
}

/// Fa partire il thread che costruisce l'indice righe del file `path`: apre la SUA sorgente (il documento
/// non si condivide mentre indicizza) e consegna l'esito sul canale. E' il meccanismo dell'apertura,
/// riusato dopo un salvataggio riuscito (il file su disco e' cambiato).
fn spawn_index_job(path: &Path, cancel: &Arc<AtomicBool>, progress: &Arc<AtomicU64>) -> IndexJob {
    let (tx, rx) = mpsc::channel();
    let path = path.to_path_buf();
    let cancel_thread = Arc::clone(cancel);
    let progress_thread = Arc::clone(progress);
    std::thread::spawn(move || {
        let esito = FileSource::open(&path).and_then(|src| {
            LineIndex::build(&src, &cancel_thread, &progress_thread)
                .map_err(|_| io::Error::other("indicizzazione annullata"))
        });
        let _ = tx.send(esito);
    });
    IndexJob {
        rx,
        progress: Arc::clone(progress),
    }
}

/// Configurazione di partenza dell'app (i test la costruiscono a mano, il binario la ricava da argomenti e profilo).
#[derive(Debug, Clone)]
pub struct AppConfig {
    /// Tema iniziale.
    pub theme: ThemeId,
    /// Codice della lingua da usare (`None` o sconosciuto: inglese).
    pub lang_code: Option<String>,
    /// Le lingue disponibili (di norma `mrp_core::lang::registry()`); il predefinito e' solo l'inglese incorporato.
    pub langs: Vec<Lang>,
    /// Dove salvare le impostazioni quando si cambia tema o lingua; `None`: non si salva niente.
    pub settings_path: Option<PathBuf>,
    /// Vero: costruisce l'indice righe in background.
    pub index: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        AppConfig {
            theme: ThemeId::DEFAULT,
            lang_code: None,
            langs: vec![Lang::english()],
            settings_path: None,
            index: true,
        }
    }
}

/// Il dialogo aperto (se c'e'): tiene i tasti per se' finche' non si chiude.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogKind {
    /// F1: elenco dei tasti.
    Help,
    /// F6: menu dei temi.
    Theme,
    /// F7: menu delle lingue.
    Lang,
    /// F5 / Ctrl+G: prompt "vai a".
    Goto,
    /// F3 / Ctrl+F: prompt di ricerca.
    Find,
    /// Conferma all'uscita con modifiche non salvate.
    Confirm,
    /// Salvataggio in corso (avanzamento, Esc annulla).
    Saving,
}

/// L'indicizzazione in corso: il canale col thread che costruisce l'indice e i byte gia' esaminati.
struct IndexJob {
    /// Il risultato del thread: l'indice pronto, o l'errore di apertura della sorgente.
    rx: Receiver<io::Result<LineIndex>>,
    /// Byte di file gia' esaminati, per la percentuale nella riga di stato.
    progress: Arc<AtomicU64>,
}

/// Il salvataggio in corso: il canale col thread che tiene il documento, i byte gia' scritti e l'ordine di
/// uscire dopo un salvataggio riuscito.
struct SaveJob {
    /// Il risultato del thread: il documento (torna comunque, anche con errore: non si perde mai) e l'esito
    /// del salvataggio.
    rx: Receiver<(Document, Result<(), SaveError>)>,
    /// Byte di file gia' scritti, per la percentuale nella riga di stato e nella barra del dialogo.
    progress: Arc<AtomicU64>,
    /// Chiede al thread di annullare (lo fa Esc nel dialogo `Saving`).
    cancel: Arc<AtomicBool>,
    /// La lunghezza del documento quando il salvataggio e' partito: il totale della percentuale.
    len: u64,
    /// Vero: dopo un salvataggio riuscito l'app esce (la 'S' del dialogo di uscita).
    quit_after: bool,
}

/// Lo stato dell'app.
pub struct App {
    /// Il documento aperto: `None` quando si esce (il file si rilascia) o mentre un salvataggio lo tiene
    /// nel suo thread.
    doc: Option<Document>,
    /// Il nome del file, per il titolo.
    file_name: String,
    /// Offset del primo byte della prima riga visuale dello schermo (sempre un inizio di riga visuale).
    top: u64,
    /// Offset in byte del cursore (un confine di carattere).
    cursor: u64,
    /// La colonna ricordata per i movimenti verticali (celle di terminale).
    want_col: u64,
    /// La prima colonna visibile (scorrimento orizzontale).
    hscroll: usize,
    /// Dimensioni dello schermo in celle.
    w: u16,
    h: u16,
    /// Vero in modifica.
    edit: bool,
    /// Il messaggio transitorio della riga di stato (sparisce al primo tasto successivo).
    flash: Option<String>,
    /// Il vocabolario attivo.
    cat: Catalog,
    /// Il tema attivo.
    theme: ThemeId,
    /// Chiede al thread dell'indice di fermarsi (lo fa `Drop`).
    cancel: Arc<AtomicBool>,
    /// L'indicizzazione in corso, se c'e'.
    index_job: Option<IndexJob>,
    /// Il salvataggio in corso, se c'e': il documento e' nel suo thread e qui non c'e'.
    save_job: Option<SaveJob>,
    /// Le righe di testo dell'ultimo disegno (gutter e testo, gia' pronte): le ridisegna `ui::draw`
    /// mentre un salvataggio tiene il documento nel suo thread.
    drawn_rows: Vec<(String, String)>,
    /// Vero quando si e' chiesto di uscire.
    quit: bool,
    /// Dove sta il cursore sullo schermo dopo l'ultimo disegno (lo scrive `ui::draw`).
    cursor_xy: Option<(u16, u16)>,
    /// Il dialogo aperto: tiene tutti i tasti per se' finche' non si chiude.
    dialog: Option<DialogKind>,
    /// La voce selezionata nel menu aperto (tema o lingua).
    menu_index: usize,
    /// Il testo scritto nel prompt "vai a".
    goto_text: String,
    /// Le lingue disponibili: il menu di F7 e i cambi del `Catalog`.
    langs: Vec<Lang>,
    /// Dove salvare le impostazioni quando si cambia tema o lingua; `None`: non si salva niente.
    settings_path: Option<PathBuf>,
    /// Il codice della lingua scelta dall'utente (`None`: l'inglese incorporato): e' la `lang` delle impostazioni.
    lang_picked: Option<String>,
    /// Vero: l'indice righe si costruisce in background (all'apertura e dopo un salvataggio riuscito).
    index: bool,
}

impl App {
    /// Apre `path`. Errore se manca o non e' un file regolare.
    pub fn open(path: &Path, cfg: AppConfig) -> io::Result<App> {
        let doc = Document::open(path)?;
        let file_name = doc
            .path()
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        // La lingua attiva: quella col codice richiesto fra le disponibili, altrimenti l'inglese incorporato. La
        // scelta conta come "fatta dall'utente" (per le impostazioni) solo se il codice esiste davvero.
        let (cat, lang_picked) = match &cfg.lang_code {
            Some(code) => match cfg.langs.iter().find(|l| &l.code == code) {
                Some(l) => (Catalog::with(l.clone()), Some(code.clone())),
                None => (Catalog::english(), None),
            },
            None => (Catalog::english(), None),
        };
        let cancel = Arc::new(AtomicBool::new(false));
        let progress = Arc::new(AtomicU64::new(0));
        let index_job = if cfg.index {
            // Il thread apre la SUA sorgente sullo stesso file: il documento non si condivide mentre indicizza.
            Some(spawn_index_job(doc.path(), &cancel, &progress))
        } else {
            None
        };
        Ok(App {
            doc: Some(doc),
            file_name,
            top: 0,
            cursor: 0,
            want_col: 0,
            hscroll: 0,
            w: 80,
            h: 24,
            edit: false,
            flash: None,
            cat,
            theme: cfg.theme,
            cancel,
            index_job,
            save_job: None,
            drawn_rows: Vec::new(),
            quit: false,
            cursor_xy: None,
            dialog: None,
            menu_index: 0,
            goto_text: String::new(),
            langs: cfg.langs,
            settings_path: cfg.settings_path,
            lang_picked,
            index: cfg.index,
        })
    }

    /// Nuove dimensioni dello schermo; il cursore resta visibile.
    pub fn resize(&mut self, w: u16, h: u16) {
        self.w = w;
        self.h = h;
        // Un salvataggio tiene il documento: cursore, `top` e `hscroll` restano com'erano (senza documento
        // non si possono riaggiustare); tornato il documento li riaggiusta la fine del salvataggio.
        if self.doc.is_none() && self.save_job.is_some() {
            return;
        }
        self.clamp_cursor();
        self.scroll_adjust();
    }

    /// Un tasto premuto.
    pub fn handle_key(&mut self, key: KeyEvent) {
        // Il messaggio transitorio sparisce al primo tasto premuto dopo, prima di eseguirlo.
        self.flash = None;
        // Un dialogo aperto riceve TUTTI i tasti: il documento non si muove finche' non si chiude.
        if self.dialog.is_some() {
            self.dialog_key(key);
            return;
        }
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let alt = key.modifiers.contains(KeyModifiers::ALT);
        let shift = key.modifiers.contains(KeyModifiers::SHIFT);
        // I tasti che non continuano la scrittura chiudono il passo della storia prima di agire: i caratteri
        // e i Backspace di seguito sono un passo solo, Enter allunga il passo corrente e poi lo chiude da
        // solo.
        let continua = !ctrl
            && !alt
            && matches!(
                key.code,
                KeyCode::Char(_) | KeyCode::Enter | KeyCode::Backspace
            );
        if !continua {
            self.seal_history();
        }
        match key.code {
            KeyCode::Up => self.move_up(),
            KeyCode::Down => self.move_down(),
            KeyCode::Left => self.move_left(),
            KeyCode::Right => self.move_right(),
            KeyCode::Home if ctrl => self.move_file_start(),
            KeyCode::Home => self.move_home(),
            KeyCode::End if ctrl => self.move_file_end(),
            KeyCode::End => self.move_end(),
            KeyCode::PageUp => self.page_up(),
            KeyCode::PageDown => self.page_down(),
            KeyCode::F(1) => self.open_dialog(DialogKind::Help),
            KeyCode::F(2) => self.start_save(false),
            KeyCode::F(4) => self.toggle_edit(),
            KeyCode::F(5) => self.open_dialog(DialogKind::Goto),
            KeyCode::F(6) => self.open_dialog(DialogKind::Theme),
            KeyCode::F(7) => self.open_dialog(DialogKind::Lang),
            KeyCode::F(10) => self.quit_request(),
            // Ctrl+Z annulla; Ctrl+Y e Ctrl+Shift+Z ripetono (col maiuscolo SHIFT arriva anche 'Z').
            KeyCode::Char('z') | KeyCode::Char('Z') if ctrl && !shift => self.undo(),
            KeyCode::Char('z') | KeyCode::Char('Z') if ctrl => self.redo(),
            KeyCode::Char('y') | KeyCode::Char('Y') if ctrl => self.redo(),
            KeyCode::Char('s') if ctrl => self.start_save(false),
            KeyCode::Char('g') if ctrl => self.open_dialog(DialogKind::Goto),
            KeyCode::Char('q') if ctrl => self.quit_request(),
            // I tasti di scrittura: un carattere (anche con SHIFT) si inserisce al cursore, Enter mette un
            // a-capo, Tab un tabulatore, Backspace e Delete tolgono il carattere prima o sotto il cursore
            // (in lettura sono rifiutati con un suggerimento).
            KeyCode::Char(c) if !ctrl && !alt => self.insert_char(c),
            KeyCode::Enter if !ctrl && !alt => self.insert_enter(),
            KeyCode::Backspace if !ctrl && !alt => self.delete_before(),
            KeyCode::Delete if !ctrl && !alt => self.delete_after(),
            KeyCode::Tab if !ctrl && !alt => self.insert_tab(),
            _ => {}
        }
    }

    /// Un evento del mouse (per ora solo la rotella); anche il mouse chiude il passo della storia.
    pub fn handle_mouse(&mut self, m: MouseEvent) {
        self.seal_history();
        match m.kind {
            MouseEventKind::ScrollDown => self.wheel(true),
            MouseEventKind::ScrollUp => self.wheel(false),
            _ => {}
        }
    }

    /// Si esce subito (niente modifiche da salvare): il documento si rilascia e il file torna libero,
    /// l'indicizzazione in corso si annulla.
    fn quit_now(&mut self) {
        self.quit = true;
        self.doc = None;
        self.index_job = None;
        self.cancel.store(true, Ordering::Relaxed);
    }

    /// F10 / Ctrl+Q: l'uscita. Con modifiche non salvate chiede prima (dialogo `Confirm`), senza esce
    /// subito.
    fn quit_request(&mut self) {
        if self.is_modified() {
            self.open_dialog(DialogKind::Confirm);
        } else {
            self.quit_now();
        }
    }

    // ----- dialoghi -----

    /// Apre un dialogo: l'aiuto, i menu (che partono dalla voce corrente) o il prompt "vai a" (vuoto).
    fn open_dialog(&mut self, kind: DialogKind) {
        match kind {
            DialogKind::Theme => {
                self.menu_index = ThemeId::ALL
                    .iter()
                    .position(|&t| t == self.theme)
                    .unwrap_or(0);
            }
            DialogKind::Lang => {
                self.menu_index = self
                    .langs
                    .iter()
                    .position(|l| l.code == self.cat.code())
                    .unwrap_or(0);
            }
            DialogKind::Goto => self.goto_text.clear(),
            _ => {}
        }
        self.dialog = Some(kind);
    }

    /// I tasti di un dialogo aperto: valgono solo i suoi, tutto il resto si ignora.
    fn dialog_key(&mut self, key: KeyEvent) {
        let Some(kind) = self.dialog else {
            return;
        };
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let alt = key.modifiers.contains(KeyModifiers::ALT);
        match kind {
            // Aiuto: Esc, F1 e Enter chiudono.
            DialogKind::Help => match key.code {
                KeyCode::Esc | KeyCode::Enter | KeyCode::F(1) => self.dialog = None,
                _ => {}
            },
            // Menu (tema e lingua): le frecce scelgono (ferme agli estremi), Enter applica, Esc annulla.
            DialogKind::Theme | DialogKind::Lang => match key.code {
                KeyCode::Up => self.menu_index = self.menu_index.saturating_sub(1),
                KeyCode::Down => {
                    if self.menu_index + 1 < self.menu_len() {
                        self.menu_index += 1;
                    }
                }
                KeyCode::Enter => self.menu_apply(),
                KeyCode::Esc => self.dialog = None,
                _ => {}
            },
            // Prompt "vai a": i caratteri si scrivono, Backspace toglie, Enter conferma, Esc annulla.
            DialogKind::Goto => match key.code {
                KeyCode::Esc => self.dialog = None,
                KeyCode::Enter => {
                    let testo = std::mem::take(&mut self.goto_text);
                    self.dialog = None;
                    self.goto_run(&testo);
                }
                KeyCode::Backspace => {
                    self.goto_text.pop();
                }
                KeyCode::Char(c) if !ctrl && !alt => self.goto_text.push(c),
                _ => {}
            },
            // Conferma all'uscita con modifiche non salvate: S salva e poi esce, D esce scartando, C o Esc
            // annullano (maiuscole comprese).
            DialogKind::Confirm => match key.code {
                KeyCode::Char('s') | KeyCode::Char('S') => {
                    self.dialog = None;
                    self.start_save(true);
                }
                KeyCode::Char('d') | KeyCode::Char('D') => {
                    self.dialog = None;
                    self.quit_now();
                }
                KeyCode::Char('c') | KeyCode::Char('C') | KeyCode::Esc => self.dialog = None,
                _ => {}
            },
            // Salvataggio in corso: solo Esc, che chiede al thread di annullare (il dialogo resta aperto
            // finche' il lavoro non torna, col suo esito).
            DialogKind::Saving if key.code == KeyCode::Esc => {
                if let Some(job) = &self.save_job {
                    job.cancel.store(true, Ordering::Relaxed);
                }
            }
            // Il prompt di ricerca (E11) non prende ancora nessun tasto.
            _ => {}
        }
    }

    /// Quante voci ha il menu aperto.
    fn menu_len(&self) -> usize {
        match self.dialog {
            Some(DialogKind::Theme) => ThemeId::ALL.len(),
            Some(DialogKind::Lang) => self.langs.len(),
            _ => 0,
        }
    }

    /// Enter in un menu: applica la scelta, chiude e salva le impostazioni. La lingua cambia il `Catalog` PRIMA
    /// del messaggio, cosi' `flash.lang` e' nella nuova lingua.
    fn menu_apply(&mut self) {
        match self.dialog {
            Some(DialogKind::Theme) => {
                let Some(&tema) = ThemeId::ALL.get(self.menu_index) else {
                    return;
                };
                self.theme = tema;
                self.dialog = None;
                self.flash = Some(self.cat.tf("flash.theme", &[self.cat.t(tema.key())]));
                self.save_settings();
            }
            Some(DialogKind::Lang) => {
                let Some(l) = self.langs.get(self.menu_index).cloned() else {
                    return;
                };
                let nome = l.name.clone();
                self.cat = Catalog::with(l);
                self.lang_picked = Some(self.cat.code().to_string());
                self.dialog = None;
                self.flash = Some(self.cat.tf("flash.lang", &[&nome]));
                self.save_settings();
            }
            _ => {}
        }
    }

    /// Scrive le impostazioni (il tema e la lingua scelta) se c'e' un posto dove salvarle. Un errore di scrittura
    /// si ignora: il cambiamento resta valido per questa sessione.
    fn save_settings(&mut self) {
        let Some(path) = self.settings_path.clone() else {
            return;
        };
        let s = Settings {
            theme: self.theme,
            lang: self.lang_picked.clone(),
        };
        let _ = s.save(&path);
    }

    /// Esegue il testo del prompt "vai a" (il dialogo e' gia' chiuso): `N` = riga N (1-based, serve l'indice
    /// pronto), `@N` = offset in byte, `%P` = percentuale 0..=100. Qualunque altra cosa, o in errore, il cursore
    /// non si muove.
    fn goto_run(&mut self, testo: &str) {
        // Testo vuoto: chiuso senza chiedere niente.
        if testo.is_empty() {
            return;
        }
        if let Some(resto) = testo.strip_prefix('@') {
            if let Some(n) = parse_cifre(resto) {
                // Oltre la fine: l'ultima posizione consentita dal modo.
                self.goto_move(n.min(self.max_cursor()));
                return;
            }
        } else if let Some(resto) = testo.strip_prefix('%') {
            if let Some(p) = parse_cifre(resto) {
                if p <= 100 {
                    let len = self.document_len();
                    self.goto_move(len.saturating_mul(p) / 100);
                    return;
                }
            }
        } else if let Some(n) = parse_cifre(testo) {
            if n == 0 {
                self.flash = Some(self.cat.tf("flash.goto_bad", &[testo]));
                return;
            }
            // Il numero di riga serve l'indice pronto; offset e percentuale no.
            if !self.doc.as_ref().is_some_and(|doc| doc.has_lines()) {
                self.flash = Some(self.cat.t("flash.goto_wait").to_string());
                return;
            }
            let Some(conteggio) = self.doc.as_mut().and_then(|doc| doc.line_count()) else {
                return;
            };
            if n > conteggio {
                self.flash = Some(self.cat.tf(
                    "flash.goto_range",
                    &[&n.to_string(), &conteggio.to_string()],
                ));
                return;
            }
            let Some(pos) = self.doc.as_mut().and_then(|doc| doc.offset_of_line(n - 1)) else {
                return;
            };
            self.goto_move(pos);
            return;
        }
        self.flash = Some(self.cat.tf("flash.goto_bad", &[testo]));
    }

    /// Un "vai a" riuscito: il cursore va su un confine di carattere e la sua riga si vede (centrata se non si
    /// vedeva gia').
    fn goto_move(&mut self, pos: u64) {
        let pos = self.snap_to_char(pos.min(self.max_cursor()));
        self.cursor = pos;
        self.remember_col();
        self.center_cursor_row();
        self.scroll_adjust();
    }

    /// Centraggio del "vai a": se la riga del cursore non e' visibile, `top` diventa l'inizio della riga visuale
    /// `righe_di_testo / 2` righe PRIMA di quella del cursore (non prima dell'inizio del file), poi limitato
    /// all'ultima pagina.
    fn center_cursor_row(&mut self) {
        let text_rows = self.text_rows();
        if text_rows == 0 {
            return;
        }
        let row = self.cursor_row();
        if row == self.top || self.rows_after(self.top, text_rows - 1).contains(&row) {
            return;
        }
        let mut top = self
            .rows_before(row, text_rows / 2)
            .last()
            .copied()
            .unwrap_or(row);
        let last = self.last_top();
        if top > last {
            top = last;
        }
        self.top = top;
    }

    /// Porta `pos` sul confine di carattere piu' vicino non oltre `pos`: un carattere UTF-8 valido occupa al piu'
    /// quattro byte, quindi bastano tre passi indietro (un byte di prosecuzione `0b10xxxxxx` sta dentro un
    /// carattere multibyte, un confine non lo e' mai).
    fn snap_to_char(&self, pos: u64) -> u64 {
        let Some(doc) = self.doc.as_ref() else {
            return pos;
        };
        let mut pos = pos.min(doc.len());
        for _ in 0..3 {
            if pos == 0 {
                break;
            }
            let sotto = doc.read_range(pos, 1);
            match sotto.first() {
                Some(b) if (b & 0b1100_0000) == 0b1000_0000 => pos -= 1,
                _ => break,
            }
        }
        pos
    }

    /// Raccoglie i lavori in background: prima il salvataggio (quando torna rimette il documento al suo
    /// posto e, dopo uno riuscito, puo' ripartire l'indice), poi l'indice righe.
    pub fn tick(&mut self) {
        if self.save_job.is_some() {
            self.tick_save();
        }
        let esito = match self.index_job.as_mut() {
            Some(job) => job.rx.try_recv(),
            None => return,
        };
        match esito {
            Ok(Ok(indice)) => {
                // A indice pronto non c'e' nessun messaggio: sparisce solo l'indicatore di avanzamento.
                if let Some(doc) = self.doc.as_mut() {
                    doc.install_lines(indice);
                }
                self.index_job = None;
            }
            Ok(Err(e)) => {
                self.flash = Some(self.cat.tf("flash.index_failed", &[&e.to_string()]));
                self.index_job = None;
            }
            Err(mpsc::TryRecvError::Empty) => {}
            Err(mpsc::TryRecvError::Disconnected) => {
                // Difensivo: il thread e' terminato senza consegnare nulla, non si aspetta piu' nulla.
                self.index_job = None;
            }
        }
    }

    /// Chiama `tick` finche' non c'e' piu' un lavoro in corso (salvataggio o indicizzazione) o scade
    /// `timeout`. Vero se non c'e' piu' niente in corso.
    pub fn wait_idle(&mut self, timeout: Duration) -> bool {
        let inizio = Instant::now();
        loop {
            self.tick();
            if self.save_job.is_none() && self.index_job.is_none() {
                return true;
            }
            if inizio.elapsed() >= timeout {
                return false;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    pub fn should_quit(&self) -> bool {
        self.quit
    }

    /// Offset del primo byte della prima riga visuale dello schermo.
    pub fn top(&self) -> u64 {
        self.top
    }

    /// Offset in byte del cursore.
    pub fn cursor(&self) -> u64 {
        self.cursor
    }

    /// Prima colonna visibile (scorrimento orizzontale).
    pub fn hscroll(&self) -> usize {
        self.hscroll
    }

    pub fn is_edit(&self) -> bool {
        self.edit
    }

    pub fn is_modified(&self) -> bool {
        match self.doc.as_ref() {
            Some(doc) => doc.is_modified(),
            // Un salvataggio in corso tiene il documento: si salva solo se c'e' una modifica da scrivere.
            None => self.save_job.is_some(),
        }
    }

    pub fn document_len(&self) -> u64 {
        match self.doc.as_ref() {
            Some(doc) => doc.len(),
            // Un salvataggio in corso tiene il documento: la lunghezza e' quella di partenza (il salvataggio
            // non cambia il documento).
            None => self.save_job.as_ref().map_or(0, |job| job.len),
        }
    }

    /// Posizione del cursore sullo schermo (colonna, riga) dopo l'ultimo `draw`; `None` se un dialogo e' aperto o
    /// non si e' ancora disegnato. Nei prompt e' la posizione dentro il prompt, dopo il testo scritto.
    pub fn cursor_xy(&self) -> Option<(u16, u16)> {
        match self.dialog {
            Some(DialogKind::Goto) => self.cursor_xy,
            Some(_) => None,
            None => self.cursor_xy,
        }
    }

    /// Il dialogo aperto.
    pub fn dialog_kind(&self) -> Option<DialogKind> {
        self.dialog
    }

    /// La voce selezionata di un menu (tema, lingua) aperto; `None` se non c'e' un menu.
    pub fn menu_index(&self) -> Option<usize> {
        match self.dialog {
            Some(DialogKind::Theme) | Some(DialogKind::Lang) => Some(self.menu_index),
            _ => None,
        }
    }

    pub fn theme_id(&self) -> ThemeId {
        self.theme
    }

    /// Codice della lingua attiva.
    pub fn lang_code(&self) -> &str {
        self.cat.code()
    }

    /// Il contenuto del documento (per i test: legge tutto, solo per file piccoli).
    pub fn document_text(&self) -> Vec<u8> {
        const PEZZO: u64 = 64 * 1024;
        let mut out = Vec::new();
        let Some(doc) = self.doc.as_ref() else {
            return out;
        };
        let len = doc.len();
        let mut p = 0;
        while p < len {
            let n = PEZZO.min(len - p);
            let blocco = doc.read_range(p, n);
            out.extend_from_slice(&blocco);
            p += n;
        }
        out
    }

    // ----- quello che guarda `ui::draw` -----

    /// Il documento, per il disegno (`view::rows_from` e compagnia); `None` quando si e' usciti.
    pub(crate) fn document(&self) -> Option<&Document> {
        self.doc.as_ref()
    }

    /// Il nome del file per il titolo.
    pub(crate) fn file_name(&self) -> &str {
        &self.file_name
    }

    /// Il messaggio transitorio della riga di stato, se c'e'.
    pub(crate) fn flash(&self) -> Option<&str> {
        self.flash.as_deref()
    }

    /// Il vocabolario attivo (ogni stringa visibile viene da qui).
    pub(crate) fn catalog(&self) -> &Catalog {
        &self.cat
    }

    /// Le lingue disponibili (le righe del menu di F7).
    pub(crate) fn langs(&self) -> &[Lang] {
        &self.langs
    }

    /// Il testo scritto nel prompt "vai a".
    pub(crate) fn goto_text(&self) -> &str {
        &self.goto_text
    }

    /// La percentuale dell'indicizzazione in corso (0..=100); `None` se non c'e' un lavoro in corso.
    pub(crate) fn index_progress(&self) -> Option<u64> {
        let job = self.index_job.as_ref()?;
        let len = self.document_len();
        if len == 0 {
            return Some(0);
        }
        Some(job.progress.load(Ordering::Relaxed).saturating_mul(100) / len)
    }

    /// La percentuale del salvataggio in corso (0..=100); `None` se non c'e' un salvataggio in corso.
    pub(crate) fn save_progress(&self) -> Option<u64> {
        let job = self.save_job.as_ref()?;
        if job.len == 0 {
            return Some(0);
        }
        Some(job.progress.load(Ordering::Relaxed).saturating_mul(100) / job.len)
    }

    /// Le righe di testo dell'ultimo disegno (gutter e testo): le ridisegna `ui::draw` mentre un
    /// salvataggio tiene il documento nel suo thread.
    pub(crate) fn drawn_rows(&self) -> &[(String, String)] {
        &self.drawn_rows
    }

    /// Ricorda le righe di testo di questo disegno (lo chiama `ui::draw`).
    pub(crate) fn set_drawn_rows(&mut self, righe: &[(String, String)]) {
        self.drawn_rows = righe.to_vec();
    }

    /// Larghezza del gutter: le cifre della lunghezza del documento piu' il separatore.
    pub(crate) fn gutter_w(&self) -> usize {
        cifre(self.document_len()) + 1
    }

    /// Il numero (1-based) della riga vera che inizia a `start`, per il gutter: `None` se l'indice non e' pronto
    /// o `start` e' oltre la fine (la riga vuota finale ha il numero `line_count + 1`).
    pub(crate) fn gutter_number(&mut self, start: u64) -> Option<u64> {
        let doc = self.doc.as_mut()?;
        if !doc.has_lines() {
            return None;
        }
        let len = doc.len();
        if start >= len {
            doc.line_count().map(|conteggio| conteggio + 1)
        } else {
            doc.line_at_offset(start).map(|riga| riga + 1)
        }
    }

    /// Il numero (1-based) della riga del cursore per la riga di stato; `None` se l'indice non e' pronto.
    pub(crate) fn cursor_line(&mut self) -> Option<u64> {
        let doc = self.doc.as_mut()?;
        if !doc.has_lines() {
            return None;
        }
        let len = doc.len();
        if self.cursor < len {
            doc.line_at_offset(self.cursor).map(|riga| riga + 1)
        } else {
            let conteggio = doc.line_count()?;
            let finisce_nl = len > 0 && doc.read_range(len - 1, 1).first() == Some(&b'\n');
            Some(if len == 0 || finisce_nl {
                conteggio + 1
            } else {
                conteggio
            })
        }
    }

    /// Inizio della riga visuale che contiene il cursore.
    pub(crate) fn cursor_row(&self) -> u64 {
        match self.doc.as_ref() {
            Some(doc) => scan::row_start(doc, self.cursor, MAX_SCAN).0,
            None => 0,
        }
    }

    /// Colonna di terminale (da 0) del cursore nella sua riga.
    pub(crate) fn cursor_col(&self) -> usize {
        let Some(doc) = self.doc.as_ref() else {
            return 0;
        };
        view::col_of(doc, self.cursor_row(), self.cursor, &ViewOptions::default())
    }

    /// Ricorda la posizione del cursore sullo schermo (la scrive `ui::draw`).
    pub(crate) fn set_cursor_xy(&mut self, xy: Option<(u16, u16)>) {
        self.cursor_xy = xy;
    }

    // ----- geometria dello schermo -----

    /// Le righe di testo dello schermo (le altre tre: titolo, stato, barra dei tasti).
    fn text_rows(&self) -> usize {
        (self.h as usize).saturating_sub(3)
    }

    /// Le celle di testo per riga (la larghezza meno il gutter).
    fn cells(&self) -> usize {
        (self.w as usize).saturating_sub(self.gutter_w())
    }

    /// Il limite del cursore: in lettura non oltre l'ultimo a-capo di un file che ci finisce.
    fn max_cursor(&self) -> u64 {
        let Some(doc) = self.doc.as_ref() else {
            return 0;
        };
        let len = doc.len();
        if self.edit || len == 0 {
            len
        } else if doc.read_range(len - 1, 1).first() == Some(&b'\n') {
            len - 1
        } else {
            len
        }
    }

    /// Il cursore resta entro i limiti del modo.
    fn clamp_cursor(&mut self) {
        let max = self.max_cursor();
        if self.cursor > max {
            self.cursor = max;
        }
    }

    /// Ricalcola la colonna ricordata dalla posizione del cursore (la usano i movimenti orizzontali).
    fn remember_col(&mut self) {
        let Some(doc) = self.doc.as_ref() else {
            return;
        };
        let row = scan::row_start(doc, self.cursor, MAX_SCAN).0;
        self.want_col = view::col_of(doc, row, self.cursor, &ViewOptions::default()) as u64;
    }

    /// Porta il cursore sulla riga che inizia in `row`, alla colonna ricordata.
    fn cursor_at_col(&mut self, row: u64) {
        let Some(doc) = self.doc.as_ref() else {
            return;
        };
        self.cursor = view::offset_at_col(doc, row, self.want_col, &ViewOptions::default())
            .min(self.max_cursor());
    }

    // ----- movimenti -----

    fn move_up(&mut self) {
        let Some(doc) = self.doc.as_ref() else {
            return;
        };
        let row = scan::row_start(doc, self.cursor, MAX_SCAN).0;
        let Some(prev) = scan::prev_row_start(doc, row, MAX_SCAN) else {
            return;
        };
        self.cursor_at_col(prev);
        self.scroll_adjust();
    }

    fn move_down(&mut self) {
        let Some(doc) = self.doc.as_ref() else {
            return;
        };
        let row = scan::row_start(doc, self.cursor, MAX_SCAN).0;
        let prossima = scan::next_row(doc, row, MAX_SCAN);
        let len = doc.len();
        let finisce_nl = len > 0 && doc.read_range(len - 1, 1).first() == Some(&b'\n');
        match prossima {
            Next::Line(start) | Next::Cut(start) => self.cursor_at_col(start),
            Next::End => {
                // Ultima riga: in modifica il cursore puo' scendere sulla riga vuota finale.
                if self.edit && (len == 0 || finisce_nl) {
                    self.cursor = len;
                }
            }
        }
        self.scroll_adjust();
    }

    fn move_left(&mut self) {
        if self.cursor == 0 {
            return;
        }
        let Some(doc) = self.doc.as_ref() else {
            return;
        };
        let da = self.cursor.saturating_sub(4);
        let buf = doc.read_range(da, self.cursor - da);
        self.cursor -= ultimo_carattere(&buf) as u64;
        self.remember_col();
        self.scroll_adjust();
    }

    fn move_right(&mut self) {
        let max = self.max_cursor();
        if self.cursor >= max {
            return;
        }
        let Some(doc) = self.doc.as_ref() else {
            return;
        };
        let restanti = (doc.len() - self.cursor).min(4);
        let buf = doc.read_range(self.cursor, restanti);
        self.cursor = (self.cursor + primo_carattere(&buf) as u64).min(max);
        self.remember_col();
        self.scroll_adjust();
    }

    fn move_home(&mut self) {
        let Some(doc) = self.doc.as_ref() else {
            return;
        };
        let row = scan::row_start(doc, self.cursor, MAX_SCAN).0;
        self.cursor = row.min(self.max_cursor());
        self.remember_col();
        self.scroll_adjust();
    }

    fn move_end(&mut self) {
        let Some(doc) = self.doc.as_ref() else {
            return;
        };
        let row = scan::row_start(doc, self.cursor, MAX_SCAN).0;
        let fine = scan::row_end(doc, row, MAX_SCAN);
        self.cursor = fine.min(self.max_cursor());
        self.remember_col();
        self.scroll_adjust();
    }

    fn move_file_start(&mut self) {
        self.cursor = 0;
        self.top = 0;
        self.hscroll = 0;
        self.remember_col();
    }

    fn move_file_end(&mut self) {
        let len = self.document_len();
        self.cursor = if self.edit || len == 0 {
            len
        } else {
            let Some(doc) = self.doc.as_ref() else {
                return;
            };
            // L'ultima riga e' quella che contiene l'ultimo byte; la sua fine sta prima dell'eventuale a-capo.
            let ultima = scan::row_start(doc, len - 1, MAX_SCAN).0;
            scan::row_end(doc, ultima, MAX_SCAN).min(self.max_cursor())
        };
        self.remember_col();
        self.scroll_adjust();
    }

    /// Pagina in giu': cursore e `top` si spostano dello stesso numero di righe, il cursore resta alla stessa riga
    /// dello schermo; ai bordi del file si ferma.
    fn page_down(&mut self) {
        let page = self.text_rows().saturating_sub(1);
        if page == 0 {
            return;
        }
        let last = self.last_top();
        let sotto_cur = self.rows_after(self.cursor_row(), page);
        let sotto_top: Vec<u64> = self
            .rows_after(self.top, page)
            .into_iter()
            .take_while(|&r| r <= last)
            .collect();
        let n = sotto_cur.len().min(sotto_top.len());
        if n == 0 {
            return;
        }
        self.cursor_at_col(sotto_cur[n - 1]);
        self.top = sotto_top[n - 1];
        self.scroll_adjust();
    }

    /// Pagina in su', come `page_down` all'indietro.
    fn page_up(&mut self) {
        let page = self.text_rows().saturating_sub(1);
        if page == 0 {
            return;
        }
        let sopra_cur = self.rows_before(self.cursor_row(), page);
        let sopra_top = self.rows_before(self.top, page);
        let n = sopra_cur.len().min(sopra_top.len());
        if n == 0 {
            return;
        }
        self.cursor_at_col(sopra_cur[n - 1]);
        self.top = sopra_top[n - 1];
        self.scroll_adjust();
    }

    /// La rotella: tre righe per scatto, mai oltre l'ultima pagina.
    fn wheel(&mut self, down: bool) {
        if self.text_rows() == 0 || self.doc.is_none() {
            return;
        }
        let last = self.last_top();
        if down {
            for _ in 0..WHEEL_ROWS {
                if self.top >= last {
                    break;
                }
                let Some(doc) = self.doc.as_ref() else {
                    break;
                };
                match scan::next_row(doc, self.top, MAX_SCAN) {
                    Next::Line(r) | Next::Cut(r) => self.top = r.min(last),
                    Next::End => break,
                }
            }
        } else {
            for _ in 0..WHEEL_ROWS {
                let Some(doc) = self.doc.as_ref() else {
                    break;
                };
                match scan::prev_row_start(doc, self.top, MAX_SCAN) {
                    Some(r) => self.top = r,
                    None => break,
                }
            }
        }
        self.follow_window();
    }

    /// Dopo uno spostamento di `top`: se il cursore e' uscito dalla finestra va sulla riga visibile piu' vicina,
    /// alla colonna ricordata.
    fn follow_window(&mut self) {
        let text_rows = self.text_rows();
        if text_rows == 0 {
            return;
        }
        let row = self.cursor_row();
        if row < self.top {
            self.cursor_at_col(self.top);
            return;
        }
        let visibili = self.rows_after(self.top, text_rows - 1);
        if row == self.top || visibili.contains(&row) {
            return;
        }
        let ultima = visibili.last().copied().unwrap_or(self.top);
        self.cursor_at_col(ultima);
    }

    /// Il cursore resta visibile: `top` e `hscroll` si spostano il meno possibile (per Ctrl+End la riga del
    /// cursore e' l'ultima dello schermo; in orizzontale la colonna sta in `[hscroll, hscroll + celle - 1]`).
    fn scroll_adjust(&mut self) {
        let text_rows = self.text_rows();
        if text_rows == 0 {
            return;
        }
        let row = self.cursor_row();
        if row < self.top {
            // Sopra la finestra: la riga del cursore diventa la prima visibile.
            self.top = row;
        } else {
            let visibili = self.rows_after(self.top, text_rows - 1);
            if row != self.top && !visibili.contains(&row) {
                // Sotto la finestra: la riga del cursore diventa l'ultima visibile.
                let sopra = self.rows_before(row, text_rows - 1);
                self.top = sopra.last().copied().unwrap_or(row);
            }
        }
        let cells = self.cells();
        if cells == 0 {
            return;
        }
        let col = self.cursor_col();
        if col < self.hscroll {
            self.hscroll = col;
        } else if col > self.hscroll + cells - 1 {
            self.hscroll = col + 1 - cells;
        }
    }

    /// Gli inizi delle righe visuali dopo `da`, al piu' `quante` (la prima e' la riga successiva).
    fn rows_after(&self, da: u64, quante: usize) -> Vec<u64> {
        let mut v = Vec::new();
        let Some(doc) = self.doc.as_ref() else {
            return v;
        };
        let mut p = da;
        while v.len() < quante {
            match scan::next_row(doc, p, MAX_SCAN) {
                Next::Line(r) | Next::Cut(r) => {
                    v.push(r);
                    p = r;
                }
                Next::End => break,
            }
        }
        v
    }

    /// Gli inizi delle righe visuali prima di `da`, al piu' `quante` (la prima e' la riga precedente).
    fn rows_before(&self, da: u64, quante: usize) -> Vec<u64> {
        let mut v = Vec::new();
        let Some(doc) = self.doc.as_ref() else {
            return v;
        };
        let mut p = da;
        while v.len() < quante {
            match scan::prev_row_start(doc, p, MAX_SCAN) {
                Some(r) => {
                    v.push(r);
                    p = r;
                }
                None => break,
            }
        }
        v
    }

    /// Il primo `top` dell'ultima pagina: l'ultima riga del documento in fondo allo schermo.
    fn last_top(&self) -> u64 {
        let Some(doc) = self.doc.as_ref() else {
            return 0;
        };
        let len = doc.len();
        if len == 0 {
            return 0;
        }
        let ultima = scan::row_start(doc, len - 1, MAX_SCAN).0;
        self.rows_before(ultima, self.text_rows().saturating_sub(1))
            .last()
            .copied()
            .unwrap_or(ultima)
    }

    // ----- modo e scrittura -----

    /// F4: alterna lettura e modifica; tornando in lettura il cursore si limita all'ultimo a-capo.
    fn toggle_edit(&mut self) {
        self.edit = !self.edit;
        if self.edit {
            self.flash = Some(self.cat.t("flash.edit_on").to_string());
        } else {
            self.flash = Some(self.cat.t("flash.view_on").to_string());
            let max = self.max_cursor();
            if self.cursor > max {
                self.cursor = max;
                self.scroll_adjust();
            }
        }
    }

    /// In lettura la scrittura e' rifiutata con un suggerimento.
    fn refuse_write(&mut self) {
        if !self.edit {
            self.flash = Some(self.cat.t("flash.read_only").to_string());
        }
    }

    /// Un carattere della tastiera (anche con SHIFT): si inserisce al cursore e il cursore avanza di tutti
    /// i suoi byte UTF-8.
    fn insert_char(&mut self, c: char) {
        let mut buf = [0u8; 4];
        let testo = c.encode_utf8(&mut buf);
        self.insert_at_cursor(testo.as_bytes());
    }

    /// Enter: un a-capo. Si aggiunge al passo di storia corrente e poi lo chiude.
    fn insert_enter(&mut self) {
        if self.insert_at_cursor(b"\n") {
            self.seal_history();
        }
    }

    /// Tab: un tabulatore (le celle le calcola la visualizzazione: qui e' un byte come gli altri).
    fn insert_tab(&mut self) {
        self.insert_at_cursor(b"\t");
    }

    /// Inserisce `testo` al cursore (in modifica) e porta il cursore alla fine di cio' che ha scritto;
    /// dopo ogni modifica la colonna ricordata si ricalcola e il cursore resta visibile. Vero se ha
    /// inserito; un errore del core diventa un messaggio.
    fn insert_at_cursor(&mut self, testo: &[u8]) -> bool {
        if !self.edit {
            self.refuse_write();
            return false;
        }
        let Some(doc) = self.doc.as_mut() else {
            return false;
        };
        match doc.insert(self.cursor, testo) {
            Ok(()) => {
                self.cursor += testo.len() as u64;
                self.remember_col();
                self.scroll_adjust();
                true
            }
            Err(e) => {
                self.flash = Some(self.cat.tf("flash.edit_failed", &[&format!("{:?}", e)]));
                false
            }
        }
    }

    /// Backspace: toglie il carattere prima del cursore, intero (anche multibyte: l'a-capo e' un carattere
    /// come gli altri e unisce le righe). Al cursore 0 non fa nulla.
    fn delete_before(&mut self) {
        if !self.edit {
            self.refuse_write();
            return;
        }
        if self.cursor == 0 {
            return;
        }
        let Some(doc) = self.doc.as_mut() else {
            return;
        };
        // Il carattere precedente sta nei 4 byte prima del cursore (un carattere UTF-8 occupa al piu' 4
        // byte; un byte non valido e' un carattere tutto suo).
        let da = self.cursor.saturating_sub(4);
        let buf = doc.read_range(da, self.cursor - da);
        let n = ultimo_carattere(&buf) as u64;
        if let Err(e) = doc.delete(self.cursor - n, n) {
            self.flash = Some(self.cat.tf("flash.edit_failed", &[&format!("{:?}", e)]));
            return;
        }
        self.cursor -= n;
        self.remember_col();
        self.scroll_adjust();
    }

    /// Delete: toglie il carattere sotto il cursore, intero (anche multibyte). Alla fine del documento non
    /// fa nulla.
    fn delete_after(&mut self) {
        if !self.edit {
            self.refuse_write();
            return;
        }
        let Some(doc) = self.doc.as_mut() else {
            return;
        };
        let len = doc.len();
        if self.cursor >= len {
            return;
        }
        // Il carattere successivo comincia al cursore: ne bastano 4 byte.
        let restanti = (len - self.cursor).min(4);
        let buf = doc.read_range(self.cursor, restanti);
        let n = primo_carattere(&buf) as u64;
        if let Err(e) = doc.delete(self.cursor, n) {
            self.flash = Some(self.cat.tf("flash.edit_failed", &[&format!("{:?}", e)]));
            return;
        }
        self.remember_col();
        self.scroll_adjust();
    }

    /// Ctrl+Z: annulla l'ultimo passo della storia (solo in modifica). Il cursore va a `pos +
    /// max(delta, 0)` con `pos` restituito dal core e delta = lunghezza dopo meno lunghezza prima.
    fn undo(&mut self) {
        if !self.edit {
            self.refuse_write();
            return;
        }
        let Some(doc) = self.doc.as_mut() else {
            return;
        };
        let prima = doc.len();
        match doc.undo() {
            Some(pos) => {
                let avanzo = doc.len().saturating_sub(prima);
                self.cursor = pos + avanzo;
                self.flash = Some(self.cat.t("flash.undo").to_string());
                self.remember_col();
                self.scroll_adjust();
            }
            None => self.flash = Some(self.cat.t("flash.nothing_to_undo").to_string()),
        }
    }

    /// Ctrl+Y / Ctrl+Shift+Z: ripete l'ultimo passo annullato (solo in modifica); il cursore si piazza
    /// come nell'annulla.
    fn redo(&mut self) {
        if !self.edit {
            self.refuse_write();
            return;
        }
        let Some(doc) = self.doc.as_mut() else {
            return;
        };
        let prima = doc.len();
        match doc.redo() {
            Some(pos) => {
                let avanzo = doc.len().saturating_sub(prima);
                self.cursor = pos + avanzo;
                self.flash = Some(self.cat.t("flash.redo").to_string());
                self.remember_col();
                self.scroll_adjust();
            }
            None => self.flash = Some(self.cat.t("flash.nothing_to_redo").to_string()),
        }
    }

    /// Chiude il passo di storia aperto: lo chiamano i tasti e il mouse che non continuano la scrittura
    /// (frecce, Home/End, Delete, Tab, Ctrl+..., F-tasti).
    fn seal_history(&mut self) {
        if let Some(doc) = self.doc.as_mut() {
            doc.seal();
        }
    }

    // ----- salvataggio in background -----

    /// F2 / Ctrl+S (e la 'S' del dialogo di uscita): il salvataggio. Il documento SI MUOVE nel thread,
    /// che lo restituisce comunque con l'esito; finche' gira il dialogo `Saving` tiene tutti i tasti e lo
    /// schermo ridisegna il testo dell'ultimo disegno. Senza modifiche non c'e' niente da salvare.
    /// `quit_after`: dopo un salvataggio riuscito l'app esce.
    fn start_save(&mut self, quit_after: bool) {
        if !self.is_modified() {
            self.flash = Some(self.cat.t("flash.nothing_to_save").to_string());
            return;
        }
        let len = self.document_len();
        let Some(mut doc) = self.doc.take() else {
            return;
        };
        let cancel = Arc::new(AtomicBool::new(false));
        let progress = Arc::new(AtomicU64::new(0));
        let (tx, rx) = mpsc::channel();
        let cancel_thread = Arc::clone(&cancel);
        let progress_thread = Arc::clone(&progress);
        std::thread::spawn(move || {
            let esito = doc.save(&cancel_thread, &progress_thread);
            // Il documento torna comunque: anche con errore non si perde.
            let _ = tx.send((doc, esito));
        });
        self.save_job = Some(SaveJob {
            rx,
            progress,
            cancel,
            len,
            quit_after,
        });
        self.dialog = Some(DialogKind::Saving);
    }

    /// La fine del salvataggio (la chiama `tick`): il documento torna al suo posto con l'esito e il
    /// dialogo si chiude; dopo un salvataggio riuscito l'indice righe si ricostruisce in background (il
    /// file su disco e' cambiato) e, se era "salva ed esci", l'app esce.
    fn tick_save(&mut self) {
        let Some(job) = self.save_job.as_mut() else {
            return;
        };
        match job.rx.try_recv() {
            Ok((doc, esito)) => {
                let quit_after = job.quit_after;
                self.save_job = None;
                // Il dialogo si chiude col lavoro: finche' girava teneva tutti i tasti.
                self.dialog = None;
                self.doc = Some(doc);
                // Il cursore, `top` e `hscroll` restano com'erano (riaggiustati se serve).
                self.scroll_adjust();
                match esito {
                    Ok(()) => {
                        self.flash = Some(self.cat.tf("flash.saved", &[&self.file_name]));
                        self.rebuild_index();
                        // "Salva ed esci": solo un salvataggio riuscito fa uscire.
                        if quit_after {
                            self.quit = true;
                        }
                    }
                    Err(SaveError::Cancelled) => {
                        self.flash = Some(self.cat.t("flash.save_cancelled").to_string());
                    }
                    Err(SaveError::SourceChanged) => {
                        self.flash = Some(self.cat.t("flash.source_changed").to_string());
                    }
                    Err(e) => {
                        self.flash = Some(self.cat.tf("flash.save_failed", &[&e.to_string()]));
                    }
                }
            }
            Err(mpsc::TryRecvError::Empty) => {}
            Err(mpsc::TryRecvError::Disconnected) => {
                // Difensivo: il thread e' terminato senza consegnare il documento, non si aspetta piu'.
                self.save_job = None;
                self.dialog = None;
            }
        }
    }

    /// Fa ripartire la costruzione dell'indice righe in background dopo un salvataggio riuscito: il file
    /// su disco e' cambiato e l'indice del vecchio non vale piu'. Stesso meccanismo dell'apertura.
    fn rebuild_index(&mut self) {
        if !self.index {
            return;
        }
        let Some(doc) = self.doc.as_ref() else {
            return;
        };
        // Un'indicizzazione partita prima del salvataggio si ferma: indicizzerebbe il file vecchio.
        self.cancel.store(true, Ordering::Relaxed);
        self.cancel = Arc::new(AtomicBool::new(false));
        let progress = Arc::new(AtomicU64::new(0));
        self.index_job = Some(spawn_index_job(doc.path(), &self.cancel, &progress));
    }
}

impl Drop for App {
    fn drop(&mut self) {
        // I thread in corso si fermano: non servono piu' a nessuno.
        self.cancel.store(true, Ordering::Relaxed);
        if let Some(job) = &self.save_job {
            job.cancel.store(true, Ordering::Relaxed);
        }
    }
}
