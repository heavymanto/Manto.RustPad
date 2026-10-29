//! L'app del TUI: una macchina a stati testabile senza terminale (il contratto sta in `docs/TEAM/DESIGN_TUI.md`).
//! Riceve tasti, mouse e dimensioni dello schermo; non disegna (lo fa `ui.rs`); tutta la logica di testo e' nel core.

use crate::theme::ThemeId;
use mrp_core::lang::Lang;
use ratatui::crossterm::event::{KeyEvent, MouseEvent};
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

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

/// Lo stato dell'app.
pub struct App {
    _todo: (),
}

impl App {
    /// Apre `path`. Errore se manca o non e' un file regolare.
    pub fn open(_path: &Path, _cfg: AppConfig) -> io::Result<App> {
        todo!("E8")
    }

    /// Nuove dimensioni dello schermo; il cursore resta visibile.
    pub fn resize(&mut self, _w: u16, _h: u16) {
        todo!("E8")
    }

    /// Un tasto premuto.
    pub fn handle_key(&mut self, _key: KeyEvent) {
        todo!("E8")
    }

    /// Un evento del mouse (per ora solo la rotella).
    pub fn handle_mouse(&mut self, _m: MouseEvent) {
        todo!("E8")
    }

    /// Raccoglie i lavori in background (l'indice righe).
    pub fn tick(&mut self) {
        todo!("E8")
    }

    /// Chiama `tick` finche' non c'e' piu' un lavoro in corso o scade `timeout`. Vero se non c'e' piu' niente in corso.
    pub fn wait_idle(&mut self, _timeout: Duration) -> bool {
        todo!("E8")
    }

    pub fn should_quit(&self) -> bool {
        todo!("E8")
    }

    /// Offset del primo byte della prima riga visuale dello schermo.
    pub fn top(&self) -> u64 {
        todo!("E8")
    }

    /// Offset in byte del cursore.
    pub fn cursor(&self) -> u64 {
        todo!("E8")
    }

    /// Prima colonna visibile (scorrimento orizzontale).
    pub fn hscroll(&self) -> usize {
        todo!("E8")
    }

    pub fn is_edit(&self) -> bool {
        todo!("E8")
    }

    pub fn is_modified(&self) -> bool {
        todo!("E8")
    }

    pub fn document_len(&self) -> u64 {
        todo!("E8")
    }

    /// Posizione del cursore sullo schermo (colonna, riga) dopo l'ultimo `draw`; `None` se un dialogo e' aperto o
    /// non si e' ancora disegnato.
    pub fn cursor_xy(&self) -> Option<(u16, u16)> {
        todo!("E8")
    }

    /// Il dialogo aperto.
    pub fn dialog_kind(&self) -> Option<DialogKind> {
        todo!("E9")
    }

    /// La voce selezionata di un menu (tema, lingua) aperto; `None` se non c'e' un menu.
    pub fn menu_index(&self) -> Option<usize> {
        todo!("E9")
    }

    pub fn theme_id(&self) -> ThemeId {
        todo!("E9")
    }

    /// Codice della lingua attiva.
    pub fn lang_code(&self) -> &str {
        todo!("E9")
    }

    /// Il contenuto del documento (per i test: legge tutto, solo per file piccoli).
    pub fn document_text(&self) -> Vec<u8> {
        todo!("E10")
    }
}
