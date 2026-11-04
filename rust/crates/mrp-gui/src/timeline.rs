//! La barra della timeline (F2): a sinistra della scrollbar, larga abbastanza da leggere le date, una "mappa" del file nel tempo
//! come quella di Visual Studio: una riga per bucket, lunga quanto il volume di righe di quel periodo, con la data accanto e un
//! riquadro trasparente sulla parte che si vede. Un clic salta a quel punto, trascinando si scorre, e un suggerimento dice l'orario
//! sotto il puntatore. In alto un menu a tendina sceglie il passo (minuto, dieci minuti, ora, giorno); il clic destro sceglie come
//! si dispongono le righe (l'altezza e' il tempo, oppure sono i byte). I colori sono quelli del tema (`Tavolozza`).

use crate::stile::{miscela, Tavolozza};
use eframe::egui::{self, Align2, Color32, FontId, Frame, Pos2, Rect, RichText, Sense, Vec2};
use mrp_core::timeline::Step;
use mrp_tui::app::timeline::{TimelineMode, TimelineStatus};
use mrp_tui::workspace::Workspace;

/// La larghezza della barra, in punti: larga per leggere le date a colpo d'occhio.
pub const LARGHEZZA: f32 = 96.0;
/// L'altezza minima di una riga: con piu' bucket di cosi' si raggruppano.
const RIGA_MIN: f32 = 3.0;
/// L'altezza dell'intestazione col menu del passo.
const INTESTAZIONE: f32 = 52.0;
/// L'altezza della prima riga dell'intestazione (il menu del passo); sotto ci sono i pulsanti dei livelli.
const RIGA_PASSO: f32 = 28.0;
/// La larghezza di ogni segno di livello sul bordo destro della barra.
const SEGNO: f32 = 5.0;
/// Fra due scritte c'e' almeno questo spazio.
const SCRITTA_ALTEZZA: f32 = 13.0;

/// Cosa si e' disegnato l'ultima volta (per i test).
#[derive(Debug, Default, Clone)]
pub struct StatoTimeline {
    /// Il rettangolo del pannello (`None`: non c'e').
    pub rect: Option<Rect>,
    /// Il rettangolo del corpo (sotto l'intestazione), dove si clicca.
    pub corpo: Option<Rect>,
    /// Quante righe si sono disegnate, e quante scritte.
    pub righe: usize,
    pub scritte: usize,
    /// Il rettangolo del menu del passo.
    pub passo: Option<Rect>,
    /// I pulsanti dei livelli (E, W, I, D) e quello che apre la vista filtrata, con la loro etichetta (per i test).
    pub chip: Vec<(Rect, String)>,
    /// Quanti segni di livello si sono disegnati (uno per riga e per livello presente).
    pub strisce: usize,
    /// Quando si e' scelto o tolto l'ultimo livello: poco dopo (se non se ne sceglie un altro) la vista filtrata si apre da sola.
    pub livelli_in_attesa: Option<std::time::Instant>,
}

/// Quanto si aspetta, dopo l'ultima scelta di un livello, prima di aprire la vista filtrata (il tempo di sceglierne un altro).
pub const ATTESA_LIVELLI: std::time::Duration = std::time::Duration::from_millis(700);

const PASSI: [(Step, &str); 4] = [
    (Step::Minute, "timeline.step.minute"),
    (Step::TenMinutes, "timeline.step.ten_minutes"),
    (Step::Hour, "timeline.step.hour"),
    (Step::Day, "timeline.step.day"),
];

fn nome_passo(passo: Step) -> &'static str {
    PASSI
        .iter()
        .find(|(p, _)| *p == passo)
        .map_or("timeline.step.day", |(_, k)| k)
}

/// Il colore `c` con la trasparenza `alfa` (0 - 255).
fn con_alfa(c: Color32, alfa: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), alfa)
}

