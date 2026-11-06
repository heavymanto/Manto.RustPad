//! La timeline nel terminale: una colonna accanto al testo (a sinistra, o a destra se la preferenza e' quella) con una riga
//! per periodo: la data, e una barra lunga quanto il volume del periodo, divisa fra i livelli (ERROR, WARN, INFO, DEBUG). Il
//! tratto che si sta leggendo ha il filetto in risalto; un clic o un trascinamento porta il cursore a quel punto del file.
//! Qui c'e' solo cosa mostrare e dove si clicca (`ui.rs` lo disegna): il calcolo dei periodi e' il motore della timeline
//! (`timeline.rs`), lo stesso della finestra.

use super::timeline::TimelineStatus;
use super::App;
use mrp_core::timeline::Step;
use ratatui::crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use ratatui::text::Span;
use std::cmp::Reverse;

/// Le colonne del pannello, il filetto compreso.
pub(crate) const PANNELLO_W: u16 = 25;
/// La data o l'ora di un periodo (`aaaa-mm-gg` o `mm-gg hh:mm`).
pub(crate) const ETICHETTA_W: usize = 11;
/// Le celle della barra di un periodo (la piu' pesante le riempie tutte).
pub(crate) const BARRA_W: usize = 12;
/// Le colonne che restano al testo almeno: su un terminale piu' stretto il pannello non c'e'.
const TESTO_MIN: u16 = 40;

/// Una riga di schermo del pannello.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RigaPannello {
    /// La data del periodo, solo sulla prima riga di schermo che il periodo occupa (le altre, vuota).
    pub etichetta: String,
    /// Le celle piene della barra: ERROR, WARN, INFO, DEBUG e, ultime, quelle dei livelli non ancora contati. Tutte a zero:
    /// nessun periodo qui.
    pub celle: [usize; 5],
    /// Il tratto del file che si legge passa da questa riga.
    pub in_vista: bool,
}

/// Cosa c'e' nel corpo del pannello.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pannello {
    /// Un messaggio (le tacche si stanno cercando, non ce ne sono, sono troppe...), a righe gia' spezzate.
    Testo(Vec<String>),
    /// Una riga per ogni riga di schermo del corpo.
    Righe(Vec<RigaPannello>),
}

/// Le celle di una barra di `piene` celle divise fra i livelli secondo i `conti` (ERROR, WARN, INFO, DEBUG): in proporzione, col
/// resto ai livelli che ne hanno di piu', ma un livello presente non sparisce mai se c'e' posto (un errore fra mille righe di INFO
/// deve vedersi: e' la ragione per cui si guarda la barra). Tutto a zero se non ci sono righe con un livello.
pub fn celle_livelli(conti: [u64; 4], piene: usize) -> [usize; 4] {
    let totale: u64 = conti.iter().sum();
    let mut celle = [0usize; 4];
    if totale == 0 || piene == 0 {
        return celle;
    }
    // la parte intera di ciascuno, poi le celle che mancano ai livelli con il resto piu' grande (a parita' il piu' grave)
    let mut resti = [0u64; 4];
    for i in 0..4 {
        let prodotto = u128::from(conti[i]) * piene as u128;
        celle[i] = (prodotto / u128::from(totale)) as usize;
        resti[i] = (prodotto % u128::from(totale)) as u64;
    }
    let mut date: usize = celle.iter().sum();
    while date < piene {
        let Some(i) = (0..4)
            .filter(|&i| conti[i] > 0)
            .max_by_key(|&i| (resti[i], Reverse(i)))
        else {
            break;
        };
        celle[i] += 1;
        resti[i] = 0;
        date += 1;
    }
    // un livello presente ma senza celle ne prende una da quello che ne ha di piu' (a parita' il meno grave): da uno che ne ha
    // due, o anche l'unica di uno meno grave (a posto per tutti non c'e': prima i piu' gravi)
    for i in 0..4 {
        if conti[i] > 0 && celle[i] == 0 {
            let donatore = (0..4)
                .filter(|&j| j != i && (celle[j] >= 2 || (j > i && celle[j] >= 1)))
                .max_by_key(|&j| (celle[j], j));
            if let Some(j) = donatore {
                celle[j] -= 1;
                celle[i] = 1;
            }
        }
    }
    celle
}

