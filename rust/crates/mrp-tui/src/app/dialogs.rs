//! I dialoghi come dati: `App::dialog_view` descrive il dialogo aperto (testi e valori), e le azioni lo guidano. Il TUI
//! continua a disegnarli a testo (`ui.rs`); la finestra li disegna con widget veri. Sta sotto `app` per leggere i suoi campi
//! privati: le azioni fanno quello che farebbero i tasti (stesse funzioni), cosi' i due modi non possono divergere.

use super::{App, DialogKind};
use crate::ui::HELP_KEYS;
use mrp_core::encoding::Encoding;
pub use mrp_core::pattern::Mode;
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

/// I testi delle opzioni di ricerca (Cerca e Sostituisci): il modo e le sue tre scelte, le maiuscole, l'ambito.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchLabels {
    pub mode: String,
    pub text: String,
    pub glob: String,
    pub regex: String,
    pub case: String,
    pub scope_one: String,
    pub scope_all: String,
    /// "Nascondi le righe che corrispondono" (solo nel dialogo del filtro).
    pub invert: String,
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
    /// Le pagine dei tre salti: il titolo, il suggerimento, l'etichetta e il testo dei tre numeri e quale ha il fuoco.
    Jumps {
        title: String,
        hint: String,
        labels: [String; 3],
        values: [String; 3],
        field: u8,
    },
    /// Cerca: il testo, il modo (testo, glob, regex), se le maiuscole contano, i testi delle opzioni e del pulsante.
    Find {
        title: String,
        hint: String,
        text: String,
        mode: Mode,
        case: bool,
        labels: SearchLabels,
        go: String,
        /// `Some` solo nel dialogo del filtro: se mostra le righe che NON corrispondono.
        invert: Option<bool>,
    },
    /// Sostituisci: i due campi (e quale ha il fuoco), le opzioni, l'ambito, il messaggio e se l'anteprima e' pronta (il
    /// pulsante diventa "applica").
    Replace {
        title: String,
        find_label: String,
        with_label: String,
        find: String,
        with: String,
        field: u8,
        mode: Mode,
        case: bool,
        all_tabs: bool,
        status: String,
        ready: bool,
        labels: SearchLabels,
        preview: String,
        apply: String,
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
    /// La codifica con cui scrivere il file di "salva con nome": le codifiche (quella evidenziata) e il BOM (la casella c'e' solo
    /// per le codifiche che lo hanno).
    SaveEncoding {
        title: String,
        items: Vec<MenuItem>,
        selected: usize,
        /// `Some(acceso)` se la codifica evidenziata puo' avere il BOM, `None` altrimenti (la casella non c'e').
        bom: Option<bool>,
        bom_label: String,
        ok: String,
        cancel: String,
    },
    /// Spegnere l'indice righe: il titolo, la spiegazione, le funzioni che non ci saranno piu' e la nota finale.
    IndexOff {
        title: String,
        text: String,
        features: Vec<String>,
        note: String,
    },
    /// L'intervallo di tempo: i due campi (da, a) e quale ha il fuoco, l'errore (vuoto: nessuno) e, se c'e' una finestra di tempo,
    /// il testo del pulsante che la toglie.
    TimeRange {
        title: String,
        hint: String,
        from_label: String,
        to_label: String,
        from: String,
        to: String,
        field: u8,
        error: String,
        clear: Option<String>,
    },
    /// Il formato dell'orario: il pattern e entro quanti caratteri, il fuoco, l'errore, le prime righe del file con l'orario che il
    /// pattern ne legge, e il testo del pulsante che lo fa riconoscere di nuovo.
    TimeMask {
        title: String,
        hint: String,
        pattern_label: String,
        within_label: String,
        pattern: String,
        within: String,
        field: u8,
        error: String,
        preview_label: String,
        preview: Vec<(String, String)>,
        auto: String,
        /// I formati noti (pattern, esempio) da scegliere, quello evidenziato e il titolo della lista.
        presets: Vec<(String, String)>,
        preset: usize,
        presets_label: String,
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
                    | DialogKind::SaveEncoding
                    | DialogKind::Confirm
                    | DialogKind::Saving
                    | DialogKind::Find
                    | DialogKind::Replace
                    | DialogKind::Jumps
                    | DialogKind::TimeRange
                    | DialogKind::TimeMask
                    | DialogKind::IndexOff
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
                items: self
                    .theme_choices()
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
            DialogKind::Jumps => DialogView::Jumps {
                title: t("jump.title"),
                hint: t("jump.hint"),
                labels: [t("jump.label1"), t("jump.label2"), t("jump.label3")],
                values: self.jump_text.clone(),
                field: self.jump_field,
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
            DialogKind::SaveEncoding => {
                let tutte = Encoding::all();
                let scelta = tutte.get(self.menu_index).copied();
                DialogView::SaveEncoding {
                    title: t("saveenc.title"),
                    items: tutte
                        .iter()
                        .map(|e| MenuItem {
                            label: e.name().to_string(),
                            detail: String::new(),
                        })
                        .collect(),
                    selected: self.menu_index,
                    bom: scelta
                        .filter(|e| e.is_utf8() || e.is_utf16() || e.is_utf32())
                        .map(|_| self.save_enc_bom),
                    bom_label: t("saveenc.bom"),
                    ok: t("saveenc.ok"),
                    cancel: t("saveenc.cancel"),
                }
            }
            DialogKind::IndexOff => DialogView::IndexOff {
                title: t("index_off.title"),
                text: t("index_off.text"),
                features: ["f1", "f2", "f3", "f4", "f5"]
                    .iter()
                    .map(|k| t(&format!("index_off.{k}")))
                    .collect(),
                note: t("index_off.note"),
            },
            DialogKind::TimeRange => DialogView::TimeRange {
                title: t("time.range.title"),
                hint: t("time.range.hint"),
                from_label: t("time.range.from"),
                to_label: t("time.range.to"),
                from: self.time_range_text(0).to_string(),
                to: self.time_range_text(1).to_string(),
                field: self.time_range_field(),
                error: self.time_range_error().to_string(),
                clear: self.time_window().map(|_| t("time.range.clear")),
            },
            DialogKind::TimeMask => DialogView::TimeMask {
                title: t("time.mask.title"),
                hint: t("time.mask.hint"),
                pattern_label: t("time.mask.pattern"),
                within_label: t("time.mask.within"),
                pattern: self.time_mask_text(0).to_string(),
                within: self.time_mask_text(1).to_string(),
                field: self.time_mask_field(),
                error: self.time_mask_error().to_string(),
                preview_label: t("time.mask.preview"),
                preview: self.time_mask_preview(),
                auto: t("time.mask.auto"),
                presets: self.time_presets().0,
                preset: self.time_presets().1,
                presets_label: t("time.mask.known"),
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
            DialogKind::Find => DialogView::Find {
                title: t(if self.filter_mode {
                    "prompt.filter.title"
                } else {
                    "prompt.find.title"
                }),
                hint: t(if self.filter_mode {
                    "prompt.filter.hint"
                } else {
                    "prompt.find.hint"
                }),
                text: self.find_text.clone(),
                mode: self.search_mode,
                case: self.search_case,
                labels: self.search_labels(),
                go: t(if self.filter_mode {
                    "gui.filter_go"
                } else {
                    "gui.find_go"
                }),
                invert: self.filter_mode.then_some(self.find_invert),
            },
            DialogKind::Replace => DialogView::Replace {
                title: t("prompt.replace.title"),
                find_label: t("prompt.replace.find").trim().to_string(),
                with_label: t("prompt.replace.with").trim().to_string(),
                find: self.rep_find.clone(),
                with: self.rep_with.clone(),
                field: self.rep_field,
                mode: self.search_mode,
                case: self.search_case,
                all_tabs: self.rep_all,
                status: self.rep_status.clone(),
                ready: self.rep_ready,
                labels: self.search_labels(),
                preview: t("gui.replace_preview"),
                apply: t("gui.replace_apply"),
            },
        })
    }

    /// I testi delle opzioni di ricerca nella lingua attiva.
    fn search_labels(&self) -> SearchLabels {
        let cat = self.catalog();
        let t = |k: &str| cat.t(k).to_string();
        SearchLabels {
            mode: t("prompt.opt.mode"),
            text: t("prompt.opt.text"),
            glob: t("prompt.opt.glob"),
            regex: t("prompt.opt.regex"),
            case: t("prompt.opt.case"),
            scope_one: t("prompt.opt.one"),
            scope_all: t("prompt.opt.all"),
            invert: t("filter.invert"),
        }
    }

    /// Le righe dell'aiuto come (tasti, descrizione) nella lingua attiva: la stessa tabella di F1.
    pub fn help_rows(&self) -> Vec<(String, String)> {
        let cat = self.catalog();
        HELP_KEYS.iter().map(|k| dividi_aiuto(cat.t(k))).collect()
    }

    /// Imposta il testo di uno dei due campi di Sostituisci (0 = cosa cercare, 1 = con cosa sostituire; un altro numero non
    /// fa niente): toglie l'anteprima, come scrivere col tasto.
    pub fn dialog_set_field(&mut self, campo: u8, testo: &str) {
        if matches!(
            self.dialog,
            Some(DialogKind::TimeRange | DialogKind::TimeMask)
        ) {
            self.time_set_field(campo, testo);
            return;
        }
        if self.dialog != Some(DialogKind::Replace) {
            return;
        }
        match campo {
            0 => self.rep_find = testo.to_string(),
            1 => self.rep_with = testo.to_string(),
            _ => return,
        }
        self.rep_reset();
    }

    /// Porta il fuoco di Sostituisci su un campo (0 o 1); un altro numero non fa niente.
    pub fn dialog_focus(&mut self, campo: u8) {
        if matches!(
            self.dialog,
            Some(DialogKind::TimeRange | DialogKind::TimeMask)
        ) {
            self.time_focus(campo);
            return;
        }
        if self.dialog == Some(DialogKind::Replace) && campo < 2 {
            self.rep_field = campo;
        }
    }

    /// Il modo di ricerca (testo, glob, regex) di Cerca o Sostituisci; in Sostituisci toglie l'anteprima.
    pub fn dialog_set_mode(&mut self, modo: Mode) {
        if matches!(self.dialog, Some(DialogKind::Find | DialogKind::Replace)) {
            self.search_mode = modo;
            if self.dialog == Some(DialogKind::Replace) {
                self.rep_reset();
            }
        }
    }

    /// Se le maiuscole contano, in Cerca o Sostituisci (qui toglie l'anteprima).
    pub fn dialog_set_case(&mut self, maiuscole: bool) {
        if matches!(self.dialog, Some(DialogKind::Find | DialogKind::Replace)) {
            self.search_case = maiuscole;
            if self.dialog == Some(DialogKind::Replace) {
                self.rep_reset();
            }
        }
    }

    /// Nel dialogo dei salti: scrive nel campo `i` (0, 1 o 2) il testo `testo` (solo le cifre, al piu' cinque) e gli da' il
    /// fuoco; altrove, o con un altro campo, non fa niente.
    pub fn dialog_set_jump(&mut self, i: u8, testo: &str) {
        if self.dialog == Some(DialogKind::Jumps) && i < 3 {
            self.jump_text[usize::from(i)] =
                testo.chars().filter(char::is_ascii_digit).take(5).collect();
            self.jump_field = i;
        }
    }

    /// Nel dialogo del filtro: mostra le righe che NON corrispondono (come Alt+I); altrove non fa niente.
    pub fn dialog_set_invert(&mut self, invertito: bool) {
        if self.dialog == Some(DialogKind::Find) && self.filter_mode {
            self.find_invert = invertito;
        }
    }

    /// L'ambito di Sostituisci: vero su tutte le schede, falso su questa (toglie l'anteprima).
    pub fn dialog_set_scope_all(&mut self, tutte: bool) {
        if self.dialog == Some(DialogKind::Replace) {
            self.rep_all = tutte;
            self.rep_reset();
        }
    }

    /// Imposta il testo del campo del dialogo aperto (vai a, apri, salva con nome, cerca); negli altri non fa niente.
    pub fn dialog_set_text(&mut self, testo: &str) {
        match self.dialog {
            Some(DialogKind::Find) => self.find_text = testo.to_string(),
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
