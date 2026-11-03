//! La barra di stato e la scrollbar della finestra senza cornice (B2b). I testi della barra (`testi_stato`) e la
//! geometria del pollice della scrollbar (`pollice`, `frazione_da_posizione`) sono funzioni pure, quindi provabili
//! senza finestra; il disegno con egui sta in `gui.rs` e usa questi pezzi.

use mrp_core::lang::Catalog;
use mrp_tui::app::{Busy, ScrollInfo, StatusInfo};
use mrp_tui::commands::Cmd;
use mrp_tui::ui::fmt_size;

/// Il pollice della scrollbar non scende sotto quest'altezza (punti): piu' piccolo si perde.
pub const POLLICE_MINIMO: f32 = 16.0;

/// Un pezzo della parte destra della barra di stato: il testo e, se un clic fa qualcosa, il comando che esegue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pezzo {
    pub testo: String,
    pub comando: Option<Cmd>,
}

/// I testi della barra di stato: a sinistra la posizione del cursore (o il messaggio transitorio), a destra i
/// pezzi, da sinistra a destra.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestiStato {
    pub sinistra: String,
    pub destra: Vec<Pezzo>,
}

/// I testi della barra di stato per lo stato `s`, nella lingua di `cat`. A sinistra il messaggio transitorio, se
/// c'e', altrimenti la posizione del cursore (senza gli spazi in testa e in coda che il catalogo mette); a destra,
/// da sinistra a destra: l'eventuale lavoro in corso, il modo (con la modalita' colonna), la codifica, l'a-capo e
/// la dimensione del documento.
pub fn testi_stato(s: &StatusInfo, cat: &Catalog) -> TestiStato {
    let sinistra = match &s.flash {
        Some(messaggio) => messaggio.clone(),
        None => {
            let riga = match s.line {
                Some(n) => n.to_string(),
                None => cat.t("status.unknown_line").to_string(),
            };
            let totale = match s.line_count {
                Some(n) => n.to_string(),
                None => cat.t("status.unknown_line").to_string(),
            };
            let mut testo = cat
                .tf(
                    "gui.status.pos",
                    &[
                        &riga,
                        &totale,
                        &s.column.to_string(),
                        &s.offset.to_string(),
                        &s.percent.to_string(),
                    ],
                )
                .trim()
                .to_string();
            // la selezione, subito dopo la posizione: un blocco (righe x colonne) o quanti byte e quante righe
            if let Some(sel) = s.selection {
                let aggiunta = match (sel.block, sel.lines) {
                    (Some((r, c)), _) => {
                        cat.tf("gui.status.block", &[&r.to_string(), &c.to_string()])
                    }
                    (None, Some(l)) => cat.tf(
                        "gui.status.sel_lines",
                        &[&sel.bytes.to_string(), &l.to_string()],
                    ),
                    (None, None) => cat.tf("gui.status.sel_bytes", &[&sel.bytes.to_string()]),
                };
                testo.push_str("   ");
                testo.push_str(&aggiunta);
            }
            testo
        }
    };
    let mut destra = Vec::new();
    // il lavoro in corso, prima di tutto il resto
    if let Some((lavoro, percentuale)) = s.busy {
        let chiave = match lavoro {
            Busy::Saving => "status.saving",
            Busy::Searching => "status.searching",
            Busy::Indexing => "status.indexing",
        };
        destra.push(Pezzo {
            testo: cat.tf(chiave, &[&percentuale.to_string()]),
            comando: None,
        });
    }
    // l'orario della prima riga visibile (se il file ha una maschera): un clic apre l'intervallo di tempo
    if let Some(t) = &s.time {
        destra.push(Pezzo {
            testo: t.clone(),
            comando: Some(Cmd::TimeRange),
        });
    }
    // il modo cambia al clic, la modalita' colonna pure, la codifica apre il suo menu; a-capo e dimensione no
    destra.push(Pezzo {
        testo: cat
            .t(if s.edit {
                "mode.edit"
            } else if s.follow {
                "mode.follow"
            } else {
                "mode.view"
            })
            .to_string(),
        comando: Some(Cmd::EditMode),
    });
    if s.column_mode {
        destra.push(Pezzo {
            testo: cat.t("mode.column").to_string(),
            comando: Some(Cmd::ColumnMode),
        });
    }
    // gli interruttori accesi, a colpo d'occhio: caratteri speciali e colori dei log (un clic li spegne)
    if s.special {
        destra.push(Pezzo {
            testo: "\u{b6}".to_string(),
            comando: Some(Cmd::Special),
        });
    }
    if s.log_colors {
        destra.push(Pezzo {
            testo: cat.t("status.log").to_string(),
            comando: Some(Cmd::LogColors),
        });
    }
    destra.push(Pezzo {
        testo: s.encoding.clone(),
        comando: Some(Cmd::Encoding),
    });
    destra.push(Pezzo {
        testo: s.line_ending.to_string(),
        comando: None,
    });
    destra.push(Pezzo {
        testo: fmt_size(s.size),
        comando: None,
    });
    TestiStato { sinistra, destra }
}

/// Il pollice della scrollbar per la finestra `info` su una pista alta `altezza`: `(cima, alto)` in pixel. E'
/// alto come la parte di documento che si vede, almeno `minimo` (o quanto la pista, se e' piu' bassa lei), e non
/// esce dalla pista. Le frazioni dell'`App` sono f64 e i pixel f32: il prodotto si fa in f64 e poi si taglia.
pub fn pollice(info: ScrollInfo, altezza: f32, minimo: f32) -> (f32, f32) {
    let alto = ((info.size * f64::from(altezza)) as f32).clamp(minimo.min(altezza), altezza);
    let cima = ((info.start * f64::from(altezza)) as f32).clamp(0.0, altezza - alto);
    (cima, alto)
}

/// La frazione del documento (0..1) a cui va la cima della finestra quando il puntatore e' alla `y` sulla pista e
/// il pollice si tiene per `presa` dalla sua cima: il clic sulla pista, con la presa a meta' pollice, lo centra
/// sul puntatore.
pub fn frazione_da_posizione(y: f32, presa: f32, altezza: f32) -> f64 {
    if altezza <= 0.0 {
        return 0.0;
    }
    (f64::from(y - presa) / f64::from(altezza)).clamp(0.0, 1.0)
}