/// Il testo spezzato in righe di al piu' `larghezza` colonne: alle parole, e dentro una parola piu' lunga della riga (il
/// giapponese non ha spazi) al carattere.
fn a_capo(testo: &str, larghezza: usize) -> Vec<String> {
    let colonne = |s: &str| Span::raw(s).width();
    let mut righe: Vec<String> = Vec::new();
    let mut corrente = String::new();
    for parola in testo.split_whitespace() {
        let pezzi: Vec<String> = if colonne(parola) <= larghezza {
            vec![parola.to_string()]
        } else {
            // la parola non entra in una riga: si taglia ai caratteri
            let mut tagli: Vec<String> = Vec::new();
            let mut pezzo = String::new();
            for c in parola.chars() {
                if colonne(&pezzo) + colonne(&c.to_string()) > larghezza && !pezzo.is_empty() {
                    tagli.push(std::mem::take(&mut pezzo));
                }
                pezzo.push(c);
            }
            tagli.push(pezzo);
            tagli
        };
        for (k, pezzo) in pezzi.into_iter().enumerate() {
            // un pezzo di una parola tagliata comincia sempre una riga nuova (dopo il primo)
            let attaccato = k == 0 && !corrente.is_empty();
            if attaccato && colonne(&corrente) + 1 + colonne(&pezzo) <= larghezza {
                corrente.push(' ');
                corrente.push_str(&pezzo);
            } else {
                if !corrente.is_empty() {
                    righe.push(std::mem::take(&mut corrente));
                }
                corrente = pezzo;
            }
        }
    }
    if !corrente.is_empty() {
        righe.push(corrente);
    }
    righe
}

/// L'intestazione del pannello: il nome e, mentre i livelli si contano, a che punto sono (`avanzamento`: da 0 a 1).
fn intestazione(nome: &str, avanzamento: Option<f32>) -> String {
    match avanzamento {
        Some(p) => format!(" {nome} {}%", (p.clamp(0.0, 1.0) * 100.0) as u32),
        None => format!(" {nome}"),
    }
}

/// Le colonne della legenda dei livelli (`█E▓W▒I░D`) a destra dell'intestazione.
pub(crate) const LEGENDA_W: usize = 8;
/// Le colonne che restano al titolo nell'intestazione: il pannello meno il filetto e meno la legenda.
const TITOLO_W: usize = PANNELLO_W as usize - 1 - LEGENDA_W;

/// L'intestazione col passo dei periodi: `nome passo` e, se si stanno contando i livelli, la percentuale. Se non entra nelle colonne
/// del titolo (`TITOLO_W`) cede prima il nome, poi la percentuale: il passo c'e' sempre.
fn titolo_con_passo(nome: &str, passo: &str, avanzamento: Option<f32>) -> String {
    [
        intestazione(&format!("{nome} {passo}"), avanzamento),
        intestazione(passo, avanzamento),
        intestazione(passo, None),
    ]
    .into_iter()
    .find(|t| Span::raw(t.as_str()).width() <= TITOLO_W)
    .unwrap_or_else(|| intestazione(passo, None))
}

impl App {
    /// Le colonne che il pannello occupa adesso: `PANNELLO_W` se la timeline e' accesa, c'e' la cornice del terminale (la finestra
    /// grafica ha la sua barra) e al testo restano almeno `TESTO_MIN` colonne; altrimenti 0.
    pub(crate) fn timeline_panel_w(&self) -> u16 {
        if self.chrome && self.timeline.on && self.w >= PANNELLO_W + TESTO_MIN {
            PANNELLO_W
        } else {
            0
        }
    }

