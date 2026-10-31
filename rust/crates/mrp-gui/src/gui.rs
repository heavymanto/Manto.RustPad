//! La finestra: un `App` del TUI mostrato come griglia di celle. Ogni giro: si misura la cella, si adatta l'`App` allo
//! spazio (colonne e righe), si portano all'`App` tastiera, incolla e rotella, si fa un `tick`, si disegna la griglia
//! che l'`App` produce (`mrp_tui::cells::render_grid`) e si mandano agli appunti di sistema le copie.

use crate::fonts::{self, Scrittura};
use crate::input;
use crate::paint;
use eframe::egui::{
    self, Context, CursorIcon, Event, FontId, Frame, Pos2, Sense, Vec2, ViewportCommand,
};
use mrp_tui::app::{App, AppConfig};
use mrp_tui::cells::{render_grid, BG_PREDEFINITO};
use ratatui::crossterm::event::{
    KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
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

/// La finestra dell'editor.
pub struct Finestra {
    app: App,
    /// Come si apre un altro file (trascinato sulla finestra): la configurazione di partenza.
    nuova_config: Box<dyn Fn() -> AppConfig>,
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
}

impl Finestra {
    /// La finestra per `app`; `nuova_config` serve ad aprire altri file.
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

    /// Le scritture di partenza: quella della codifica del file, se e' asiatica.
    fn scritture_iniziali(app: &App) -> Vec<Scrittura> {
        match fonts::scrittura_di(app.encoding()) {
            Scrittura::Nessuna => Vec::new(),
            s => vec![s],
        }
    }

    /// La finestra senza toccare il contesto (i font si installano a parte): per i test senza finestra vera.
    pub fn da_app(app: App, nuova_config: Box<dyn Fn() -> AppConfig>) -> Finestra {
        let scritture = Finestra::scritture_iniziali(&app);
        Finestra {
            app,
            nuova_config,
            corpo: CORPO,
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
        }
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

    /// L'`App` sotto la finestra (per i test).
    pub fn app(&mut self) -> &mut App {
        &mut self.app
    }

    /// Porta all'`App` gli eventi di egui di questo giro. Restituisce `true` se ne ha portato qualcuno.
    fn eventi(&mut self, ctx: &Context, altezza_cella: f32) -> bool {
        let (eventi, zoom) = ctx.input(|i| (i.events.clone(), i.zoom_delta()));
        let mut tocco = false;
        if (zoom - 1.0).abs() > f32::EPSILON {
            self.corpo = (self.corpo * zoom).clamp(CORPO_MIN, CORPO_MAX);
        }
        for e in eventi {
            match e {
                Event::Text(t) => {
                    for k in input::testo(&t) {
                        self.app.handle_key(k);
                        tocco = true;
                    }
                }
                Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } => {
                    // Ctrl+'+' / Ctrl+'-' / Ctrl+0: il corpo del testo
                    if modifiers.command || modifiers.ctrl {
                        match key {
                            egui::Key::Plus | egui::Key::Equals => {
                                self.corpo = (self.corpo + 1.0).min(CORPO_MAX);
                                continue;
                            }
                            egui::Key::Minus => {
                                self.corpo = (self.corpo - 1.0).max(CORPO_MIN);
                                continue;
                            }
                            egui::Key::Num0 => {
                                self.corpo = CORPO;
                                continue;
                            }
                            _ => {}
                        }
                    }
                    if let Some(k) = input::tasto(key, modifiers) {
                        self.app.handle_key(k);
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
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers,
                } => {
                    if let Some((o, c, colonne, righe)) = self.geometria {
                        if pressed {
                            if let Some((x, y)) = input::cella_sotto(pos, o, c, colonne, righe) {
                                self.premuto = true;
                                self.app.handle_mouse(MouseEvent {
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
                            self.app.handle_mouse(MouseEvent {
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
                        self.app.handle_mouse(MouseEvent {
                            kind: MouseEventKind::Drag(MouseButton::Left),
                            column: x,
                            row: y + u16::from(pos.y >= o.y),
                            modifiers: KeyModifiers::NONE,
                        });
                        tocco = true;
                    }
                }
                Event::Copy => {
                    self.app.handle_key(ctrl('c'));
                    tocco = true;
                }
                Event::Cut => {
                    self.app.handle_key(ctrl('x'));
                    tocco = true;
                }
                Event::Paste(s) => {
                    if s.is_empty() {
                        // gli appunti di sistema sono vuoti o illeggibili: restano quelli interni dell'app
                        self.app.handle_key(ctrl('v'));
                    } else {
                        self.app.paste_text(&s);
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
        self.app.handle_mouse(MouseEvent {
            kind,
            column: 0,
            row: 0,
            modifiers: KeyModifiers::NONE,
        });
    }

    /// Apre un file trascinato sulla finestra, se non c'e' nulla da salvare.
    fn file_trascinato(&mut self, ctx: &Context) {
        let file = ctx.input(|i| i.raw.dropped_files.iter().find_map(|f| f.path.clone()));
        let Some(percorso) = file else {
            return;
        };
        if self.app.is_modified() {
            return;
        }
        if let Ok(nuova) = App::open(&percorso, (self.nuova_config)()) {
            self.app = nuova;
            self.dimensione = (0, 0);
            self.titolo.clear();
        }
    }

    /// La chiusura della finestra: con modifiche non salvate si chiede all'`App` (il suo dialogo), senza si chiude.
    fn chiusura(&mut self, ctx: &Context) {
        if ctx.input(|i| i.viewport().close_requested()) && !self.chiusura {
            if self.app.is_modified() {
                ctx.send_viewport_cmd(ViewportCommand::CancelClose);
                self.app
                    .handle_key(KeyEvent::new(KeyCode::F(10), KeyModifiers::NONE));
            } else {
                self.chiusura = true;
            }
        }
        if self.app.should_quit() && !self.chiusura {
            self.chiusura = true;
            ctx.send_viewport_cmd(ViewportCommand::Close);
        }
    }

    /// Il titolo della finestra: il nome del file, un asterisco se e' modificato.
    fn titolo(&mut self, ctx: &Context) {
        let mut t = self.app.file_name().to_string();
        if self.app.is_modified() {
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
        // il font per la scrittura della codifica attuale (F9 la puo' cambiare)
        let s = fonts::scrittura_di(self.app.encoding());
        if s != Scrittura::Nessuna && !self.scritture.contains(&s) {
            self.scritture.push(s);
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
        egui::CentralPanel::default()
            .frame(Frame::NONE.fill(paint::colore(BG_PREDEFINITO)))
            .show(ctx, |ui| {
                let area = ui.available_rect_before_wrap();
                let colonne = input::celle_in(area.width(), cella.x, COLONNE_MIN);
                let righe = input::celle_in(area.height(), cella.y, RIGHE_MIN);
                if (colonne, righe) != self.dimensione {
                    self.app.resize(colonne, righe);
                    self.dimensione = (colonne, righe);
                }
                self.app.tick();
                let griglia = render_grid(&mut self.app, colonne, righe);
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
                    self.app.is_edit(),
                    acceso,
                );
                risposta.on_hover_cursor(CursorIcon::Text);
            });
        // le copie vanno agli appunti di sistema
        if let Some(byte) = self.app.take_copied() {
            ctx.copy_text(String::from_utf8_lossy(&byte).into_owned());
        }
        ctx.request_repaint_after(RIDISEGNO);
    }
}
