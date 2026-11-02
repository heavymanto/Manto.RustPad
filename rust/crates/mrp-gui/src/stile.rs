//! I colori della finestra secondo il tema (G2): la tavolozza che `egui` usa per i pannelli (menu, barre, stato), per i popup
//! (dialoghi, impostazioni, menu a tendina) e per la testata dei popup, presa dai colori del tema del motore
//! (`mrp_tui::theme::Theme`): lo stesso blu del testo nei popup del tema Blu (stile Total Commander), i grigi di debconf
//! nei popup del tema Debian (sfondo blu, finestre grigio chiaro con il testo nero), e cosi' via per Scuro e Chiaro.

use eframe::egui::{Color32, Stroke, Style, Visuals};
use mrp_tui::cells::{colore, BG_PREDEFINITO, FG_PREDEFINITO};
use mrp_tui::theme::{Theme, ThemeId};
use ratatui::style::Style as StileTerm;

/// I colori di un tema per la finestra.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tavolozza {
    /// Il tema e' scuro (la base di egui e' quella scura).
    pub scuro: bool,
    /// Sfondo e testo dei pannelli (menu, barre, stato).
    pub schermo_bg: Color32,
    pub schermo_fg: Color32,
    /// Sfondo e testo dei popup.
    pub popup_bg: Color32,
    pub popup_fg: Color32,
    /// Il bordo delle finestre e dei separatori.
    pub bordo: Color32,
    /// La testata dei popup e la voce scelta: fondo e testo (lo stile `selected` del tema).
    pub risalto_bg: Color32,
    pub risalto_fg: Color32,
    /// Lo sfondo dei campi di testo nei popup.
    pub campo_bg: Color32,
    /// Un pulsante a due stati (modifica, colonna, barra dei salti) quando e' ACCESO: fondo, testo e contorno decisi, ben
    /// diversi da quando e' spento (nei temi Scuro e Chiaro il risalto da solo e' troppo discreto sui pannelli).
    pub acceso_bg: Color32,
    pub acceso_fg: Color32,
    pub acceso_bordo: Color32,
}

fn rgb(c: [u8; 3]) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}

fn sfondo(s: StileTerm) -> Color32 {
    rgb(s.bg.map_or(BG_PREDEFINITO, |c| colore(c, BG_PREDEFINITO)))
}

fn primo_piano(s: StileTerm) -> Color32 {
    rgb(s.fg.map_or(FG_PREDEFINITO, |c| colore(c, FG_PREDEFINITO)))
}

/// `a` portato verso `b` di `t` (da 0 a 1).
pub fn miscela(a: Color32, b: Color32, t: f32) -> Color32 {
    let m = |x: u8, y: u8| (f32::from(x) * (1.0 - t) + f32::from(y) * t).round() as u8;
    Color32::from_rgb(m(a.r(), b.r()), m(a.g(), b.g()), m(a.b(), b.b()))
}

/// La luminosita' approssimata di un colore, da 0 a 255.
fn luce(c: Color32) -> f32 {
    0.299 * f32::from(c.r()) + 0.587 * f32::from(c.g()) + 0.114 * f32::from(c.b())
}

impl Tavolozza {
    /// La tavolozza del tema `id`.
    pub fn di(id: ThemeId) -> Tavolozza {
        let t = Theme::for_id(id);
        let popup_bg = sfondo(t.dialog);
        let popup_fg = primo_piano(t.dialog);
        // il campo di testo: bianco sui popup chiari, piu' scuro del popup sui popup scuri
        let campo_bg = if luce(popup_bg) > 140.0 {
            Color32::WHITE
        } else {
            miscela(popup_bg, Color32::BLACK, 0.35)
        };
        let risalto_bg = sfondo(t.selected);
        let risalto_fg = primo_piano(t.selected);
        // acceso: il risalto del tema, schiarito se e' troppo scuro per staccare dal pannello (Scuro), con un contorno netto
        // (bianco sui temi scuri, quasi nero sul Chiaro)
        let acceso_bg = if luce(risalto_bg) < 90.0 {
            miscela(risalto_bg, Color32::from_rgb(0x2a, 0xa0, 0xff), 0.75)
        } else {
            risalto_bg
        };
        let acceso_bordo = if id.is_dark() {
            Color32::WHITE
        } else {
            Color32::from_rgb(0x14, 0x14, 0x14)
        };
        Tavolozza {
            scuro: id.is_dark(),
            schermo_bg: sfondo(t.screen),
            schermo_fg: primo_piano(t.screen),
            popup_bg,
            popup_fg,
            bordo: primo_piano(t.border),
            risalto_bg,
            risalto_fg,
            campo_bg,
            acceso_bg,
            acceso_fg: if luce(acceso_bg) > 150.0 {
                Color32::BLACK
            } else {
                Color32::WHITE
            },
            acceso_bordo,
        }
    }

