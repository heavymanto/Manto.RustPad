//! L'app del TUI: una macchina a stati testabile senza terminale (il contratto sta in `docs/TEAM/DESIGN_TUI.md`).
//! Riceve tasti, mouse e dimensioni dello schermo; non disegna (lo fa `ui.rs`); tutta la logica di testo e' nel core.

use crate::settings::{Settings, JUMP_PAGES};
use crate::theme::ThemeId;
use mrp_core::block::{
    block_spans, block_text, delete_block, insert_block, nth_row, paste_block, BlockError,
    ColumnBlock, RowSpan,
};
use mrp_core::docsearch::{SearchError, Snapshot};
use mrp_core::encoding::Encoding;
use mrp_core::lang::{Catalog, Lang};
use mrp_core::lineending::LineEnding;
use mrp_core::newline::Newline;
use mrp_core::pattern::{Edit, Mode, Pattern, Replacement};
use mrp_core::scan::{self, Next};
use mrp_core::session::TabRecord;
use mrp_core::view::{self, ViewOptions};
use mrp_core::{Document, FileSource, LineIndex, RestoreError, SaveError};
use ratatui::crossterm::event::{
    KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::text::Span;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Quanto si guarda avanti e indietro per gli a-capo: il limite di scansione del core.
const MAX_SCAN: u64 = scan::DEFAULT_MAX_SCAN;
/// Il tempo massimo fra due clic perche' siano un doppio clic.
const MULTI_CLICK: Duration = Duration::from_millis(500);
/// Quanti caratteri al massimo una parola selezionata con il doppio clic si allarga per lato (righe enormi).
const MAX_WORD_CHARS: usize = 100_000;

/// Le righe della rotella per scatto.
const WHEEL_ROWS: usize = 3;

/// Quante cifre servono per scrivere `n` in decimale (almeno 1): la larghezza dei numeri di riga.
fn cifre(n: u64) -> usize {
    let mut conto = 1;
    let mut v = n;
    while v >= 10 {
        v /= 10;
        conto += 1;
    }
    conto
}

/// Un numero intero non negativo scritto in cifre ASCII: `None` se e' vuoto, contiene altro o non ci sta in un
/// `u64` (la grammatica del "vai a").
fn parse_cifre(s: &str) -> Option<u64> {
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    s.parse().ok()
}

/// La larghezza in celle di `testo` come se iniziasse alla colonna `col`: un tabulatore avanza fino al
/// prossimo multiplo della larghezza dei tabulatori, un carattere di controllo vale 1 e un carattere
/// che la codifica non definisce e' un carattere tutto suo largo 1 (come nel modello di
/// `mrp_core::view`); per il resto la larghezza Unicode (i caratteri larghi 2, i combinanti 0). I byte
/// si leggono come caratteri con `enc` (`Encoding::first_char`). E' la colonna a cui arriva il testo
/// scritto sul blocco della modalita' colonna.
fn larghezza_da(enc: Encoding, testo: &[u8], col: u64) -> u64 {
    let tab = ViewOptions::default().tab_width.max(1) as u64;
    let mut colonna = col;
    let mut i = 0;
    while i < testo.len() {
        // il primo carattere e quanti byte occupa; qui lo slice non e' mai vuoto (lo esclude il
        // while), ma un ripiego non puo' fare danno
        let (c, n) = enc.first_char(&testo[i..]).unwrap_or(('\u{fffd}', 1));
        colonna = colonna.saturating_add(match c {
            '\t' => tab - colonna % tab,
            c if c.is_control() => 1,
            c => {
                let mut buf = [0u8; 4];
                let s: &str = c.encode_utf8(&mut buf);
                Span::raw(s).width() as u64
            }
        });
        i += n;
    }
    colonna.saturating_sub(col)
}

/// Vero se il documento finisce con un a-capo della sua forma: la lunghezza e' multipla dell'unita'
/// e l'ultima unita' e' l'a-capo (un byte finale spaiato, in UTF-16, non lo e' mai). Il documento
/// vuoto no. Serve ai limiti del cursore in lettura e al numero dell'ultima riga.
fn finisce_con_a_capo(doc: &Document) -> bool {
    let newline = doc.newline();
    let unit = newline.unit() as u64;
    let len = doc.len();
    len >= unit && len.is_multiple_of(unit) && newline.is_newline(&doc.read_range(len - unit, unit))
}

/// Fa partire il thread che costruisce l'indice righe del file `path` con la forma dell'a-capo
/// `newline` (quella del documento: in UTF-16 l'a-capo e' un'unita' a 16 bit allineata): apre la
/// SUA sorgente (il documento non si condivide mentre indicizza) e consegna l'esito sul canale.
/// E' il meccanismo dell'apertura, riusato dopo un salvataggio riuscito (il file su disco e'
/// cambiato) e dopo un cambio di forma dell'a-capo.
fn spawn_index_job(
    path: &Path,
    newline: Newline,
    cancel: &Arc<AtomicBool>,
    progress: &Arc<AtomicU64>,
) -> IndexJob {
    let (tx, rx) = mpsc::channel();
    let path = path.to_path_buf();
    let cancel_thread = Arc::clone(cancel);
    let progress_thread = Arc::clone(progress);
    std::thread::spawn(move || {
        let esito = FileSource::open(&path).and_then(|src| {
            LineIndex::build_for(&src, newline, &cancel_thread, &progress_thread)
                .map_err(|_| io::Error::other("indicizzazione annullata"))
        });
        let _ = tx.send(esito);
    });
    IndexJob {
        rx,
        progress: Arc::clone(progress),
    }
}

/// La ricerca completa su una fotografia del documento: prima da `da` nella direzione voluta e,
/// se non c'e' niente, dall'altra parte (in avanti si riparte dall'inizio, indietro dalla
/// fine); il secondo esito porta l'avviso di giro completo. Un errore di lettura vale come
/// "non trovata", l'annullamento si riporta perche' il chiamante lo dice all'utente.
fn search_all(
    snap: &Snapshot,
    ago: &Needle,
    forward: bool,
    da: u64,
    cancel: &AtomicBool,
    progress: &AtomicU64,
) -> SearchOutcome {
    let len = snap.len();
    // Da dove si riparte se la prima ricerca non trova niente: l'altra parte del documento.
    let ripartenza = if forward { 0 } else { len };
    let primo = cerca(snap, ago, forward, da, cancel, progress);
    match primo {
        // Trovata senza bisogno del giro completo: nessun avviso.
        Ok(Some((pos, lunga))) => SearchOutcome::Found(pos, lunga, false),
        Ok(None) => {
            // Non c'e' niente da questa parte: si riprova dall'altra.
            let secondo = cerca(snap, ago, forward, ripartenza, cancel, progress);
            match secondo {
                Ok(Some((pos, lunga))) => SearchOutcome::Found(pos, lunga, true),
                Ok(None) => SearchOutcome::NotFound,
                Err(SearchError::Cancelled) => SearchOutcome::Cancelled,
                Err(SearchError::Io(_)) => SearchOutcome::NotFound,
            }
        }
        Err(SearchError::Cancelled) => SearchOutcome::Cancelled,
        Err(SearchError::Io(_)) => SearchOutcome::NotFound,
    }
}

/// Cosa si cerca: byte esatti (la ricerca di sempre, qualunque codifica) o un `Pattern` (glob, regex, senza maiuscole;
/// solo documenti UTF-8).
enum Needle {
    Literal(Vec<u8>),
    Pattern(Pattern),
}

/// Una ricerca da `da` nella direzione voluta: inizio e lunghezza dell'occorrenza.
fn cerca(
    snap: &Snapshot,
    ago: &Needle,
    forward: bool,
    da: u64,
    cancel: &AtomicBool,
    progress: &AtomicU64,
) -> Result<Option<(u64, u64)>, SearchError> {
    match ago {
        Needle::Literal(b) => {
            let r = if forward {
                snap.find_next(b, da, cancel, progress)
            } else {
                snap.find_prev(b, da, cancel, progress)
            };
            r.map(|o| o.map(|p| (p, b.len() as u64)))
        }
        Needle::Pattern(p) => {
            if forward {
                snap.find_pattern_next(p, da, cancel, progress)
            } else {
                snap.find_pattern_prev(p, da, cancel, progress)
            }
        }
    }
}

/// Fa partire il thread che cerca `ago` nella fotografia del documento e consegna l'esito sul
/// canale: il meccanismo dell'indicizzazione e del salvataggio (cancel e progress condivisi).
fn spawn_search_job(
    snap: Snapshot,
    ago: Needle,
    testo: &str,
    forward: bool,
    da: u64,
    cancel: &Arc<AtomicBool>,
    progress: &Arc<AtomicU64>,
) -> SearchJob {
    let (tx, rx) = mpsc::channel();
    let cancel_thread = Arc::clone(cancel);
    let progress_thread = Arc::clone(progress);
    let len = snap.len();
    std::thread::spawn(move || {
        let esito = search_all(&snap, &ago, forward, da, &cancel_thread, &progress_thread);
        let _ = tx.send(esito);
    });
    SearchJob {
        rx,
        progress: Arc::clone(progress),
        cancel: Arc::clone(cancel),
        len,
        needle: testo.to_string(),
    }
}

/// Configurazione di partenza dell'app (i test la costruiscono a mano, il binario la ricava da argomenti e profilo).
#[derive(Debug, Clone)]
pub struct AppConfig {
    /// Tema iniziale.
    pub theme: ThemeId,
    /// Codice della lingua da usare (`None` o sconosciuto: inglese).
    pub lang_code: Option<String>,
    /// Le lingue disponibili (di norma `mrp_core::lang::registry()`); il predefinito e' solo l'inglese incorporato.
    pub langs: Vec<Lang>,
    /// Dove salvare le impostazioni quando si cambia tema o lingua; `None`: non si salva niente.
    pub settings_path: Option<PathBuf>,
    /// Vero: costruisce l'indice righe in background.
    pub index: bool,
    /// La codifica dei caratteri: `Some(e)` quella, `None` la riconosce all'apertura dai primi byte del file
    /// (`Encoding::detect`).
    pub encoding: Option<Encoding>,
    /// Di quante pagine saltano Alt+PgUp/PgDn, Alt+Maiusc+PgUp/PgDn e Ctrl+Alt+PgUp/PgDn (vedi `Settings`).
    pub jump_pages: [usize; 3],
}

impl Default for AppConfig {
    fn default() -> Self {
        AppConfig {
            theme: ThemeId::DEFAULT,
            lang_code: None,
            langs: vec![Lang::english()],
            settings_path: None,
            index: true,
            encoding: Some(Encoding::Utf8),
            jump_pages: JUMP_PAGES,
        }
    }
}

/// Il dialogo aperto (se c'e'): tiene i tasti per se' finche' non si chiude.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogKind {
    /// F1: elenco dei tasti.
    Help,
    /// F6: menu dei temi.
    Theme,
    /// F7: menu delle lingue.
    Lang,
    /// F9: menu delle codifiche.
    Encoding,
    /// F5 / Ctrl+G: prompt "vai a".
    Goto,
    /// F3 / Ctrl+F: prompt di ricerca.
    Find,
    /// Ctrl+H: prompt "sostituisci" (cerca, con, opzioni; anteprima prima di applicare).
    Replace,
    /// Ctrl+O: prompt del percorso di un file da aprire (lo apre chi ospita l'app: `take_open_request`).
    Open,
    /// Conferma all'uscita con modifiche non salvate.
    Confirm,
    /// Salvataggio in corso (avanzamento, Esc annulla).
    Saving,
}

/// L'indicizzazione in corso: il canale col thread che costruisce l'indice e i byte gia' esaminati.
struct IndexJob {
    /// Il risultato del thread: l'indice pronto, o l'errore di apertura della sorgente.
    rx: Receiver<io::Result<LineIndex>>,
    /// Byte di file gia' esaminati, per la percentuale nella riga di stato.
    progress: Arc<AtomicU64>,
}

/// Il salvataggio in corso: il canale col thread che tiene il documento, i byte gia' scritti e l'ordine di
/// uscire dopo un salvataggio riuscito.
struct SaveJob {
    /// Il risultato del thread: il documento (torna comunque, anche con errore: non si perde mai) e l'esito
    /// del salvataggio.
    rx: Receiver<(Document, Result<(), SaveError>)>,
    /// Byte di file gia' scritti, per la percentuale nella riga di stato e nella barra del dialogo.
    progress: Arc<AtomicU64>,
    /// Chiede al thread di annullare (lo fa Esc nel dialogo `Saving`).
    cancel: Arc<AtomicBool>,
    /// La lunghezza del documento quando il salvataggio e' partito: il totale della percentuale.
    len: u64,
    /// Vero: dopo un salvataggio riuscito l'app esce (la 'S' del dialogo di uscita).
    quit_after: bool,
}

/// L'esito della ricerca in background, consegnato dal thread su `tick`.
enum SearchOutcome {
    /// Trovata: l'inizio dell'occorrenza e se l'ha raggiunta solo dopo il giro completo del
    /// documento.
    Found(u64, u64, bool),
    /// Non trovata in nessuna delle due direzioni (un errore di lettura vale come non trovata).
    NotFound,
    /// Annullata con Esc mentre girava.
    Cancelled,
}

/// La ricerca in corso: il canale col thread che cerca nella fotografia, i byte gia' esaminati
/// e l'ago (serve per la fine dell'occorrenza trovata e per il messaggio di "non trovato").
struct SearchJob {
    /// Il risultato del thread.
    rx: Receiver<SearchOutcome>,
    /// Byte gia' esaminati, per la percentuale nella riga di stato.
    progress: Arc<AtomicU64>,
    /// Chiede al thread di fermarsi (lo fa Esc mentre la ricerca gira).
    cancel: Arc<AtomicBool>,
    /// La lunghezza della fotografia quando la ricerca e' partita: il totale della percentuale.
    len: u64,
    /// L'ago cercato: il testo del prompt, per i messaggi.
    needle: String,
}

/// Il massimo di occorrenze che una sostituzione conta o applica in una volta.
pub const MAX_REPLACE: usize = 2_000_000;

/// Cosa chiede il dialogo "Sostituisci": cosa cercare, con cosa, e come si legge il testo cercato.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplaceSpec {
    pub find: String,
    pub with: String,
    pub mode: Mode,
    /// Vero: le maiuscole contano.
    pub case: bool,
}

/// La richiesta che esce dal dialogo con Invio: prima si conta (`apply` falso: l'anteprima), poi si applica.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplaceAsk {
    pub spec: ReplaceSpec,
    pub apply: bool,
    /// Vero: su tutte le schede (lo esegue il `Workspace`).
    pub all_tabs: bool,
}

/// L'esito di una sostituzione su un documento.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplaceDone {
    /// L'anteprima: quante occorrenze.
    Counted(u64),
    /// Piu' di `MAX_REPLACE`.
    TooMany,
    /// Applicata: quante occorrenze.
    Applied(usize),
    Cancelled,
    Failed(String),
}

/// Cosa il thread della sostituzione consegna.
enum ReplaceOutcome {
    Count(u64),
    Edits(Vec<Edit>),
    TooMany,
    Cancelled,
    Failed(String),
}

/// La sostituzione in corso su questo documento.
struct ReplaceJob {
    rx: Receiver<ReplaceOutcome>,
    progress: Arc<AtomicU64>,
    cancel: Arc<AtomicBool>,
    len: u64,
}

/// La ricerca pronta per una sostituzione: byte esatti o un `Pattern`.
enum Resolved {
    Literal { needle: Vec<u8>, with: Vec<u8> },
    Pattern { p: Pattern, r: Replacement },
}

/// Il lavoro del thread della sostituzione: contare (`apply` falso) o preparare le sostituzioni.
fn replace_work(
    snap: &Snapshot,
    ris: &Resolved,
    apply: bool,
    cancel: &AtomicBool,
    progress: &AtomicU64,
) -> ReplaceOutcome {
    let fin = |e: SearchError| match e {
        SearchError::Cancelled => ReplaceOutcome::Cancelled,
        SearchError::Io(e) => ReplaceOutcome::Failed(e.to_string()),
    };
    match ris {
        Resolved::Literal { needle, with } => {
            match snap.find_all_literal(needle, MAX_REPLACE, cancel, progress) {
                Err(e) => fin(e),
                Ok(Err(_)) => ReplaceOutcome::TooMany,
                Ok(Ok(v)) if !apply => ReplaceOutcome::Count(v.len() as u64),
                Ok(Ok(v)) => ReplaceOutcome::Edits(
                    v.into_iter()
                        .map(|start| Edit {
                            start,
                            len: needle.len() as u64,
                            new: with.clone(),
                        })
                        .collect(),
                ),
            }
        }
        Resolved::Pattern { p, r } => {
            if !apply {
                return match snap.count_pattern(p, MAX_REPLACE as u64, cancel, progress) {
                    Err(e) => fin(e),
                    Ok(n) if n > MAX_REPLACE as u64 => ReplaceOutcome::TooMany,
                    Ok(n) => ReplaceOutcome::Count(n),
                };
            }
            match snap.replace_edits(p, r, MAX_REPLACE, cancel, progress) {
                Err(e) => fin(e),
                Ok(Err(_)) => ReplaceOutcome::TooMany,
                Ok(Ok(v)) => ReplaceOutcome::Edits(v),
            }
        }
    }
}

/// Byte massimi che una copia (Ctrl+C / Ctrl+X) puo' mettere negli appunti interni: oltre, il messaggio
/// `flash.copy_too_big` e niente copia (la memoria non deve dipendere dalla dimensione del file).
pub const COPY_LIMIT: u64 = 64 << 20;

/// Colonna virtuale massima del cursore in modalita' colonna (E18): oltre, il cursore non va (e il riempimento di
/// una riga corta non puo' diventare enorme).
pub const MAX_VCOL: u64 = 4096;

/// Righe massime del blocco della modalita' colonna (E18): oltre, il messaggio `flash.block_too_big` e niente
/// modifica ne' copia (`block_spans` si chiama sempre con questo limite).
const MAX_BLOCK_ROWS: u64 = 200_000;

