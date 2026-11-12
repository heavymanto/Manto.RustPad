//! Il check dei tracciati a lunghezza fissa (Alt+H): l'istogramma delle lunghezze delle righe, spuntabile.
//! Una passata in background conta quante righe hanno ogni lunghezza (byte di contenuto, a-capo escluso);
//! il dialogo le mostra ordinate per conteggio — i formati dominanti prima, le anomalie in fondo, dove
//! l'occhio le becca — e chi legge spunta le lunghezze da vedere: tutte = nessun filtro, solo qualcuna =
//! la vista filtrata con quelle righe. `SPAZIO` spunta, `A` inverte (la via piu' corta per "solo le anomale"),
//! `INVIO` applica, `ESC` annulla.

use super::{App, DialogKind, FilterConditions};
use mrp_core::pattern::{LineFilter, Mode};
use mrp_core::Document;
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, AtomicU64};
use std::sync::mpsc::TryRecvError;
use std::sync::Arc;

impl App {
    /// Alt+H: apre il dialogo del check dei tracciati. La passata parte qui se l'istogramma non c'e' ancora;
    /// la selezione parte dal filtro attuale (o tutta spuntata, se non c'e' filtro).
    pub fn open_lengths_dialog(&mut self) {
        // se siamo in una vista filtrata, la selezione parte dalle condizioni della vista
        // (non da tutte spuntate: si vede cosa si era scelto)
        if self.lengths_filter.is_none() {
            if let Some(v) = &self.filter_view {
                if let Some(c) = &v.conditions {
                    if let Some(lens) = &c.lengths {
                        self.lengths_filter = Some(lens.clone());
                    }
                }
            }
        }
        if self.lengths_job.is_none() && self.lengths_hist.is_none() {
            if let Some(snap) = self.doc.as_ref().and_then(Document::snapshot) {
                let snap = snap.with_boundary(self.enc);
                let progress = Arc::new(AtomicU64::new(0));
                let cancel = Arc::new(AtomicBool::new(false));
                self.lengths_job = Some(super::spawn_lengths_job(snap, &cancel, &progress));
            }
        }
        // la selezione del dialogo: il filtro com'e', oppure tutte (l'informazione pura); se l'istogramma deve ancora
        // arrivare lo decide la passata quando lo consegna (`lengths_tick`): finche' conta, la lista non si vede
        self.lengths_checked = match (&self.lengths_filter, &self.lengths_hist) {
            (Some(attuale), _) => Some(attuale.clone()),
            (None, Some(hist)) => Some(hist.iter().map(|(l, _)| *l).collect()),
            (None, None) => None,
        };
        self.menu_index = 0;
        self.open_dialog(DialogKind::Lengths);
    }

