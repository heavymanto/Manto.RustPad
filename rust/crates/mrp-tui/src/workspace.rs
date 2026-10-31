//! Le schede: piu' documenti aperti insieme, ognuno con la sua `App` (il suo indice righe, la sua ricerca, la sua storia
//! di modifiche). Il `Workspace` sceglie la scheda attiva, le manda i tasti e il mouse, apre e chiude le schede e disegna
//! la barra (nel TUI: una riga in cima, solo con piu' di una scheda; la GUI disegna la sua con egui e spegne questa).
//!
//! Tasti (con un dialogo aperto vanno tutti al dialogo): Ctrl+O apre un file in una scheda nuova (o porta avanti quella
//! che lo ha gia'), Ctrl+W chiude la scheda (con modifiche chiede prima), Ctrl+PgSu/PgGiu e Alt+Sinistra/Destra cambiano
//! scheda, Alt+1..9 va alla scheda col numero; F10 e Ctrl+Q chiudono tutto, chiedendo scheda per scheda per quelle con
//! modifiche.

use crate::app::{App, AppConfig};
use crate::theme::{Theme, ThemeId};
use crate::ui;
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
use std::path::Path;
use std::time::Duration;

/// Quanti caratteri al massimo per il nome di una scheda nella barra.
const NOME_MAX: usize = 24;

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
}

impl Workspace {
    /// Un `Workspace` con una sola scheda, `app`; `cfg` serve ad aprire le altre.
    pub fn new(app: App, cfg: AppConfig) -> Workspace {
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

    /// Adatta la scheda attiva allo spazio (meno la barra).
    fn apply_size(&mut self) {
        let (w, h) = self.size;
        let barra = self.bar_height();
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
        self.tabs.push(app);
        self.select(self.tabs.len() - 1);
        Ok(())
    }

    /// Chiede di chiudere la scheda `i`: senza modifiche si chiude subito, con modifiche la porta avanti e apre il
    /// suo dialogo (salva, scarta, annulla).
    pub fn close(&mut self, i: usize) {
        if i >= self.tabs.len() {
            return;
        }
        self.select(i);
        self.tabs[i].request_close();
        self.settle();
    }

    /// Chiude tutto: chiede, una scheda alla volta, per quelle con modifiche; poi `should_quit`.
    pub fn close_all(&mut self) {
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
                // l'ultima scheda: non si toglie (il `Workspace` non resta mai vuoto), si esce
                self.quit = true;
                return;
            }
            self.tabs.remove(i);
            if i < self.active {
                self.active -= 1;
            }
            self.active = self.active.min(self.tabs.len() - 1);
            self.apply_size();
            if self.closing_all {
                self.ask_next();
            }
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

    /// Il tema e la lingua della scheda attiva (quelli che le altre adottano).
    fn look(&self) -> (ThemeId, String) {
        let a = &self.tabs[self.active];
        (a.theme_id(), a.lang_code().to_string())
    }

    /// Un tasto: quelli delle schede si tengono qui (se non c'e' un dialogo aperto), gli altri vanno alla scheda attiva.
    pub fn handle_key(&mut self, k: KeyEvent) {
        if self.tabs[self.active].dialog_kind().is_none() {
            let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
            let alt = k.modifiers.contains(KeyModifiers::ALT);
            match k.code {
                KeyCode::PageDown if ctrl => return self.next(),
                KeyCode::PageUp if ctrl => return self.prev(),
                KeyCode::Right if alt => return self.next(),
                KeyCode::Left if alt => return self.prev(),
                KeyCode::Char(c @ '1'..='9') if alt => {
                    return self.select(usize::from(c as u8 - b'1'));
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
        if self.bar_height() == 1 && m.row == 0 {
            if let MouseEventKind::Down(MouseButton::Left) = m.kind {
                if let Some(i) = self
                    .spans
                    .iter()
                    .position(|&(da, a)| a > da && m.column >= da && m.column < a)
                {
                    self.select(i);
                }
                return;
            }
        } else if self.bar_height() == 1 {
            m.row -= 1;
        }
        self.tabs[self.active].handle_mouse(m);
    }

    /// Un testo incollato dal terminale.
    pub fn paste_text(&mut self, testo: &str) {
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
        if self.bar_height() == 0 || w == 0 || h < 5 {
            self.spans.clear();
            ui::draw(f, &mut self.tabs[self.active]);
            return;
        }
        // la scheda attiva su un terminale di prova alto una riga di meno, poi copiato sotto la barra
        let Ok(mut prova) = Terminal::new(TestBackend::new(w, h - 1));
        let attiva = &mut self.tabs[self.active];
        if prova.draw(|g| ui::draw(g, attiva)).is_err() {
            return;
        }
        let sorgente = prova.backend().buffer();
        let buf = f.buffer_mut();
        for y in 0..h - 1 {
            for x in 0..w {
                if let (Some(da), Some(a)) = (
                    sorgente.cell((x, y)),
                    buf.cell_mut((area.x + x, area.y + y + 1)),
                ) {
                    *a = da.clone();
                }
            }
        }
        if let Some((x, y)) = attiva.cursor_xy() {
            f.set_cursor_position((area.x + x, area.y + y + 1));
            attiva.set_cursor_xy(Some((x, y + 1)));
        }
        self.draw_bar(f, Rect::new(area.x, area.y, w, 1));
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
        if self.bar_height() == 0 {
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