/// Lo stato dell'app.
pub struct App {
    /// Il documento aperto: `None` quando si esce (il file si rilascia) o mentre un salvataggio lo tiene
    /// nel suo thread.
    doc: Option<Document>,
    /// Il nome del file, per il titolo.
    file_name: String,
    /// Il percorso canonico del file aperto (non cambia mentre il documento e' in mano al salvataggio).
    path: PathBuf,
    /// Offset del primo byte della prima riga visuale dello schermo (sempre un inizio di riga visuale).
    top: u64,
    /// Offset in byte del cursore (un confine di carattere).
    cursor: u64,
    /// La colonna ricordata per i movimenti verticali (celle di terminale).
    want_col: u64,
    /// La prima colonna visibile (scorrimento orizzontale).
    hscroll: usize,
    /// Dimensioni dello schermo in celle.
    w: u16,
    h: u16,
    /// Vero in modifica.
    edit: bool,
    /// Il messaggio transitorio della riga di stato (sparisce al primo tasto successivo).
    flash: Option<String>,
    /// L'ultima occorrenza trovata dalla ricerca (inizio, fine esclusa): come il messaggio
    /// transitorio, sparisce al primo tasto successivo che non sia F3 / Maiusc+F3.
    found: Option<(u64, u64)>,
    /// L'ancora della selezione: l'altro estremo e' il cursore. Resta finche' la selezione e' attiva (anche
    /// se torna vuota); la tolgono Esc, i movimenti senza Maiusc, i salti riusciti e le modifiche.
    anchor: Option<u64>,
    /// Vero se la modalita' colonna (F8) e' accesa: Maiusc+frecce disegnano un blocco rettangolare di
    /// celle invece della selezione, e scrivere agisce su tutte le righe del blocco.
    col_mode: bool,
    /// Il blocco rettangolare della modalita' colonna, se c'e' (solo a modalita' accesa).
    block: Option<BlockState>,
    /// Gli appunti interni: li riempie la copia (Ctrl+C / Ctrl+X) e li incolla Ctrl+V.
    clip: Vec<u8>,
    /// I byte dell'ultima copia che il ciclo del terminale deve ancora mandare con OSC 52, una volta sola.
    copied: Option<Vec<u8>>,
    /// Il vocabolario attivo.
    cat: Catalog,
    /// Il tema attivo.
    theme: ThemeId,
    /// Chiede al thread dell'indice di fermarsi (lo fa `Drop`).
    cancel: Arc<AtomicBool>,
    /// L'indicizzazione in corso, se c'e'.
    index_job: Option<IndexJob>,
    /// Vero: la forma dell'a-capo del documento e' cambiata (un cambio di codifica fra UTF-16 e il
    /// resto) e l'indice righe va rifatto con la forma nuova; lo fa il primo `tick`, cosi' il
    /// messaggio del cambio di codifica resta da solo nella riga di stato.
    index_pending: bool,
    /// Il salvataggio in corso, se c'e': il documento e' nel suo thread e qui non c'e'.
    save_job: Option<SaveJob>,
    /// La ricerca in corso, se c'e': gira su una fotografia, il documento non si muove.
    search_job: Option<SearchJob>,
    /// Le righe di testo dell'ultimo disegno (gutter e testo, gia' pronte): le ridisegna `ui::draw`
    /// mentre un salvataggio tiene il documento nel suo thread.
    drawn_rows: Vec<(String, String)>,
    /// Dove comincia ogni riga disegnata (lo stesso ordine di `drawn_rows`): serve a portare un clic del mouse a un
    /// offset del documento.
    row_starts: Vec<u64>,
    /// Dove e' cominciato il trascinamento del mouse con il tasto sinistro (l'ancora della selezione che si fa).
    drag_from: Option<u64>,
    /// L'ultimo clic sinistro (istante, colonna, riga, quanti di fila): serve al doppio e al triplo clic.
    last_click: Option<(Instant, u16, u16, u8)>,
    /// Vero quando si e' chiesto di uscire.
    quit: bool,
    /// Dove sta il cursore sullo schermo dopo l'ultimo disegno (lo scrive `ui::draw`).
    cursor_xy: Option<(u16, u16)>,
    /// Il dialogo aperto: tiene tutti i tasti per se' finche' non si chiude.
    dialog: Option<DialogKind>,
    /// La voce selezionata nel menu aperto (tema o lingua).
    menu_index: usize,
    /// Il testo scritto nel prompt "vai a".
    goto_text: String,
    /// Il testo del prompt "apri" e la richiesta confermata (percorso) che l'ospite deve ancora raccogliere.
    open_text: String,
    open_request: Option<PathBuf>,
    /// Il testo scritto nel prompt di ricerca.
    find_text: String,
    /// L'ultimo testo cercato: F3 e Maiusc+F3 lo cercano di nuovo, senza prompt.
    last_find: Option<String>,
    /// Come si legge il testo cercato (testo, glob, regex) e se le maiuscole contano: vale per la ricerca e per Sostituisci.
    search_mode: Mode,
    search_case: bool,
    /// Il dialogo "Sostituisci": i due campi, quale ha il cursore (0 cerca, 1 con), se vale per tutte le schede, lo
    /// stato (l'anteprima) e se l'anteprima e' pronta (il prossimo Invio applica).
    rep_find: String,
    rep_with: String,
    rep_field: u8,
    rep_all: bool,
    rep_status: String,
    rep_ready: bool,
    replace_job: Option<ReplaceJob>,
    replace_ask: Option<ReplaceAsk>,
    replace_done: Option<ReplaceDone>,
    /// Le lingue disponibili: il menu di F7 e i cambi del `Catalog`.
    langs: Vec<Lang>,
    /// Dove salvare le impostazioni quando si cambia tema o lingua; `None`: non si salva niente.
    settings_path: Option<PathBuf>,
    /// I moltiplicatori dei salti di pagina (vedi `AppConfig::jump_pages`).
    jump_pages: [usize; 3],
    /// Il codice della lingua scelta dall'utente (`None`: l'inglese incorporato): e' la `lang` delle impostazioni.
    lang_picked: Option<String>,
    /// Vero: l'indice righe si costruisce in background (all'apertura e dopo un salvataggio riuscito).
    index: bool,
    /// La codifica con cui i byte del documento si leggono e si scrivono come caratteri.
    enc: Encoding,
    /// Lo stile di fine riga del file (da un campione dell'inizio, all'apertura): lo scrive Invio e lo mostra la riga di stato.
    eol: LineEnding,
    /// Quanti byte ha il BOM all'inizio del file nella codifica attiva (0 se non c'e'): il cursore puo' stargli davanti
    /// in lettura, ma scrivere o cancellare davanti al BOM non si fa (lo romperebbe).
    bom: u64,
}

/// Un movimento del cursore: quello del tasto senza Maiusc. La selezione con Maiusc fa lo stesso movimento
/// del tasto semplice (colonna ricordata, limite del cursore e scorrimento compresi).
enum Mov {
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    FileStart,
    FileEnd,
    PageUp,
    PageDown,
}

/// Il blocco rettangolare della modalita' colonna. Le righe si identificano con l'inizio della prima
/// (`first_row`) e quante sono (`rows`), perche' dopo una modifica gli inizi delle righe successive si
/// spostano mentre `first_row` no: il resto si ricalcola camminando il documento. L'ancora sta a
/// `anchor_col` su una riga estrema del blocco, il cursore a `vcol` (colonna virtuale: puo' stare oltre
/// la fine della riga) sull'altra.
#[derive(Debug, Clone, Copy)]
struct BlockState {
    /// Inizio (offset in byte) della prima riga del blocco.
    first_row: u64,
    /// Quante righe visuali copre il blocco.
    rows: u64,
    /// Vero se il cursore sta sull'ultima riga del blocco e l'ancora sulla prima; falso: il contrario.
    cursor_at_bottom: bool,
    /// La colonna dell'ancora.
    anchor_col: u64,
    /// La colonna virtuale del cursore (da 0 a `MAX_VCOL`).
    vcol: u64,
}

impl App {
    /// Apre `path`. Errore se manca o non e' un file regolare.
    pub fn open(path: &Path, cfg: AppConfig) -> io::Result<App> {
        let mut doc = Document::open(path)?;
        let path = doc.path().to_path_buf();
        let file_name = doc
            .path()
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        // La lingua attiva: quella col codice richiesto fra le disponibili, altrimenti l'inglese incorporato. La
        // scelta conta come "fatta dall'utente" (per le impostazioni) solo se il codice esiste davvero.
        let (cat, lang_picked) = match &cfg.lang_code {
            Some(code) => match cfg.langs.iter().find(|l| &l.code == code) {
                Some(l) => (Catalog::with(l.clone()), Some(code.clone())),
                None => (Catalog::english(), None),
            },
            None => (Catalog::english(), None),
        };
        // La codifica: quella richiesta, o quella che l'inizio del file suggerisce (i primi 64 KiB,
        // con `Encoding::detect`). I byte del documento non cambiano: cambia solo come si leggono.
        // Serve prima dell'indice: la forma dell'a-capo della codifica (in UTF-16 un'unita' a 16
        // bit) decide come si contano le righe.
        let campione = doc.read_range(0, doc.len().min(65536));
        let enc = cfg.encoding.unwrap_or_else(|| Encoding::detect(&campione));
        // Lo stile di fine riga (LF, CRLF, CR da solo) dal campione e dalla codifica: decide la forma
        // dell'a-capo del documento (il CR da solo ha la sua) e cosa scrive Invio.
        let eol = LineEnding::detect(&campione, enc).for_encoding(enc);
        let bom = enc.bom_len(&campione) as u64;
        // La forma dell'a-capo della codifica, al documento appena aperto: senza storia di
        // modifiche non puo' fallire (il risultato bool si ignora per quello).
        let newline = eol.newline_for(enc);
        doc.set_newline(newline);
        let cancel = Arc::new(AtomicBool::new(false));
        let progress = Arc::new(AtomicU64::new(0));
        let index_job = if cfg.index {
            // Il thread apre la SUA sorgente sullo stesso file: il documento non si condivide mentre indicizza.
            Some(spawn_index_job(doc.path(), newline, &cancel, &progress))
        } else {
            None
        };
        Ok(App {
            doc: Some(doc),
            file_name,
            path,
            top: 0,
            cursor: 0,
            want_col: 0,
            hscroll: 0,
            w: 80,
            h: 24,
            edit: false,
            flash: None,
            found: None,
            anchor: None,
            col_mode: false,
            block: None,
            clip: Vec::new(),
            copied: None,
            cat,
            theme: cfg.theme,
            cancel,
            index_job,
            index_pending: false,
            save_job: None,
            search_job: None,
            drawn_rows: Vec::new(),
            row_starts: Vec::new(),
            drag_from: None,
            last_click: None,
            quit: false,
            cursor_xy: None,
            dialog: None,
            menu_index: 0,
            goto_text: String::new(),
            open_text: String::new(),
            open_request: None,
            find_text: String::new(),
            last_find: None,
            search_mode: Mode::Text,
            search_case: true,
            rep_find: String::new(),
            rep_with: String::new(),
            rep_field: 0,
            rep_all: false,
            rep_status: String::new(),
            rep_ready: false,
            replace_job: None,
            replace_ask: None,
            replace_done: None,
            langs: cfg.langs,
            settings_path: cfg.settings_path,
            jump_pages: cfg.jump_pages,
            lang_picked,
            index: cfg.index,
            enc,
            eol,
            bom,
        })
    }

    /// Nuove dimensioni dello schermo; il cursore resta visibile.
    pub fn resize(&mut self, w: u16, h: u16) {
        self.w = w;
        self.h = h;
        // Un salvataggio tiene il documento: cursore, `top` e `hscroll` restano com'erano (senza documento
        // non si possono riaggiustare); tornato il documento li riaggiusta la fine del salvataggio.
        if self.doc.is_none() && self.save_job.is_some() {
            return;
        }
        self.clamp_cursor();
        self.scroll_adjust();
    }

    /// Un tasto premuto.
    pub fn handle_key(&mut self, key: KeyEvent) {
        // Il messaggio transitorio sparisce al primo tasto premuto dopo, prima di eseguirlo.
        self.flash = None;
        // Come il messaggio transitorio, l'evidenziazione dell'ultima ricerca si azzera al
        // primo tasto premuto dopo, prima di eseguirlo: la rimette solo la ricerca che parte
        // con questo tasto (F3 / Maiusc+F3).
        self.found = None;
        // Una ricerca in corso tiene TUTTI i tasti tranne Esc, che chiede al thread di
        // annullarla: finche' gira il documento non si tocca (la fotografia resta valida).
        if self.search_job.is_some() || self.replace_job.is_some() {
            if key.code == KeyCode::Esc {
                if let Some(job) = &self.search_job {
                    job.cancel.store(true, Ordering::Relaxed);
                }
                if let Some(job) = &self.replace_job {
                    job.cancel.store(true, Ordering::Relaxed);
                }
            }
            return;
        }
        // Un dialogo aperto riceve TUTTI i tasti: il documento non si muove finche' non si chiude.
        if self.dialog.is_some() {
            self.dialog_key(key);
            return;
        }
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let alt = key.modifiers.contains(KeyModifiers::ALT);
        let shift = key.modifiers.contains(KeyModifiers::SHIFT);
        // I tasti che non continuano la scrittura chiudono il passo della storia prima di agire: i caratteri
        // e i Backspace di seguito sono un passo solo, Enter allunga il passo corrente e poi lo chiude da
        // solo.
        let continua = !ctrl
            && !alt
            && matches!(
                key.code,
                KeyCode::Char(_) | KeyCode::Enter | KeyCode::Backspace
            );
        if !continua {
            self.seal_history();
        }
        match key.code {
            // Con Maiusc il movimento estende la selezione: senza ancora la si mette al cursore, poi il
            // movimento e' quello del tasto semplice. Con Ctrl+Maiusc, Home ed End vanno agli estremi del file.
            KeyCode::Up if shift => self.extend_selection(Mov::Up),
            KeyCode::Down if shift => self.extend_selection(Mov::Down),
            KeyCode::Left if shift => self.extend_selection(Mov::Left),
            KeyCode::Right if shift => self.extend_selection(Mov::Right),
            KeyCode::Home if ctrl && shift => self.extend_selection(Mov::FileStart),
            KeyCode::Home if shift => self.extend_selection(Mov::Home),
            KeyCode::End if ctrl && shift => self.extend_selection(Mov::FileEnd),
            KeyCode::End if shift => self.extend_selection(Mov::End),
            // Alt+Pag: salti di piu' pagine (x10; con Maiusc x100; con Ctrl x1000, i moltiplicatori sono nelle impostazioni).
            // Sono spostamenti semplici: tolgono la selezione.
            KeyCode::PageUp if alt => self.jump_key(false, ctrl, shift),
            KeyCode::PageDown if alt => self.jump_key(true, ctrl, shift),
            KeyCode::PageUp if shift => self.extend_selection(Mov::PageUp),
            KeyCode::PageDown if shift => self.extend_selection(Mov::PageDown),
            // Senza Maiusc i movimenti tolgono l'ancora: Left e Right con una selezione ci collassano
            // sopra (all'inizio e alla fine, senza muoversi oltre), gli altri si muovono dal cursore.
            KeyCode::Up => self.move_no_anchor(Mov::Up),
            KeyCode::Down => self.move_no_anchor(Mov::Down),
            KeyCode::Left => self.collapse_left(),
            KeyCode::Right => self.collapse_right(),
            KeyCode::Home if ctrl => self.move_no_anchor(Mov::FileStart),
            KeyCode::Home => self.move_no_anchor(Mov::Home),
            KeyCode::End if ctrl => self.move_no_anchor(Mov::FileEnd),
            KeyCode::End => self.move_no_anchor(Mov::End),
            KeyCode::PageUp => self.move_no_anchor(Mov::PageUp),
            KeyCode::PageDown => self.move_no_anchor(Mov::PageDown),
            // Esc toglie l'ancora: la selezione collassa sulla sua fine.
            KeyCode::Esc => self.collapse_escape(),
            KeyCode::F(1) => self.open_dialog(DialogKind::Help),
            KeyCode::F(2) => self.start_save(false),
            // F3 cerca la prossima occorrenza dell'ultimo testo cercato, Maiusc+F3 la
            // precedente; senza un testo cercato si apre il prompt.
            KeyCode::F(3) if shift => self.find_key(false),
            KeyCode::F(3) => self.find_key(true),
            KeyCode::F(4) => self.toggle_edit(),
            KeyCode::F(5) => self.open_dialog(DialogKind::Goto),
            KeyCode::F(6) => self.open_dialog(DialogKind::Theme),
            KeyCode::F(7) => self.open_dialog(DialogKind::Lang),
            // F8 accende e spegne la modalita' colonna.
            KeyCode::F(8) => self.toggle_column(),
            // F9 apre il menu delle codifiche (con un altro dialogo aperto non fa nulla: i tasti
            // arrivano al dialogo, come per gli altri menu).
            KeyCode::F(9) => self.open_dialog(DialogKind::Encoding),
            KeyCode::F(10) => self.quit_request(),
            // Ctrl+Z annulla; Ctrl+Y e Ctrl+Shift+Z ripetono (col maiuscolo SHIFT arriva anche 'Z').
            KeyCode::Char('z') | KeyCode::Char('Z') if ctrl && !shift => self.undo(),
            KeyCode::Char('z') | KeyCode::Char('Z') if ctrl => self.redo(),
            KeyCode::Char('y') | KeyCode::Char('Y') if ctrl => self.redo(),
            KeyCode::Char('s') if ctrl => self.start_save(false),
            // Ctrl+F apre sempre il prompt di ricerca, con l'ultimo testo cercato gia' scritto.
            KeyCode::Char('f') if ctrl => self.open_dialog(DialogKind::Find),
            KeyCode::Char('g') if ctrl => self.open_dialog(DialogKind::Goto),
            KeyCode::Char('o') if ctrl => self.open_dialog(DialogKind::Open),
            KeyCode::Char('h') if ctrl => self.open_dialog(DialogKind::Replace),
            KeyCode::Char('q') if ctrl => self.quit_request(),
            // Ctrl+A seleziona tutto; Ctrl+C copia, Ctrl+X taglia e Ctrl+V incolla gli appunti interni.
            KeyCode::Char('a') if ctrl => self.select_all(),
            KeyCode::Char('c') if ctrl => {
                self.copy_to_clip();
            }
            KeyCode::Char('x') if ctrl => self.cut_selection(),
            KeyCode::Char('v') if ctrl => self.paste_clipboard(),
            // I tasti di scrittura: un carattere (anche con SHIFT) si inserisce al cursore, Enter mette un
            // a-capo, Tab un tabulatore, Backspace e Delete tolgono il carattere prima o sotto il cursore;
            // con una selezione agiscono su quella (un carattere, Enter e Tab la sostituiscono, Backspace e
            // Delete la tolgono e basta). In lettura sono rifiutati con un suggerimento.
            KeyCode::Char(c) if !ctrl && !alt => self.insert_char(c),
            KeyCode::Enter if !ctrl && !alt => self.insert_enter(),
            KeyCode::Backspace if !ctrl && !alt => self.backspace_key(),
            KeyCode::Delete if !ctrl && !alt => self.delete_key(),
            KeyCode::Tab if !ctrl && !alt => self.insert_tab(),
            _ => {}
        }
    }

    /// Un evento del mouse; anche il mouse chiude il passo della storia.
    pub fn handle_mouse(&mut self, m: MouseEvent) {
        self.handle_mouse_at(m, Instant::now());
    }

