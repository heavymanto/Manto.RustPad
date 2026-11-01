//! La finestra: un `App` del TUI mostrato come griglia di celle. Ogni giro: si misura la cella, si adatta l'`App` allo
//! spazio (colonne e righe), si portano all'`App` tastiera, incolla e rotella, si fa un `tick`, si disegna la griglia
//! che l'`App` produce (`mrp_tui::cells::render_grid_workspace`) e si mandano agli appunti di sistema le copie.

use crate::barre::{self, TestiStato, POLLICE_MINIMO};
use crate::dialoghi::{self, StatoDialogo};
use crate::fonts::{self, Scrittura};
use crate::impostazioni::{self, Richiesta, StatoImpostazioni};
use crate::input;
use crate::menu;
use crate::paint;
use crate::strumenti::{glifo, gruppi, salti, tooltip, Pulsante, SaltoAzione, SaltoVoce};
use eframe::egui::{
    self, Align, Context, CursorIcon, Event, FontId, Frame, Label, Layout, Pos2, Rect, Sense, Vec2,
    ViewportCommand,
};
use mrp_tui::app::{App, AppConfig};
use mrp_tui::cells::{render_grid_workspace, BG_PREDEFINITO};
use mrp_tui::commands::Cmd;
use mrp_tui::workspace::Workspace;
use ratatui::crossterm::event::{
    KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Il corpo del testo di partenza, in punti.
const CORPO: f32 = 15.0;
/// Il corpo minimo e massimo.
const CORPO_MIN: f32 = 8.0;
const CORPO_MAX: f32 = 48.0;
/// La griglia minima: sotto non si disegna (l'`App` vuole almeno 4 righe).
const COLONNE_MIN: u16 = 20;
const RIGHE_MIN: u16 = 6;
/// Dopo un tasto il cursore resta acceso per questo tempo, poi lampeggia.
const FISSO: Duration = Duration::from_millis(700);
/// Il mezzo periodo del lampeggio.
const LAMPEGGIO: Duration = Duration::from_millis(530);
/// Ogni quanto si ridisegna anche senza eventi (i lavori in background: indice, ricerca, salvataggio).
const RIDISEGNO: Duration = Duration::from_millis(60);

/// Come si sceglie un file da aprire: riceve il titolo del dialogo, restituisce il percorso o `None` se si annulla.
pub type Selettore = Box<dyn FnMut(&str) -> Option<PathBuf>>;

/// Come si sceglie il nome con cui salvare: riceve il titolo del dialogo e il nome suggerito, restituisce il percorso o
/// `None` se si annulla.
pub type SelettoreSalva = Box<dyn FnMut(&str, &str) -> Option<PathBuf>>;

/// Cosa si e' scelto nella barra delle schede.
enum Scheda {
    Seleziona(usize),
    Chiudi(usize),
}

/// La finestra dell'editor.
pub struct Finestra {
    /// Le schede (la barra la disegna questa finestra, non il `Workspace`).
    ws: Workspace,
    corpo: f32,
    /// Le scritture asiatiche per cui si e' caricato un font.
    scritture: Vec<Scrittura>,
    dimensione: (u16, u16),
    resto_rotella: f32,
    titolo: String,
    ultimo_input: Instant,
    chiusura: bool,
    /// Uso da sviluppatore: salva la schermata in questo file dopo qualche giro ed esce.
    scatto: Option<std::path::PathBuf>,
    giri: u32,
    scatto_chiesto: bool,
    /// Dove sta la griglia nell'ultimo disegno: l'angolo, la cella, colonne e righe (serve a portare il mouse alle celle).
    geometria: Option<(Pos2, Vec2, u16, u16)>,
    /// Il tasto sinistro e' premuto (si sta trascinando).
    premuto: bool,
    /// Come si sceglie un file da aprire: riceve il titolo del dialogo, restituisce il percorso o `None` se si annulla.
    selettore: Selettore,
    /// Come si sceglie il nome con cui salvare (un documento senza nome, o "salva con nome").
    salvataggio: SelettoreSalva,
    /// I rettangoli dei pezzi della barra di stato nell'ultimo disegno, da sinistra a destra, col loro comando.
    pezzi: Vec<(Rect, Option<Cmd>)>,
    /// Il rettangolo della barra di stato nell'ultimo disegno (`None`: l'`App` ha la cornice del terminale).
    rettangolo_stato: Option<Rect>,
    /// Il rettangolo della pista della scrollbar nell'ultimo disegno (`None`: come sopra).
    rettangolo_scroll: Option<Rect>,
    /// I testi dell'ultima barra di stato disegnata (`None`: come sopra).
    testi: Option<TestiStato>,
    /// Dove si e' afferrato il pollice della scrollbar (dalla sua cima), mentre si preme sulla pista.
    presa_scroll: Option<f32>,
    /// I pulsanti della barra degli strumenti nell'ultimo disegno, nell'ordine di disegno (vuoti: l'`App` ha la
    /// cornice del terminale).
    pulsanti: Vec<Pulsante>,
    /// Il rettangolo della barra degli strumenti nell'ultimo disegno (`None`: come sopra).
    rettangolo_strumenti: Option<Rect>,
    /// I pulsanti della barra dei salti nell'ultimo disegno, da sinistra a destra, con la loro voce (vuoti: come
    /// sopra, o la barra spenta).
    pulsanti_salti_v: Vec<(Rect, SaltoVoce)>,
    /// La finestra delle impostazioni (Ctrl+,).
    impostazioni: StatoImpostazioni,
    /// L'ingranaggio in fondo alla barra dei salti nell'ultimo disegno (apre il dialogo dei salti).
    rettangolo_ingranaggio: Option<Rect>,
    /// Il rettangolo della barra dei salti nell'ultimo disegno (`None`: come sopra).
    rettangolo_salti: Option<Rect>,
    /// Lo stato del dialogo disegnato nell'ultimo giro (B4b): i rettangoli per i test e i ricordi fra un giro e l'altro
    /// (si azzera quando non c'e' un dialogo da disegnare).
    dialogo: StatoDialogo,
    /// Dove si e' aperto il menu contestuale della griglia (tasto destro); `None`: chiuso.
    contestuale: Option<Pos2>,
    /// Le voci del menu contestuale nell'ultimo disegno e il rettangolo di tutto il menu (vuote / `NOTHING`: chiuso).
    contestuale_voci: Vec<(Rect, Cmd)>,
    contestuale_rect: Rect,
    /// Il colore di fondo della griglia nell'ultimo disegno (l'ultima cella): riempie anche il resto dell'area che non
    /// basta per una riga o una colonna intera.
    fondo: [u8; 3],
}

impl Finestra {
    /// La finestra per `app`; `nuova_config` e' la configurazione con cui si aprono gli altri file (schede nuove).
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        app: App,
        nuova_config: Box<dyn Fn() -> AppConfig>,
    ) -> Finestra {
        let scritture = Finestra::scritture_iniziali(&app);
        fonts::installa(&cc.egui_ctx, &scritture);
        // niente animazioni: un editor di testo non le vuole
        cc.egui_ctx.style_mut(|s| s.animation_time = 0.0);
        Finestra::da_app(app, nuova_config)
    }

    /// La finestra per un `Workspace` gia' pronto (le schede di una sessione, o piu' file); fa anche quello che fa `new`
    /// per il contesto di egui.
    pub fn new_ws(cc: &eframe::CreationContext<'_>, ws: Workspace) -> Finestra {
        let finestra = Finestra::da_workspace(ws);
        fonts::installa(&cc.egui_ctx, &finestra.scritture);
        cc.egui_ctx.style_mut(|s| s.animation_time = 0.0);
        finestra
    }

    /// Le scritture di partenza: quella della codifica del file, se e' asiatica.
    fn scritture_iniziali(app: &App) -> Vec<Scrittura> {
        match fonts::scrittura_di(app.encoding()) {
            Scrittura::Nessuna => Vec::new(),
            s => vec![s],
        }
    }

    /// La finestra senza toccare il contesto (i font si installano a parte): per i test senza finestra vera.
    pub fn da_app(app: App, nuova_config: Box<dyn Fn() -> AppConfig>) -> Finestra {
        Finestra::da_workspace(Workspace::new(app, nuova_config()))
    }

    /// La finestra per `ws`, senza toccare il contesto (i font si installano a parte).
    pub fn da_workspace(ws: Workspace) -> Finestra {
        let scritture = Finestra::scritture_iniziali(ws.active());
        // il corpo scelto la volta scorsa, se c'e'
        let corpo = ws
            .font_size()
            .map_or(CORPO, |n| f32::from(n).clamp(CORPO_MIN, CORPO_MAX));
        Finestra {
            ws: ws.senza_barra(),
            corpo,
            impostazioni: StatoImpostazioni::default(),
            scritture,
            dimensione: (0, 0),
            resto_rotella: 0.0,
            titolo: String::new(),
            ultimo_input: Instant::now(),
            chiusura: false,
            scatto: None,
            giri: 0,
            scatto_chiesto: false,
            geometria: None,
            premuto: false,
            selettore: Box::new(|_| None),
            salvataggio: Box::new(|_, _| None),
            pezzi: Vec::new(),
            rettangolo_stato: None,
            rettangolo_scroll: None,
            testi: None,
            presa_scroll: None,
            pulsanti: Vec::new(),
            rettangolo_strumenti: None,
            pulsanti_salti_v: Vec::new(),
            rettangolo_salti: None,
            rettangolo_ingranaggio: None,
            dialogo: StatoDialogo::default(),
            contestuale: None,
            contestuale_voci: Vec::new(),
            contestuale_rect: Rect::NOTHING,
            fondo: BG_PREDEFINITO,
        }
    }

    /// Imposta come si sceglie un file da aprire (il dialogo nativo, o un finto nei test).
    pub fn con_selettore(mut self, selettore: Selettore) -> Finestra {
        self.selettore = selettore;
        self
    }

    /// Imposta come si sceglie il nome con cui salvare (il dialogo nativo, o un finto nei test).
    pub fn con_salvataggio(mut self, selettore: SelettoreSalva) -> Finestra {
        self.salvataggio = selettore;
        self
    }

    /// Se l'utente non ha scelto un tema (`Some(scuro)`: com'e' il sistema), la finestra parte con Scuro o Chiaro secondo il
    /// sistema; con `None` (il sistema non lo dice) resta il tema com'e'. Vale per tutte le schede, senza salvare niente.
    pub fn con_tema_di_sistema(mut self, scuro: Option<bool>) -> Finestra {
        if let Some(scuro) = scuro {
            let tema = if scuro {
                mrp_tui::theme::ThemeId::Dark
            } else {
                mrp_tui::theme::ThemeId::Light
            };
            self.ws.set_theme(tema);
        }
        self
    }

    /// Fa salvare la schermata in `percorso` (PNG) dopo qualche giro e poi chiude la finestra: serve a guardare il
    /// disegno senza una persona davanti.
    pub fn con_scatto(mut self, percorso: std::path::PathBuf) -> Finestra {
        self.scatto = Some(percorso);
        self
    }

    /// Dove sta la griglia nell'ultimo disegno: (angolo, cella, colonne, righe), per i test e per portare il mouse alle celle.
    pub fn geometria(&self) -> Option<(Pos2, Vec2, u16, u16)> {
        self.geometria
    }

    /// Le voci del menu contestuale della griglia nell'ultimo disegno, dall'alto in basso, col loro comando (per i test);
    /// vuote se e' chiuso.
    pub fn contestuale_voci(&self) -> &[(Rect, Cmd)] {
        &self.contestuale_voci
    }

    /// Il colore di fondo della griglia nell'ultimo disegno (per i test).
    pub fn fondo(&self) -> [u8; 3] {
        self.fondo
    }

    /// L'`App` della scheda attiva (per i test).
    pub fn app(&mut self) -> &mut App {
        self.ws.active_mut()
    }

    /// Le schede (per i test e per aprire altri file all'avvio).
    pub fn workspace(&mut self) -> &mut Workspace {
        &mut self.ws
    }

    /// Porta all'`App` gli eventi di egui di questo giro. Restituisce `true` se ne ha portato qualcuno.
    fn eventi(&mut self, ctx: &Context, altezza_cella: f32) -> bool {
        let (eventi, zoom) = ctx.input(|i| (i.events.clone(), i.zoom_delta()));
        let mut tocco = false;
        if (zoom - 1.0).abs() > f32::EPSILON {
            self.corpo = (self.corpo * zoom).clamp(CORPO_MIN, CORPO_MAX);
            self.ws.set_font_size(self.corpo.round() as u16);
        }
        // un dialogo con vista aperto (senza la cornice) tiene per se' tastiera, testo, incolla, copie e clic: li
        // prendono i widget della sua finestra (il tasto che l'ha aperto e' gia' arrivato all'App)
        let dialogo = !self.ws.active().chrome() && self.ws.active().dialog_has_view();
        for e in eventi {
            match e {
                Event::Text(t) => {
                    if dialogo {
                        continue;
                    }
                    for k in input::testo(&t) {
                        self.ws.handle_key(k);
                        tocco = true;
                    }
                }
                Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } => {
                    if dialogo {
                        continue;
                    }
                    // Esc chiude il menu contestuale e non arriva al documento
                    if key == egui::Key::Escape && self.contestuale.is_some() {
                        self.contestuale = None;
                        continue;
                    }
                    // F2: salvare un documento senza nome chiede il nome col dialogo di sistema
                    if key == egui::Key::F2 && !modifiers.any() && self.salva_chiede_il_nome() {
                        self.salva_con_nome();
                        tocco = true;
                        continue;
                    }
                    // Ctrl+'+' / Ctrl+'-' / Ctrl+0: il corpo del testo; Ctrl+O: apre un file
                    if modifiers.command || modifiers.ctrl {
                        match key {
                            // Ctrl+Maiusc+S: salva con nome; Ctrl+S su un documento senza nome: lo stesso
                            egui::Key::S if modifiers.shift || self.salva_chiede_il_nome() => {
                                self.salva_con_nome();
                                tocco = true;
                                continue;
                            }
                            egui::Key::Comma => {
                                self.impostazioni.aperta = !self.impostazioni.aperta;
                                continue;
                            }
                            egui::Key::Plus | egui::Key::Equals => {
                                self.zoom(Cmd::ZoomIn);
                                continue;
                            }
                            egui::Key::Minus => {
                                self.zoom(Cmd::ZoomOut);
                                continue;
                            }
                            egui::Key::Num0 => {
                                self.zoom(Cmd::ZoomReset);
                                continue;
                            }
                            egui::Key::O => {
                                self.apri();
                                tocco = true;
                                continue;
                            }
                            // Ctrl+Tab / Ctrl+Maiusc+Tab: la scheda dopo / prima
                            egui::Key::Tab => {
                                if modifiers.shift {
                                    self.ws.prev();
                                } else {
                                    self.ws.next();
                                }
                                tocco = true;
                                continue;
                            }
                            _ => {}
                        }
                    }
                    if let Some(k) = input::tasto(key, modifiers) {
                        self.ws.handle_key(k);
                        tocco = true;
                    }
                }
                Event::Screenshot { image, .. } => {
                    if let Some(percorso) = &self.scatto {
                        if let Err(e) = salva_png(percorso, &image) {
                            eprintln!("{e}");
                        }
                        self.chiusura = true;
                        ctx.send_viewport_cmd(ViewportCommand::Close);
                    }
                }
                Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Secondary,
                    pressed: true,
                    ..
                } => {
                    // il tasto destro sul testo apre il menu contestuale (non sotto un dialogo, un menu o una barra)
                    let sul_testo = ctx
                        .layer_id_at(pos)
                        .is_none_or(|l| l.order == egui::Order::Background);
                    let sulla_griglia = self.geometria.is_some_and(|(o, c, colonne, righe)| {
                        input::cella_sotto(pos, o, c, colonne, righe).is_some()
                    });
                    if !dialogo && sul_testo && sulla_griglia {
                        self.contestuale = Some(pos);
                    }
                }
                Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers,
                } => {
                    // un clic fuori dal menu contestuale lo chiude (e prosegue sulla griglia)
                    if pressed && self.contestuale.is_some() && !self.contestuale_rect.contains(pos)
                    {
                        self.contestuale = None;
                    }
                    // un dialogo aperto (senza cornice) tiene i clic per la sua finestra: alla griglia non arrivano (il
                    // rilascio di un trascinamento cominciato prima passa, come per i menu aperti)
                    if dialogo && pressed {
                        continue;
                    }
                    if let Some((o, c, colonne, righe)) = self.geometria {
                        if pressed {
                            // un menu aperto sta sopra la griglia: il clic e' suo
                            let sul_testo = ctx
                                .layer_id_at(pos)
                                .is_none_or(|l| l.order == egui::Order::Background);
                            if !sul_testo {
                                continue;
                            }
                            if let Some((x, y)) = input::cella_sotto(pos, o, c, colonne, righe) {
                                self.premuto = true;
                                self.ws.handle_mouse(MouseEvent {
                                    kind: MouseEventKind::Down(MouseButton::Left),
                                    column: x,
                                    row: y,
                                    modifiers: input::modificatori(modifiers),
                                });
                                tocco = true;
                            }
                        } else if self.premuto {
                            self.premuto = false;
                            let (x, y) = input::cella_vicina(pos, o, c, colonne, righe);
                            self.ws.handle_mouse(MouseEvent {
                                kind: MouseEventKind::Up(MouseButton::Left),
                                column: x,
                                row: y,
                                modifiers: KeyModifiers::NONE,
                            });
                        }
                    }
                }
                Event::PointerMoved(pos) => {
                    if let (true, Some((o, c, colonne, righe))) = (self.premuto, self.geometria) {
                        let (x, y) = input::cella_vicina(pos, o, c, colonne, righe);
                        self.ws.handle_mouse(MouseEvent {
                            kind: MouseEventKind::Drag(MouseButton::Left),
                            column: x,
                            row: y + u16::from(pos.y >= o.y + c.y * f32::from(righe)),
                            modifiers: KeyModifiers::NONE,
                        });
                        tocco = true;
                    }
                }
                Event::Copy => {
                    if dialogo {
                        continue;
                    }
                    self.ws.handle_key(ctrl('c'));
                    tocco = true;
                }
                Event::Cut => {
                    if dialogo {
                        continue;
                    }
                    self.ws.handle_key(ctrl('x'));
                    tocco = true;
                }
                Event::Paste(s) => {
                    if dialogo {
                        continue;
                    }
                    if s.is_empty() {
                        // gli appunti di sistema sono vuoti o illeggibili: restano quelli interni dell'app
                        self.ws.handle_key(ctrl('v'));
                    } else {
                        self.ws.paste_text(&s);
                    }
                    tocco = true;
                }
                Event::MouseWheel {
                    unit,
                    delta,
                    modifiers,
                } => {
                    if modifiers.command || modifiers.ctrl {
                        continue;
                    }
                    let (su, giu) = match unit {
                        // una tacca = una rotella dell'app
                        egui::MouseWheelUnit::Line | egui::MouseWheelUnit::Page => {
                            let n = delta.y.abs().ceil() as u32;
                            if delta.y > 0.0 {
                                (n, 0)
                            } else {
                                (0, n)
                            }
                        }
                        egui::MouseWheelUnit::Point => {
                            let (su, giu, resto) =
                                input::rotella(self.resto_rotella, delta.y, altezza_cella);
                            self.resto_rotella = resto;
                            (su, giu)
                        }
                    };
                    for _ in 0..su {
                        self.rotella(MouseEventKind::ScrollUp);
                    }
                    for _ in 0..giu {
                        self.rotella(MouseEventKind::ScrollDown);
                    }
                    tocco = tocco || su + giu > 0;
                }
                _ => {}
            }
        }
        tocco
    }

    fn rotella(&mut self, kind: MouseEventKind) {
        self.ws.handle_mouse(MouseEvent {
            kind,
            column: 0,
            row: 0,
            modifiers: KeyModifiers::NONE,
        });
    }

    /// Apre i file trascinati sulla finestra, ognuno in una scheda.
    fn file_trascinato(&mut self, ctx: &Context) {
        let file: Vec<_> = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .filter_map(|f| f.path.clone())
                .collect()
        });
        for percorso in file {
            self.apri_file(&percorso);
        }
    }

    /// Apre `percorso` in una scheda nuova (o porta avanti quella che lo ha gia'); se non si apre lo dice nella riga
    /// di stato e le schede restano come sono.
    fn apri_file(&mut self, percorso: &Path) {
        if let Err(e) = self.ws.open(percorso) {
            let a = self.ws.active_mut();
            let testo = a.text_with(
                "cli.err.open",
                &[&percorso.display().to_string(), &e.to_string()],
            );
            a.set_flash(testo);
        }
        self.dimensione = (0, 0);
    }

    /// Vero se il salvataggio della scheda attiva passa dal dialogo di sistema: un documento senza nome con modifiche non ha
    /// un file su cui salvare (un file vero si salva e basta).
    fn salva_chiede_il_nome(&self) -> bool {
        let a = self.ws.active();
        a.is_untitled() && a.is_modified()
    }

    /// Fa scegliere il nome (Ctrl+Maiusc+S, menu File, o il salvataggio di un documento senza nome) e salva li'.
    fn salva_con_nome(&mut self) {
        let a = self.ws.active();
        if a.dialog_kind().is_some() {
            return;
        }
        let titolo = a.text("gui.save_as_title").to_string();
        let suggerito = if a.is_untitled() {
            format!("{}.txt", a.file_name())
        } else {
            a.file_name().to_string()
        };
        if let Some(percorso) = (self.salvataggio)(&titolo, &suggerito) {
            self.ws.active_mut().save_as_path(percorso);
        }
    }

    /// Fa scegliere un file (Ctrl+O, menu File) e lo apre in una scheda.
    fn apri(&mut self) {
        let titolo = self.ws.active().text("gui.open_title").to_string();
        if let Some(percorso) = (self.selettore)(&titolo) {
            self.apri_file(&percorso);
        }
    }

    /// La barra delle schede (solo con piu' di una): un'etichetta per scheda con il segno delle modifiche e una x per
    /// chiuderla; il clic centrale chiude. Restituisce cosa si e' scelto in questo giro.
    fn schede(&mut self, ctx: &Context) -> Option<Scheda> {
        if self.ws.len() < 2 {
            return None;
        }
        let nomi = self.ws.titles();
        let attiva = self.ws.active_index();
        let mut scelta = None;
        egui::TopBottomPanel::top("schede").show(ctx, |ui| {
            egui::ScrollArea::horizontal().show(ui, |ui| {
                ui.horizontal(|ui| {
                    for (i, (nome, modificata)) in nomi.iter().enumerate() {
                        let etichetta = if *modificata {
                            format!("{nome} *")
                        } else {
                            nome.clone()
                        };
                        let percorso = self.ws.tabs()[i].path().display().to_string();
                        let r = ui
                            .selectable_label(i == attiva, etichetta)
                            .on_hover_text(percorso);
                        if r.clicked() {
                            scelta = Some(Scheda::Seleziona(i));
                        }
                        if r.middle_clicked() {
                            scelta = Some(Scheda::Chiudi(i));
                        }
                        if ui.add(egui::Button::new("×").frame(false)).clicked() {
                            scelta = Some(Scheda::Chiudi(i));
                        }
                        ui.separator();
                    }
                });
            });
        });
        scelta
    }

    /// La barra degli strumenti (B3): sotto il menu, i comandi del registro a pulsanti d'icona, in gruppi separati da
    /// un separatore. Un pulsante e' spento quando il comando non ha senso; i toggle (modifica, modalita' colonna,
    /// barra dei salti) restano premuti mentre sono accesi; sopra un pulsante si legge il nome e la scorciatoia.
    fn strumenti(&mut self, ctx: &Context) {
        let mut pulsanti = Vec::new();
        let mut scelta = None;
        let pannello = egui::TopBottomPanel::top("strumenti").show(ctx, |ui| {
            ui.horizontal(|ui| {
                for (n, gruppo) in gruppi().into_iter().enumerate() {
                    if n > 0 {
                        ui.separator();
                    }
                    for cmd in gruppo {
                        let info = cmd.info();
                        let attivo = mrp_tui::commands::enabled(&self.ws, cmd);
                        let acceso = match cmd {
                            Cmd::EditMode => self.ws.active().is_edit(),
                            Cmd::ColumnMode => self.ws.active().column_mode(),
                            Cmd::NavBar => self.ws.nav_bar(),
                            _ => false,
                        };
                        let b = egui::Button::new(egui::RichText::new(glifo(info.icon)).size(16.0))
                            .selected(acceso);
                        let r = ui
                            .add_enabled(attivo, b)
                            .on_hover_text(tooltip(cmd, self.ws.active().catalog()));
                        if r.clicked() {
                            scelta = Some(cmd);
                        }
                        pulsanti.push(Pulsante {
                            rect: r.rect,
                            cmd,
                            attivo,
                            acceso,
                        });
                    }
                }
            });
        });
        self.pulsanti = pulsanti;
        self.rettangolo_strumenti = Some(pannello.response.rect);
        if let Some(cmd) = scelta {
            self.azione(cmd);
        }
    }

    /// La barra dei salti (B3): senza la cornice, se Ctrl+B l'ha accesa, una striscia di pulsanti al posto della barra
    /// a celle del terminale (che questa finestra spegne): inizio e fine del file e i salti di pagina delle
    /// impostazioni, prima indietro poi avanti.
    fn salti(&mut self, ctx: &Context) {
        let voci = salti(self.ws.jump_pages());
        let mut rettangoli = Vec::new();
        let mut scelta = None;
        let ingranaggio = std::cell::Cell::new(None);
        let mut configura = false;
        let tip = self.ws.active().text("gui.jump_config").to_string();
        let pannello = egui::TopBottomPanel::top("salti").show(ctx, |ui| {
            ui.horizontal(|ui| {
                for voce in &voci {
                    let r = ui.add(egui::Button::new(voce.etichetta.as_str()));
                    if r.clicked() {
                        scelta = Some(voce.azione);
                    }
                    rettangoli.push((r.rect, voce.clone()));
                }
                // l'ingranaggio, a destra: apre il dialogo con le pagine dei tre salti
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let r = ui
                        .add(egui::Button::new(glifo("jump_config")))
                        .on_hover_text(&tip);
                    ingranaggio.set(Some(r.rect));
                    if r.clicked() {
                        configura = true;
                    }
                });
            });
        });
        if configura {
            self.ws.active_mut().open_jump_dialog();
            self.ultimo_input = Instant::now();
        }
        self.pulsanti_salti_v = rettangoli;
        self.rettangolo_ingranaggio = ingranaggio.get();
        self.rettangolo_salti = Some(pannello.response.rect);
        match scelta {
            Some(SaltoAzione::Inizio) => self.ws.go_start(),
            Some(SaltoAzione::Fine) => self.ws.go_end(),
            Some(SaltoAzione::Pagine(avanti, pagine)) => self.ws.jump(avanti, pagine),
            None => {}
        }
        if scelta.is_some() {
            self.ultimo_input = Instant::now();
        }
    }

    /// La barra di stato (B2b): in basso, larga quanto la finestra. A sinistra la posizione del cursore o il
    /// messaggio transitorio; a destra i pezzi, da sinistra a destra (l'ultimo e' il piu' a destra): l'eventuale
    /// lavoro in corso, il modo, la codifica, l'a-capo e la dimensione. I pezzi col comando sono cliccabili.
    fn barra_stato(&mut self, ctx: &Context) {
        let stato = self.ws.active_mut().status();
        let t = barre::testi_stato(&stato, self.ws.active().catalog());
        let mut scelta = None;
        let mut rettangoli = Vec::new();
        let pannello = egui::TopBottomPanel::bottom("stato").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(&t.sinistra);
                // i pezzi di destra si aggiungono da destra a sinistra, attaccati al bordo (l'ultimo di
                // `t.destra` e' il piu' a destra), con un piccolo spazio in mezzo
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    for (i, pezzo) in t.destra.iter().enumerate().rev() {
                        if i + 1 < t.destra.len() {
                            ui.add_space(6.0);
                        }
                        let r = match pezzo.comando {
                            Some(cmd) => {
                                let l = Label::new(&pezzo.testo).sense(Sense::click());
                                let r = ui.add(l).on_hover_cursor(CursorIcon::PointingHand);
                                if r.clicked() {
                                    scelta = Some(cmd);
                                }
                                r
                            }
                            None => ui.label(&pezzo.testo),
                        };
                        rettangoli.push((r.rect, pezzo.comando));
                    }
                });
            });
        });
        // i rettangoli si sono raccolti da destra a sinistra: valgono da sinistra a destra, come `t.destra`
        rettangoli.reverse();
        self.pezzi = rettangoli;
        self.rettangolo_stato = Some(pannello.response.rect);
        self.testi = Some(t);
        if let Some(cmd) = scelta {
            self.azione(cmd);
        }
    }

    /// La scrollbar (B2b): a destra, fra il menu e la barra di stato, la pista col pollice che dice dov'e' la
    /// finestra nel documento. Premere e trascinare muove la finestra: il pollice segue il puntatore per come lo
    /// si e' afferrato; un clic sulla pista lo porta li', centrato.
    fn scrollbar(&mut self, ctx: &Context) {
        self.rettangolo_scroll = Some(
            egui::SidePanel::right("scorrimento")
                .exact_width(14.0)
                .frame(Frame::NONE)
                .show(ctx, |ui| {
                    let pista = ui.max_rect();
                    let (cima, alto) = barre::pollice(
                        self.ws.active().scroll_info(),
                        pista.height(),
                        POLLICE_MINIMO,
                    );
                    let pollice = Rect::from_min_size(
                        Pos2::new(pista.min.x, pista.min.y + cima),
                        Vec2::new(pista.width(), alto),
                    );
                    // la pista e' un colore tenue del tema; il pollice e' piu' marcato, e ancora di piu' al
                    // passaggio o mentre lo si trascina
                    let tenue = ui.style().visuals.faint_bg_color;
                    let marcato = ui.style().visuals.weak_text_color();
                    let vivo = ui.style().visuals.strong_text_color();
                    ui.painter().rect_filled(pista, 0.0, tenue);
                    let risposta =
                        ui.interact(pista, ui.id().with("pista"), Sense::click_and_drag());
                    let colore = if risposta.hovered() || risposta.dragged() {
                        vivo
                    } else {
                        marcato
                    };
                    ui.painter().rect_filled(pollice, 4.0, colore);
                    // dove si e' afferrato il pollice si decide quando si preme (il clic e' un trascinamento
                    // che non si muove) e resta per tutto il trascinamento; fuori dal pollice, il pollice si
                    // centra sul puntatore
                    if risposta.is_pointer_button_down_on() && self.presa_scroll.is_none() {
                        self.presa_scroll = Some(match risposta.interact_pointer_pos() {
                            Some(p) if pollice.contains(p) => p.y - (pista.min.y + cima),
                            _ => alto / 2.0,
                        });
                    }
                    if risposta.is_pointer_button_down_on() || risposta.clicked() {
                        if let (Some(p), Some(presa)) =
                            (risposta.interact_pointer_pos(), self.presa_scroll)
                        {
                            let f = barre::frazione_da_posizione(
                                p.y - pista.min.y,
                                presa,
                                pista.height(),
                            );
                            self.ws.active_mut().scroll_to_fraction(f);
                        }
                    }
                    if !risposta.is_pointer_button_down_on() {
                        self.presa_scroll = None;
                    }
                    pista
                })
                .inner,
        );
    }

    /// Cambia il corpo del testo: `Cmd::ZoomIn`, `ZoomOut` o `ZoomReset`.
    fn zoom(&mut self, a: Cmd) {
        self.corpo = match a {
            Cmd::ZoomIn => (self.corpo + 1.0).min(CORPO_MAX),
            Cmd::ZoomOut => (self.corpo - 1.0).max(CORPO_MIN),
            _ => CORPO,
        };
        // il corpo scelto si ricorda per la prossima volta
        self.ws.set_font_size(self.corpo.round() as u16);
    }

    /// La finestra delle impostazioni e' aperta (per i test).
    pub fn impostazioni_aperta(&self) -> bool {
        self.impostazioni.aperta
    }

    /// Il rettangolo della finestra delle impostazioni nell'ultimo disegno (per i test); `None` se e' chiusa.
    pub fn impostazioni_rect(&self) -> Option<Rect> {
        self.impostazioni.rect
    }

    /// I controlli della finestra delle impostazioni nell'ultimo disegno, col loro testo (per i test).
    pub fn impostazioni_controlli(&self) -> &[(Rect, String)] {
        &self.impostazioni.controlli
    }

    /// Le righe (nome, scorciatoia) della scheda dei tasti nell'ultimo disegno (per i test).
    pub fn impostazioni_tasti(&self) -> &[(String, String)] {
        &self.impostazioni.tasti
    }

    /// Il corpo del testo in punti (per i test).
    pub fn corpo(&self) -> f32 {
        self.corpo
    }

    /// Il rettangolo della barra di stato nell'ultimo disegno (per i test); `None` se non c'e' (cornice).
    pub fn barra_stato_rect(&self) -> Option<Rect> {
        self.rettangolo_stato
    }

    /// Il rettangolo della pista della scrollbar nell'ultimo disegno (per i test); `None` se non c'e'.
    pub fn scrollbar_rect(&self) -> Option<Rect> {
        self.rettangolo_scroll
    }

    /// I testi dell'ultima barra di stato disegnata (per i test); `None` se non c'e'.
    pub fn stato_disegnato(&self) -> Option<&TestiStato> {
        self.testi.as_ref()
    }

    /// I rettangoli dei pezzi della barra di stato nell'ultimo disegno, da sinistra a destra, col loro comando
    /// (per i test); vuoto se non c'e'.
    pub fn pezzi_stato(&self) -> &[(Rect, Option<Cmd>)] {
        &self.pezzi
    }

    /// Il rettangolo della barra degli strumenti nell'ultimo disegno (per i test); `None` se non c'e' (cornice).
    pub fn strumenti_rect(&self) -> Option<Rect> {
        self.rettangolo_strumenti
    }

    /// I pulsanti della barra degli strumenti nell'ultimo disegno, nell'ordine di disegno (per i test); vuoti se
    /// non c'e'.
    pub fn pulsanti(&self) -> &[Pulsante] {
        &self.pulsanti
    }

    /// Il rettangolo della barra dei salti nell'ultimo disegno (per i test); `None` se non c'e' (cornice, o spenta).
    pub fn salti_rect(&self) -> Option<Rect> {
        self.rettangolo_salti
    }

    /// I pulsanti della barra dei salti nell'ultimo disegno, da sinistra a destra, con la loro voce (per i test);
    /// vuoti se non c'e'.
    pub fn ingranaggio_rect(&self) -> Option<Rect> {
        self.rettangolo_ingranaggio
    }

    /// I pulsanti della barra dei salti nell'ultimo disegno (per i test), da sinistra a destra, con la loro voce.
    pub fn pulsanti_salti(&self) -> &[(Rect, SaltoVoce)] {
        &self.pulsanti_salti_v
    }

    /// Il rettangolo della finestra del dialogo disegnato nell'ultimo giro (per i test, B4b); `None` se non c'e'.
    pub fn dialogo_rect(&self) -> Option<Rect> {
        self.dialogo.rect
    }

    /// Il titolo della finestra del dialogo disegnato nell'ultimo giro (per i test); `None` se non c'e'.
    pub fn dialogo_titolo(&self) -> Option<&str> {
        self.dialogo.titolo.as_deref()
    }

    /// I pulsanti della finestra del dialogo nell'ultimo disegno, da sinistra a destra, col loro testo (per i test);
    /// vuoti se non c'e'.
    pub fn dialogo_pulsanti(&self) -> &[(Rect, String)] {
        &self.dialogo.pulsanti
    }

    /// I rettangoli delle righe del menu del dialogo nell'ultimo disegno, nell'ordine delle voci (per i test); vuoti
    /// se non c'e'.
    pub fn dialogo_voci(&self) -> &[Rect] {
        &self.dialogo.voci
    }

    /// Il rettangolo del campo di testo del prompt del dialogo nell'ultimo disegno (per i test); `None` se non c'e'.
    pub fn dialogo_campo(&self) -> Option<Rect> {
        self.dialogo.campo
    }

    /// I rettangoli dei campi di testo del dialogo (prompt, cerca, sostituisci) nell'ultimo disegno, nell'ordine di
    /// disegno (per i test, B4c); vuoti se non c'e'.
    pub fn dialogo_campi(&self) -> &[Rect] {
        &self.dialogo.campi
    }

    /// Le opzioni del dialogo (il modo e l'ambito di ricerca, le maiuscole) nell'ultimo disegno, nell'ordine di
    /// disegno, con la loro etichetta (per i test); vuote se non c'e'.
    pub fn dialogo_opzioni(&self) -> &[(Rect, String)] {
        &self.dialogo.opzioni
    }

    /// Esegue il comando scelto (menu o pulsante): quelli della finestra (aprire, salvare con nome, zoom) li fa lei, gli altri
    /// vanno al `Workspace` (`exec`: spenti non fanno niente).
    pub fn azione(&mut self, a: Cmd) {
        match a {
            Cmd::Open => self.apri(),
            Cmd::SaveAs => self.salva_con_nome(),
            // il salvataggio di un documento senza nome passa dal dialogo di sistema, non dal prompt a testo dell'App
            Cmd::Save if self.salva_chiede_il_nome() => self.salva_con_nome(),
            Cmd::ZoomIn | Cmd::ZoomOut | Cmd::ZoomReset => self.zoom(a),
            Cmd::Settings => self.impostazioni.aperta = !self.impostazioni.aperta,
            c => {
                self.ws.exec(c);
            }
        }
        self.ultimo_input = Instant::now();
    }

    /// La chiusura della finestra: con modifiche non salvate in qualche scheda si chiede (i dialoghi dell'`App`, una
    /// scheda alla volta), senza si chiude.
    fn chiusura(&mut self, ctx: &Context) {
        if ctx.input(|i| i.viewport().close_requested()) && !self.chiusura {
            if self.ws.tabs().iter().any(|a| a.is_modified()) {
                ctx.send_viewport_cmd(ViewportCommand::CancelClose);
                self.ws.close_all();
            } else {
                self.chiusura = true;
            }
        }
        if self.ws.should_quit() && !self.chiusura {
            self.chiusura = true;
            ctx.send_viewport_cmd(ViewportCommand::Close);
        }
    }

    /// Il titolo della finestra: il nome del file della scheda attiva, un asterisco se e' modificata.
    fn titolo(&mut self, ctx: &Context) {
        let a = self.ws.active();
        let mut t = a.file_name().to_string();
        if a.is_modified() {
            t.push_str(" *");
        }
        t.push_str(" - Manto.RustPad");
        if t != self.titolo {
            ctx.send_viewport_cmd(ViewportCommand::Title(t.clone()));
            self.titolo = t;
        }
    }
}

