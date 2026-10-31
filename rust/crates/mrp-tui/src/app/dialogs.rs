//! I dialoghi come dati: `App::dialog_view` descrive il dialogo aperto (testi e valori), e le azioni lo guidano. Il TUI
//! continua a disegnarli a testo (`ui.rs`); la finestra li disegna con widget veri. Sta sotto `app` per leggere i suoi campi
//! privati: le azioni fanno quello che farebbero i tasti (stesse funzioni), cosi' i due modi non possono divergere.

use super::{App, DialogKind};
use crate::theme::ThemeId;
use crate::ui::HELP_KEYS;
use mrp_core::encoding::Encoding;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Una voce di un menu: il nome e un'annotazione (la descrizione di un tema, il marchio AI di una lingua; vuota per le
/// codifiche).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuItem {
    pub label: String,
    pub detail: String,
}

/// Cosa si sceglie nella conferma di uscita.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmChoice {
    /// Salva ed esci.
    Save,
    /// Scarta le modifiche ed esci.
    Discard,
    /// Torna al documento.
    Cancel,
}

/// Un dialogo aperto, con i suoi testi nella lingua attiva.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DialogView {
    /// L'aiuto: una tabella (tasti, descrizione).
    Help {
        title: String,
        rows: Vec<(String, String)>,
    },
    /// Un menu (tema, lingua, codifica): le voci, quella evidenziata e una nota (l'avviso sulle lingue tradotte da una IA).
    Menu {
        title: String,
        items: Vec<MenuItem>,
        selected: usize,
        note: Option<String>,
    },
    /// Un campo di testo (vai a, apri, salva con nome): il titolo, il suggerimento e il testo scritto.
    Prompt {
        title: String,
        hint: String,
        text: String,
    },
    /// Il file scelto per "salva con nome" esiste gia': il titolo e la domanda.
    Overwrite { title: String, text: String },
    /// La conferma di uscita con modifiche non salvate: i testi delle tre scelte (senza la lettera del tasto).
    Confirm {
        title: String,
        text: String,
        save: String,
        discard: String,
        cancel: String,
    },
    /// Il salvataggio in corso: il nome del file e la percentuale.
    Saving {
        title: String,
        text: String,
        percent: u8,
    },
}

/// Divide una riga d'aiuto al primo gruppo di due o piu' spazi: il tasto e la descrizione. Senza, la riga e' tutta tasto.
fn dividi_aiuto(riga: &str) -> (String, String) {
    match riga.find("  ") {
        Some(i) => (
            riga[..i].trim_end().to_string(),
            riga[i..].trim_start().to_string(),
        ),
        None => (riga.trim_end().to_string(), String::new()),
    }
}

/// Toglie la lettera del tasto davanti a una scelta ("S Save and quit" diventa "Save and quit"), se c'e'.
fn senza_lettera(testo: &str) -> String {
    match testo.split_once(' ') {
        Some((a, resto)) if a.chars().count() == 1 => resto.to_string(),
        _ => testo.to_string(),
    }
}

impl App {
    /// Il dialogo aperto ha una vista (`dialog_view` e' `Some`)? Senza la cornice del terminale quei dialoghi non si
    /// disegnano a testo: li disegna la finestra.
    pub fn dialog_has_view(&self) -> bool {
        matches!(
            self.dialog,
            Some(
                DialogKind::Help
                    | DialogKind::Theme
                    | DialogKind::Lang
                    | DialogKind::Encoding
                    | DialogKind::Goto
                    | DialogKind::Open
                    | DialogKind::SaveAs
                    | DialogKind::Overwrite
                    | DialogKind::Confirm
                    | DialogKind::Saving
            )
        )
    }