    /// Come `handle_mouse`, con l'istante dell'evento (i test lo passano a mano; i clic ravvicinati nella stessa
    /// cella fanno il doppio e il triplo clic).
    pub fn handle_mouse_at(&mut self, m: MouseEvent, adesso: Instant) {
        self.seal_history();
        match m.kind {
            MouseEventKind::ScrollDown => self.wheel(true),
            MouseEventKind::ScrollUp => self.wheel(false),
            // il tasto sinistro: un clic porta il cursore li' (con Maiusc estende la selezione), trascinare seleziona;
            // il secondo clic di fila seleziona la parola, il terzo la riga
            MouseEventKind::Down(MouseButton::Left) => {
                let maiusc = m.modifiers.contains(KeyModifiers::SHIFT);
                let posato = self.click(m.column, m.row, maiusc, false);
                self.multi_click(m.column, m.row, maiusc, adesso, posato);
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                self.click(m.column, m.row, true, true);
            }
            MouseEventKind::Up(MouseButton::Left) => self.drag_from = None,
            _ => {}
        }
    }

    /// Un clic (o un trascinamento) del tasto sinistro sulla cella (`col`, `riga`) dello schermo: il cursore va al
    /// carattere che sta li' (una colonna dentro un carattere largo da' il suo inizio, una oltre la fine della riga la fine
    /// della riga, il gutter l'inizio della riga). Con `estendi` (Maiusc, o trascinando) la selezione parte dal cursore
    /// di prima (dal punto del clic, trascinando) e arriva li'. Senza testo disegnato sotto il punto un clic non fa
    /// niente; un trascinamento oltre l'ultima riga arriva all'ultima. Ignorato con un dialogo aperto e in modalita'
    /// colonna.
    fn click(&mut self, col: u16, riga: u16, estendi: bool, trascina: bool) -> bool {
        if self.dialog.is_some() || self.col_mode || self.row_starts.is_empty() {
            return false;
        }
        let Some(doc) = self.doc.as_ref() else {
            return false;
        };
        // la riga 0 e' il titolo (un clic li' non fa niente, un trascinamento che ci arriva sta sulla prima riga);
        // le righe di testo cominciano alla 1
        if riga == 0 && !trascina {
            return false;
        }
        let mut indice = usize::from(riga).saturating_sub(1);
        if indice >= self.row_starts.len() {
            if !trascina {
                return false;
            }
            indice = self.row_starts.len() - 1;
        }
        let colonna = self.hscroll + usize::from(col).saturating_sub(self.gutter_w());
        let opts = ViewOptions {
            show_eof_row: self.edit,
            ..self.view_opts()
        };
        let offset = view::offset_at_col(doc, self.row_starts[indice], colonna as u64, &opts)
            .min(self.max_cursor());
        if trascina {
            if self.anchor.is_none() {
                self.anchor = Some(self.drag_from.unwrap_or(self.cursor));
            }
        } else if estendi {
            if self.anchor.is_none() {
                self.anchor = Some(self.cursor);
            }
        } else {
            self.anchor = None;
            self.drag_from = Some(offset);
        }
        self.cursor = offset;
        self.found = None;
        self.remember_col();
        self.scroll_adjust();
        true
    }

    /// Il secondo clic sinistro di fila nella stessa cella (entro `MULTI_CLICK`) seleziona la parola sotto il cursore, il
    /// terzo la riga; il quarto ricomincia da uno. Un clic che non porta il cursore (fuori dal testo, con un dialogo aperto,
    /// in modalita' colonna) o con Maiusc (estende) non conta e azzera la serie.
    fn multi_click(&mut self, col: u16, riga: u16, maiusc: bool, adesso: Instant, posato: bool) {
        if maiusc || !posato {
            self.last_click = None;
            return;
        }
        let n = match self.last_click {
            Some((t, c, r, n))
                if c == col
                    && r == riga
                    && n < 3
                    && adesso.saturating_duration_since(t) <= MULTI_CLICK =>
            {
                n + 1
            }
            _ => 1,
        };
        self.last_click = Some((adesso, col, riga, n));
        match n {
            2 => self.select_word(),
            3 => self.select_row(),
            _ => {}
        }
    }

    /// Il carattere che comincia a `off` e la sua lunghezza in byte (`None` oltre la fine del documento).
    fn char_after(&self, off: u64) -> Option<(char, u64)> {
        let doc = self.doc.as_ref()?;
        let restanti = doc.len().checked_sub(off)?.min(4);
        let buf = doc.read_range(off, restanti);
        self.enc.first_char(&buf).map(|(c, n)| (c, n as u64))
    }

    /// Il carattere che finisce a `off` e la sua lunghezza in byte (`None` all'inizio del documento).
    fn char_before(&self, off: u64) -> Option<(char, u64)> {
        let doc = self.doc.as_ref()?;
        if off == 0 {
            return None;
        }
        let da = off.saturating_sub(self.enc.back_window() as u64);
        let buf = doc.read_range(da, off - da);
        let n = self.enc.last_char_len(&buf);
        let (c, _) = self.enc.first_char(&buf[buf.len() - n..])?;
        Some((c, n as u64))
    }

    /// Doppio clic: seleziona la parola (lettere, cifre e '_') sotto il cursore; su spazi o segni la sequenza di
    /// caratteri della stessa specie; a fine riga l'ultima parola. Resta sulla riga e su caratteri interi.
    fn select_word(&mut self) {
        let Some(doc) = self.doc.as_ref() else {
            return;
        };
        let off = self.cursor;
        let riga = scan::row_start(doc, off, MAX_SCAN).0;
        let fine = scan::row_end(doc, riga, MAX_SCAN);
        let dopo = if off < fine {
            self.char_after(off)
        } else {
            None
        };
        let prima = if off > riga {
            self.char_before(off)
        } else {
            None
        };
        let Some((c, _)) = dopo.or(prima) else {
            return;
        };
        let classe = char_class(c);
        let (mut da, mut a) = (off, off);
        for _ in 0..MAX_WORD_CHARS {
            match self.char_before(da) {
                Some((c, n)) if da > riga && char_class(c) == classe => da -= n,
                _ => break,
            }
        }
        for _ in 0..MAX_WORD_CHARS {
            match self.char_after(a) {
                Some((c, n)) if a < fine && char_class(c) == classe => a += n,
                _ => break,
            }
        }
        self.select_span(da, a);
    }

    /// Triplo clic: seleziona la riga sotto il cursore, senza il suo a-capo.
    fn select_row(&mut self) {
        let Some(doc) = self.doc.as_ref() else {
            return;
        };
        let riga = scan::row_start(doc, self.cursor, MAX_SCAN).0;
        let fine = scan::row_end(doc, riga, MAX_SCAN);
        self.select_span(riga, fine);
    }

    /// Mette la selezione su `da..a` (il cursore alla fine); vuota non fa niente.
    fn select_span(&mut self, da: u64, a: u64) {
        let a = a.min(self.max_cursor());
        if da >= a {
            return;
        }
        self.anchor = Some(da);
        self.cursor = a;
        self.found = None;
        self.remember_col();
        self.scroll_adjust();
    }

    /// Si esce subito (niente modifiche da salvare): il documento si rilascia e il file torna libero,
    /// l'indicizzazione in corso si annulla.
    fn quit_now(&mut self) {
        self.quit = true;
        self.doc = None;
        self.index_job = None;
        self.cancel.store(true, Ordering::Relaxed);
    }

    /// F10 / Ctrl+Q: l'uscita. Con modifiche non salvate chiede prima (dialogo `Confirm`), senza esce
    /// subito.
    fn quit_request(&mut self) {
        if self.is_modified() {
            self.open_dialog(DialogKind::Confirm);
        } else {
            self.quit_now();
        }
    }

    // ----- dialoghi -----

    /// Apre un dialogo: l'aiuto, i menu (che partono dalla voce corrente) o il prompt "vai a" (vuoto).
    fn open_dialog(&mut self, kind: DialogKind) {
        match kind {
            DialogKind::Theme => {
                self.menu_index = ThemeId::ALL
                    .iter()
                    .position(|&t| t == self.theme)
                    .unwrap_or(0);
            }
            DialogKind::Lang => {
                self.menu_index = self
                    .langs
                    .iter()
                    .position(|l| l.code == self.cat.code())
                    .unwrap_or(0);
            }
            DialogKind::Encoding => {
                // Come gli altri menu: parte dalla codifica corrente.
                self.menu_index = Encoding::all()
                    .iter()
                    .position(|&e| e == self.enc)
                    .unwrap_or(0);
            }
            DialogKind::Goto => self.goto_text.clear(),
            DialogKind::Open => self.open_text.clear(),
            DialogKind::Replace => {
                self.rep_find = self.last_find.clone().unwrap_or_default();
                self.rep_field = 0;
                self.rep_status.clear();
                self.rep_ready = false;
            }
            // Il prompt di ricerca riparte dall'ultimo testo cercato (vuoto se non c'e').
            DialogKind::Find => self.find_text = self.last_find.clone().unwrap_or_default(),
            _ => {}
        }
        self.dialog = Some(kind);
    }

    /// I tasti di un dialogo aperto: valgono solo i suoi, tutto il resto si ignora.
    fn dialog_key(&mut self, key: KeyEvent) {
        let Some(kind) = self.dialog else {
            return;
        };
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let alt = key.modifiers.contains(KeyModifiers::ALT);
        match kind {
            // Aiuto: Esc, F1 e Enter chiudono.
            DialogKind::Help => match key.code {
                KeyCode::Esc | KeyCode::Enter | KeyCode::F(1) => self.dialog = None,
                _ => {}
            },
            // Menu (tema e lingua): le frecce scelgono (ferme agli estremi), Enter applica, Esc annulla.
            DialogKind::Theme | DialogKind::Lang => match key.code {
                KeyCode::Up => self.menu_index = self.menu_index.saturating_sub(1),
                KeyCode::Down => {
                    if self.menu_index + 1 < self.menu_len() {
                        self.menu_index += 1;
                    }
                }
                KeyCode::Enter => self.menu_apply(),
                KeyCode::Esc => self.dialog = None,
                _ => {}
            },
            // Menu delle codifiche: come tema e lingua, ma Home e End vanno alla prima e all'ultima
            // voce (la lista e' lunga e scorre: la finestra la tiene visibile il disegno).
            DialogKind::Encoding => match key.code {
                KeyCode::Up => self.menu_index = self.menu_index.saturating_sub(1),
                KeyCode::Down => {
                    if self.menu_index + 1 < self.menu_len() {
                        self.menu_index += 1;
                    }
                }
                KeyCode::Home => self.menu_index = 0,
                KeyCode::End => self.menu_index = self.menu_len().saturating_sub(1),
                KeyCode::Enter => self.menu_apply(),
                KeyCode::Esc => self.dialog = None,
                _ => {}
            },
            // Prompt "vai a": i caratteri si scrivono, Backspace toglie, Enter conferma, Esc annulla.
            DialogKind::Goto => match key.code {
                KeyCode::Esc => self.dialog = None,
                KeyCode::Enter => {
                    let testo = std::mem::take(&mut self.goto_text);
                    self.dialog = None;
                    self.goto_run(&testo);
                }
                KeyCode::Backspace => {
                    self.goto_text.pop();
                }
                KeyCode::Char(c) if !ctrl && !alt => self.goto_text.push(c),
                _ => {}
            },
            // Prompt "apri": come il "vai a"; Enter lascia il percorso all'ospite (un testo vuoto non apre niente).
            DialogKind::Open => match key.code {
                KeyCode::Esc => self.dialog = None,
                KeyCode::Enter => {
                    let testo = std::mem::take(&mut self.open_text);
                    self.dialog = None;
                    let testo = testo.trim();
                    if !testo.is_empty() {
                        self.open_request = Some(PathBuf::from(testo));
                    }
                }
                KeyCode::Backspace => {
                    self.open_text.pop();
                }
                KeyCode::Char(c) if !ctrl && !alt => self.open_text.push(c),
                _ => {}
            },
            // Prompt di ricerca: gli stessi tasti del "vai a"; Enter ricorda il testo e fa
            // partire la ricerca in avanti, Esc chiude senza toccare l'ultimo testo cercato.
            // Sostituisci: Tab cambia campo; Alt+M cambia modo, Alt+C le maiuscole, Alt+A l'ambito; Invio fa l'anteprima e,
            // se e' pronta, applica (ogni modifica al testo o alle opzioni toglie l'anteprima).
            DialogKind::Replace => match key.code {
                KeyCode::Esc => self.dialog = None,
                KeyCode::Tab | KeyCode::Down | KeyCode::Up => self.rep_field ^= 1,
                KeyCode::Enter => {
                    if self.rep_find.is_empty() {
                        self.rep_status = self.cat.t("replace.empty").to_string();
                    } else {
                        self.replace_ask = Some(ReplaceAsk {
                            spec: self.replace_spec(),
                            apply: self.rep_ready,
                            all_tabs: self.rep_all,
                        });
                    }
                }
                KeyCode::Backspace => {
                    let campo = if self.rep_field == 0 {
                        &mut self.rep_find
                    } else {
                        &mut self.rep_with
                    };
                    campo.pop();
                    self.rep_reset();
                }
                KeyCode::Char('m') | KeyCode::Char('M') if alt => {
                    self.cycle_mode();
                    self.rep_reset();
                }
                KeyCode::Char('c') | KeyCode::Char('C') if alt => {
                    self.search_case = !self.search_case;
                    self.rep_reset();
                }
                KeyCode::Char('a') | KeyCode::Char('A') if alt => {
                    self.rep_all = !self.rep_all;
                    self.rep_reset();
                }
                KeyCode::Char(c) if !ctrl && !alt => {
                    if self.rep_field == 0 {
                        self.rep_find.push(c);
                    } else {
                        self.rep_with.push(c);
                    }
                    self.rep_reset();
                }
                _ => {}
            },
            DialogKind::Find => match key.code {
                KeyCode::Esc => self.dialog = None,
                KeyCode::Char('m') | KeyCode::Char('M') if alt => self.cycle_mode(),
                KeyCode::Char('c') | KeyCode::Char('C') if alt => {
                    self.search_case = !self.search_case
                }
                KeyCode::Enter => {
                    let testo = std::mem::take(&mut self.find_text);
                    self.dialog = None;
                    self.find_run(&testo);
                }
                KeyCode::Backspace => {
                    self.find_text.pop();
                }
                KeyCode::Char(c) if !ctrl && !alt => self.find_text.push(c),
                _ => {}
            },
            // Conferma all'uscita con modifiche non salvate: S salva e poi esce, D esce scartando, C o Esc
            // annullano (maiuscole comprese).
            DialogKind::Confirm => match key.code {
                KeyCode::Char('s') | KeyCode::Char('S') => {
                    self.dialog = None;
                    self.start_save(true);
                }
                KeyCode::Char('d') | KeyCode::Char('D') => {
                    self.dialog = None;
                    self.quit_now();
                }
                KeyCode::Char('c') | KeyCode::Char('C') | KeyCode::Esc => self.dialog = None,
                _ => {}
            },
            // Salvataggio in corso: solo Esc, che chiede al thread di annullare (il dialogo resta aperto
            // finche' il lavoro non torna, col suo esito).
            DialogKind::Saving if key.code == KeyCode::Esc => {
                if let Some(job) = &self.save_job {
                    job.cancel.store(true, Ordering::Relaxed);
                }
            }
            // Ogni altro tasto del salvataggio in corso si ignora (solo Esc fa qualcosa).
            _ => {}
        }
    }

    /// Quante voci ha il menu aperto.
    fn menu_len(&self) -> usize {
        match self.dialog {
            Some(DialogKind::Theme) => ThemeId::ALL.len(),
            Some(DialogKind::Lang) => self.langs.len(),
            Some(DialogKind::Encoding) => Encoding::all().len(),
            _ => 0,
        }
    }

    /// Enter in un menu: applica la scelta, chiude e salva le impostazioni. La lingua cambia il `Catalog` PRIMA
    /// del messaggio, cosi' `flash.lang` e' nella nuova lingua.
    fn menu_apply(&mut self) {
        match self.dialog {
            Some(DialogKind::Theme) => {
                let Some(&tema) = ThemeId::ALL.get(self.menu_index) else {
                    return;
                };
                self.theme = tema;
                self.dialog = None;
                self.flash = Some(self.cat.tf("flash.theme", &[self.cat.t(tema.key())]));
                self.save_settings();
            }
            Some(DialogKind::Lang) => {
                let Some(l) = self.langs.get(self.menu_index).cloned() else {
                    return;
                };
                let nome = l.name.clone();
                self.cat = Catalog::with(l);
                self.lang_picked = Some(self.cat.code().to_string());
                self.dialog = None;
                self.flash = Some(self.cat.tf("flash.lang", &[&nome]));
                self.save_settings();
            }
            Some(DialogKind::Encoding) => {
                let Some(&e) = Encoding::all().get(self.menu_index) else {
                    return;
                };
                self.dialog = None;
                // La scelta non si salva nelle impostazioni: la codifica e' del file, non dell'utente.
                self.set_encoding(e);
            }
            _ => {}
        }
    }

    /// Scrive le impostazioni (il tema e la lingua scelta) se c'e' un posto dove salvarle. Un errore di scrittura
    /// si ignora: il cambiamento resta valido per questa sessione.
    fn save_settings(&mut self) {
        let Some(path) = self.settings_path.clone() else {
            return;
        };
        let s = Settings {
            theme: self.theme,
            lang: self.lang_picked.clone(),
            jump_pages: self.jump_pages,
        };
        let _ = s.save(&path);
    }

    /// Esegue il testo del prompt "vai a" (il dialogo e' gia' chiuso): `N` = riga N (1-based, serve l'indice
    /// pronto), `@N` = offset in byte, `%P` = percentuale 0..=100. Qualunque altra cosa, o in errore, il cursore
    /// non si muove.
    fn goto_run(&mut self, testo: &str) {
        // Testo vuoto: chiuso senza chiedere niente.
        if testo.is_empty() {
            return;
        }
        if let Some(resto) = testo.strip_prefix('@') {
            if let Some(n) = parse_cifre(resto) {
                // Oltre la fine: l'ultima posizione consentita dal modo.
                self.goto_move(n.min(self.max_cursor()));
                return;
            }
        } else if let Some(resto) = testo.strip_prefix('%') {
            if let Some(p) = parse_cifre(resto) {
                if p <= 100 {
                    let len = self.document_len();
                    self.goto_move(len.saturating_mul(p) / 100);
                    return;
                }
            }
        } else if let Some(n) = parse_cifre(testo) {
            if n == 0 {
                self.flash = Some(self.cat.tf("flash.goto_bad", &[testo]));
                return;
            }
            // Il numero di riga serve l'indice pronto; offset e percentuale no.
            if !self.doc.as_ref().is_some_and(|doc| doc.has_lines()) {
                self.flash = Some(self.cat.t("flash.goto_wait").to_string());
                return;
            }
            let Some(conteggio) = self.doc.as_mut().and_then(|doc| doc.line_count()) else {
                return;
            };
            if n > conteggio {
                self.flash = Some(self.cat.tf(
                    "flash.goto_range",
                    &[&n.to_string(), &conteggio.to_string()],
                ));
                return;
            }
            let Some(pos) = self.doc.as_mut().and_then(|doc| doc.offset_of_line(n - 1)) else {
                return;
            };
            self.goto_move(pos);
            return;
        }
        self.flash = Some(self.cat.tf("flash.goto_bad", &[testo]));
    }