/// Disegna la barra se la timeline della scheda attiva e' accesa (e lavora sulla scheda attiva: clic, trascinamento, menu).
pub fn disegna(
    ctx: &egui::Context,
    ws: &mut Workspace,
    tav: &Tavolozza,
    stato: &mut StatoTimeline,
) {
    if !ws.active().timeline_on() {
        *stato = StatoTimeline::default();
        return;
    }
    stato.righe = 0;
    stato.scritte = 0;
    stato.chip.clear();
    stato.strisce = 0;
    let pannello = egui::SidePanel::right("timeline")
        .exact_width(LARGHEZZA)
        .frame(Frame::NONE)
        .show(ctx, |ui| {
            let area = ui.max_rect();
            // l'intestazione: il menu del passo
            let (testa, riga_livelli, corpo) = (
                Rect::from_min_size(area.min, Vec2::new(area.width(), RIGA_PASSO)),
                Rect::from_min_size(
                    Pos2::new(area.min.x, area.min.y + RIGA_PASSO),
                    Vec2::new(area.width(), INTESTAZIONE - RIGA_PASSO),
                ),
                Rect::from_min_max(Pos2::new(area.min.x, area.min.y + INTESTAZIONE), area.max),
            );
            let passo = ws.active().timeline_step();
            let automatico = ws.active().timeline_auto();
            let nomi: Vec<String> = PASSI
                .iter()
                .map(|(_, k)| ws.active().text(k).to_string())
                .collect();
            let t_auto = ws.active().text("timeline.step.auto").to_string();
            let nome_effettivo = ws.active().text(nome_passo(passo)).to_string();
            let selezionato = if automatico {
                format!("{t_auto}: {nome_effettivo}")
            } else {
                nome_effettivo
            };
            let mut scelto = None;
            let mut torna_auto = false;
            let mut ui_testa = ui.new_child(egui::UiBuilder::new().max_rect(testa.shrink(3.0)));
            let combo = egui::ComboBox::from_id_salt("timeline_passo")
                .width(testa.width() - 14.0)
                .selected_text(RichText::new(selezionato).small())
                .show_ui(&mut ui_testa, |ui| {
                    if ui.selectable_label(automatico, &t_auto).clicked() {
                        torna_auto = true;
                    }
                    for (i, (p, _)) in PASSI.iter().enumerate() {
                        if ui
                            .selectable_label(!automatico && *p == passo, &nomi[i])
                            .clicked()
                        {
                            scelto = Some(*p);
                        }
                    }
                });
            stato.passo = Some(combo.response.rect);
            if torna_auto {
                ws.active_mut().set_timeline_auto();
            }
            if let Some(p) = scelto {
                ws.active_mut().set_timeline_step(p);
            }
            // i livelli: E W I D (scegli quali), e il pulsante che apre la vista con le sole righe scelte
            let colori = [tav.log_errore, tav.log_avviso, tav.log_info, tav.log_debug];
            let lettere = ["E", "W", "I", "D"];
            let chiavi = [
                "timeline.level.error",
                "timeline.level.warn",
                "timeline.level.info",
                "timeline.level.debug",
            ];
            let nomi_livelli: Vec<String> = chiavi
                .iter()
                .map(|k| ws.active().text(k).to_string())
                .collect();
            let t_filtra = ws.active().text("timeline.filter_tip").to_string();
            let scelti = ws.active().timeline_filter_levels();
            let mut ui_liv = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(riga_livelli.shrink2(Vec2::new(3.0, 1.0)))
                    .layout(egui::Layout::left_to_right(egui::Align::Center)),
            );
            ui_liv.spacing_mut().item_spacing.x = 2.0;
            for i in 0..4 {
                let testo = RichText::new(lettere[i]).size(10.0).strong();
                let b = if scelti[i] {
                    // scelto: pieno del colore del livello, con la lettera che stacca
                    let luce = 0.299 * f32::from(colori[i].r())
                        + 0.587 * f32::from(colori[i].g())
                        + 0.114 * f32::from(colori[i].b());
                    let lettera = testo.color(if luce > 150.0 {
                        Color32::BLACK
                    } else {
                        Color32::WHITE
                    });
                    egui::Button::new(lettera)
                        .min_size(Vec2::new(17.0, 19.0))
                        .fill(colori[i])
                } else {
                    egui::Button::new(testo.color(colori[i])).min_size(Vec2::new(17.0, 19.0))
                };
                let r = ui_liv.add(b).on_hover_text(nomi_livelli[i].as_str());
                stato
                    .chip
                    .push((r.rect, ["ERROR", "WARN", "INFO", "DEBUG"][i].to_string()));
                if r.clicked() {
                    ws.active_mut().set_timeline_filter_level(i, !scelti[i]);
                    stato.livelli_in_attesa = Some(std::time::Instant::now());
                }
            }
            let f = ui_liv
                .add(egui::Button::new("").min_size(Vec2::new(20.0, 19.0)))
                .on_hover_text(t_filtra.as_str());
            // l'imbuto si disegna a mano (nel font il glifo non c'e' e uscirebbe un quadratino)
            let colore_imbuto = ui_liv.style().interact(&f).fg_stroke.color;
            crate::strumenti::imbuto(ui_liv.painter(), f.rect.center(), colore_imbuto, 10.0);
            stato.chip.push((f.rect, "filtra".to_string()));
            if f.clicked() {
                stato.livelli_in_attesa = None;
                ws.active_mut().timeline_apply_level_filter();
            }
            // scelti i livelli, la vista filtrata si apre da sola dopo un attimo (senza dover premere l'imbuto); togliendo l'ultimo no
            if let Some(quando) = stato.livelli_in_attesa {
                if quando.elapsed() >= ATTESA_LIVELLI {
                    stato.livelli_in_attesa = None;
                    if ws.active().timeline_filter_levels().contains(&true) {
                        ws.active_mut().timeline_apply_level_filter();
                    }
                } else {
                    ctx.request_repaint_after(std::time::Duration::from_millis(100));
                }
            }
            // il corpo: lo sfondo tenue e le righe
            let altezza = corpo.height().max(1.0);
            let max_righe = ((altezza / RIGA_MIN) as usize).max(1);
            let tenue = ui.style().visuals.faint_bg_color;
            ui.painter().rect_filled(corpo, 0.0, tenue);
            let stato_t = ws.active().timeline_status();
            let righe = ws.active().timeline_rows(max_righe);
            let modo = ws.active().timeline_mode();
            let barra = miscela(tav.acceso_bg, tav.schermo_bg, 0.15);
            let testo = tav.schermo_fg;
            let mut ultima_scritta = f32::NEG_INFINITY;
            // il bucket con piu' righe di ogni livello: i segni si misurano su di lui
            let massimi = [
                righe.iter().map(|r| r.levels.error).max().unwrap_or(0),
                righe.iter().map(|r| r.levels.warn).max().unwrap_or(0),
                righe.iter().map(|r| r.levels.info).max().unwrap_or(0),
                righe.iter().map(|r| r.levels.debug).max().unwrap_or(0),
            ];
            for r in &righe {
                let (y0, y1) = (corpo.min.y + r.y0 * altezza, corpo.min.y + r.y1 * altezza);
                if y1 - y0 < 0.5 && modo == TimelineMode::Bytes {
                    continue;
                }
                let alto = (y1 - y0 - 1.0).max(1.0);
                let lunga = (r.weight.sqrt() * (corpo.width() - 2.0)).max(if r.bytes > 0 {
                    2.0
                } else {
                    0.0
                });
                if lunga > 0.0 {
                    ui.painter().rect_filled(
                        Rect::from_min_size(Pos2::new(corpo.min.x, y0), Vec2::new(lunga, alto)),
                        0.0,
                        con_alfa(barra, 200),
                    );
                    stato.righe += 1;
                }
                // i livelli: un segno per livello sul bordo destro, piu' marcato dove ce ne sono di piu' (rispetto al bucket piu' pieno)
                if r.levels_known && r.bytes > 0 {
                    let valori = [r.levels.error, r.levels.warn, r.levels.info, r.levels.debug];
                    for (j, &n) in valori.iter().enumerate() {
                        if n == 0 || massimi[j] == 0 {
                            continue;
                        }
                        let forza = (n as f32 / massimi[j] as f32).sqrt();
                        let x1 = corpo.max.x - j as f32 * SEGNO;
                        ui.painter().rect_filled(
                            Rect::from_min_max(Pos2::new(x1 - SEGNO, y0), Pos2::new(x1, y0 + alto)),
                            0.0,
                            con_alfa(colori[j], (70.0 + 185.0 * forza) as u8),
                        );
                        stato.strisce += 1;
                    }
                }
                if y0 - ultima_scritta >= SCRITTA_ALTEZZA {
                    ui.painter().text(
                        Pos2::new(corpo.min.x + 3.0, y0),
                        Align2::LEFT_TOP,
                        &r.label,
                        FontId::monospace(9.5),
                        con_alfa(testo, 235),
                    );
                    ultima_scritta = y0;
                    stato.scritte += 1;
                }
            }
            // la passata dei livelli sta contando: una linea sottile in cima al corpo che avanza
            if let Some(f) = ws.active().timeline_levels_progress() {
                ui.painter().rect_filled(
                    Rect::from_min_size(corpo.min, Vec2::new(corpo.width() * f, 2.0)),
                    0.0,
                    tav.acceso_bordo,
                );
            }
            // la parte che si vede: un riquadro trasparente col contorno
            if let Some((da, a)) = ws.active().timeline_viewport(max_righe) {
                let (y0, y1) = (corpo.min.y + da * altezza, corpo.min.y + a * altezza);
                let r = Rect::from_min_max(
                    Pos2::new(corpo.min.x, y0),
                    Pos2::new(corpo.max.x, y1.max(y0 + 4.0)),
                );
                ui.painter()
                    .rect_filled(r, 0.0, con_alfa(tav.acceso_bordo, 60));
                ui.painter().rect_stroke(
                    r,
                    0.0,
                    egui::Stroke::new(1.5_f32, tav.acceso_bordo),
                    egui::StrokeKind::Inside,
                );
            }
            // cosa sta succedendo, se non c'e' niente da disegnare
            if righe.is_empty() {
                let cat = ws.active();
                let msg = match stato_t {
                    TimelineStatus::Building(n) => {
                        cat.text_with("timeline.building", &[&n.to_string()])
                    }
                    TimelineStatus::NoTimes => cat.text("timeline.no_times").to_string(),
                    TimelineStatus::TooMany(n) => {
                        cat.text_with("timeline.too_many", &[&n.to_string()])
                    }
                    TimelineStatus::Unavailable => cat.text("timeline.unavailable").to_string(),
                    _ => String::new(),
                };
                if !msg.is_empty() {
                    let mut ui_msg =
                        ui.new_child(egui::UiBuilder::new().max_rect(corpo.shrink(5.0)));
                    ui_msg.label(RichText::new(msg).small().weak());
                }
            }
            // il clic e il trascinamento portano al punto; il passaggio dice l'orario; il clic destro sceglie la disposizione
            let risposta = ui.interact(
                corpo,
                ui.id().with("timeline_corpo"),
                Sense::click_and_drag(),
            );
            let y_di = |p: Pos2| ((p.y - corpo.min.y) / altezza).clamp(0.0, 1.0);
            if risposta.is_pointer_button_down_on() || risposta.clicked() {
                if let Some(p) = risposta.interact_pointer_pos() {
                    ws.active_mut().timeline_jump(y_di(p), max_righe);
                }
            }
            if let Some(p) = risposta
                .hover_pos()
                .or_else(|| risposta.interact_pointer_pos())
            {
                if let Some(orario) = ws.active().timeline_time_at(y_di(p), max_righe) {
                    let _ = risposta.clone().on_hover_text(orario);
                }
            }
            let nomi_modo = [
                (
                    TimelineMode::Time,
                    ws.active().text("timeline.mode.time").to_string(),
                ),
                (
                    TimelineMode::Bytes,
                    ws.active().text("timeline.mode.bytes").to_string(),
                ),
            ];
            risposta.context_menu(|ui| {
                for (m, nome) in &nomi_modo {
                    if ui.selectable_label(*m == modo, nome).clicked() {
                        ws.active_mut().set_timeline_mode(*m);
                        ui.close();
                    }
                }
            });
            stato.corpo = Some(corpo);
            // la linea che la separa dalla scrollbar
            ui.painter().line_segment(
                [
                    Pos2::new(area.max.x, area.min.y),
                    Pos2::new(area.max.x, area.max.y),
                ],
                egui::Stroke::new(1.0_f32, tav.bordo),
            );
            area
        });
    stato.rect = Some(pannello.inner);
}