    /// Il giro della passata: raccoglie l'istogramma quando il thread lo consegna.
    pub fn lengths_tick(&mut self) {
        let Some(job) = self.lengths_job.as_ref() else {
            return;
        };
        match job.rx.try_recv() {
            Ok(Ok(hist)) => {
                self.lengths_job = None;
                // per conteggio decrescente (i formati dominanti prima), a parita' la piu' corta
                let mut v: Vec<(u64, u64)> = hist.into_iter().collect();
                v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
                self.lengths_hist = Some(Arc::new(v));
                // il dialogo era aperto in attesa dell'istogramma: la selezione parte adesso, tutta spuntata
                if self.dialog == Some(DialogKind::Lengths) && self.lengths_checked.is_none() {
                    let tutte: HashSet<u64> = self
                        .lengths_hist
                        .as_ref()
                        .unwrap()
                        .iter()
                        .map(|(l, _)| *l)
                        .collect();
                    self.lengths_checked = Some(tutte);
                }
            }
            Ok(Err(_)) => {
                // annullata: si riparte alla prossima apertura
                self.lengths_job = None;
            }
            Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Disconnected) => {
                self.lengths_job = None;
            }
        }
    }

    /// Quanto e' lunga la lista del dialogo (le voci dell'istogramma).
    pub(crate) fn lengths_menu_len(&self) -> usize {
        self.lengths_hist.as_ref().map_or(0, |h| h.len())
    }

    /// La selezione del dialogo (vuota se non c'e'): le lunghezze spuntate.
    pub(crate) fn lengths_selection(&self) -> &HashSet<u64> {
        static VUOTA: std::sync::OnceLock<HashSet<u64>> = std::sync::OnceLock::new();
        self.lengths_checked
            .as_ref()
            .unwrap_or_else(|| VUOTA.get_or_init(HashSet::new))
    }
    /// `SPAZIO`: spunta o toglie la voce sotto il cursore.
    pub(crate) fn lengths_toggle(&mut self) {
        let Some(hist) = self.lengths_hist.clone() else {
            return;
        };
        let Some(&(lunghezza, _)) = hist.get(self.menu_index) else {
            return;
        };
        if let Some(sel) = &mut self.lengths_checked {
            if !sel.insert(lunghezza) {
                sel.remove(&lunghezza);
            }
        }
    }

    /// `A`: inverte la selezione (dalla selezione piena diventa "solo quelle che non erano").
    pub(crate) fn lengths_invert(&mut self) {
        let Some(hist) = self.lengths_hist.clone() else {
            return;
        };
        let sel = self.lengths_selection().clone();
        self.lengths_checked = Some(
            hist.iter()
                .map(|(l, _)| *l)
                .filter(|l| !sel.contains(l))
                .collect(),
        );
    }

    /// `INVIO`: applica la selezione. Tutte spuntate = nessun filtro (si torna al file, se la vista era solo
    /// lunghezze); qualcuna spuntata = la vista filtrata con quelle righe, composta con le altre condizioni.
    pub(crate) fn lengths_apply(&mut self) {
        if self.lengths_hist.is_none() {
            // la passata sta ancora contando: niente da applicare, il dialogo resta aperto
            return;
        }
        let sel = self.lengths_selection().clone();
        let tutte = self.lengths_menu_len() > 0 && sel.len() >= self.lengths_menu_len();
        self.lengths_filter = if tutte { None } else { Some(sel) };
        self.dialog = None;
        // la vista: come i livelli della timeline — in una vista filtrata si cambiano le condizioni che
        // l'hanno fatta, fuori si parte dal file
        if let Some(v) = &self.filter_view {
            if let Some(mut c) = v.conditions.clone() {
                c.lengths = self.lengths_filter.clone();
                if c.is_empty() {
                    // nessuna condizione resta: il file intero
                    self.filter_goto();
                } else {
                    self.refilter = Some(c);
                }
                return;
            }
        }
        if self.lengths_filter.is_some() {
            let nome = self.nome_vista_lunghezze();
            let filtro = LineFilter {
                pattern: None,
                invert: false,
                levels: [false; 4],
                lengths: self.lengths_filter.clone(),
            };
            let condizioni = FilterConditions {
                levels: [false; 4],
                text: String::new(),
                mode: Mode::Text,
                case: false,
                invert: false,
                time: self.window.is_some(),
                from: self.range_text[0].clone(),
                to: self.range_text[1].clone(),
                lengths: self.lengths_filter.clone(),
            };
            self.filter_start(filtro, Some((self.lo(), self.hi())), &nome, condizioni);
        }
    }

    /// `ESC`: chiude senza toccare il filtro (la passata, se gira, continua: serve al prossimo giro).
    pub(crate) fn lengths_cancel(&mut self) {
        self.dialog = None;
        self.lengths_checked = None;
    }

    /// Il nome della vista filtrata: "lunghezze: 120, 350 B".
    pub(crate) fn nome_vista_lunghezze(&self) -> String {
        let mut v: Vec<u64> = self.lengths_filter.iter().flatten().copied().collect();
        v.sort_unstable();
        let elenco = v
            .iter()
            .map(|l| format!("{l} B"))
            .collect::<Vec<_>>()
            .join(", ");
        self.cat.tf("lengths.view_name", &[&elenco])
    }
}

impl App {
    /// L'istogramma delle lunghezze, se la passata e' finita: `(lunghezza, conteggio)` per conteggio decrescente.
    pub(crate) fn lengths_hist(&self) -> Option<&[(u64, u64)]> {
        self.lengths_hist.as_ref().map(|v| v.as_slice())
    }

    /// L'istogramma come clone (il Workspace la usa per copiarlo nella vista filtrata).
    pub(crate) fn lengths_hist_clone(&self) -> Option<Vec<(u64, u64)>> {
        self.lengths_hist.as_ref().map(|v| v.as_ref().clone())
    }

    /// Copia l'istogramma da un'altra scheda (il Workspace la chiama quando apre una vista filtrata,
    /// cosi' il dialogo riaperto dalla vista mostra le lunghezze del file originale, non solo quelle filtrate).
    pub fn set_lengths_hist(&mut self, hist: Vec<(u64, u64)>) {
        self.lengths_hist = Some(Arc::new(hist));
    }

    /// La passata e' in corso: `(byte letti, totale)` per la percentuale; `None` se non gira.
    pub(crate) fn lengths_in_corso(&self) -> Option<(u64, u64)> {
        self.lengths_job
            .as_ref()
            .map(|j| (j.progress.load(std::sync::atomic::Ordering::Relaxed), j.len))
    }
}

impl App {
    /// Spunta o toglie la lunghezza `len` (il click di una casella nella finestra).
    pub fn lengths_toggle_value(&mut self, len: u64) {
        if let Some(sel) = &mut self.lengths_checked {
            if !sel.insert(len) {
                sel.remove(&len);
            }
        }
    }

    /// Inverte la selezione (il pulsante della finestra).
    pub fn lengths_invert_pub(&mut self) {
        self.lengths_invert();
    }

    /// Invio: applica (il pulsante OK della finestra).
    pub fn lengths_apply_pub(&mut self) {
        self.lengths_apply();
    }

    /// Esc: chiude (il pulsante Annulla della finestra).
    pub fn lengths_cancel_pub(&mut self) {
        self.lengths_cancel();
    }
}
