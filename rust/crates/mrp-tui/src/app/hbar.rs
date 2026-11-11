//! La barra orizzontale (BUG 1 dell'utente, 11 nov 2026: le righe lunghe si vedevano solo nella prima parte, e l'unico
//! modo di scorrere era camminarci col cursore). Una riga sotto il testo — sopra la riga di stato — con la pista e il
//! pollice che dicono dov'e' la finestra sulla riga piu' larga; il clic e il trascinamento la muovono (il pollice segue
//! il puntatore per come lo si e' afferrato, un clic sulla pista lo centra), e la finestra grafica ci aggiunge la
//! Maiusc+rotella. C'e' solo quando serve: una riga piu' larga della finestra, o si e' gia' scorsi. Il mondo della barra
//! e' la riga piu' larga dell'ultimo disegno (`Row::width`): per una riga oltre il limite di lettura e' un limite
//! inferiore onesto, che cresce mentre si scorre — il pollice non mente mai su dov'e' la finestra.

use super::App;
use ratatui::crossterm::event::{MouseButton, MouseEvent, MouseEventKind};

/// Quante celle scorre la Maiusc+rotella della finestra per ogni scatto (come le 3 righe della rotella, ma le colonne
/// dei tracciati si coprono prima).
const WHEEL_COLS: usize = 8;

impl App {
    /// La larghezza (celle) della riga piu' larga dell'ultimo disegno: la ricordano i disegni (`ui::draw_text`).
    pub(crate) fn note_hbar(&mut self, max: usize) {
        self.hbar_max = max;
    }

    /// Serve la barra orizzontale: c'e' una riga piu' larga della finestra, o si e' gia' scorsi.
    pub fn hbar_needed(&self) -> bool {
        self.hscroll > 0 || self.hbar_max > self.cells()
    }

    /// Il mondo della barra orizzontale: la riga piu' larga, almeno quanto la finestra attuale (il pollice deve poter
    /// arrivare dov'e' la finestra adesso).
    pub(crate) fn hbar_world(&self) -> usize {
        self.hbar_max.max(self.hscroll + self.cells())
    }

    /// La posizione della finestra orizzontale come frazione del mondo (0..1) e quanto ne copre: per la barra della
    /// finestra, la stessa forma della scrollbar verticale (`barre::pollice`).
    pub fn hscroll_info(&self) -> super::ScrollInfo {
        let max = self.hbar_world();
        if max <= self.cells() {
            return super::ScrollInfo {
                start: 0.0,
                size: 1.0,
            };
        }
        super::ScrollInfo {
            start: self.hscroll as f64 / max as f64,
            size: self.cells() as f64 / max as f64,
        }
    }

    /// Porta la finestra orizzontale alla frazione [0, 1] della pista (la cima del pollice, come la
    /// scrollbar verticale: il trascinamento segue il puntatore; in fondo il pollice si ferma).
    pub fn hscroll_to_fraction(&mut self, f: f64) {
        let max = self.hbar_world();
        if max <= self.cells() {
            return;
        }
        let f = if f.is_nan() { 0.0 } else { f.clamp(0.0, 1.0) };
        self.hscroll = ((f * max as f64).round() as usize).min(max - self.cells());
    }

    /// La cella del testo al bordo sinistro della finestra (il trascinamento della barra del terminale).
    pub fn set_hscroll(&mut self, n: usize) {
        let max = self.hbar_world();
        self.hscroll = n.min(max.saturating_sub(self.cells()));
    }

    /// Maiusc+rotella della finestra: la finestra orizzontale scorre di `passi` scatti da `WHEEL_COLS` celle.
    pub fn hbar_wheel(&mut self, destra: bool, passi: usize) {
        let max = self.hbar_world();
        if max <= self.cells() {
            return;
        }
        let limite = max - self.cells();
        let delta = WHEEL_COLS.saturating_mul(passi).min(limite);
        self.hscroll = if destra {
            (self.hscroll + delta).min(limite)
        } else {
            self.hscroll.saturating_sub(delta)
        };
    }

    /// La geometria della barra del terminale: (prima colonna dello schermo, celle della pista) — sotto il testo,
    /// allineata alle sue celle (dopo il gutter). `None` se non serve o non c'e' spazio.
    pub(crate) fn hbar_geometry(&self) -> Option<(u16, usize)> {
        if !self.chrome || !self.hbar_needed() || self.h < 3 {
            return None;
        }
        let cells = self.cells();
        if cells == 0 {
            return None;
        }
        Some((self.text_x0() + self.gutter_w() as u16, cells))
    }

    /// Il pollice della barra del terminale: (cella della pista dove comincia, celle che copre).
    pub(crate) fn hbar_pollice(&self, cells: usize) -> (usize, usize) {
        let max = self.hbar_world();
        let pollice = (cells * self.cells() / max).clamp(1, cells);
        let inizio = (self.hscroll * cells / max).min(cells - pollice);
        (inizio, pollice)
    }

    // SEGUE

    /// Il clic e il trascinamento della barra orizzontale: il pollice segue il puntatore per come lo si e' afferrato
    /// (un clic sulla pista lo centra su di esso). Vero se l'evento e' della barra: il testo non lo vede.
    pub(crate) fn hbar_mouse(&mut self, m: &MouseEvent) -> bool {
        if self.dialog.is_some() {
            self.hbar_grab = None;
            return false;
        }
        let Some((x0, cells)) = self.hbar_geometry() else {
            self.hbar_grab = None;
            return false;
        };
        let riga_barra = self.h as usize - 3;
        match m.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if m.row as usize != riga_barra {
                    return false;
                }
                let x = usize::from(m.column).saturating_sub(usize::from(x0));
                let (inizio, pollice) = self.hbar_pollice(cells);
                // sul pollice si tiene per dove lo si e' preso; sulla pista il pollice si centra sul puntatore
                self.hbar_grab = Some(if x >= inizio && x < inizio + pollice {
                    x - inizio
                } else {
                    pollice / 2
                });
                self.hbar_drag(x);
                true
            }
            // il trascinamento vale su qualunque riga: conta solo la colonna del puntatore
            MouseEventKind::Drag(MouseButton::Left) => {
                if self.hbar_grab.is_none() {
                    return false;
                }
                self.hbar_drag(usize::from(m.column).saturating_sub(usize::from(x0)));
                true
            }
            MouseEventKind::Up(MouseButton::Left) => self.hbar_grab.take().is_some(),
            _ => false,
        }
    }

    /// La finestra orizzontale quando il pollice e' afferrato e il puntatore sta alla cella `x` della pista.
    fn hbar_drag(&mut self, x: usize) {
        let Some((_, cells)) = self.hbar_geometry() else {
            return;
        };
        let Some(presa) = self.hbar_grab else {
            return;
        };
        let max = self.hbar_world();
        if max <= self.cells() || cells == 0 {
            return;
        }
        // la frazione della pista (come `barre::frazione_da_posizione`, in celle): la cima del pollice,
        // che in fondo si ferma (la finestra non passa oltre la fine della riga)
        let f = ((x.saturating_sub(presa)) as f64 / cells as f64).clamp(0.0, 1.0);
        self.hscroll = ((f * max as f64).round() as usize).min(max - self.cells());
    }
}
