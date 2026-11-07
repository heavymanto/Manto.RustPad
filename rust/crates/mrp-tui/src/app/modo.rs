//! I tre modi d'uso dell'editor, a mutua esclusione: VIEW (sola lettura), EDIT (modifica) e TAIL (sola lettura che segue il file
//! mentre cresce, come `tail -f`). Si scelgono da un menu (`Alt+M`, o un clic sul modo nella riga di stato) o con i tasti:
//! `Alt+V` (VIEW), `Alt+E` (EDIT), `Alt+T` (TAIL); i tasti F restano quelli di tutti i giorni (F4 e' Sostituisci). La finestra ha gli stessi tre nella barra
//! degli strumenti.

use super::dialogs::MenuItem;
use super::{App, DialogKind};
use ratatui::crossterm::event::{MouseButton, MouseEvent, MouseEventKind};

/// Le voci del menu dei modi.
pub(super) const MODI_VOCI: usize = 3;

/// Un modo d'uso.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Modo {
    /// Sola lettura.
    View,
    /// Modifica.
    Edit,
    /// Sola lettura che segue la fine del file.
    Tail,
}

impl App {
    /// Il modo di adesso: la modifica vince sul resto (le due cose non stanno insieme: accendere una spegne l'altra).
    pub fn work_mode(&self) -> Modo {
        if self.edit {
            Modo::Edit
        } else if self.follow {
            Modo::Tail
        } else {
            Modo::View
        }
    }

    /// Il nome del modo di adesso come si legge nella riga di stato (`VIEW`, `EDIT`, `TAIL`), con `COL` dopo se c'e' la modalita'
    /// colonna.
    pub fn mode_text(&self) -> String {
        let chiave = match self.work_mode() {
            Modo::Edit => "mode.edit",
            Modo::Tail => "mode.tail",
            Modo::View => "mode.view",
        };
        let mut testo = self.cat.t(chiave).to_string();
        if self.column_mode() {
            testo.push(' ');
            testo.push_str(self.cat.t("mode.column"));
        }
        testo
    }

    /// Alt+V: sola lettura, qualunque sia il modo di adesso (uscendo dalla modifica le modifiche restano da salvare).
    pub(super) fn set_view(&mut self) {
        if self.edit {
            self.toggle_edit();
        } else if self.follow {
            self.follow = false;
            self.flash = Some(self.cat.t("flash.view_on").to_string());
        }
    }

    /// Passa alla modifica se non ci si e' gia' (accendere la modifica spegne il tail).
    pub(super) fn set_edit(&mut self) {
        if !self.edit {
            self.toggle_edit();
        }
    }

    /// Passa al tail se non ci si e' gia'. Dalla modifica senza modifiche da salvare si passa direttamente; con delle modifiche
    /// non salvate si resta dov'e' e si dice di salvarle o scartarle (il tail le butterebbe via: guarda solo il file).
    pub(super) fn set_tail(&mut self) {
        if !self.follow {
            self.toggle_follow();
        }
    }

    /// Alt+M (e il clic sul modo): il menu dei tre modi, con quello di adesso evidenziato.
    pub(super) fn open_mode_menu(&mut self) {
        self.open_dialog(DialogKind::Mode);
    }

    /// La voce del menu che corrisponde al modo di adesso.
    pub(super) fn mode_menu_index(&self) -> usize {
        match self.work_mode() {
            Modo::View => 0,
            Modo::Edit => 1,
            Modo::Tail => 2,
        }
    }

    /// Le voci del menu: il nome del modo e, accanto, cosa vuol dire.
    pub(crate) fn mode_menu_items(&self) -> Vec<MenuItem> {
        [
            ("mode.view", "mode.view_hint"),
            ("mode.edit", "mode.edit_hint"),
            ("mode.tail", "mode.tail_hint"),
        ]
        .iter()
        .map(|(nome, spiega)| MenuItem {
            label: self.text(nome).to_string(),
            detail: self.text(spiega).to_string(),
        })
        .collect()
    }

    /// Sceglie la voce `i` del menu dei modi.
    pub(super) fn mode_menu_choose(&mut self, i: usize) {
        match i {
            0 => self.set_view(),
            1 => self.set_edit(),
            _ => self.set_tail(),
        }
    }

    /// Un clic sul modo nella riga di stato (le ultime colonne della riga, dopo il testo del modo e uno spazio) apre il menu dei
    /// modi. Vero se il clic e' stato di questo menu.
    pub(super) fn mode_status_mouse(&mut self, m: MouseEvent) -> bool {
        if !matches!(m.kind, MouseEventKind::Down(MouseButton::Left))
            || self.dialog.is_some()
            || !self.chrome
        {
            return false;
        }
        let riga = self.top_offset() + u16::try_from(self.text_rows()).unwrap_or(u16::MAX);
        let larghezza = u16::try_from(self.mode_text().chars().count()).unwrap_or(u16::MAX);
        let da = self.w.saturating_sub(larghezza + 1);
        if m.row != riga || m.column < da || m.column >= self.w.saturating_sub(1) {
            return false;
        }
        self.open_mode_menu();
        true
    }
}