    /// Un "vai a" riuscito: il cursore va su un confine di carattere e la sua riga si vede (centrata se non si
    /// vedeva gia').
    fn goto_move(&mut self, pos: u64) {
        let pos = self.snap_to_char(pos.min(self.max_cursor()));
        self.cursor = pos;
        // Il salto riuscito toglie la selezione: il cursore e' arrivato altrove per conto suo.
        // Toglie anche il blocco della modalita' colonna.
        self.anchor = None;
        self.block = None;
        self.remember_col();
        self.center_cursor_row();
        self.scroll_adjust();
    }

    /// Centraggio del "vai a": se la riga del cursore non e' visibile, `top` diventa l'inizio della riga visuale
    /// `righe_di_testo / 2` righe PRIMA di quella del cursore (non prima dell'inizio del file), poi limitato
    /// all'ultima pagina.
    fn center_cursor_row(&mut self) {
        let text_rows = self.text_rows();
        if text_rows == 0 {
            return;
        }
        let row = self.cursor_row();
        if row == self.top || self.rows_after(self.top, text_rows - 1).contains(&row) {
            return;
        }
        let mut top = self
            .rows_before(row, text_rows / 2)
            .last()
            .copied()
            .unwrap_or(row);
        let last = self.last_top();
        if top > last {
            top = last;
        }
        self.top = top;
    }

    /// Porta `pos` sul confine di carattere piu' vicino non oltre `pos`: un carattere UTF-8 valido occupa al piu'
    /// quattro byte, quindi bastano tre passi indietro (un byte di prosecuzione `0b10xxxxxx` sta dentro un
    /// carattere multibyte, un confine non lo e' mai).
    fn snap_to_char(&self, pos: u64) -> u64 {
        let Some(doc) = self.doc.as_ref() else {
            return pos;
        };
        let mut pos = pos.min(doc.len());
        for _ in 0..3 {
            if pos == 0 {
                break;
            }
            let sotto = doc.read_range(pos, 1);
            match sotto.first() {
                Some(b) if (b & 0b1100_0000) == 0b1000_0000 => pos -= 1,
                _ => break,
            }
        }
        pos
    }

    /// Porta `pos` all'inizio del carattere che lo contiene (la codifica attiva e' una multibyte asiatica): si
    /// guardano i byte prima fino a un inizio sicuro (`Encoding::char_start`) e quelli dopo per la lunghezza.
    fn snap_to_multibyte(&self, pos: u64) -> u64 {
        let Some(doc) = self.doc.as_ref() else {
            return pos;
        };
        let pos = pos.min(doc.len());
        let da = pos.saturating_sub(self.enc.back_window() as u64);
        let fine = pos.saturating_add(4).min(doc.len());
        let ctx = doc.read_range(da, fine - da);
        let i = (pos - da) as usize;
        if i >= ctx.len() {
            return pos;
        }
        da + self.enc.char_start(&ctx, i) as u64
    }

    /// Porta `pos` sul confine di unita' piu' vicino non oltre `pos` (la codifica attiva e' a 16 o a
    /// 32 bit): prima si arrotonda per difetto a un multiplo della dimensione dell'unita' (un'unita'
    /// comincia sempre a una posizione multipla), poi, in UTF-16, se `pos` sta sul surrogato BASSO di
    /// una coppia (il carattere che finisce li' e' lungo 4), si torna indietro di 2, al suo inizio.
    /// In UTF-32 ogni carattere e' una sola unita'.
    fn snap_to_unita(&self, pos: u64) -> u64 {
        let Some(doc) = self.doc.as_ref() else {
            return pos;
        };
        let mut pos = pos.min(doc.len());
        pos -= pos % self.enc.unit() as u64;
        // La coppia che contiene l'unita' di `pos` comincia 2 byte prima: se c'e' davvero (un'alta
        // seguita da una bassa, come la vede `first_char`), `pos` sta a meta' del carattere e si
        // torna al suo inizio.
        if self.enc.is_utf16() && pos >= 2 && pos + 2 <= doc.len() {
            let buf = doc.read_range(pos - 2, 4);
            if self.enc.first_char(&buf).is_some_and(|(_, n)| n == 4) {
                pos -= 2;
            }
        }
        pos
    }

    // ----- selezione e appunti -----

    /// Ctrl+A: seleziona tutto: l'ancora a 0 e il cursore al limite del modo (in lettura non oltre l'ultimo
    /// a-capo di un file che ci finisce).
    fn select_all(&mut self) {
        self.anchor = Some(0);
        self.cursor = self.max_cursor();
        self.remember_col();
        self.scroll_adjust();
    }

    /// La selezione negli appunti interni (e in sospeso per il terminale), col messaggio di quanti byte:
    /// `false`, col messaggio giusto, se non c'e' selezione o supera `COPY_LIMIT`. Non tocca ne' il
    /// documento ne' la selezione: vale anche in lettura.
    fn copy_to_clip(&mut self) -> bool {
        // In modalita' colonna si copia il blocco, non la selezione (che non esiste).
        if self.col_mode {
            return self.column_copy();
        }
        let Some((da, fino)) = self.selection() else {
            self.flash = Some(self.cat.t("flash.nothing_selected").to_string());
            return false;
        };
        let quanto = fino - da;
        if quanto > COPY_LIMIT {
            self.flash = Some(
                self.cat
                    .tf("flash.copy_too_big", &[&(COPY_LIMIT >> 20).to_string()]),
            );
            return false;
        }
        // Solo i byte della selezione, e solo ora che servono: mai una copia del documento intero. In
        // una code page gli appunti sono testo UTF-8: i byte si decodificano; in UTF-8 restano byte
        // per byte, anche quelli non validi.
        let Some(doc) = self.doc.as_ref() else {
            return false;
        };
        let letto = doc.read_range(da, quanto);
        self.clip = if self.enc.is_utf8() {
            letto
        } else {
            self.enc.decode_lossy(&letto).into_bytes()
        };
        // Al terminale si manda solo cio' che OSC 52 puo' portare: una copia grande resta negli appunti interni
        // senza un secondo esemplare in memoria.
        self.copied = (self.clip.len() <= crate::clip::OSC52_LIMIT).then(|| self.clip.clone());
        self.flash = Some(self.cat.tf("flash.copied", &[&quanto.to_string()]));
        true
    }

    /// Ctrl+X: in lettura il messaggio di sola lettura e niente copia; in modifica copia come Ctrl+C e poi
    /// toglie la selezione nello stesso passo di storia (il tasto con Ctrl ha gia' chiuso il passo prima).
    fn cut_selection(&mut self) {
        if !self.edit {
            self.refuse_write();
            return;
        }
        // In modalita' colonna si taglia il blocco.
        if self.col_mode {
            self.column_cut();
            return;
        }
        if !self.copy_to_clip() {
            return;
        }
        self.delete_selection();
    }

    /// Ctrl+V: gli appunti interni al posto dell'eventuale selezione, al cursore. In lettura il messaggio
    /// di sola lettura; appunti vuoti il loro messaggio. In un documento non UTF-8 gli appunti (testo
    /// UTF-8) si codificano con la codifica attiva, tutto o niente.
    fn paste_clipboard(&mut self) {
        if !self.edit {
            self.refuse_write();
            return;
        }
        if self.clip.is_empty() {
            self.flash = Some(self.cat.t("flash.clipboard_empty").to_string());
            return;
        }
        // Gli appunti si tolgono un momento: l'inserimento prende in prestito tutta l'app.
        let testo = std::mem::take(&mut self.clip);
        if self.enc.is_utf8() {
            self.paste_bytes(&testo);
        } else {
            match self.enc.encode_str(&String::from_utf8_lossy(&testo)) {
                Ok(bytes) => self.paste_bytes(&bytes),
                Err(c) => {
                    self.flash = Some(
                        self.cat
                            .tf("flash.not_encodable", &[&c.to_string(), self.enc.name()]),
                    );
                }
            }
        }
        self.clip = testo;
    }

    /// L'inserimento di un incolla (Ctrl+V o l'incolla dal terminale): il testo entra al posto
    /// dell'eventuale selezione e il cursore finisce dopo di lui, tutto in un passo di storia (chiuso prima
    /// di agire, cosi' l'incolla e' un passo suo). Testo vuoto: niente. In lettura il messaggio.
    fn paste_bytes(&mut self, testo: &[u8]) {
        if testo.is_empty() {
            return;
        }
        if !self.edit {
            self.refuse_write();
            return;
        }
        self.seal_history();
        // In modalita' colonna l'incolla va per righe nel blocco (o alla riga del cursore).
        if self.col_mode {
            self.column_paste(testo);
            return;
        }
        self.insert_over_selection(testo);
    }

    /// Toglie la selezione (in modifica) portando il cursore al suo inizio, nel passo di storia corrente
    /// (lo apre o lo continua: un passo suo se lo chiude il chiamante). Vero se ha tolto qualcosa.
    fn delete_selection(&mut self) -> bool {
        let Some((da, fino)) = self.selection() else {
            return false;
        };
        let Some(doc) = self.doc.as_mut() else {
            return false;
        };
        if let Err(e) = doc.delete(da, fino - da) {
            self.flash = Some(self.cat.tf("flash.edit_failed", &[&format!("{:?}", e)]));
            return false;
        }
        self.cursor = da;
        self.anchor = None;
        self.remember_col();
        self.scroll_adjust();
        true
    }

    // ----- modalita' colonna -----

    /// F8: accende e spegne la modalita' colonna. Spegnendo, il blocco sparisce (il cursore resta
    /// dov'e'); il modo lettura/modifica non cambia.
    fn toggle_column(&mut self) {
        self.col_mode = !self.col_mode;
        // Cambiando modalita' la selezione di flusso e il blocco non valgono piu': ne' l'una ne' l'altro
        // tornano al prossimo cambio.
        self.anchor = None;
        self.block = None;
        if self.col_mode {
            self.flash = Some(self.cat.t("flash.column_on").to_string());
        } else {
            self.block = None;
            self.flash = Some(self.cat.t("flash.column_off").to_string());
        }
    }

    /// Maiusc+movimento in modalita' colonna: disegna il blocco rettangolare. Se il blocco manca lo si
    /// crea con l'ancora alla riga e alla colonna del cursore (una riga sola); poi il movimento muove
    /// il cursore del blocco (l'ancora non si muove mai): le colonne con Left/Right/Home/End, le righe
    /// con Up/Down/PgUp/PgDn. Il cursore del documento segue la colonna virtuale sulla riga del
    /// cursore del blocco. Ctrl+Maiusc+Home/End non sono movimenti del blocco: non fanno niente.
    fn extend_block(&mut self, m: Mov) {
        if matches!(m, Mov::FileStart | Mov::FileEnd) {
            return;
        }
        if self.doc.is_none() {
            return;
        }
        if self.block.is_none() {
            let col = (self.cursor_col() as u64).min(MAX_VCOL);
            let row = self.cursor_row();
            self.block = Some(BlockState {
                first_row: row,
                rows: 1,
                cursor_at_bottom: true,
                anchor_col: col,
                vcol: col,
            });
        }
        match m {
            Mov::Right => {
                if let Some(b) = self.block.as_mut() {
                    b.vcol = (b.vcol + 1).min(MAX_VCOL);
                }
            }
            Mov::Left => {
                if let Some(b) = self.block.as_mut() {
                    b.vcol = b.vcol.saturating_sub(1);
                }
            }
            Mov::Home => {
                if let Some(b) = self.block.as_mut() {
                    b.vcol = 0;
                }
            }
            Mov::End => {
                // La colonna del cursore diventa la larghezza della sua riga.
                if let Some(larga) = self.block_cursor_row().and_then(|row| self.row_width(row)) {
                    if let Some(b) = self.block.as_mut() {
                        b.vcol = larga.min(MAX_VCOL);
                    }
                }
            }
            Mov::Up => self.block_up(),
            Mov::Down => self.block_down(),
            Mov::PageUp => {
                // Come Up/Down per "righe di testo - 1" righe.
                for _ in 0..self.text_rows().saturating_sub(1) {
                    self.block_up();
                }
            }
            Mov::PageDown => {
                for _ in 0..self.text_rows().saturating_sub(1) {
                    self.block_down();
                }
            }
            Mov::FileStart | Mov::FileEnd => {}
        }
        self.align_cursor();
        self.scroll_adjust();
    }

    /// Il cursore del blocco sale di una riga: il blocco si accorcia da sotto se il cursore sta
    /// sull'ultima riga, altrimenti si allunga in alto (l'ancora, in fondo, non si muove); fermo alla
    /// prima riga del documento.
    fn block_up(&mut self) {
        let Some(doc) = self.doc.as_ref() else {
            return;
        };
        let Some(b) = self.block.as_mut() else {
            return;
        };
        if b.cursor_at_bottom {
            if b.rows > 1 {
                // Il cursore resta in fondo: la nuova ultima riga e' una piu' su.
                b.rows -= 1;
            } else if let Some(prima) = scan::prev_row_start(doc, b.first_row, MAX_SCAN) {
                // Riga unica: il blocco cresce in alto, col cursore in cima e l'ancora in fondo.
                b.first_row = prima;
                b.rows = 2;
                b.cursor_at_bottom = false;
            }
        } else if let Some(prima) = scan::prev_row_start(doc, b.first_row, MAX_SCAN) {
            // Il cursore e' sulla prima riga: il blocco si allunga in alto.
            b.first_row = prima;
            b.rows += 1;
        }
    }

    /// Il cursore del blocco scende di una riga: il blocco si allunga in basso se il cursore sta
    /// sull'ultima riga, altrimenti si accorcia da sopra (l'ancora, in cima, non si muove); fermo
    /// all'ultima riga del documento.
    fn block_down(&mut self) {
        // Le opzioni si prendono prima: sotto servono i prestiti del documento e del blocco.
        let opts = self.view_opts();
        let Some(doc) = self.doc.as_ref() else {
            return;
        };
        let Some(b) = self.block.as_mut() else {
            return;
        };
        if b.cursor_at_bottom {
            // C'e' una riga dopo l'ultima del blocco? (fermo all'ultima riga del documento)
            if nth_row(doc, b.first_row, b.rows, &opts).is_some() {
                b.rows += 1;
            }
        } else if b.rows > 1 {
            // Il cursore e' sulla prima riga: scende di una e il blocco perde quella di sopra.
            if let Some(dopo) = nth_row(doc, b.first_row, 1, &opts) {
                b.first_row = dopo;
                b.rows -= 1;
                if b.rows == 1 {
                    b.cursor_at_bottom = true;
                }
            }
        } else if nth_row(doc, b.first_row, 1, &opts).is_some() {
            // Riga unica: il blocco cresce in basso, col cursore in fondo.
            b.rows = 2;
            b.cursor_at_bottom = true;
        }
    }

    /// La riga del cursore del blocco: l'ultima se il cursore sta in fondo, altrimenti la prima.
    fn block_cursor_row(&self) -> Option<u64> {
        let b = self.block.as_ref()?;
        if b.cursor_at_bottom {
            let doc = self.doc.as_ref()?;
            nth_row(
                doc,
                b.first_row,
                b.rows.saturating_sub(1),
                &self.view_opts(),
            )
        } else {
            Some(b.first_row)
        }
    }

    /// La larghezza in celle della riga visuale che inizia in `row`.
    fn row_width(&self, row: u64) -> Option<u64> {
        let doc = self.doc.as_ref()?;
        let fine = scan::row_end(doc, row, MAX_SCAN);
        Some(view::col_of(doc, row, fine, &self.view_opts()) as u64)
    }

    /// Rimette il cursore del documento sulla riga del cursore del blocco, alla colonna virtuale: il
    /// carattere che inizia a `vcol` o la fine della riga (`view::offset_at_col`: una colonna dentro
    /// un carattere largo o un tabulatore da' l'inizio di quel carattere).
    fn align_cursor(&mut self) {
        let Some(b) = self.block else {
            return;
        };
        let Some(row) = self.block_cursor_row() else {
            return;
        };
        let Some(doc) = self.doc.as_ref() else {
            return;
        };
        self.cursor =
            view::offset_at_col(doc, row, b.vcol, &self.view_opts()).min(self.max_cursor());
    }

    /// Il blocco del core che corrisponde allo stato del blocco: le colonne di ancora e cursore in
    /// ordine, le righe dalla prima all'ultima (`rows - 1` righe dopo `first_row`, camminando il
    /// documento: dopo una modifica gli inizi delle righe successive si sono spostati).
    fn column_block_of(&self, b: &BlockState) -> Option<ColumnBlock> {
        let doc = self.doc.as_ref()?;
        let last_row = nth_row(
            doc,
            b.first_row,
            b.rows.saturating_sub(1),
            &self.view_opts(),
        )?;
        Some(ColumnBlock {
            first_row: b.first_row,
            last_row,
            col_a: b.anchor_col.min(b.vcol),
            col_b: b.anchor_col.max(b.vcol),
        })
    }

    /// Le righe del blocco con cio' che coprono, col limite di righe della modalita' colonna: oltre,
    /// il messaggio `flash.block_too_big` e `None` (niente modifica ne' copia).
    fn block_spans_limited(&mut self, blocco: &ColumnBlock) -> Option<Vec<RowSpan>> {
        let doc = self.doc.as_ref()?;
        match block_spans(doc, blocco, &self.view_opts(), MAX_BLOCK_ROWS) {
            Ok(spans) => Some(spans),
            Err(BlockError::TooManyRows) => {
                self.flash = Some(
                    self.cat
                        .tf("flash.block_too_big", &[&MAX_BLOCK_ROWS.to_string()]),
                );
                None
            }
            // Difensivo: gli altri errori non vengono da block_spans.
            Err(_) => None,
        }
    }