    /// La colonna dello schermo da cui comincia il pannello.
    pub(crate) fn timeline_panel_x(&self) -> u16 {
        if self.timeline_pref.right {
            self.w - self.timeline_panel_w()
        } else {
            0
        }
    }

    /// La colonna dello schermo da cui comincia il testo (col gutter): dopo il pannello se sta a sinistra.
    pub(crate) fn text_x0(&self) -> u16 {
        if self.timeline_pref.right {
            0
        } else {
            self.timeline_panel_w()
        }
    }

    /// Dove sta il corpo del pannello: la riga dello schermo da cui comincia (sotto l'intestazione) e quante righe e'.
    pub(crate) fn timeline_panel_body(&self) -> (u16, usize) {
        (self.top_offset() + 1, self.text_rows().saturating_sub(1))
    }

    /// Il nome del passo in uso, con `~` davanti se lo sceglie la barra da sola (sono i nomi del menu della finestra).
    fn timeline_step_label(&self) -> String {
        // fisso: quello scelto (le tacche si ricalcolano un attimo dopo); automatico: quello che hanno le tacche
        if self.timeline_auto() {
            format!("~{}", self.timeline_step_name(self.timeline_step()))
        } else {
            self.timeline_step_name(self.timeline.step)
        }
    }

    /// L'intestazione del pannello: il nome, il passo dei periodi quando le tacche ci sono e, finche' i livelli non sono contati,
    /// a che punto sono. Se tutto insieme non entra nelle colonne del pannello, il nome cede il posto al passo.
    pub(crate) fn timeline_panel_title(&self) -> String {
        let pronta = self.timeline_status() == TimelineStatus::Ready;
        let avanzamento = if pronta {
            self.timeline_levels_progress()
        } else {
            None
        };
        let nome = self.text("gui.timeline");
        if !pronta {
            return intestazione(nome, avanzamento);
        }
        titolo_con_passo(nome, &self.timeline_step_label(), avanzamento)
    }

    /// Il nome di un passo scelto, per dirlo a chi preme Alt+P.
    fn timeline_step_name(&self, passo: Step) -> String {
        self.text(match passo {
            Step::Minute => "timeline.step.minute",
            Step::TenMinutes => "timeline.step.ten_minutes",
            Step::Hour => "timeline.step.hour",
            Step::Day => "timeline.step.day",
        })
        .to_string()
    }

    /// Alt+P: il passo dei periodi della timeline al successivo, nell'ordine automatico, 1 minuto, 10 minuti, 1 ora, 1 giorno e da
    /// capo. Dice quale e' diventato. Non fa niente se la timeline e' spenta.
    pub fn timeline_step_cycle(&mut self) {
        if !self.timeline_on() {
            return;
        }
        const GIRO: [Step; 4] = [Step::Minute, Step::TenMinutes, Step::Hour, Step::Day];
        let successivo = if self.timeline_auto() {
            Some(GIRO[0])
        } else {
            GIRO.iter()
                .position(|&s| s == self.timeline.step)
                .and_then(|i| GIRO.get(i + 1).copied())
        };
        let nome = match successivo {
            Some(passo) => {
                self.set_timeline_step(passo);
                self.timeline_step_name(passo)
            }
            None => {
                self.set_timeline_auto();
                self.text("timeline.step.auto").to_string()
            }
        };
        self.flash = Some(nome);
    }

