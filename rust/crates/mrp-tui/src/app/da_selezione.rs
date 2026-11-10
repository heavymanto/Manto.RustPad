//! Le azioni del menu del tasto destro che partono dal testo selezionato: filtrare le righe che lo hanno, escluderle, cercarlo.
//! Il testo e' quello selezionato, preso alla lettera (maiuscole comprese); le condizioni di una vista filtrata (livelli, date) restano.

use super::{App, FilterConditions};
use mrp_core::pattern::Mode;

/// La selezione piu' lunga (in byte) che serve da testo di un filtro o di una ricerca: oltre, e' un blocco da copiare, non una parola.
pub const SELEZIONE_MAX: u64 = 256;

impl App {
    /// Il testo selezionato, se serve da filtro o da ricerca: su una riga sola, non fatto di soli spazi e non piu' lungo di
    /// `SELEZIONE_MAX` byte. `None` altrimenti (senza selezione, a capo dentro, troppo lunga, in modalita' colonna).
    pub fn selection_as_text(&self) -> Option<String> {
        let (da, fino) = self.selection()?;
        if fino - da > SELEZIONE_MAX {
            return None;
        }
        let letto = self.doc.as_ref()?.read_range(da, fino - da);
        let testo = if self.enc.is_utf8() {
            String::from_utf8(letto).ok()?
        } else {
            self.enc.decode_lossy(&letto)
        };
        (!testo.contains(['\n', '\r']) && !testo.trim().is_empty()).then_some(testo)
    }

    /// "Filtra le righe con questo testo" (o, con `escludi`, "Escludi le righe con questo testo"): apre la vista filtrata col testo
    /// selezionato, come se fosse scritto nella finestra Filtri. I livelli e le date che gia' valgono (una vista filtrata, un filtro
    /// per data acceso) restano; il testo, il modo e le maiuscole li sostituisce la selezione. Senza una selezione utile non fa niente.
    pub fn filter_by_selection(&mut self, escludi: bool) {
        let Some(testo) = self.selection_as_text() else {
            return;
        };
        let mut c = match self.filter_view.as_ref().and_then(|v| v.conditions.clone()) {
            Some(c) => c,
            None => FilterConditions {
                levels: [false; 4],
                text: String::new(),
                mode: Mode::Text,
                case: true,
                invert: false,
                time: self.window.is_some(),
                from: self.range_text[0].clone(),
                to: self.range_text[1].clone(),
            },
        };
        c.text = testo;
        c.mode = Mode::Text;
        c.case = true;
        c.invert = escludi;
        self.apply_filter_conditions(&c);
    }

    /// "Cerca questo testo": la prossima occorrenza del testo selezionato (alla lettera, maiuscole comprese) dopo la selezione; F3 e
    /// Maiusc+F3 proseguono con lo stesso testo.
    pub fn find_selection(&mut self) {
        let (Some(testo), Some((da, _))) = (self.selection_as_text(), self.selection()) else {
            return;
        };
        self.find_from_selection(&testo, da, true);
    }

    /// Cerca il testo selezionato, alla lettera e maiuscole comprese: e' la ricerca nuova (F3 col testo selezionato,
    /// il "Cerca" del menu del tasto destro). In avanti parte dall'inizio della selezione, cosi' la salta; indietro
    /// da li', e trova quelle di prima.
    pub(crate) fn find_from_selection(&mut self, testo: &str, da: u64, forward: bool) {
        self.search_mode = Mode::Text;
        self.search_case = true;
        self.last_find = Some(testo.to_string());
        self.anchor = None;
        self.cursor = da;
        self.start_search(testo, forward);
    }
}