    /// Toglie dal documento le righe del blocco `blocco`, nel passo di storia corrente (lo apre o lo
    /// continua il chiamante). Le righe si tolgono dall'ultima alla prima, come vuole il core. Vero se
    /// ha tolto (o non c'era niente da togliere senza errore).
    fn column_delete_region(&mut self, blocco: &ColumnBlock) -> bool {
        let Some(spans) = self.block_spans_limited(blocco) else {
            return false;
        };
        let Some(doc) = self.doc.as_mut() else {
            return false;
        };
        match delete_block(doc, &spans) {
            Ok(_) => true,
            Err(e) => {
                self.flash = Some(self.cat.tf("flash.edit_failed", &[&format!("{:?}", e)]));
                false
            }
        }
    }

    /// La scrittura di `testo` (un carattere o un tabulatore) sul blocco in modalita' colonna: il
    /// blocco largo si toglie prima, poi il testo entra alla colonna `col_a` sulle righe che ci
    /// arrivano (senza riempimento: le righe corte restano intatte). Il blocco diventa di larghezza
    /// zero a `col_a + larghezza del testo` (calcolata come se iniziasse a `col_a`), sulle stesse
    /// righe; tutto nello stesso passo di storia dei caratteri successivi, come nel testo normale.
    fn column_type(&mut self, testo: &[u8]) {
        if !self.edit {
            self.refuse_write();
            return;
        }
        let Some(b) = self.block else {
            return;
        };
        let Some(blocco) = self.column_block_of(&b) else {
            return;
        };
        let mut largo_zero = blocco;
        if blocco.col_b > blocco.col_a {
            if !self.column_delete_region(&blocco) {
                return;
            }
            // Le righe dopo la prima si sono spostate: il blocco si ricalcola.
            let Some(b2) = self.block else {
                return;
            };
            let Some(fresco) = self.column_block_of(&b2) else {
                return;
            };
            largo_zero = fresco;
        }
        largo_zero.col_b = largo_zero.col_a;
        let Some(spans) = self.block_spans_limited(&largo_zero) else {
            return;
        };
        let Some(doc) = self.doc.as_mut() else {
            return;
        };
        if let Err(e) = insert_block(doc, &spans, largo_zero.col_a, testo, false) {
            self.flash = Some(self.cat.tf("flash.edit_failed", &[&format!("{:?}", e)]));
            return;
        }
        let arrivo = largo_zero
            .col_a
            .saturating_add(larghezza_da(self.enc, testo, largo_zero.col_a))
            .min(MAX_VCOL);
        if let Some(b) = self.block.as_mut() {
            b.anchor_col = arrivo;
            b.vcol = arrivo;
        }
        self.align_cursor();
        self.scroll_adjust();
    }

    /// Backspace sul blocco: largo, si toglie e resta di larghezza zero alla colonna `col_a`; largo
    /// zero, si cancella il blocco `[col_a - 1, col_a)` e la colonna scende di uno; a colonna 0 non
    /// fa nulla. In lettura il messaggio e il blocco resta.
    fn column_backspace(&mut self) {
        if !self.edit {
            self.refuse_write();
            return;
        }
        let Some(b) = self.block else {
            return;
        };
        let Some(blocco) = self.column_block_of(&b) else {
            return;
        };
        if blocco.col_b > blocco.col_a {
            if self.column_delete_region(&blocco) {
                if let Some(b) = self.block.as_mut() {
                    b.anchor_col = blocco.col_a;
                    b.vcol = blocco.col_a;
                }
                self.align_cursor();
                self.scroll_adjust();
            }
            return;
        }
        if blocco.col_a == 0 {
            return;
        }
        let mut prima = blocco;
        prima.col_a = blocco.col_a - 1;
        prima.col_b = blocco.col_a;
        if !self.column_delete_region(&prima) {
            return;
        }
        if let Some(b) = self.block.as_mut() {
            b.anchor_col = prima.col_a;
            b.vcol = prima.col_a;
        }
        self.align_cursor();
        self.scroll_adjust();
    }

    /// Delete sul blocco: largo, si toglie e resta di larghezza zero alla colonna `col_a`; largo
    /// zero, si cancella il blocco `[col_a, col_a + 1)` e la colonna resta. In lettura il messaggio
    /// e il blocco resta.
    fn column_delete(&mut self) {
        if !self.edit {
            self.refuse_write();
            return;
        }
        let Some(b) = self.block else {
            return;
        };
        let Some(blocco) = self.column_block_of(&b) else {
            return;
        };
        if blocco.col_b > blocco.col_a {
            if self.column_delete_region(&blocco) {
                if let Some(b) = self.block.as_mut() {
                    b.anchor_col = blocco.col_a;
                    b.vcol = blocco.col_a;
                }
                self.align_cursor();
                self.scroll_adjust();
            }
            return;
        }
        let mut dopo = blocco;
        dopo.col_b = blocco.col_a + 1;
        if self.column_delete_region(&dopo) {
            self.align_cursor();
            self.scroll_adjust();
        }
    }

    /// La copia del blocco in modalita' colonna: il testo delle sue celle (righe unite da un `\n`,
    /// nessuno in fondo) negli appunti interni e in sospeso per il terminale, col messaggio di quanti
    /// byte. `false`, col messaggio giusto, se non c'e' blocco (niente di selezionato) o supera i
    /// limiti (righe o byte). Non tocca il documento: vale anche in lettura.
    fn column_copy(&mut self) -> bool {
        let Some(b) = self.block else {
            self.flash = Some(self.cat.t("flash.nothing_selected").to_string());
            return false;
        };
        let Some(blocco) = self.column_block_of(&b) else {
            return false;
        };
        let Some(spans) = self.block_spans_limited(&blocco) else {
            return false;
        };
        let Some(doc) = self.doc.as_ref() else {
            return false;
        };
        let testo = match block_text(doc, &spans, COPY_LIMIT) {
            Ok(testo) => testo,
            Err(BlockError::TooManyBytes) => {
                self.flash = Some(
                    self.cat
                        .tf("flash.copy_too_big", &[&(COPY_LIMIT >> 20).to_string()]),
                );
                return false;
            }
            // Difensivo: gli altri errori non vengono da block_text.
            Err(_) => return false,
        };
        let quanto = testo.len() as u64;
        // Come la copia della selezione: i byte delle celle del blocco, letti come testo nella
        // codifica attiva (in UTF-8 restano byte per byte); al terminale solo cio' che OSC 52 puo'
        // portare.
        self.clip = if self.enc.is_utf8() {
            testo
        } else {
            self.enc.decode_lossy(&testo).into_bytes()
        };
        self.copied = (self.clip.len() <= crate::clip::OSC52_LIMIT).then(|| self.clip.clone());
        self.flash = Some(self.cat.tf("flash.copied", &[&quanto.to_string()]));
        true
    }

    /// Ctrl+X in modalita' colonna: copia come Ctrl+C e poi toglie il blocco, che resta di larghezza
    /// zero alla colonna `col_a` sulle stesse righe. Il tasto con Ctrl ha gia' chiuso il passo della
    /// storia prima di agire: un annulla solo.
    fn column_cut(&mut self) {
        if !self.column_copy() {
            return;
        }
        let Some(b) = self.block else {
            return;
        };
        let Some(blocco) = self.column_block_of(&b) else {
            return;
        };
        if blocco.col_b > blocco.col_a && !self.column_delete_region(&blocco) {
            return;
        }
        if let Some(b) = self.block.as_mut() {
            b.anchor_col = blocco.col_a;
            b.vcol = blocco.col_a;
        }
        self.align_cursor();
        self.scroll_adjust();
    }

    /// L'incolla in modalita' colonna: il testo, gia' nei byte della codifica attiva, diviso in
    /// righe sugli a-capo della forma del documento (l'ultima vuota di un testo che finisce con
    /// a-capo si ignora) entra alla colonna `col_a` del blocco — una riga sola, su
    /// tutte le righe del blocco; piu' righe, una per riga dalla prima del blocco, con le righe che
    /// servono (estese con `nth_row` se il blocco non basta). Senza blocco si parte dalla riga del
    /// cursore, alla sua colonna. Se il documento finisce prima delle righe da incollare:
    /// `flash.paste_rows` e niente cambia. Il blocco largo si toglie prima. Un passo di storia solo
    /// (lo chiude il chiamante); dopo l'incolla il blocco e' di larghezza zero alla colonna d'arrivo,
    /// sulle stesse righe, e senza blocco non se ne crea uno (il cursore va dopo la prima riga
    /// incollata, se la sua riga l'ha ricevuta).
    fn column_paste(&mut self, testo: &[u8]) {
        // Le righe dell'incolla: il testo e' gia' nei byte della codifica attiva, quindi i suoi
        // a-capo hanno la forma del documento (in UTF-16 l'unita' a 16 bit). Ogni riga e' il
        // tratto fra due a-capo; l'ultimo tratto vuoto di un testo che finisce con a-capo si
        // ignora.
        let newline = match self.doc.as_ref() {
            Some(doc) => doc.newline(),
            None => return,
        };
        let mut righe: Vec<&[u8]> = Vec::new();
        let mut inizio = 0;
        for p in newline.positions(testo) {
            righe.push(&testo[inizio..p]);
            inizio = p + newline.unit();
        }
        if inizio < testo.len() {
            righe.push(&testo[inizio..]);
        }
        if righe.is_empty() {
            return;
        }
        // Riga di partenza, quante righe servono e a quale colonna: quelle del blocco, o della riga
        // e della colonna del cursore se il blocco non c'e'.
        let (prima, quante, col_a) = match self.block.as_ref() {
            Some(b) => (
                b.first_row,
                b.rows.max(righe.len() as u64),
                b.anchor_col.min(b.vcol),
            ),
            None => (
                self.cursor_row(),
                righe.len() as u64,
                self.cursor_col() as u64,
            ),
        };
        // Le righe da incollare devono starci tutte: se il documento finisce prima, niente cambia.
        if self
            .doc
            .as_ref()
            .and_then(|doc| nth_row(doc, prima, quante - 1, &self.view_opts()))
            .is_none()
        {
            self.flash = Some(self.cat.t("flash.paste_rows").to_string());
            return;
        }
        // Il blocco largo si toglie prima, nello stesso passo di storia.
        if let Some(b) = self.block {
            if let Some(blocco) = self.column_block_of(&b) {
                if blocco.col_b > blocco.col_a && !self.column_delete_region(&blocco) {
                    return;
                }
            }
        }
        // Le righe dell'incolla, come blocco di larghezza zero: si ricalcolano DOPO l'eventuale
        // cancellazione, che sposta gli inizi delle righe dopo la prima.
        let Some(ultima) = self
            .doc
            .as_ref()
            .and_then(|doc| nth_row(doc, prima, quante - 1, &self.view_opts()))
        else {
            // Difensivo: prima della cancellazione c'erano tutte.
            return;
        };
        let target = ColumnBlock {
            first_row: prima,
            last_row: ultima,
            col_a,
            col_b: col_a,
        };
        let Some(spans) = self.block_spans_limited(&target) else {
            return;
        };
        let Some(doc) = self.doc.as_mut() else {
            return;
        };
        let esito = if righe.len() == 1 {
            insert_block(doc, &spans, col_a, righe[0], false)
        } else {
            paste_block(doc, &spans, col_a, &righe, false)
        };
        if let Err(e) = esito {
            self.flash = Some(self.cat.tf("flash.edit_failed", &[&format!("{:?}", e)]));
            return;
        }
        // Dopo l'incolla: il blocco di larghezza zero alla colonna d'arrivo (col_a + larghezza della
        // prima riga incollata, come se iniziasse a col_a), stesse righe; senza blocco il cursore
        // va dopo la prima riga incollata se la sua riga l'ha ricevuta.
        let arrivo = col_a
            .saturating_add(larghezza_da(self.enc, righe[0], col_a))
            .min(MAX_VCOL);
        if self.block.is_some() {
            if let Some(b) = self.block.as_mut() {
                b.anchor_col = arrivo;
                b.vcol = arrivo;
            }
            self.align_cursor();
        } else if let Some(span) = spans.first() {
            if span.reaches && !righe[0].is_empty() {
                self.cursor = (span.start + righe[0].len() as u64).min(self.max_cursor());
            }
        }
        self.scroll_adjust();
    }

    // ----- ricerca -----

    // ----- sostituzione -----

    /// Cambia il modo di leggere il testo cercato: testo, glob, regex, e da capo.
    fn cycle_mode(&mut self) {
        self.search_mode = match self.search_mode {
            Mode::Text => Mode::Glob,
            Mode::Glob => Mode::Regex,
            Mode::Regex => Mode::Text,
        };
    }

    /// Ogni modifica ai campi o alle opzioni toglie l'anteprima.
    fn rep_reset(&mut self) {
        self.rep_ready = false;
        self.rep_status.clear();
    }

    /// Cosa chiede il dialogo adesso.
    fn replace_spec(&self) -> ReplaceSpec {
        ReplaceSpec {
            find: self.rep_find.clone(),
            with: self.rep_with.clone(),
            mode: self.search_mode,
            case: self.search_case,
        }
    }

    /// La richiesta uscita dal dialogo con Invio, una volta sola: chi ospita l'app (il `Workspace`) la esegue.
    pub fn take_replace_request(&mut self) -> Option<ReplaceAsk> {
        self.replace_ask.take()
    }

    /// Ricorda `testo` come ultimo cercato (F3 e Maiusc+F3 lo cercano ancora).
    pub fn remember_find(&mut self, testo: &str) {
        self.last_find = Some(testo.to_string());
    }

    /// L'esito della sostituzione su questo documento, una volta sola.
    pub fn take_replace_done(&mut self) -> Option<ReplaceDone> {
        self.replace_done.take()
    }

    /// Vero mentre una sostituzione (anteprima o applicazione) gira su questo documento.
    pub fn replace_busy(&self) -> bool {
        self.replace_job.is_some()
    }

    /// Ferma la sostituzione in corso, se c'e'.
    pub fn replace_cancel(&self) {
        if let Some(job) = &self.replace_job {
            job.cancel.store(true, Ordering::Relaxed);
        }
    }

    /// Scrive lo stato del dialogo (l'anteprima o l'errore); `pronta`: il prossimo Invio applica.
    pub fn set_replace_status(&mut self, testo: String, pronta: bool) {
        self.rep_status = testo;
        self.rep_ready = pronta;
    }

    /// Chiude il dialogo "Sostituisci" (finita la sostituzione) e mostra `messaggio` nella riga di stato.
    pub fn finish_replace(&mut self, messaggio: String) {
        if self.dialog == Some(DialogKind::Replace) {
            self.dialog = None;
        }
        self.rep_ready = false;
        self.rep_status.clear();
        self.flash = Some(messaggio);
    }

    /// Come si legge il testo cercato (testo, glob, regex) e se le maiuscole contano.
    pub fn search_options(&self) -> (Mode, bool) {
        (self.search_mode, self.search_case)
    }

    /// Cambia come si legge il testo cercato (per i test e per chi ospita l'app).
    pub fn set_search_options(&mut self, mode: Mode, case: bool) {
        self.search_mode = mode;
        self.search_case = case;
    }

    /// Prepara la ricerca per `spec`: byte esatti (testo con le maiuscole, anche nelle altre codifiche) o un `Pattern`
    /// (solo UTF-8). L'errore e' gia' il messaggio da mostrare.
    fn resolve(&self, spec: &ReplaceSpec) -> Result<Resolved, String> {
        if spec.find.is_empty() {
            return Err(self.cat.t("replace.empty").to_string());
        }
        if spec.mode == Mode::Text && spec.case {
            let codifica = |s: &str| -> Result<Vec<u8>, String> {
                if self.enc.is_utf8() {
                    Ok(s.as_bytes().to_vec())
                } else {
                    self.enc
                        .encode_str(s)
                        .map_err(|_| self.cat.tf("flash.not_encodable", &[s, self.enc.name()]))
                }
            };
            return Ok(Resolved::Literal {
                needle: codifica(&spec.find)?,
                with: codifica(&spec.with)?,
            });
        }
        if !self.enc.is_utf8() {
            return Err(self.cat.t("flash.regex_utf8").to_string());
        }
        let p = Pattern::new(&spec.find, spec.mode, spec.case)
            .map_err(|e| self.cat.tf("replace.bad", &[&e.to_string()]))?;
        Ok(Resolved::Pattern {
            p,
            r: Replacement::new(&spec.with, spec.mode),
        })
    }

    /// Fa partire in background l'anteprima (`apply` falso: quante occorrenze) o la preparazione delle sostituzioni
    /// (`apply` vero: alla fine le applica); l'esito si prende con `take_replace_done`. Errore (gia' messaggio) se la
    /// ricerca non e' valida o il documento e' occupato.
    pub fn replace_start(&mut self, spec: &ReplaceSpec, apply: bool) -> Result<(), String> {
        if self.replace_job.is_some() || self.search_job.is_some() {
            return Err(self.cat.t("replace.counting").to_string());
        }
        let ris = self.resolve(spec)?;
        let Some(snap) = self.doc.as_ref().and_then(Document::snapshot) else {
            return Err(self.cat.t("replace.counting").to_string());
        };
        let snap = snap.with_boundary(self.enc);
        let (tx, rx) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let progress = Arc::new(AtomicU64::new(0));
        let (c, p) = (Arc::clone(&cancel), Arc::clone(&progress));
        let len = snap.len();
        std::thread::spawn(move || {
            let _ = tx.send(replace_work(&snap, &ris, apply, &c, &p));
        });
        self.replace_job = Some(ReplaceJob {
            rx,
            progress,
            cancel,
            len,
        });
        Ok(())
    }

    /// La fine della sostituzione (la chiama `tick`): l'anteprima lascia il conteggio; le sostituzioni pronte si
    /// applicano al documento come un solo passo (un solo annulla le toglie) e il documento passa in modifica.
    fn tick_replace(&mut self) {
        let Some(job) = self.replace_job.as_ref() else {
            return;
        };
        let esito = match job.rx.try_recv() {
            Ok(e) => e,
            Err(mpsc::TryRecvError::Empty) => return,
            Err(mpsc::TryRecvError::Disconnected) => ReplaceOutcome::Cancelled,
        };
        self.replace_job = None;
        self.replace_done = Some(match esito {
            ReplaceOutcome::Count(n) => ReplaceDone::Counted(n),
            ReplaceOutcome::TooMany => ReplaceDone::TooMany,
            ReplaceOutcome::Cancelled => ReplaceDone::Cancelled,
            ReplaceOutcome::Failed(e) => ReplaceDone::Failed(e),
            ReplaceOutcome::Edits(edits) => self.apply_replace(&edits),
        });
    }