    /// Alt+E: il filtro per livelli della timeline al successivo, nell'ordine nessuno, solo ERROR, ERROR e WARN, fino a INFO e da
    /// capo (DEBUG si prende con la finestra). Apre la vista con le sole righe scelte, o torna al file se non ne resta nessuno. Non
    /// fa niente se la timeline e' spenta.
    pub fn timeline_levels_cycle(&mut self) {
        if !self.timeline_on() {
            return;
        }
        let con = |n: usize| -> [bool; 4] { std::array::from_fn(|i| i < n) };
        let attuale = self.timeline_filter_levels();
        let n = (0..=3).find(|&n| attuale == con(n));
        // fuori dal giro (scelti altrove, per esempio DEBUG da solo) si riparte da solo ERROR
        let prossimo = match n {
            Some(3) => 0,
            Some(n) => n + 1,
            None => 1,
        };
        let nuovi = con(prossimo);
        for (i, &scelto) in nuovi.iter().enumerate() {
            self.set_timeline_filter_level(i, scelto);
        }
        if prossimo == 0 {
            self.filter_view_back_if_no_levels();
        } else {
            self.timeline_apply_level_filter();
        }
    }

    /// Il corpo del pannello; `None` se la timeline e' spenta.
    pub(crate) fn timeline_panel(&self) -> Option<Pannello> {
        let testo = |chiave: &str, argomenti: &[&str]| {
            Pannello::Testo(a_capo(
                &self.text_with(chiave, argomenti),
                usize::from(PANNELLO_W) - 2,
            ))
        };
        match self.timeline_status() {
            TimelineStatus::Off => None,
            TimelineStatus::Building(n) => Some(testo("timeline.building", &[&n.to_string()])),
            TimelineStatus::NoTimes => Some(testo("timeline.no_times", &[])),
            TimelineStatus::TooMany(n) => Some(testo("timeline.too_many", &[&n.to_string()])),
            TimelineStatus::Unavailable => Some(testo("timeline.unavailable", &[])),
            TimelineStatus::Ready => Some(Pannello::Righe(self.timeline_panel_rows())),
        }
    }

    /// Le righe di schermo del corpo quando le tacche ci sono: ogni riga prende il periodo che copre il suo centro (un periodo
    /// puo' occupare piu' righe, e allora la barra si ripete e la data sta solo sulla prima).
    fn timeline_panel_rows(&self) -> Vec<RigaPannello> {
        let (_, h) = self.timeline_panel_body();
        if h == 0 {
            return Vec::new();
        }
        let righe = self.timeline_rows(h);
        let vista = self.timeline_viewport(h);
        let mut fuori: Vec<RigaPannello> = Vec::with_capacity(h);
        let mut precedente: Option<usize> = None;
        for i in 0..h {
            let (alto, basso) = (i as f32 / h as f32, (i + 1) as f32 / h as f32);
            let centro = (i as f32 + 0.5) / h as f32;
            // il tratto che si legge tocca questa riga (anche se e' piu' stretto di una riga)
            let in_vista = vista.is_some_and(|(v0, v1)| v0 < basso && v1 > alto);
            let indice = righe
                .iter()
                .position(|r| r.y1 > r.y0 && centro >= r.y0 && centro < r.y1);
            let Some(k) = indice else {
                precedente = None;
                fuori.push(RigaPannello {
                    etichetta: String::new(),
                    celle: [0; 5],
                    in_vista,
                });
                continue;
            };
            let r = &righe[k];
            let piene = if r.bytes == 0 {
                0
            } else {
                ((r.weight * BARRA_W as f32).ceil() as usize).clamp(1, BARRA_W)
            };
            let mut celle = [0usize; 5];
            if r.levels_known {
                let conti =
                    [r.levels.error, r.levels.warn, r.levels.info, r.levels.debug].map(u64::from);
                let divise = celle_livelli(conti, piene);
                celle[..4].copy_from_slice(&divise);
                // righe senza livello: la barra c'e' lo stesso, di un colore solo
                if divise.iter().sum::<usize>() == 0 {
                    celle[4] = piene;
                }
            } else {
                celle[4] = piene;
            }
            let prima = precedente != Some(k);
            fuori.push(RigaPannello {
                etichetta: if prima {
                    r.label.clone()
                } else {
                    String::new()
                },
                celle,
                in_vista,
            });
            precedente = Some(k);
        }
        fuori
    }