/// Scrive una schermata di egui come PNG.
fn salva_png(percorso: &std::path::Path, img: &egui::ColorImage) -> Result<(), String> {
    let [w, h] = img.size;
    let file = std::fs::File::create(percorso).map_err(|e| e.to_string())?;
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), w as u32, h as u32);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    let mut scrittore = enc.write_header().map_err(|e| e.to_string())?;
    let dati: Vec<u8> = img.pixels.iter().flat_map(|c| c.to_array()).collect();
    scrittore.write_image_data(&dati).map_err(|e| e.to_string())
}

/// Ctrl + una lettera.
fn ctrl(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
}

impl eframe::App for Finestra {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        self.giro(ctx);
    }
}

impl Finestra {
    /// Un giro della finestra (un `update`): si puo' fare anche in un contesto di egui senza finestra, per i test.
    pub fn giro(&mut self, ctx: &Context) {
        // senza la cornice del terminale la barra dei salti la disegna questa finestra, con i suoi pulsanti: quella a
        // celle si spegne (con la cornice resta a celle, come oggi, e la disegna l'App nella sua griglia)
        self.ws.set_nav_cells(self.ws.active().chrome());
        // i controlli di egui seguono il tema: chiari solo con Chiaro
        let scuro = self.ws.active().theme_id().is_dark();
        if ctx.style().visuals.dark_mode != scuro {
            ctx.set_visuals(if scuro {
                egui::Visuals::dark()
            } else {
                egui::Visuals::light()
            });
        }
        // il font per la scrittura della codifica attuale (F9 la puo' cambiare) e della lingua dei menu (F7)
        let mut nuova = false;
        for s in [
            fonts::scrittura_di(self.ws.active().encoding()),
            fonts::scrittura_della_lingua(self.ws.active().lang_code()),
        ] {
            if s != Scrittura::Nessuna && !self.scritture.contains(&s) {
                self.scritture.push(s);
                nuova = true;
            }
        }
        if nuova {
            fonts::installa(ctx, &self.scritture);
        }
        let font = FontId::monospace(self.corpo);
        let (larga, alta) = ctx.fonts(|f| (f.glyph_width(&font, 'M'), f.row_height(&font)));
        let cella = Vec2::new(larga.max(1.0), alta.ceil().max(1.0));
        self.file_trascinato(ctx);
        self.giri += 1;
        if self.scatto.is_some() && self.giri == 8 && !self.scatto_chiesto {
            self.scatto_chiesto = true;
            ctx.send_viewport_cmd(ViewportCommand::Screenshot(egui::UserData::default()));
        }
        let tocco = self.eventi(ctx, cella.y);
        if tocco {
            self.ultimo_input = Instant::now();
        }
        self.chiusura(ctx);
        self.titolo(ctx);
        let scelta = egui::TopBottomPanel::top("menu")
            .show(ctx, |ui| menu::disegna(ui, &self.ws))
            .inner;
        if let Some(a) = scelta {
            self.azione(a);
        }
        // la barra degli strumenti, solo senza la cornice del terminale (B3): sta sotto il menu, sopra le schede
        if !self.ws.active().chrome() {
            self.strumenti(ctx);
        } else {
            self.pulsanti.clear();
            self.rettangolo_strumenti = None;
        }
        // la barra dei salti, senza la cornice e solo se accesa (Ctrl+B): sta sotto gli strumenti e sopra le schede, al
        // posto della barra a celle del terminale
        if !self.ws.active().chrome() && self.ws.nav_bar() {
            self.salti(ctx);
        } else {
            self.pulsanti_salti_v.clear();
            self.rettangolo_salti = None;
            self.rettangolo_ingranaggio = None;
        }
        if let Some(s) = self.schede(ctx) {
            match s {
                Scheda::Seleziona(i) => self.ws.select(i),
                Scheda::Chiudi(i) => self.ws.close(i),
            }
            self.ultimo_input = Instant::now();
        }
        // la barra di stato e la scrollbar, solo senza la cornice del terminale (con la cornice le disegna l'App
        // nella sua griglia): la barra resta il pannello piu' esterno in basso, la scrollbar prende il lato destro
        // fra il menu e la barra
        if !self.ws.active().chrome() {
            self.barra_stato(ctx);
            self.scrollbar(ctx);
        } else {
            self.pezzi.clear();
            self.rettangolo_stato = None;
            self.rettangolo_scroll = None;
            self.testi = None;
            self.presa_scroll = None;
        }
        egui::CentralPanel::default()
            .frame(Frame::NONE.fill(paint::colore(self.fondo)))
            .show(ctx, |ui| {
                let area = ui.available_rect_before_wrap();
                let colonne = input::celle_in(area.width(), cella.x, COLONNE_MIN);
                let righe = input::celle_in(area.height(), cella.y, RIGHE_MIN);
                if (colonne, righe) != self.dimensione {
                    self.ws.resize(colonne, righe);
                    self.dimensione = (colonne, righe);
                }
                self.ws.tick();
                let griglia = render_grid_workspace(&mut self.ws, colonne, righe);
                if let Some(c) = griglia.celle.last() {
                    self.fondo = c.bg;
                }
                // testo asiatico in un file che non lo dichiara (UTF-8, UTF-16...): il font si carica al primo che si vede
                let mut nuova = false;
                for c in griglia.celle.iter().flat_map(|c| c.simbolo.chars()) {
                    if let Some(s) = fonts::scrittura_del_carattere(c) {
                        if !self.scritture.contains(&s) {
                            self.scritture.push(s);
                            nuova = true;
                        }
                    }
                }
                if nuova {
                    fonts::installa(ctx, &self.scritture);
                }
                let acceso = {
                    let da = self.ultimo_input.elapsed();
                    da < FISSO || ((da - FISSO).as_millis() / LAMPEGGIO.as_millis()) % 2 == 1
                };
                let (rect, risposta) = ui.allocate_exact_size(area.size(), Sense::hover());
                self.geometria = Some((rect.min, cella, colonne, righe));
                paint::disegna(
                    ui.painter(),
                    rect.min,
                    cella,
                    &griglia,
                    &font,
                    self.ws.active().is_edit(),
                    acceso,
                );
                risposta.on_hover_cursor(CursorIcon::Text);
            });
        // il menu contestuale della griglia (tasto destro): solo senza la cornice del terminale e senza un dialogo aperto
        if self.ws.active().chrome() || self.ws.active().dialog_has_view() {
            self.contestuale = None;
        }
        match self.contestuale {
            Some(pos) => {
                let (voci, rect, scelta) = menu::disegna_contestuale(ctx, &self.ws, pos);
                self.contestuale_voci = voci;
                self.contestuale_rect = rect;
                if let Some(c) = scelta {
                    self.contestuale = None;
                    self.contestuale_voci.clear();
                    self.azione(c);
                }
            }
            None => {
                self.contestuale_voci.clear();
                self.contestuale_rect = Rect::NOTHING;
            }
        }
        // la finestra delle impostazioni: i cambi del corpo del testo li fa chi disegna
        if let Some(r) =
            impostazioni::disegna(ctx, &mut self.ws, &mut self.impostazioni, self.corpo)
        {
            self.zoom(match r {
                Richiesta::Piu => Cmd::ZoomIn,
                Richiesta::Meno => Cmd::ZoomOut,
                Richiesta::Predefinito => Cmd::ZoomReset,
            });
        }
        // i dialoghi con una vista (B4b), solo senza la cornice del terminale: una finestra egui al centro, sopra la
        // griglia, che guida l'App con le azioni dialog_*; senza (o con la cornice) lo stato del disegno si azzera
        if !self.ws.active().chrome() && self.ws.active().dialog_has_view() {
            dialoghi::disegna(ctx, &mut self.ws, &mut self.dialogo);
        } else {
            self.dialogo = StatoDialogo::default();
        }
        // le copie vanno agli appunti di sistema
        if let Some(byte) = self.ws.take_copied() {
            ctx.copy_text(String::from_utf8_lossy(&byte).into_owned());
        }
        ctx.request_repaint_after(RIDISEGNO);
    }
}