    /// Un colore per i pulsanti neutri di un popup: un po' staccato dallo sfondo.
    pub fn pulsante_neutro(&self) -> Color32 {
        if luce(self.popup_bg) > 140.0 {
            miscela(self.popup_bg, Color32::BLACK, 0.1)
        } else {
            miscela(self.popup_bg, Color32::WHITE, 0.15)
        }
    }
}

/// Ricorda la tavolozza del giro nel contesto di egui: i popup (dialoghi, impostazioni, menu a tendina) la leggono con `corrente`
/// senza doverla ricevere da chi li chiama.
pub fn registra(ctx: &eframe::egui::Context, t: &Tavolozza) {
    ctx.data_mut(|d| d.insert_temp(eframe::egui::Id::new("mrp_tavolozza"), *t));
}

/// La tavolozza registrata in questo giro (quella del tema Scuro se nessuno l'ha registrata).
pub fn corrente(ctx: &eframe::egui::Context) -> Tavolozza {
    ctx.data(|d| d.get_temp(eframe::egui::Id::new("mrp_tavolozza")))
        .unwrap_or_else(|| Tavolozza::di(ThemeId::Dark))
}

/// Mette in `style` i colori di una zona della finestra: `bg` e `fg` sono lo sfondo e il testo, `campo` lo sfondo dei campi di
/// testo; la voce sotto il mouse o scelta prende il risalto del tema.
fn colora(style: &mut Style, t: &Tavolozza, bg: Color32, fg: Color32, campo: Color32) {
    let neutro = if luce(bg) > 140.0 {
        miscela(bg, Color32::BLACK, 0.1)
    } else {
        miscela(bg, Color32::WHITE, 0.15)
    };
    let v = &mut style.visuals;
    v.override_text_color = None;
    v.extreme_bg_color = campo;
    v.faint_bg_color = miscela(bg, fg, 0.06);
    v.hyperlink_color = t.risalto_bg;
    v.selection.bg_fill = t.risalto_bg;
    v.selection.stroke = Stroke::new(1.0_f32, t.risalto_fg);
    let w = &mut v.widgets;
    w.noninteractive.bg_fill = bg;
    w.noninteractive.weak_bg_fill = bg;
    w.noninteractive.bg_stroke = Stroke::new(1.0_f32, miscela(bg, fg, 0.3));
    w.noninteractive.fg_stroke = Stroke::new(1.0_f32, fg);
    w.inactive.bg_fill = neutro;
    w.inactive.weak_bg_fill = neutro;
    w.inactive.fg_stroke = Stroke::new(1.0_f32, fg);
    w.inactive.bg_stroke = Stroke::new(1.0_f32, miscela(bg, fg, 0.35));
    for parte in [&mut w.hovered, &mut w.active, &mut w.open] {
        parte.bg_fill = t.risalto_bg;
        parte.weak_bg_fill = t.risalto_bg;
        parte.fg_stroke = Stroke::new(1.0_f32, t.risalto_fg);
        parte.bg_stroke = Stroke::new(1.0_f32, t.risalto_bg);
    }
}

/// I `Visuals` globali del tema: i pannelli con lo sfondo e il testo dello schermo; le finestre e i menu a tendina con lo
/// sfondo del popup (il testo dei popup lo mette `popup` dentro la finestra).
pub fn visuals(t: &Tavolozza) -> Visuals {
    let base = if t.scuro {
        Visuals::dark()
    } else {
        Visuals::light()
    };
    let mut stile = Style {
        visuals: base,
        ..Style::default()
    };
    colora(&mut stile, t, t.schermo_bg, t.schermo_fg, t.campo_bg);
    let mut v = stile.visuals;
    v.dark_mode = t.scuro;
    v.panel_fill = t.schermo_bg;
    v.window_fill = t.popup_bg;
    v.window_stroke = Stroke::new(1.0_f32, t.bordo);
    v.popup_shadow.color = Color32::from_black_alpha(90);
    v
}

/// Le voci di un menu a tendina sono piatte: niente fondo e niente bordo finche' non ci si passa sopra (la voce sotto il mouse
/// prende il risalto del tema). Senza, ogni voce avrebbe il suo riquadro.
pub fn voci_piatte(style: &mut Style) {
    let i = &mut style.visuals.widgets.inactive;
    i.bg_fill = Color32::TRANSPARENT;
    i.weak_bg_fill = Color32::TRANSPARENT;
    i.bg_stroke = Stroke::NONE;
}

/// Dentro un popup: lo sfondo e il testo del popup (nel tema Debian grigio chiaro con il testo nero, anche se i pannelli sono
/// blu con il testo bianco).
pub fn popup(style: &mut Style, t: &Tavolozza) {
    colora(style, t, t.popup_bg, t.popup_fg, t.campo_bg);
}