    /// Il dialogo aperto come dati; `None` senza dialogo o per quelli che non hanno ancora una vista.
    pub fn dialog_view(&self) -> Option<DialogView> {
        let cat = self.catalog();
        let t = |k: &str| cat.t(k).to_string();
        Some(match self.dialog? {
            DialogKind::Help => DialogView::Help {
                title: t("help.title"),
                rows: HELP_KEYS.iter().map(|k| dividi_aiuto(cat.t(k))).collect(),
            },
            DialogKind::Theme => DialogView::Menu {
                title: t("menu.theme.title"),
                items: ThemeId::ALL
                    .iter()
                    .map(|tema| MenuItem {
                        label: t(tema.key()),
                        detail: t(tema.blurb_key()),
                    })
                    .collect(),
                selected: self.menu_index,
                note: None,
            },
            DialogKind::Lang => DialogView::Menu {
                title: t("menu.lang.title"),
                items: self
                    .langs
                    .iter()
                    .map(|l| MenuItem {
                        label: l.name.clone(),
                        detail: if l.ai {
                            t("menu.lang.ai_tag")
                        } else {
                            String::new()
                        },
                    })
                    .collect(),
                selected: self.menu_index,
                note: self
                    .langs
                    .iter()
                    .any(|l| l.ai)
                    .then(|| t("menu.lang.ai_warning")),
            },
            DialogKind::Encoding => DialogView::Menu {
                title: t("menu.encoding.title"),
                items: Encoding::all()
                    .iter()
                    .map(|e| MenuItem {
                        label: e.name().to_string(),
                        detail: String::new(),
                    })
                    .collect(),
                selected: self.menu_index,
                note: None,
            },
            DialogKind::Goto => DialogView::Prompt {
                title: t("prompt.goto.title"),
                hint: t("prompt.goto.hint"),
                text: self.goto_text.clone(),
            },
            DialogKind::Open => DialogView::Prompt {
                title: t("prompt.open.title"),
                hint: t("prompt.open.hint"),
                text: self.open_text.clone(),
            },
            DialogKind::SaveAs => DialogView::Prompt {
                title: t("prompt.saveas.title"),
                hint: t("prompt.saveas.hint"),
                text: self.save_as_text.clone(),
            },
            DialogKind::Overwrite => DialogView::Overwrite {
                title: t("overwrite.title"),
                text: cat.tf("overwrite.text", &[&self.save_as_pending_name()]),
            },
            DialogKind::Confirm => DialogView::Confirm {
                title: t("confirm.title"),
                text: t("confirm.text"),
                save: senza_lettera(cat.t("confirm.save")),
                discard: senza_lettera(cat.t("confirm.discard")),
                cancel: senza_lettera(cat.t("confirm.cancel")),
            },
            DialogKind::Saving => DialogView::Saving {
                title: t("save.title"),
                text: cat.tf("save.text", &[&self.file_name]),
                percent: self.save_progress().unwrap_or(0).min(100) as u8,
            },
            // Cerca e Sostituisci non hanno ancora la vista
            DialogKind::Find | DialogKind::Replace => return None,
        })
    }

    /// Imposta il testo del campo del dialogo aperto (vai a, apri, salva con nome); negli altri non fa niente.
    pub fn dialog_set_text(&mut self, testo: &str) {
        match self.dialog {
            Some(DialogKind::Goto) => self.goto_text = testo.to_string(),
            Some(DialogKind::Open) => self.open_text = testo.to_string(),
            Some(DialogKind::SaveAs) => self.save_as_text = testo.to_string(),
            _ => {}
        }
    }

    /// Evidenzia la voce `i` di un menu (senza applicarla); una voce che non c'e' non fa niente.
    pub fn dialog_select(&mut self, i: usize) {
        if i < self.menu_len() {
            self.menu_index = i;
        }
    }

    /// Sceglie la voce `i` di un menu e la applica (come Invio): tema, lingua o codifica; una voce che non c'e' non fa
    /// niente.
    pub fn dialog_pick(&mut self, i: usize) {
        if i < self.menu_len() {
            self.menu_index = i;
            self.menu_apply();
        }
    }

    /// Conferma il dialogo (come Invio): esegue "vai a", apre il percorso, salva con nome, sostituisce il file, chiude l'aiuto.
    pub fn dialog_submit(&mut self) {
        self.dialog_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    }

    /// Annulla il dialogo (come Esc); durante un salvataggio chiede di annullarlo.
    pub fn dialog_cancel(&mut self) {
        self.dialog_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    }

    /// La scelta nella conferma di uscita (come i tasti S, D, C); negli altri dialoghi non fa niente.
    pub fn dialog_confirm(&mut self, scelta: ConfirmChoice) {
        if self.dialog != Some(DialogKind::Confirm) {
            return;
        }
        let c = match scelta {
            ConfirmChoice::Save => 's',
            ConfirmChoice::Discard => 'd',
            ConfirmChoice::Cancel => 'c',
        };
        self.dialog_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
    }
}