    /// Il mouse sul pannello: un clic o un trascinamento col sinistro porta il cursore al punto del file che sta a quell'altezza.
    /// Vero se l'evento era del pannello (e il testo non lo deve vedere); un trascinamento cominciato li' resta suo anche se il
    /// puntatore esce.
    pub(super) fn timeline_panel_mouse(&mut self, m: MouseEvent) -> bool {
        let larghezza = self.timeline_panel_w();
        if larghezza == 0 {
            self.panel_drag = false;
            return false;
        }
        let (prima, h) = self.timeline_panel_body();
        match m.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                let x = self.timeline_panel_x();
                let sopra = self.top_offset();
                let dentro = self.dialog.is_none()
                    && m.column >= x
                    && m.column < x + larghezza
                    && m.row >= sopra
                    && usize::from(m.row - sopra) < self.text_rows();
                if !dentro {
                    return false;
                }
                // l'intestazione non fa niente (ma il clic e' del pannello: non porta il cursore del testo)
                if m.row < prima {
                    return true;
                }
                self.panel_drag = true;
                self.timeline_panel_jump(m.row, prima, h);
                true
            }
            MouseEventKind::Drag(MouseButton::Left) if self.panel_drag => {
                self.timeline_panel_jump(m.row, prima, h);
                true
            }
            MouseEventKind::Up(MouseButton::Left) if self.panel_drag => {
                self.panel_drag = false;
                true
            }
            _ => false,
        }
    }

    /// Salta al punto che sta alla riga di schermo `riga` del corpo (che comincia a `prima` ed e' alto `h`); trascinando, sopra
    /// il corpo vale la prima riga e sotto l'ultima.
    fn timeline_panel_jump(&mut self, riga: u16, prima: u16, h: usize) {
        if h == 0 {
            return;
        }
        let i = usize::from(riga.saturating_sub(prima)).min(h - 1);
        self.timeline_jump((i as f32 + 0.5) / h as f32, h);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_header_shows_the_step_and_gives_up_the_name_then_the_percentage() {
        // il titolo ha 16 colonne (il pannello ne ha 25: il filetto e la legenda dei livelli stanno fuori)
        assert_eq!(
            titolo_con_passo("Timeline", "1 day", None),
            " Timeline 1 day"
        );
        assert_eq!(titolo_con_passo("Timeline", "~1 hour", None), " ~1 hour");
        // con la percentuale cede il nome, poi la percentuale stessa
        assert_eq!(
            titolo_con_passo("Timeline", "1 day", Some(0.5)),
            " 1 day 50%"
        );
        assert_eq!(
            titolo_con_passo("Timeline", "~10 minutes", Some(1.0)),
            " ~10 minutes"
        );
        // 16 colonne entrano, 17 no
        assert_eq!(
            titolo_con_passo("Timeline", "1 hour", None),
            " Timeline 1 hour"
        );
        assert_eq!(
            titolo_con_passo("Timeline", "~1 hour", None)
                .chars()
                .count(),
            8
        );
        assert_eq!(
            titolo_con_passo("Timelin", "~1 hour", None),
            " Timelin ~1 hour"
        );
    }

    #[test]
    fn the_bar_is_split_by_proportion_with_the_rest_to_the_largest() {
        // 12 celle, 5/3/2 su 10 -> 6, 3,6 e 2,4: le parti intere sono 6+3+2 = 11, la cella che manca va al resto piu' grande (WARN)
        assert_eq!(celle_livelli([5, 3, 2, 0], 12), [6, 4, 2, 0]);
        // nessun resto: 8 celle, 4/2/2/0 -> 4,2,2
        assert_eq!(celle_livelli([4, 2, 2, 0], 8), [4, 2, 2, 0]);
    }

    #[test]
    fn a_level_that_is_present_keeps_at_least_one_cell() {
        // un errore su mille righe: 12 celle, la proporzione darebbe 0 ma l'errore si deve vedere
        assert_eq!(celle_livelli([1, 0, 999, 0], 12), [1, 0, 11, 0]);
        // due livelli piccoli: prendono una cella a testa dal grande
        assert_eq!(celle_livelli([1, 1, 998, 0], 12), [1, 1, 10, 0]);
    }

    #[test]
    fn a_bar_too_short_for_every_level_serves_the_most_serious_first() {
        // 2 celle e tre livelli presenti: ERROR e WARN vincono, INFO resta senza (nessuno ha due celle da cedere)
        assert_eq!(celle_livelli([1, 1, 98, 0], 2), [1, 1, 0, 0]);
    }

    #[test]
    fn no_levels_or_no_cells_give_nothing() {
        assert_eq!(celle_livelli([0, 0, 0, 0], 12), [0; 4]);
        assert_eq!(celle_livelli([3, 2, 1, 0], 0), [0; 4]);
    }

    #[test]
    fn the_cells_always_add_up_to_the_bar_when_there_is_a_level() {
        for piene in 1..=12usize {
            for e in 0..4u64 {
                for w in 0..4u64 {
                    for i in 0..4u64 {
                        let c = celle_livelli([e, w, i, 1], piene);
                        assert_eq!(c.iter().sum::<usize>(), piene, "{e}/{w}/{i} su {piene}");
                    }
                }
            }
        }
    }

    #[test]
    fn a_tie_goes_to_the_more_serious_level() {
        // 3 celle fra ERROR e WARN uguali: 1 e 1 di parte intera (resto 1 a testa), la cella che manca va a ERROR
        assert_eq!(celle_livelli([1, 1, 0, 0], 3), [2, 1, 0, 0]);
        // ERROR da solo fra due uguali: 12 su 1/50/50 -> 0, 5, 5 (resti 12, 95, 95): le due celle che mancano a INFO e DEBUG, poi
        // ERROR prende una cella da chi ne ha di piu' e, a parita' (6 e 6), dal meno grave: DEBUG
        assert_eq!(celle_livelli([1, 0, 50, 50], 12), [1, 0, 6, 5]);
    }

    #[test]
    fn the_title_shows_the_progress_of_the_levels_only_while_it_runs() {
        assert_eq!(intestazione("Timeline", None), " Timeline");
        assert_eq!(intestazione("Timeline", Some(0.0)), " Timeline 0%");
        assert_eq!(intestazione("Timeline", Some(0.375)), " Timeline 37%");
        assert_eq!(intestazione("Timeline", Some(1.0)), " Timeline 100%");
        // fuori scala: tenuto fra 0 e 100
        assert_eq!(intestazione("Timeline", Some(1.5)), " Timeline 100%");
        assert_eq!(intestazione("Timeline", Some(-0.5)), " Timeline 0%");
    }

    #[test]
    fn a_message_is_wrapped_at_words_and_cuts_a_word_that_is_too_long() {
        // una parola larga quanto la riga entra, una colonna di piu' no
        assert_eq!(a_capo("abcd", 4), vec!["abcd"]);
        assert_eq!(a_capo("abcde", 4), vec!["abcd", "e"]);
        // "Looking for" sono 11 colonne: non entra in 10
        assert_eq!(
            a_capo("Looking for the times", 10),
            vec!["Looking", "for the", "times"]
        );
        assert_eq!(a_capo("Looking for", 11), vec!["Looking for"]);
        assert_eq!(a_capo("abcdefghij", 4), vec!["abcd", "efgh", "ij"]);
        assert_eq!(a_capo("", 10), Vec::<String>::new());
        // il giapponese: due colonne per carattere
        assert_eq!(a_capo("日本語日本語", 6), vec!["日本語", "日本語"]);
    }
}