    /// Applica le sostituzioni al documento (un solo passo) e rimette a posto cursore e selezione.
    fn apply_replace(&mut self, edits: &[Edit]) -> ReplaceDone {
        let Some(doc) = self.doc.as_mut() else {
            return ReplaceDone::Failed("documento occupato".to_string());
        };
        doc.seal();
        match doc.apply_edits(edits) {
            Ok(n) => {
                self.edit = true;
                // il cursore segue il testo: sposta di quanto e' cambiato prima di lui
                let c = self.cursor;
                let mut spostamento: i128 = 0;
                let mut nuovo = None;
                for e in edits {
                    let fine = e.start + e.len;
                    if fine <= c {
                        spostamento += e.new.len() as i128 - e.len as i128;
                    } else if e.start < c {
                        nuovo = Some(e.start as i128 + spostamento + e.new.len() as i128);
                        break;
                    } else {
                        break;
                    }
                }
                let nuovo = nuovo.unwrap_or(c as i128 + spostamento).max(0) as u64;
                self.anchor = None;
                self.block = None;
                self.found = None;
                self.cursor = nuovo.min(self.max_cursor());
                self.remember_col();
                self.scroll_adjust();
                ReplaceDone::Applied(n)
            }
            Err(e) => ReplaceDone::Failed(format!("{e:?}")),
        }
    }

    /// I byte esaminati dall'anteprima o dalla sostituzione in corso e la lunghezza del documento (per la percentuale).
    pub fn replace_progress(&self) -> Option<(u64, u64)> {
        self.replace_job
            .as_ref()
            .map(|j| (j.progress.load(Ordering::Relaxed), j.len))
    }

    /// F3 / Maiusc+F3: con un testo cercato parte subito la ricerca (avanti o indietro); senza,
    /// si apre il prompt (e dopo Enter si cerca in avanti).
    fn find_key(&mut self, forward: bool) {
        let Some(testo) = self.last_find.clone() else {
            self.open_dialog(DialogKind::Find);
            return;
        };
        self.start_search(&testo, forward);
    }

    /// Esegue il testo del prompt di ricerca (il dialogo e' gia' chiuso): vuoto, non fa niente e
    /// non ricorda niente; altrimenti il testo diventa l'ultimo cercato e la ricerca parte in
    /// avanti dal cursore.
    fn find_run(&mut self, testo: &str) {
        if testo.is_empty() {
            return;
        }
        self.last_find = Some(testo.to_string());
        self.start_search(testo, true);
    }

    /// Fa partire la ricerca in background: una fotografia del documento va nel thread con l'ago
    /// e la direzione (in avanti dal byte dopo il cursore, indietro dal cursore stesso). Il
    /// documento resta qui e non si modifica finche' la fotografia non torna col suo esito.
    fn start_search(&mut self, ago: &str, forward: bool) {
        let Some(snap) = self.doc.as_ref().and_then(Document::snapshot) else {
            return;
        };
        // nelle multibyte asiatiche un'occorrenza nel mezzo di un carattere non conta
        let snap = snap.with_boundary(self.enc);
        let needle = if self.search_mode == Mode::Text && self.search_case {
            // L'ago nei byte della codifica del documento; un carattere che la code page non ha non puo'
            // stare nel file: la ricerca non parte e l'esito e' il solito "non trovato".
            let ago_bytes: Vec<u8> = if self.enc.is_utf8() {
                ago.as_bytes().to_vec()
            } else {
                match self.enc.encode_str(ago) {
                    Ok(b) => b,
                    Err(_) => {
                        self.flash = Some(self.cat.tf("flash.not_found", &[ago]));
                        return;
                    }
                }
            };
            Needle::Literal(ago_bytes)
        } else if !self.enc.is_utf8() {
            self.flash = Some(self.cat.t("flash.regex_utf8").to_string());
            return;
        } else {
            match Pattern::new(ago, self.search_mode, self.search_case) {
                Ok(p) => Needle::Pattern(p),
                Err(e) => {
                    self.flash = Some(self.cat.tf("replace.bad", &[&e.to_string()]));
                    return;
                }
            }
        };
        let cancel = Arc::new(AtomicBool::new(false));
        let progress = Arc::new(AtomicU64::new(0));
        let da = if forward {
            self.cursor.saturating_add(1)
        } else {
            self.cursor
        };
        self.search_job = Some(spawn_search_job(
            snap, needle, ago, forward, da, &cancel, &progress,
        ));
    }

    /// La fine della ricerca (la chiama `tick`): trovata, il cursore va all'inizio
    /// dell'occorrenza come dopo un "vai a" (centrata se non si vedeva) e l'evidenziazione la
    /// copre; gli esiti negativi lasciano il cursore dov'e' e dicono solo il loro messaggio.
    fn tick_search(&mut self) {
        let Some(job) = self.search_job.as_mut() else {
            return;
        };
        match job.rx.try_recv() {
            Ok(SearchOutcome::Found(inizio, lunga, giro_completo)) => {
                let fine = inizio + lunga;
                self.search_job = None;
                self.goto_move(inizio);
                if giro_completo {
                    self.flash = Some(self.cat.t("flash.find_wrapped").to_string());
                }
                self.found = Some((inizio, fine));
            }
            Ok(SearchOutcome::NotFound) => {
                let ago = job.needle.clone();
                self.search_job = None;
                // Anche un errore di lettura vale come "non trovato": il cursore non si muove.
                self.flash = Some(self.cat.tf("flash.not_found", &[&ago]));
            }
            Ok(SearchOutcome::Cancelled) => {
                self.search_job = None;
                self.flash = Some(self.cat.t("flash.find_cancelled").to_string());
            }
            Err(mpsc::TryRecvError::Empty) => {}
            Err(mpsc::TryRecvError::Disconnected) => {
                // Difensivo: il thread e' terminato senza consegnare nulla, non si aspetta piu'.
                self.search_job = None;
            }
        }
    }

    /// Raccoglie i lavori in background: prima il salvataggio (quando torna rimette il documento al suo
    /// posto e, dopo uno riuscito, puo' ripartire l'indice), poi la ricerca e l'indice righe.
    pub fn tick(&mut self) {
        if self.save_job.is_some() {
            self.tick_save();
        }
        self.tick_search();
        self.tick_replace();
        // Un cambio di forma dell'a-capo ha chiesto l'indice nuovo: parte adesso (il cambio di
        // codifica lascia il suo messaggio da solo nella riga di stato, senza l'indicatore).
        if self.index_pending {
            self.index_pending = false;
            if self.index_job.is_none() {
                self.rebuild_index();
            }
        }
        let esito = match self.index_job.as_mut() {
            Some(job) => job.rx.try_recv(),
            None => return,
        };
        match esito {
            Ok(Ok(indice)) => {
                // A indice pronto non c'e' nessun messaggio: sparisce solo l'indicatore di avanzamento.
                if let Some(doc) = self.doc.as_mut() {
                    doc.install_lines(indice);
                }
                self.index_job = None;
            }
            Ok(Err(e)) => {
                self.flash = Some(self.cat.tf("flash.index_failed", &[&e.to_string()]));
                self.index_job = None;
            }
            Err(mpsc::TryRecvError::Empty) => {}
            Err(mpsc::TryRecvError::Disconnected) => {
                // Difensivo: il thread e' terminato senza consegnare nulla, non si aspetta piu' nulla.
                self.index_job = None;
            }
        }
    }

