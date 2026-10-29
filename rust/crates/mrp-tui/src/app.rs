//! L'app del TUI: una macchina a stati testabile senza terminale (il contratto sta in `docs/TEAM/DESIGN_TUI.md`).
//! Riceve tasti, mouse e dimensioni dello schermo; non disegna (lo fa `ui.rs`); tutta la logica di testo e' nel core.

use crate::theme::ThemeId;
use mrp_core::lang::{Catalog, Lang};
use mrp_core::scan::{self, Next};
use mrp_core::view::{self, ViewOptions};
use mrp_core::{Document, FileSource, LineIndex};
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

/// Lo stato dell'app.
pub struct App {
    /// Il documento aperto: `None` quando si esce (il file si rilascia) o, piu' avanti, mentre un salvataggio
    /// lo tiene nel suo thread.
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
    /// Vero quando si e' chiesto di uscire.
    quit: bool,
    /// Dove sta il cursore sullo schermo dopo l'ultimo disegno (lo scrive `ui::draw`).
    cursor_xy: Option<(u16, u16)>,
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
        // La lingua attiva: quella col codice richiesto fra le disponibili, altrimenti l'inglese incorporato.
        let cat = match &cfg.lang_code {
            Some(code) => match cfg.langs.iter().find(|l| &l.code == code) {
                Some(l) => Catalog::with(l.clone()),
                None => Catalog::english(),
            },
            None => Catalog::english(),
        };
        let cancel = Arc::new(AtomicBool::new(false));
        let progress = Arc::new(AtomicU64::new(0));
        let index_job = if cfg.index {
            // Il thread apre la SUA sorgente sullo stesso file: il documento non si condivide mentre indicizza.
            let (tx, rx) = mpsc::channel();
            let path = doc.path().to_path_buf();
            let cancel_thread = Arc::clone(&cancel);
            let progress_thread = Arc::clone(&progress);
            std::thread::spawn(move || {
                let esito = FileSource::open(&path).and_then(|src| {
                    LineIndex::build(&src, &cancel_thread, &progress_thread)
                        .map_err(|_| io::Error::other("indicizzazione annullata"))
                });
                let _ = tx.send(esito);
            });
            Some(IndexJob { rx, progress })
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
            quit: false,
            cursor_xy: None,
        })
    }

    /// Nuove dimensioni dello schermo; il cursore resta visibile.
    pub fn resize(&mut self, w: u16, h: u16) {
        self.w = w;
        self.h = h;
        self.clamp_cursor();
        self.scroll_adjust();
    }

    /// Un tasto premuto.
    pub fn handle_key(&mut self, key: KeyEvent) {
        // Il messaggio transitorio sparisce al primo tasto premuto dopo, prima di eseguirlo.
        self.flash = None;
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let alt = key.modifiers.contains(KeyModifiers::ALT);
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
            KeyCode::F(4) => self.toggle_edit(),
            KeyCode::F(10) => self.quit_now(),
            KeyCode::Char('q') if ctrl => self.quit_now(),
            // Un tasto di scrittura: in lettura e' rifiutato con un suggerimento (in modifica arriva con E10).
            KeyCode::Char(_)
            | KeyCode::Enter
            | KeyCode::Backspace
            | KeyCode::Delete
            | KeyCode::Tab
                if !ctrl && !alt =>
            {
                self.refuse_write()
            }
            _ => {}
        }
    }

    /// Un evento del mouse (per ora solo la rotella).
    pub fn handle_mouse(&mut self, m: MouseEvent) {
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

    /// Raccoglie i lavori in background (l'indice righe).
    pub fn tick(&mut self) {
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

    /// Chiama `tick` finche' non c'e' piu' un lavoro in corso o scade `timeout`. Vero se non c'e' piu' niente in corso.
    pub fn wait_idle(&mut self, timeout: Duration) -> bool {
        let inizio = Instant::now();
        loop {
            self.tick();
            if self.index_job.is_none() {
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
        self.doc.as_ref().is_some_and(|doc| doc.is_modified())
    }

    pub fn document_len(&self) -> u64 {
        self.doc.as_ref().map_or(0, |doc| doc.len())
    }

    /// Posizione del cursore sullo schermo (colonna, riga) dopo l'ultimo `draw`; `None` se un dialogo e' aperto o
    /// non si e' ancora disegnato.
    pub fn cursor_xy(&self) -> Option<(u16, u16)> {
        self.cursor_xy
    }

    /// Il dialogo aperto.
    pub fn dialog_kind(&self) -> Option<DialogKind> {
        None // i dialoghi arrivano con E9
    }

    /// La voce selezionata di un menu (tema, lingua) aperto; `None` se non c'e' un menu.
    pub fn menu_index(&self) -> Option<usize> {
        None // i menu arrivano con E9
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

    /// La percentuale dell'indicizzazione in corso (0..=100); `None` se non c'e' un lavoro in corso.
    pub(crate) fn index_progress(&self) -> Option<u64> {
        let job = self.index_job.as_ref()?;
        let len = self.document_len();
        if len == 0 {
            return Some(0);
        }
        Some(job.progress.load(Ordering::Relaxed).saturating_mul(100) / len)
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
}

impl Drop for App {
    fn drop(&mut self) {
        // Il thread dell'indice si ferma: non serve piu' a nessuno.
        self.cancel.store(true, Ordering::Relaxed);
    }
}
