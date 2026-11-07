//! L'avanzamento dei lavori lunghi (indicizzazione, ricerca, salvataggio) per file grandi: compare solo dopo un po' di tempo e solo
//! sopra una certa dimensione (sotto, la riga di stato dice la percentuale e basta), con i byte fatti, la velocita' e il tempo che
//! resta. La stima si fa sulla velocita' degli ultimi secondi, non dall'inizio: il disco passa dalla cache alla lettura vera e la
//! stima segue.

use super::Busy;
use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// Il lavoro si mostra dopo tanto tempo...
pub const MOSTRA_DOPO: Duration = Duration::from_secs(2);
/// ...e solo se il file e' almeno cosi' grande (64 MiB).
pub const MOSTRA_DA_BYTE: u64 = 64 * 1024 * 1024;
/// La velocita' si misura sugli ultimi secondi.
const FINESTRA: Duration = Duration::from_secs(3);
/// La stima del tempo che resta compare dopo tanto tempo di misure.
const STIMA_DOPO: Duration = Duration::from_millis(1500);
/// Sotto questo intervallo fra due campioni la velocita' non e' attendibile.
const INTERVALLO_MINIMO: Duration = Duration::from_millis(500);
/// Oltre tanto tempo che resta (99 ore) la stima non si dice.
const STIMA_MASSIMA_SECONDI: u64 = 99 * 3600;

/// Cio' che il lavoro in corso ha fatto finora.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Avanzamento {
    pub lavoro: Busy,
    /// I byte esaminati (o scritti).
    pub fatti: u64,
    /// I byte da fare in tutto.
    pub totale: u64,
    /// Da quanto tempo gira.
    pub trascorso: Duration,
    /// Byte al secondo, sugli ultimi secondi; `None` finche' non si puo' dire (o se il lavoro e' fermo).
    pub velocita: Option<u64>,
    /// Il tempo che resta alla velocita' di adesso; `None` se la velocita' non c'e'.
    pub resta: Option<Duration>,
}

impl Avanzamento {
    /// La percentuale fatta (0..=100).
    pub fn percento(&self) -> u8 {
        if self.totale == 0 {
            return 0;
        }
        (self.fatti.saturating_mul(100) / self.totale).min(100) as u8
    }
}

/// Le misure del lavoro in corso: quando e' partito e i campioni degli ultimi secondi.
#[derive(Debug, Default)]
pub struct Misura {
    lavoro: Option<Busy>,
    avviato: Option<Instant>,
    campioni: VecDeque<(Instant, u64)>,
}

impl Misura {
    /// Registra che `lavoro` ha fatto `fatti` byte su `totale` all'istante `ora`. `None` se non c'e' (piu') un lavoro. Restituisce
    /// l'avanzamento solo quando va mostrato: il lavoro gira da `MOSTRA_DOPO` e il totale arriva a `MOSTRA_DA_BYTE`.
    pub fn registra(
        &mut self,
        lavoro: Option<Busy>,
        fatti: u64,
        totale: u64,
        ora: Instant,
    ) -> Option<Avanzamento> {
        let Some(lavoro) = lavoro else {
            *self = Misura::default();
            return None;
        };
        // un lavoro diverso, o uno che riparte da capo, rifa' le misure
        let ripartito = self.campioni.back().is_some_and(|&(_, f)| fatti < f);
        if self.lavoro != Some(lavoro) || ripartito {
            self.lavoro = Some(lavoro);
            self.avviato = Some(ora);
            self.campioni.clear();
        }
        self.campioni.push_back((ora, fatti));
        while self.campioni.len() > 2
            && self
                .campioni
                .front()
                .is_some_and(|&(t, _)| ora.saturating_duration_since(t) > FINESTRA)
        {
            self.campioni.pop_front();
        }
        let trascorso = ora.saturating_duration_since(self.avviato.unwrap_or(ora));
        if trascorso < MOSTRA_DOPO || totale < MOSTRA_DA_BYTE {
            return None;
        }
        let velocita = if trascorso >= STIMA_DOPO {
            self.campioni.front().and_then(|&(t0, f0)| {
                let dt = ora.saturating_duration_since(t0);
                (dt >= INTERVALLO_MINIMO && fatti > f0)
                    .then(|| ((fatti - f0) as f64 / dt.as_secs_f64()) as u64)
                    .filter(|&v| v > 0)
            })
        } else {
            None
        };
        let resta = velocita.and_then(|v| {
            let secondi = totale.saturating_sub(fatti) / v;
            (secondi <= STIMA_MASSIMA_SECONDI).then(|| Duration::from_secs(secondi))
        });
        Some(Avanzamento {
            lavoro,
            fatti,
            totale,
            trascorso,
            velocita,
            resta,
        })
    }
}

/// Un tempo in colonna: `0:12`, `2:05`, `1:02:03`.
pub fn fmt_durata(d: Duration) -> String {
    let s = d.as_secs();
    if s >= 3600 {
        format!("{}:{:02}:{:02}", s / 3600, (s % 3600) / 60, s % 60)
    } else {
        format!("{}:{:02}", s / 60, s % 60)
    }
}