    /// Chiama `tick` finche' non c'e' piu' un lavoro in corso (salvataggio, ricerca o
    /// indicizzazione) o scade `timeout`. Vero se non c'e' piu' niente in corso.
    pub fn wait_idle(&mut self, timeout: Duration) -> bool {
        let inizio = Instant::now();
        loop {
            self.tick();
            if self.save_job.is_none()
                && self.index_job.is_none()
                && self.search_job.is_none()
                && self.replace_job.is_none()
            {
                return true;
            }
            if inizio.elapsed() >= timeout {
                return false;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    pub fn should_quit(&self) -> bool {
        self.quit
    }

    /// Offset del primo byte della prima riga visuale dello schermo.
    pub fn top(&self) -> u64 {
        self.top
    }

    /// Offset in byte del cursore.
    pub fn cursor(&self) -> u64 {
        self.cursor
    }

    /// Prima colonna visibile (scorrimento orizzontale).
    pub fn hscroll(&self) -> usize {
        self.hscroll
    }

    pub fn is_edit(&self) -> bool {
        self.edit
    }

    pub fn is_modified(&self) -> bool {
        match self.doc.as_ref() {
            Some(doc) => doc.is_modified(),
            // Un salvataggio in corso tiene il documento: si salva solo se c'e' una modifica da scrivere.
            None => self.save_job.is_some(),
        }
    }

    pub fn document_len(&self) -> u64 {
        match self.doc.as_ref() {
            Some(doc) => doc.len(),
            // Un salvataggio in corso tiene il documento: la lunghezza e' quella di partenza (il salvataggio
            // non cambia il documento).
            None => self.save_job.as_ref().map_or(0, |job| job.len),
        }
    }

    /// L'ultima occorrenza trovata (inizio, fine esclusa), finche' non si preme un altro tasto.
    pub fn found(&self) -> Option<(u64, u64)> {
        self.found
    }

    /// Posizione del cursore sullo schermo (colonna, riga) dopo l'ultimo `draw`; `None` se un dialogo e' aperto o
    /// non si e' ancora disegnato. Nei prompt e' la posizione dentro il prompt, dopo il testo scritto.
    pub fn cursor_xy(&self) -> Option<(u16, u16)> {
        match self.dialog {
            Some(DialogKind::Goto)
            | Some(DialogKind::Find)
            | Some(DialogKind::Open)
            | Some(DialogKind::Replace) => self.cursor_xy,
            Some(_) => None,
            None => self.cursor_xy,
        }
    }

    /// Il dialogo aperto.
    pub fn dialog_kind(&self) -> Option<DialogKind> {
        self.dialog
    }

    /// La voce selezionata di un menu (tema, lingua) aperto; `None` se non c'e' un menu.
    pub fn menu_index(&self) -> Option<usize> {
        match self.dialog {
            Some(DialogKind::Theme) | Some(DialogKind::Lang) | Some(DialogKind::Encoding) => {
                Some(self.menu_index)
            }
            _ => None,
        }
    }

    pub fn theme_id(&self) -> ThemeId {
        self.theme
    }

    /// Codice della lingua attiva.
    pub fn lang_code(&self) -> &str {
        self.cat.code()
    }

    /// Lo stile di fine riga del file (rilevato all'apertura; cosa scrive Invio).
    pub fn line_ending(&self) -> LineEnding {
        self.eol
    }

    /// La codifica attiva.
    pub fn encoding(&self) -> Encoding {
        self.enc
    }

    /// Cambia la codifica: i byte del documento non si toccano. Se cambia la forma dell'a-capo
    /// (fra UTF-16 e il resto) il documento non deve avere una storia di modifiche: se ce l'ha
    /// niente cambia (codifica, cursore e selezione compresi) e il messaggio lo dice; se la forma
    /// cambia, l'indice righe si rifà in background con la forma nuova (parte al primo `tick`, se
    /// l'indice e' abilitato). La selezione e il blocco della modalita' colonna non valgono piu',
    /// l'evidenziazione dell'ultima ricerca sparisce e il cursore va all'inizio del carattere che
    /// lo contiene nella nuova codifica (da una code page a UTF-8, o fra UTF-8 e UTF-16, puo'
    /// capitare a meta' di un carattere: si torna indietro fino al suo inizio); `want_col` si
    /// ricalcola. `top` non cambia: le righe dipendono solo dagli a-capo.
    pub fn set_encoding(&mut self, e: Encoding) {
        // La forma dell'a-capo e' del documento, non della sola codifica: cambia solo senza una
        // storia di modifiche (le posizioni da annullare o ripetere sono legate alla forma con cui
        // le modifiche sono state fatte).
        // Lo stile di fine riga che la codifica nuova ammette (il CR da solo non c'e' in UTF-16 e UTF-32)
        // e la forma dell'a-capo che ne viene.
        let eol_nuovo = self.eol.for_encoding(e);
        let forma = eol_nuovo.newline_for(e);
        let forma_cambiata = match self.doc.as_mut() {
            Some(doc) if forma != doc.newline() => {
                if !doc.set_newline(forma) {
                    self.flash = Some(self.cat.t("flash.encoding_locked").to_string());
                    return;
                }
                true
            }
            _ => false,
        };
        // Con la forma nuova l'indice righe non vale piu' (li conta gli a-capo dell'altra): uno in
        // corso si ferma, e quello nuovo parte al primo `tick`, solo se l'indice e' abilitato.
        if forma_cambiata {
            self.cancel.store(true, Ordering::Relaxed);
            self.cancel = Arc::new(AtomicBool::new(false));
            self.index_job = None;
            self.index_pending = self.index;
        }
        self.enc = e;
        self.eol = eol_nuovo;
        self.bom = self
            .doc
            .as_ref()
            .map_or(0, |d| e.bom_len(&d.read_range(0, 4)) as u64);
        self.anchor = None;
        self.block = None;
        self.found = None;
        // Il cursore va a un confine di carattere della codifica nuova: in una code page ogni
        // offset e' un confine, in UTF-8 e in UTF-16 no.
        if self.enc.is_utf8() {
            self.cursor = self.snap_to_char(self.cursor);
        } else if self.enc.is_multibyte() {
            self.cursor = self.snap_to_multibyte(self.cursor);
        } else if self.enc.unit() > 1 {
            self.cursor = self.snap_to_unita(self.cursor);
        }
        self.remember_col();
        self.flash = Some(self.cat.tf("flash.encoding", &[self.enc.name()]));
    }

    /// Vero se la modalita' colonna (F8) e' accesa.
    pub fn column_mode(&self) -> bool {
        self.col_mode
    }

    /// Il blocco rettangolare attuale, se c'e' (solo in modalita' colonna): le righe dalla prima
    /// all'ultima e le colonne dell'ancora e del cursore in ordine.
    pub fn column_block(&self) -> Option<ColumnBlock> {
        if !self.col_mode {
            return None;
        }
        self.block.as_ref().and_then(|b| self.column_block_of(b))
    }

    /// La colonna (virtuale) del cursore in modalita' colonna: quella del blocco se c'e', altrimenti
    /// la colonna vera del cursore.
    pub fn vcol(&self) -> u64 {
        match self.block.as_ref() {
            Some(b) => b.vcol,
            None => self.cursor_col() as u64,
        }
    }

    /// I moltiplicatori dei salti di pagina: Alt+Pag, Alt+Maiusc+Pag e Ctrl+Alt+Pag.
    pub fn jump_pages(&self) -> [usize; 3] {
        self.jump_pages
    }

    /// Il tasto Alt+Pag: Ctrl sceglie il terzo moltiplicatore, Maiusc il secondo, altrimenti il primo.
    fn jump_key(&mut self, avanti: bool, ctrl: bool, shift: bool) {
        let i = if ctrl {
            2
        } else if shift {
            1
        } else {
            0
        };
        self.page_jump(avanti, self.jump_pages[i]);
    }

    /// Salta di `pagine` pagine avanti o indietro (i pulsanti e i tasti Alt+Pag). Il cursore e l'inizio della vista si
    /// muovono insieme come in Pag↑/Pag↓, la selezione e il blocco si tolgono, ai bordi del file ci si ferma.
    pub fn page_jump(&mut self, avanti: bool, pagine: usize) {
        if pagine == 0 {
            return;
        }
        self.seal_history();
        self.anchor = None;
        self.block = None;
        if avanti {
            self.pages_down(pagine);
        } else {
            self.pages_up(pagine);
        }
    }

    /// La selezione normalizzata `(inizio, fine esclusa)`: l'ancora e il cursore nell'ordine; `None` se
    /// non c'e' ancora o coincide col cursore (selezione vuota). In modalita' colonna non esiste
    /// selezione di flusso: c'e' il blocco.
    pub fn selection(&self) -> Option<(u64, u64)> {
        if self.col_mode {
            return None;
        }
        let ancora = self.anchor?;
        if ancora == self.cursor {
            None
        } else {
            Some((ancora.min(self.cursor), ancora.max(self.cursor)))
        }
    }

    /// I byte dell'ultima copia (Ctrl+C / Ctrl+X), una volta sola: il ciclo del terminale li manda al
    /// terminale con OSC 52.
    pub fn take_copied(&mut self) -> Option<Vec<u8>> {
        self.copied.take()
    }

    /// Testo incollato dal terminale (l'incolla con le parentesi): come Ctrl+V ma col testo arrivato; in
    /// lettura `flash.read_only`. In un documento non UTF-8 il testo si codifica con la codifica
    /// attiva, tutto o niente.
    pub fn paste_text(&mut self, testo: &str) {
        if self.enc.is_utf8() {
            self.paste_bytes(testo.as_bytes());
            return;
        }
        match self.enc.encode_str(testo) {
            Ok(bytes) => self.paste_bytes(&bytes),
            Err(c) => {
                self.flash = Some(
                    self.cat
                        .tf("flash.not_encodable", &[&c.to_string(), self.enc.name()]),
                );
            }
        }
    }

    /// Il contenuto del documento (per i test: legge tutto, solo per file piccoli).
    pub fn document_text(&self) -> Vec<u8> {
        const PEZZO: u64 = 64 * 1024;
        let mut out = Vec::new();
        let Some(doc) = self.doc.as_ref() else {
            return out;
        };
        let len = doc.len();
        let mut p = 0;
        while p < len {
            let n = PEZZO.min(len - p);
            let blocco = doc.read_range(p, n);
            out.extend_from_slice(&blocco);
            p += n;
        }
        out
    }

    // ----- quello che guarda `ui::draw` -----

    /// Il documento, per il disegno (`view::rows_from` e compagnia); `None` quando si e' usciti.
    pub(crate) fn document(&self) -> Option<&Document> {
        self.doc.as_ref()
    }

    /// Le opzioni di vista con la codifica attiva (il resto e' il predefinito): tutto cio' che nel
    /// disegno e nei movimenti legge i byte come caratteri passa da qui.
    pub(crate) fn view_opts(&self) -> ViewOptions {
        ViewOptions {
            encoding: self.enc,
            ..ViewOptions::default()
        }
    }

    /// La scheda com'e' adesso, da ricordare nella sessione: il file, il cursore, la modalita', la codifica e le
    /// modifiche non salvate. `None` se il documento e' in mano al salvataggio o il percorso non e' testo UTF-8.
    pub fn session_record(&self) -> Option<TabRecord> {
        let doc = self.doc.as_ref()?;
        Some(TabRecord {
            path: self.path.to_str()?.to_string(),
            cursor: self.cursor,
            edit: self.edit,
            encoding: self.enc.name().to_string(),
            state: doc.edit_state(),
        })
    }

    /// La posizione del documento nella storia delle modifiche (cambia a ogni modifica, annulla, ripeti): serve a
    /// capire se la sessione va riscritta. `None` mentre il documento e' in mano al salvataggio.
    pub fn doc_version(&self) -> Option<u64> {
        self.doc.as_ref().map(Document::version)
    }

    /// Riapre una scheda della sessione: il file con la codifica di allora, le sue modifiche non salvate, il cursore e la
    /// modalita'. Se le modifiche non si possono rimettere (il file e' cambiato su disco) la scheda si apre lo stesso,
    /// com'e' il file, e il motivo e' nel secondo valore. Errore se il file non si apre.
    pub fn restore_tab(
        rec: &TabRecord,
        mut cfg: AppConfig,
    ) -> io::Result<(App, Option<RestoreError>)> {
        cfg.encoding = Encoding::by_name(&rec.encoding).or(cfg.encoding);
        let mut app = App::open(Path::new(&rec.path), cfg)?;
        let mut esito = None;
        if let (Some(stato), Some(doc)) = (&rec.state, app.doc.as_mut()) {
            esito = doc.restore_edits(stato).err();
        }
        app.edit = rec.edit && esito.is_none();
        app.cursor = rec.cursor.min(app.max_cursor());
        app.scroll_adjust();
        Ok((app, esito))
    }

    /// Il percorso canonico del file.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Il nome del file aperto (senza cartella).
    pub fn file_name(&self) -> &str {
        &self.file_name
    }

    /// Il messaggio transitorio della riga di stato, se c'e'.
    pub fn flash(&self) -> Option<&str> {
        self.flash.as_deref()
    }

    /// Il vocabolario attivo (ogni stringa visibile viene da qui).
    pub(crate) fn catalog(&self) -> &Catalog {
        &self.cat
    }

    /// Il testo di `key` nella lingua attiva (per la GUI: menu e dialoghi suoi).
    pub fn text<'a>(&'a self, key: &'a str) -> &'a str {
        self.cat.t(key)
    }

    /// Come `text`, con i segnaposto `{}` riempiti da `args`.
    pub fn text_with(&self, key: &str, args: &[&str]) -> String {
        self.cat.tf(key, args)
    }

    /// Prende il tema e la lingua scelti in un'altra scheda, senza salvare le impostazioni (le ha gia' salvate quella).
    /// Un codice di lingua sconosciuto lascia la lingua com'e'.
    pub fn adopt_look(&mut self, theme: ThemeId, lang_code: &str) {
        self.theme = theme;
        if self.cat.code() != lang_code {
            if let Some(l) = self.langs.iter().find(|l| l.code == lang_code) {
                self.cat = Catalog::with(l.clone());
            }
        }
    }

    /// Mostra `testo` nella riga di stato fino al prossimo tasto (per la GUI).
    pub fn set_flash(&mut self, testo: String) {
        self.flash = Some(testo);
    }

    /// Le lingue disponibili (le righe del menu di F7).
    pub(crate) fn langs(&self) -> &[Lang] {
        &self.langs
    }

    /// Il testo del campo "cerca" del dialogo Sostituisci.
    pub(crate) fn rep_find(&self) -> &str {
        &self.rep_find
    }

    /// Il testo del campo "con" del dialogo Sostituisci.
    pub(crate) fn rep_with(&self) -> &str {
        &self.rep_with
    }

    /// Quale campo ha il cursore nel dialogo Sostituisci (0 cerca, 1 con).
    pub(crate) fn rep_field(&self) -> u8 {
        self.rep_field
    }

    /// Se Sostituisci vale per tutte le schede.
    pub(crate) fn rep_all(&self) -> bool {
        self.rep_all
    }

    /// Lo stato (anteprima o errore) del dialogo Sostituisci.
    pub(crate) fn rep_status(&self) -> &str {
        &self.rep_status
    }

    /// Il testo scritto nel prompt "apri".
    pub(crate) fn open_text(&self) -> &str {
        &self.open_text
    }

    /// Il percorso confermato nel prompt "apri" (Ctrl+O), una volta sola: chi ospita l'app lo apre.
    pub fn take_open_request(&mut self) -> Option<PathBuf> {
        self.open_request.take()
    }

    /// Chiede di chiudere questo documento: senza modifiche si chiude subito (`should_quit`), con modifiche si chiede
    /// prima (salva, scarta o annulla), come F10.
    pub fn request_close(&mut self) {
        self.quit_request();
    }

    /// Il testo scritto nel prompt "vai a".
    pub(crate) fn goto_text(&self) -> &str {
        &self.goto_text
    }

    /// Il testo scritto nel prompt di ricerca.
    pub(crate) fn find_text(&self) -> &str {
        &self.find_text
    }

    /// La percentuale dell'indicizzazione in corso (0..=100); `None` se non c'e' un lavoro in corso.
    pub(crate) fn index_progress(&self) -> Option<u64> {
        let job = self.index_job.as_ref()?;
        let len = self.document_len();
        if len == 0 {
            return Some(0);
        }
        Some(job.progress.load(Ordering::Relaxed).saturating_mul(100) / len)
    }

    /// La percentuale della ricerca in corso (0..=100); `None` se non c'e' una ricerca in corso.
    pub(crate) fn search_progress(&self) -> Option<u64> {
        let job = self.search_job.as_ref()?;
        if job.len == 0 {
            return Some(0);
        }
        Some(job.progress.load(Ordering::Relaxed).saturating_mul(100) / job.len)
    }

    /// La percentuale del salvataggio in corso (0..=100); `None` se non c'e' un salvataggio in corso.
    pub(crate) fn save_progress(&self) -> Option<u64> {
        let job = self.save_job.as_ref()?;
        if job.len == 0 {
            return Some(0);
        }
        Some(job.progress.load(Ordering::Relaxed).saturating_mul(100) / job.len)
    }

    /// Le righe di testo dell'ultimo disegno (gutter e testo): le ridisegna `ui::draw` mentre un
    /// salvataggio tiene il documento nel suo thread.
    pub(crate) fn drawn_rows(&self) -> &[(String, String)] {
        &self.drawn_rows
    }

    /// Ricorda le righe di testo di questo disegno (lo chiama `ui::draw`).
    pub(crate) fn set_drawn_rows(&mut self, righe: &[(String, String)]) {
        self.drawn_rows = righe.to_vec();
    }

    /// Ricorda dove comincia ogni riga di questo disegno (lo chiama `ui::draw`), per i clic del mouse.
    pub(crate) fn set_row_starts(&mut self, starts: Vec<u64>) {
        self.row_starts = starts;
    }

    /// Larghezza del gutter: le cifre della lunghezza del documento piu' il separatore.
    pub(crate) fn gutter_w(&self) -> usize {
        cifre(self.document_len()) + 1
    }

    /// Il numero (1-based) della riga vera che inizia a `start`, per il gutter: `None` se l'indice non e' pronto
    /// o `start` e' oltre la fine (la riga vuota finale ha il numero `line_count + 1`).
    pub(crate) fn gutter_number(&mut self, start: u64) -> Option<u64> {
        let doc = self.doc.as_mut()?;
        if !doc.has_lines() {
            return None;
        }
        let len = doc.len();
        if start >= len {
            doc.line_count().map(|conteggio| conteggio + 1)
        } else {
            doc.line_at_offset(start).map(|riga| riga + 1)
        }
    }

    /// Il numero (1-based) della riga del cursore per la riga di stato; `None` se l'indice non e' pronto.
    pub(crate) fn cursor_line(&mut self) -> Option<u64> {
        let doc = self.doc.as_mut()?;
        if !doc.has_lines() {
            return None;
        }
        let len = doc.len();
        if self.cursor < len {
            doc.line_at_offset(self.cursor).map(|riga| riga + 1)
        } else {
            let conteggio = doc.line_count()?;
            let finisce_nl = finisce_con_a_capo(doc);
            Some(if len == 0 || finisce_nl {
                conteggio + 1
            } else {
                conteggio
            })
        }
    }

    /// Inizio della riga visuale che contiene il cursore.
    pub(crate) fn cursor_row(&self) -> u64 {
        match self.doc.as_ref() {
            Some(doc) => scan::row_start(doc, self.cursor, MAX_SCAN).0,
            None => 0,
        }
    }

    /// Colonna di terminale (da 0) del cursore nella sua riga.
    pub(crate) fn cursor_col(&self) -> usize {
        let Some(doc) = self.doc.as_ref() else {
            return 0;
        };
        view::col_of(doc, self.cursor_row(), self.cursor, &self.view_opts())
    }

    /// Ricorda la posizione del cursore sullo schermo (la scrive `ui::draw`).
    pub(crate) fn set_cursor_xy(&mut self, xy: Option<(u16, u16)>) {
        self.cursor_xy = xy;
    }

    // ----- geometria dello schermo -----

    /// Le righe di testo dello schermo (le altre tre: titolo, stato, barra dei tasti).
    fn text_rows(&self) -> usize {
        (self.h as usize).saturating_sub(3)
    }

    /// Le celle di testo per riga (la larghezza meno il gutter).
    fn cells(&self) -> usize {
        (self.w as usize).saturating_sub(self.gutter_w())
    }

    /// Il limite del cursore: in lettura non oltre l'inizio dell'ultimo a-capo di un file che ci
    /// finisce (l'unita' intera dell'a-capo, non il byte prima).
    fn max_cursor(&self) -> u64 {
        let Some(doc) = self.doc.as_ref() else {
            return 0;
        };
        let len = doc.len();
        if self.edit || len == 0 {
            len
        } else if finisce_con_a_capo(doc) {
            len - doc.newline().unit() as u64
        } else {
            len
        }
    }

    /// Il cursore resta entro i limiti del modo.
    fn clamp_cursor(&mut self) {
        let max = self.max_cursor();
        if self.cursor > max {
            self.cursor = max;
        }
    }

    /// Ricalcola la colonna ricordata dalla posizione del cursore (la usano i movimenti orizzontali).
    fn remember_col(&mut self) {
        let Some(doc) = self.doc.as_ref() else {
            return;
        };
        let row = scan::row_start(doc, self.cursor, MAX_SCAN).0;
        self.want_col = view::col_of(doc, row, self.cursor, &self.view_opts()) as u64;
    }

    /// Porta il cursore sulla riga che inizia in `row`, alla colonna ricordata.
    fn cursor_at_col(&mut self, row: u64) {
        let Some(doc) = self.doc.as_ref() else {
            return;
        };
        self.cursor =
            view::offset_at_col(doc, row, self.want_col, &self.view_opts()).min(self.max_cursor());
    }

    // ----- movimenti -----

    /// Il movimento `m` del cursore: quello del tasto senza Maiusc.
    fn muovi(&mut self, m: Mov) {
        match m {
            Mov::Up => self.move_up(),
            Mov::Down => self.move_down(),
            Mov::Left => self.move_left(),
            Mov::Right => self.move_right(),
            Mov::Home => self.move_home(),
            Mov::End => self.move_end(),
            Mov::FileStart => self.move_file_start(),
            Mov::FileEnd => self.move_file_end(),
            Mov::PageUp => self.page_up(),
            Mov::PageDown => self.page_down(),
        }
    }

    /// Maiusc+movimento: estende la selezione. Se l'ancora manca la si mette al cursore, poi il movimento
    /// e' quello del tasto senza Maiusc; l'ancora resta finche' la selezione e' attiva (anche se torna vuota).
    /// In modalita' colonna lo stesso movimento estende il blocco rettangolare.
    fn extend_selection(&mut self, m: Mov) {
        if self.col_mode {
            self.extend_block(m);
            return;
        }
        if self.anchor.is_none() {
            self.anchor = Some(self.cursor);
        }
        self.muovi(m);
    }

    /// Un movimento senza Maiusc: toglie l'ancora e muove il cursore da dove sta. In modalita' colonna
    /// toglie anche il blocco (il movimento riparte dal cursore, come fuori dalla modalita').
    fn move_no_anchor(&mut self, m: Mov) {
        self.anchor = None;
        self.block = None;
        self.muovi(m);
    }

    /// Left senza Maiusc: con una selezione il cursore va al suo inizio (ancora tolta, senza muoversi
    /// oltre); senza selezione si muove di un carattere a sinistra.
    fn collapse_left(&mut self) {
        let sel = self.selection();
        self.anchor = None;
        self.block = None;
        if let Some((da, _)) = sel {
            self.cursor = da.min(self.max_cursor());
            self.remember_col();
            self.scroll_adjust();
            return;
        }
        self.move_left();
    }

    /// Right senza Maiusc: con una selezione il cursore va alla sua fine (ancora tolta, senza muoversi
    /// oltre); senza selezione si muove di un carattere a destra.
    fn collapse_right(&mut self) {
        let sel = self.selection();
        self.anchor = None;
        self.block = None;
        if let Some((_, fino)) = sel {
            self.cursor = fino.min(self.max_cursor());
            self.remember_col();
            self.scroll_adjust();
            return;
        }
        self.move_right();
    }

    /// Esc: toglie l'ancora e il blocco della modalita' colonna; il cursore resta dov'e' (anche se la
    /// selezione era rovesciata).
    fn collapse_escape(&mut self) {
        self.anchor = None;
        self.block = None;
    }

    fn move_up(&mut self) {
        let Some(doc) = self.doc.as_ref() else {
            return;
        };
        let row = scan::row_start(doc, self.cursor, MAX_SCAN).0;
        let Some(prev) = scan::prev_row_start(doc, row, MAX_SCAN) else {
            return;
        };
        self.cursor_at_col(prev);
        self.scroll_adjust();
    }

    fn move_down(&mut self) {
        let Some(doc) = self.doc.as_ref() else {
            return;
        };
        let row = scan::row_start(doc, self.cursor, MAX_SCAN).0;
        let prossima = scan::next_row(doc, row, MAX_SCAN);
        let len = doc.len();
        let finisce_nl = finisce_con_a_capo(doc);
        match prossima {
            Next::Line(start) | Next::Cut(start) => self.cursor_at_col(start),
            Next::End => {
                // Ultima riga: in modifica il cursore puo' scendere sulla riga vuota finale.
                if self.edit && (len == 0 || finisce_nl) {
                    self.cursor = len;
                }
            }
        }
        self.scroll_adjust();
    }

    fn move_left(&mut self) {
        if self.cursor == 0 {
            return;
        }
        let Some(doc) = self.doc.as_ref() else {
            return;
        };
        // 4 byte bastano quasi sempre; le multibyte asiatiche ne vogliono di piu' (`back_window`)
        let da = self.cursor.saturating_sub(self.enc.back_window() as u64);
        let buf = doc.read_range(da, self.cursor - da);
        self.cursor -= self.enc.last_char_len(&buf) as u64;
        self.remember_col();
        self.scroll_adjust();
    }

    fn move_right(&mut self) {
        let max = self.max_cursor();
        if self.cursor >= max {
            return;
        }
        let Some(doc) = self.doc.as_ref() else {
            return;
        };
        let restanti = (doc.len() - self.cursor).min(4);
        let buf = doc.read_range(self.cursor, restanti);
        let n = self.enc.first_char(&buf).map_or(1, |(_, n)| n) as u64;
        self.cursor = (self.cursor + n).min(max);
        self.remember_col();
        self.scroll_adjust();
    }

    fn move_home(&mut self) {
        let Some(doc) = self.doc.as_ref() else {
            return;
        };
        let row = scan::row_start(doc, self.cursor, MAX_SCAN).0;
        self.cursor = row.min(self.max_cursor());
        self.remember_col();
        self.scroll_adjust();
    }

    fn move_end(&mut self) {
        let Some(doc) = self.doc.as_ref() else {
            return;
        };
        let row = scan::row_start(doc, self.cursor, MAX_SCAN).0;
        let fine = scan::row_end(doc, row, MAX_SCAN);
        self.cursor = fine.min(self.max_cursor());
        self.remember_col();
        self.scroll_adjust();
    }

    fn move_file_start(&mut self) {
        self.cursor = 0;
        self.top = 0;
        self.hscroll = 0;
        self.remember_col();
    }

    fn move_file_end(&mut self) {
        let len = self.document_len();
        self.cursor = if self.edit || len == 0 {
            len
        } else {
            let Some(doc) = self.doc.as_ref() else {
                return;
            };
            // L'ultima riga e' quella che contiene l'ultimo byte; la sua fine sta prima dell'eventuale a-capo.
            let ultima = scan::row_start(doc, len - 1, MAX_SCAN).0;
            scan::row_end(doc, ultima, MAX_SCAN).min(self.max_cursor())
        };
        self.remember_col();
        self.scroll_adjust();
    }

    /// Pagina in giu': cursore e `top` si spostano dello stesso numero di righe, il cursore resta alla stessa riga
    /// dello schermo; ai bordi del file si ferma.
    fn page_down(&mut self) {
        self.pages_down(1);
    }

    /// Salta `pagine` pagine in giu' (una pagina e' l'altezza del testo meno una riga). Pag↓ ne salta una; i salti
    /// Alt+Pag↓ ne saltano di piu'. Cammina le righe una per una, quindi il costo e' proporzionale al salto (al massimo
    /// `JUMP_MAX` pagine).
    fn pages_down(&mut self, pagine: usize) {
        let page = self.text_rows().saturating_sub(1).saturating_mul(pagine);
        if page == 0 {
            return;
        }
        let last = self.last_top();
        let sotto_cur = self.rows_after(self.cursor_row(), page);
        let sotto_top: Vec<u64> = self
            .rows_after(self.top, page)
            .into_iter()
            .take_while(|&r| r <= last)
            .collect();
        let n = sotto_cur.len().min(sotto_top.len());
        if n == 0 {
            return;
        }
        self.cursor_at_col(sotto_cur[n - 1]);
        self.top = sotto_top[n - 1];
        self.scroll_adjust();
    }

    /// Pagina in su', come `page_down` all'indietro.
    fn page_up(&mut self) {
        self.pages_up(1);
    }

    /// `pagine` pagine in su', come `pages_down` all'indietro.
    fn pages_up(&mut self, pagine: usize) {
        let page = self.text_rows().saturating_sub(1).saturating_mul(pagine);
        if page == 0 {
            return;
        }
        let sopra_cur = self.rows_before(self.cursor_row(), page);
        let sopra_top = self.rows_before(self.top, page);
        let n = sopra_cur.len().min(sopra_top.len());
        if n == 0 {
            return;
        }
        self.cursor_at_col(sopra_cur[n - 1]);
        self.top = sopra_top[n - 1];
        self.scroll_adjust();
    }

    /// La rotella: tre righe per scatto, mai oltre l'ultima pagina.
    fn wheel(&mut self, down: bool) {
        if self.text_rows() == 0 || self.doc.is_none() {
            return;
        }
        let last = self.last_top();
        if down {
            for _ in 0..WHEEL_ROWS {
                if self.top >= last {
                    break;
                }
                let Some(doc) = self.doc.as_ref() else {
                    break;
                };
                match scan::next_row(doc, self.top, MAX_SCAN) {
                    Next::Line(r) | Next::Cut(r) => self.top = r.min(last),
                    Next::End => break,
                }
            }
        } else {
            for _ in 0..WHEEL_ROWS {
                let Some(doc) = self.doc.as_ref() else {
                    break;
                };
                match scan::prev_row_start(doc, self.top, MAX_SCAN) {
                    Some(r) => self.top = r,
                    None => break,
                }
            }
        }
        self.follow_window();
    }

    /// Dopo uno spostamento di `top`: se il cursore e' uscito dalla finestra va sulla riga visibile piu' vicina,
    /// alla colonna ricordata.
    fn follow_window(&mut self) {
        let text_rows = self.text_rows();
        if text_rows == 0 {
            return;
        }
        let row = self.cursor_row();
        if row < self.top {
            self.cursor_at_col(self.top);
            return;
        }
        let visibili = self.rows_after(self.top, text_rows - 1);
        if row == self.top || visibili.contains(&row) {
            return;
        }
        let ultima = visibili.last().copied().unwrap_or(self.top);
        self.cursor_at_col(ultima);
    }

    /// Il cursore resta visibile: `top` e `hscroll` si spostano il meno possibile (per Ctrl+End la riga del
    /// cursore e' l'ultima dello schermo; in orizzontale la colonna sta in `[hscroll, hscroll + celle - 1]`).
    fn scroll_adjust(&mut self) {
        let text_rows = self.text_rows();
        if text_rows == 0 {
            return;
        }
        let row = self.cursor_row();
        if row < self.top {
            // Sopra la finestra: la riga del cursore diventa la prima visibile.
            self.top = row;
        } else {
            let visibili = self.rows_after(self.top, text_rows - 1);
            if row != self.top && !visibili.contains(&row) {
                // Sotto la finestra: la riga del cursore diventa l'ultima visibile.
                let sopra = self.rows_before(row, text_rows - 1);
                self.top = sopra.last().copied().unwrap_or(row);
            }
        }
        let cells = self.cells();
        if cells == 0 {
            return;
        }
        // In modalita' colonna resta visibile la colonna virtuale del cursore, anche oltre la fine
        // della riga (il cursore del terminale sta li').
        let col = if self.col_mode {
            usize::try_from(self.vcol()).unwrap_or(usize::MAX)
        } else {
            self.cursor_col()
        };
        if col < self.hscroll {
            self.hscroll = col;
        } else if col > self.hscroll + cells - 1 {
            self.hscroll = col + 1 - cells;
        }
    }

    /// Gli inizi delle righe visuali dopo `da`, al piu' `quante` (la prima e' la riga successiva).
    fn rows_after(&self, da: u64, quante: usize) -> Vec<u64> {
        let mut v = Vec::new();
        let Some(doc) = self.doc.as_ref() else {
            return v;
        };
        let mut p = da;
        while v.len() < quante {
            match scan::next_row(doc, p, MAX_SCAN) {
                Next::Line(r) | Next::Cut(r) => {
                    v.push(r);
                    p = r;
                }
                Next::End => break,
            }
        }
        v
    }

    /// Gli inizi delle righe visuali prima di `da`, al piu' `quante` (la prima e' la riga precedente).
    fn rows_before(&self, da: u64, quante: usize) -> Vec<u64> {
        let mut v = Vec::new();
        let Some(doc) = self.doc.as_ref() else {
            return v;
        };
        let mut p = da;
        while v.len() < quante {
            match scan::prev_row_start(doc, p, MAX_SCAN) {
                Some(r) => {
                    v.push(r);
                    p = r;
                }
                None => break,
            }
        }
        v
    }

    /// Il primo `top` dell'ultima pagina: l'ultima riga del documento in fondo allo schermo.
    fn last_top(&self) -> u64 {
        let Some(doc) = self.doc.as_ref() else {
            return 0;
        };
        let len = doc.len();
        if len == 0 {
            return 0;
        }
        let ultima = scan::row_start(doc, len - 1, MAX_SCAN).0;
        self.rows_before(ultima, self.text_rows().saturating_sub(1))
            .last()
            .copied()
            .unwrap_or(ultima)
    }

    // ----- modo e scrittura -----

    /// F4: alterna lettura e modifica; tornando in lettura il cursore si limita all'ultimo a-capo.
    fn toggle_edit(&mut self) {
        self.edit = !self.edit;
        if self.edit {
            self.flash = Some(self.cat.t("flash.edit_on").to_string());
        } else {
            self.flash = Some(self.cat.t("flash.view_on").to_string());
            let max = self.max_cursor();
            if self.cursor > max {
                self.cursor = max;
                self.scroll_adjust();
            }
        }
    }

    /// In lettura la scrittura e' rifiutata con un suggerimento.
    fn refuse_write(&mut self) {
        if !self.edit {
            self.flash = Some(self.cat.t("flash.read_only").to_string());
        }
    }

    /// Un carattere della tastiera (anche con SHIFT): si inserisce al posto dell'eventuale selezione e il
    /// cursore avanza di tutti i suoi byte. In modalita' colonna con un blocco entra su tutte le righe
    /// del blocco. In una code page il carattere deve starci: se non si puo' scrivere niente cambia
    /// (nessuna modifica, nessun passo di storia) e il messaggio lo dice.
    fn insert_char(&mut self, c: char) {
        // In una code page i byte del carattere li dice la codifica.
        if !self.enc.is_utf8() {
            let Some(testo) = self.enc.encode(c) else {
                self.flash = Some(
                    self.cat
                        .tf("flash.not_encodable", &[&c.to_string(), self.enc.name()]),
                );
                return;
            };
            if self.col_mode && self.block.is_some() {
                self.column_type(&testo);
                return;
            }
            self.insert_over_selection(&testo);
            return;
        }
        // In UTF-8 il percorso e' quello di sempre: nessuna conversione, nessun controllo.
        let mut buf = [0u8; 4];
        let testo = c.encode_utf8(&mut buf);
        if self.col_mode && self.block.is_some() {
            self.column_type(testo.as_bytes());
            return;
        }
        self.insert_over_selection(testo.as_bytes());
    }

    /// Enter: un a-capo, quello della codifica attiva (in UTF-16 un'unita' a 16 bit). Si aggiunge
    /// al passo di storia corrente e poi lo chiude. In modalita' colonna toglie prima il blocco,
    /// poi e' un Enter normale al cursore; in lettura il messaggio e il blocco resta.
    fn insert_enter(&mut self) {
        if self.col_mode && self.block.is_some() {
            if !self.edit {
                self.refuse_write();
                return;
            }
            self.block = None;
        }
        if self.insert_over_selection(&self.eol.bytes(self.enc)) {
            self.seal_history();
        }
    }

    /// Tab: un tabulatore (le celle le calcola la visualizzazione: qui e' un carattere come gli
    /// altri, nei byte della codifica attiva). In modalita' colonna con un blocco entra su tutte
    /// le righe del blocco.
    fn insert_tab(&mut self) {
        // Il tabulatore nei byte della codifica attiva ('\t' e' ASCII: ogni codifica lo scrive;
        // in UTF-16 un'unita' a 16 bit).
        let Some(testo) = self.enc.encode('\t') else {
            return;
        };
        if self.col_mode && self.block.is_some() {
            self.column_type(&testo);
            return;
        }
        self.insert_over_selection(&testo);
    }

    /// Inserisce `testo` al posto dell'eventuale selezione, che si toglie prima nello stesso passo di
    /// storia (un carattere, Enter, Tab o un incolla: un solo annulla). In lettura la scrittura e'
    /// rifiutata e la selezione non si perde. Vero se ha inserito.
    fn insert_over_selection(&mut self, testo: &[u8]) -> bool {
        if !self.edit {
            self.refuse_write();
            return false;
        }
        if self.selection().is_some() && !self.delete_selection() {
            return false;
        }
        // davanti al BOM non si scrive: il testo va subito dopo
        if self.cursor < self.bom {
            self.cursor = self.bom;
        }
        self.insert_at_cursor(testo)
    }

    /// Backspace: con una selezione toglie solo quella; senza toglie il carattere prima del cursore. In
    /// lettura e' rifiutato e la selezione non si perde. In modalita' colonna con un blocco agisce sul
    /// blocco.
    fn backspace_key(&mut self) {
        if self.col_mode && self.block.is_some() {
            self.column_backspace();
            return;
        }
        if self.selection().is_some() {
            if !self.edit {
                self.refuse_write();
                return;
            }
            self.delete_selection();
            return;
        }
        self.delete_before();
    }

    /// Delete: con una selezione toglie solo quella; senza toglie il carattere sotto il cursore. In lettura
    /// e' rifiutato e la selezione non si perde. In modalita' colonna con un blocco agisce sul blocco.
    fn delete_key(&mut self) {
        if self.col_mode && self.block.is_some() {
            self.column_delete();
            return;
        }
        if self.selection().is_some() {
            if !self.edit {
                self.refuse_write();
                return;
            }
            self.delete_selection();
            return;
        }
        self.delete_after();
    }

    /// Inserisce `testo` al cursore (in modifica) e porta il cursore alla fine di cio' che ha scritto;
    /// dopo ogni modifica la colonna ricordata si ricalcola e il cursore resta visibile. Vero se ha
    /// inserito; un errore del core diventa un messaggio.
    fn insert_at_cursor(&mut self, testo: &[u8]) -> bool {
        if !self.edit {
            self.refuse_write();
            return false;
        }
        let Some(doc) = self.doc.as_mut() else {
            return false;
        };
        match doc.insert(self.cursor, testo) {
            Ok(()) => {
                self.cursor += testo.len() as u64;
                // Ogni modifica toglie la selezione.
                self.anchor = None;
                self.remember_col();
                self.scroll_adjust();
                true
            }
            Err(e) => {
                self.flash = Some(self.cat.tf("flash.edit_failed", &[&format!("{:?}", e)]));
                false
            }
        }
    }

    /// Backspace: toglie il carattere prima del cursore, intero (anche multibyte: l'a-capo e' un carattere
    /// come gli altri e unisce le righe). Al cursore 0 non fa nulla.
    fn delete_before(&mut self) {
        if !self.edit {
            self.refuse_write();
            return;
        }
        if self.cursor <= self.bom {
            return;
        }
        let Some(doc) = self.doc.as_mut() else {
            return;
        };
        // Il carattere precedente sta nei 4 byte prima del cursore (un carattere UTF-8 occupa al piu' 4
        // byte, in una code page uno; un byte non valido e' un carattere tutto suo).
        let da = self.cursor.saturating_sub(self.enc.back_window() as u64);
        let buf = doc.read_range(da, self.cursor - da);
        let n = self.enc.last_char_len(&buf) as u64;
        if let Err(e) = doc.delete(self.cursor - n, n) {
            self.flash = Some(self.cat.tf("flash.edit_failed", &[&format!("{:?}", e)]));
            return;
        }
        self.cursor -= n;
        // Ogni modifica toglie la selezione.
        self.anchor = None;
        self.remember_col();
        self.scroll_adjust();
    }

    /// Delete: toglie il carattere sotto il cursore, intero (anche multibyte). Alla fine del documento non
    /// fa nulla.
    fn delete_after(&mut self) {
        if !self.edit {
            self.refuse_write();
            return;
        }
        // il BOM non si cancella: Delete davanti ad esso toglie il primo carattere dopo
        if self.cursor < self.bom {
            self.cursor = self.bom;
        }
        let Some(doc) = self.doc.as_mut() else {
            return;
        };
        let len = doc.len();
        if self.cursor >= len {
            return;
        }
        // Il carattere successivo comincia al cursore: ne bastano 4 byte (in una code page uno).
        let restanti = (len - self.cursor).min(4);
        let buf = doc.read_range(self.cursor, restanti);
        let n = self.enc.first_char(&buf).map_or(1, |(_, n)| n) as u64;
        if let Err(e) = doc.delete(self.cursor, n) {
            self.flash = Some(self.cat.tf("flash.edit_failed", &[&format!("{:?}", e)]));
            return;
        }
        // Ogni modifica toglie la selezione.
        self.anchor = None;
        self.remember_col();
        self.scroll_adjust();
    }

    /// Ctrl+Z: annulla l'ultimo passo della storia (solo in modifica). Il cursore va a `pos +
    /// max(delta, 0)` con `pos` restituito dal core e delta = lunghezza dopo meno lunghezza prima.
    fn undo(&mut self) {
        if !self.edit {
            self.refuse_write();
            return;
        }
        // L'annullamento toglie la selezione e il blocco della modalita' colonna: il testo sotto e'
        // cambiato.
        self.anchor = None;
        self.block = None;
        let Some(doc) = self.doc.as_mut() else {
            return;
        };
        let prima = doc.len();
        match doc.undo() {
            Some(pos) => {
                let avanzo = doc.len().saturating_sub(prima);
                self.cursor = pos + avanzo;
                self.flash = Some(self.cat.t("flash.undo").to_string());
                self.remember_col();
                self.scroll_adjust();
            }
            None => self.flash = Some(self.cat.t("flash.nothing_to_undo").to_string()),
        }
    }

    /// Ctrl+Y / Ctrl+Shift+Z: ripete l'ultimo passo annullato (solo in modifica); il cursore si piazza
    /// come nell'annulla.
    fn redo(&mut self) {
        if !self.edit {
            self.refuse_write();
            return;
        }
        // Come l'annulla: il ripeti toglie la selezione e il blocco.
        self.anchor = None;
        self.block = None;
        let Some(doc) = self.doc.as_mut() else {
            return;
        };
        let prima = doc.len();
        match doc.redo() {
            Some(pos) => {
                let avanzo = doc.len().saturating_sub(prima);
                self.cursor = pos + avanzo;
                self.flash = Some(self.cat.t("flash.redo").to_string());
                self.remember_col();
                self.scroll_adjust();
            }
            None => self.flash = Some(self.cat.t("flash.nothing_to_redo").to_string()),
        }
    }

    /// Chiude il passo di storia aperto: lo chiamano i tasti e il mouse che non continuano la scrittura
    /// (frecce, Home/End, Delete, Tab, Ctrl+..., F-tasti).
    fn seal_history(&mut self) {
        if let Some(doc) = self.doc.as_mut() {
            doc.seal();
        }
    }

    // ----- salvataggio in background -----

    /// F2 / Ctrl+S (e la 'S' del dialogo di uscita): il salvataggio. Il documento SI MUOVE nel thread,
    /// che lo restituisce comunque con l'esito; finche' gira il dialogo `Saving` tiene tutti i tasti e lo
    /// schermo ridisegna il testo dell'ultimo disegno. Senza modifiche non c'e' niente da salvare.
    /// `quit_after`: dopo un salvataggio riuscito l'app esce.
    fn start_save(&mut self, quit_after: bool) {
        if !self.is_modified() {
            self.flash = Some(self.cat.t("flash.nothing_to_save").to_string());
            return;
        }
        let len = self.document_len();
        let Some(mut doc) = self.doc.take() else {
            return;
        };
        let cancel = Arc::new(AtomicBool::new(false));
        let progress = Arc::new(AtomicU64::new(0));
        let (tx, rx) = mpsc::channel();
        let cancel_thread = Arc::clone(&cancel);
        let progress_thread = Arc::clone(&progress);
        std::thread::spawn(move || {
            let esito = doc.save(&cancel_thread, &progress_thread);
            // Il documento torna comunque: anche con errore non si perde.
            let _ = tx.send((doc, esito));
        });
        self.save_job = Some(SaveJob {
            rx,
            progress,
            cancel,
            len,
            quit_after,
        });
        self.dialog = Some(DialogKind::Saving);
    }

    /// La fine del salvataggio (la chiama `tick`): il documento torna al suo posto con l'esito e il
    /// dialogo si chiude; dopo un salvataggio riuscito l'indice righe si ricostruisce in background (il
    /// file su disco e' cambiato) e, se era "salva ed esci", l'app esce.
    fn tick_save(&mut self) {
        let Some(job) = self.save_job.as_mut() else {
            return;
        };
        match job.rx.try_recv() {
            Ok((doc, esito)) => {
                let quit_after = job.quit_after;
                self.save_job = None;
                // Il dialogo si chiude col lavoro: finche' girava teneva tutti i tasti.
                self.dialog = None;
                self.doc = Some(doc);
                // Il cursore, `top` e `hscroll` restano com'erano (riaggiustati se serve).
                self.scroll_adjust();
                match esito {
                    Ok(()) => {
                        self.flash = Some(self.cat.tf("flash.saved", &[&self.file_name]));
                        self.rebuild_index();
                        // "Salva ed esci": solo un salvataggio riuscito fa uscire.
                        if quit_after {
                            self.quit = true;
                        }
                    }
                    Err(SaveError::Cancelled) => {
                        self.flash = Some(self.cat.t("flash.save_cancelled").to_string());
                    }
                    Err(SaveError::SourceChanged) => {
                        self.flash = Some(self.cat.t("flash.source_changed").to_string());
                    }
                    Err(e) => {
                        self.flash = Some(self.cat.tf("flash.save_failed", &[&e.to_string()]));
                    }
                }
            }
            Err(mpsc::TryRecvError::Empty) => {}
            Err(mpsc::TryRecvError::Disconnected) => {
                // Difensivo: il thread e' terminato senza consegnare il documento, non si aspetta piu'.
                self.save_job = None;
                self.dialog = None;
            }
        }
    }

    /// Fa ripartire la costruzione dell'indice righe in background, con la forma dell'a-capo del
    /// documento: dopo un salvataggio riuscito (il file su disco e' cambiato) o dopo un cambio di
    /// forma (l'indice vecchio conta gli a-capo dell'altra). Stesso meccanismo dell'apertura.
    fn rebuild_index(&mut self) {
        if !self.index {
            return;
        }
        let Some(doc) = self.doc.as_ref() else {
            return;
        };
        let newline = doc.newline();
        // Un'indicizzazione partita prima si ferma: indicizzerebbe il file vecchio, o con la
        // forma vecchia dell'a-capo.
        self.cancel.store(true, Ordering::Relaxed);
        self.cancel = Arc::new(AtomicBool::new(false));
        let progress = Arc::new(AtomicU64::new(0));
        self.index_job = Some(spawn_index_job(
            doc.path(),
            newline,
            &self.cancel,
            &progress,
        ));
    }
}

impl Drop for App {
    fn drop(&mut self) {
        // I thread in corso si fermano: non servono piu' a nessuno.
        self.cancel.store(true, Ordering::Relaxed);
        if let Some(job) = &self.save_job {
            job.cancel.store(true, Ordering::Relaxed);
        }
        if let Some(job) = &self.search_job {
            job.cancel.store(true, Ordering::Relaxed);
        }
        if let Some(job) = &self.replace_job {
            job.cancel.store(true, Ordering::Relaxed);
        }
    }
}

/// La specie di un carattere per il doppio clic: 0 parola (lettere, cifre, '_'), 1 spazio, 2 segno.
fn char_class(c: char) -> u8 {
    if c.is_alphanumeric() || c == '_' {
        0
    } else if c.is_whitespace() {
        1
    } else {
        2
    }
}
