//! Modalita' colonna: un blocco rettangolare di celle di terminale sopra un `Document`, indipendente dall'interfaccia.
//!
//! Il blocco e' un intervallo di RIGHE VISUALI (come `scan`: righe vere, o pezzi di riga tagliati dopo `max_scan`) e un
//! intervallo di COLONNE `[col_a, col_b)` (colonne di terminale, come `view::col_of`). Niente indice righe: si cammina
//! con `scan::next_row`, e il costo dipende dalle righe del blocco, non dalla dimensione del file.
//!
//! Regola dei confini (la specifica esatta; i test la verificano con un oracolo ingenuo): su ogni riga un carattere
//! appartiene al blocco se la sua colonna di INIZIO sta in `[col_a, col_b)`. Un carattere largo o una tabulazione che
//! inizia prima di `col_a` ne e' fuori anche se si estende dentro; uno che inizia dentro ne e' dentro anche se si
//! estende oltre `col_b`. Un `\r` nascosto (ultimo byte di una riga che termina con `\n`, vedi `view`) e l'`\n` non
//! fanno mai parte di un blocco: la riga "finisce" prima di loro.
//!
//! Ordine delle operazioni di modifica: sempre dall'ultima riga alla prima, cosi' gli offset delle righe precedenti
//! restano veri. Un'operazione che fallisce PRIMA di modificare non cambia niente. Tutte le modifiche vanno nello
//! stesso passo della storia (il chiamante chiude il passo con `Document::seal`).

use crate::document::Document;
use crate::edit::EditError;
use crate::newline::Newline;
use crate::scan::{next_row, row_start, Next};
use crate::view::{cella, col_of, fine_riga, Caratteri, ViewOptions};

/// Errori delle operazioni sul blocco.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlockError {
    /// Il blocco ha piu' righe di `max_rows`.
    TooManyRows,
    /// Il testo del blocco supera `max_bytes`.
    TooManyBytes,
    /// `paste_block`: le righe da incollare sono piu' delle righe del blocco.
    NotEnoughRows,
    /// Una modifica del documento e' fallita.
    Edit(EditError),
}

/// Un blocco rettangolare: le righe visuali il cui inizio sta in `[first_row, last_row]` (estremi inclusi, `first_row
/// <= last_row`; di norma sono inizi di riga, ma dopo una modifica un valore che non lo e' piu' vale lo stesso) e colonne `[col_a, col_b)` con `col_a <= col_b` (uguali = blocco di larghezza zero: un punto di inserimento
/// per riga).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ColumnBlock {
    pub first_row: u64,
    pub last_row: u64,
    pub col_a: u64,
    pub col_b: u64,
}

/// Cio' che il blocco copre su una riga.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RowSpan {
    /// Inizio della riga visuale.
    pub row_start: u64,
    /// Primo byte del blocco su questa riga: il primo carattere con colonna di inizio >= `col_a`; se non ce n'e', la
    /// fine della riga.
    pub start: u64,
    /// Subito dopo l'ultimo byte del blocco: il primo carattere con colonna di inizio >= `col_b`; se non ce n'e', la
    /// fine della riga. Sempre `end >= start`.
    pub end: u64,
    /// Larghezza della riga in celle (senza il `\r` nascosto).
    pub width: u64,
    /// `col_a <= width`: sulla riga esiste un punto di inserimento a `col_a` (o al primo carattere dopo).
    pub reaches: bool,
}

/// Il blocco fra `anchor` (offset di un confine di carattere) e `cursor`. Le righe sono quelle delle due
/// posizioni (`scan::row_start`), ordinate. La colonna dell'ancora e' `view::col_of` sulla sua riga; quella del
/// cursore e' `cursor_col` se c'e' (colonna virtuale, anche oltre la fine della riga), altrimenti `view::col_of`.
/// `col_a` e' la minore delle due colonne, `col_b` la maggiore.
pub fn column_block(
    doc: &Document,
    anchor: u64,
    cursor: u64,
    cursor_col: Option<u64>,
    opts: &ViewOptions,
) -> ColumnBlock {
    let riga_ancora = row_start(doc, anchor, opts.max_scan).0;
    let riga_cursore = row_start(doc, cursor, opts.max_scan).0;
    let colonna_ancora = col_of(doc, riga_ancora, anchor, opts) as u64;
    let colonna_cursore =
        cursor_col.unwrap_or_else(|| col_of(doc, riga_cursore, cursor, opts) as u64);
    ColumnBlock {
        first_row: riga_ancora.min(riga_cursore),
        last_row: riga_ancora.max(riga_cursore),
        col_a: colonna_ancora.min(colonna_cursore),
        col_b: colonna_ancora.max(colonna_cursore),
    }
}

/// Inizio della riga visuale `n` righe dopo quella che inizia in `row_start` (`n == 0`: la stessa); `None`
/// se il documento finisce prima. Cammina con `scan::next_row(opts.max_scan)`.
pub fn nth_row(doc: &Document, row_start: u64, n: u64, opts: &ViewOptions) -> Option<u64> {
    let mut pos = row_start;
    for _ in 0..n {
        match next_row(doc, pos, opts.max_scan) {
            Next::Line(p) | Next::Cut(p) => pos = p,
            Next::End => return None,
        }
    }
    Some(pos)
}

/// Cio' che il blocco copre sulla riga visuale che inizia in `row_start`: i caratteri della riga si camminano una
/// volta sola tenendo la colonna corrente, e un carattere appartiene al blocco se la sua colonna di INIZIO sta in
/// `[col_a, col_b)` (la regola dei confini nel commento del modulo). La fine della riga "da blocco" e' quella di
/// `fine_riga` meno un eventuale `\r` nascosto, che non fa mai parte del blocco.
fn span_of_row(
    doc: &Document,
    row_start: u64,
    col_a: u64,
    col_b: u64,
    opts: &ViewOptions,
) -> RowSpan {
    let (end, _, a_capo, _) = fine_riga(doc, row_start, opts.max_scan);
    let tab = opts.tab_width.max(1);
    let mut caratteri = Caratteri::new(doc, row_start, end, opts.encoding);
    // fine della riga per il blocco: la fine vera, meno il \r nascosto se la riga finisce con lui
    let mut fine_blocco = end;
    let mut colonna = 0usize;
    let mut span_start: Option<u64> = None;
    let mut span_end: Option<u64> = None;
    while let Some((p, c, n)) = caratteri.prossimo() {
        // il \r nascosto (ultimo byte di una riga che termina con a-capo) chiude la riga: resta fuori dal blocco
        if c == '\r' && p + n as u64 == end && a_capo {
            fine_blocco = p;
            break;
        }
        let (w, _) = cella(c, colonna, tab, false);
        // un carattere di larghezza 0 vale come gli altri: conta la colonna dove INIZIA, cosi' un marchio
        // combinante alla stessa colonna di inizio del carattere dopo segue la stessa regola
        if span_start.is_none() && colonna as u64 >= col_a {
            span_start = Some(p);
        }
        if span_end.is_none() && colonna as u64 >= col_b {
            span_end = Some(p);
        }
        colonna = colonna.saturating_add(w);
    }
    // difensivo: una riga oltre la fine del documento non ha una fine vera (fine_riga resterebbe prima dell'inizio)
    let fine_blocco = fine_blocco.max(row_start);
    RowSpan {
        row_start,
        start: span_start.unwrap_or(fine_blocco),
        end: span_end.unwrap_or(fine_blocco),
        width: colonna as u64,
        reaches: col_a <= colonna as u64,
    }
}

/// Le righe del blocco con cio' che coprono, dalla prima all'ultima. `Err(TooManyRows)` se le righe sono
/// piu' di `max_rows` (senza leggerle tutte: ci si ferma a `max_rows + 1`). Le righe sono quelle
/// che iniziano in `[first_row, last_row]`; si parte da `first_row` (che deve essere un inizio di riga) e si cammina finche' l'inizio della riga e' <= `last_row`.
pub fn block_spans(
    doc: &Document,
    block: &ColumnBlock,
    opts: &ViewOptions,
    max_rows: u64,
) -> Result<Vec<RowSpan>, BlockError> {
    let mut spans = Vec::new();
    let mut riga = block.first_row;
    while riga <= block.last_row {
        if spans.len() as u64 >= max_rows {
            // questa sarebbe la (max_rows + 1)-esima riga: trovata, ma non si legge
            return Err(BlockError::TooManyRows);
        }
        spans.push(span_of_row(doc, riga, block.col_a, block.col_b, opts));
        match next_row(doc, riga, opts.max_scan) {
            Next::Line(p) | Next::Cut(p) => riga = p,
            Next::End => break,
        }
    }
    Ok(spans)
}

/// Il testo del blocco: per ogni riga i byte `[start, end)`, righe separate da un solo a-capo del
/// documento (nessun a-capo finale; un blocco di larghezza zero da' tante righe vuote). `Err(TooManyBytes)` se il
/// risultato supererebbe `max_bytes` (si ferma prima di costruirlo tutto).
pub fn block_text(
    doc: &Document,
    spans: &[RowSpan],
    max_bytes: u64,
) -> Result<Vec<u8>, BlockError> {
    // le righe si separano con l'a-capo del documento, che conta un'unita' in byte
    let nl = doc.newline();
    // il totale si conosce prima di leggere: un blocco troppo grande non viene mai costruito
    let mut totale = 0u64;
    for (i, span) in spans.iter().enumerate() {
        if i > 0 {
            totale = totale.saturating_add(nl.unit() as u64);
        }
        totale = totale.saturating_add(span.end - span.start);
    }
    if totale > max_bytes {
        return Err(BlockError::TooManyBytes);
    }
    let mut testo = Vec::new();
    for (i, span) in spans.iter().enumerate() {
        if i > 0 {
            testo.extend_from_slice(nl.bytes());
        }
        testo.extend_from_slice(&doc.read_range(span.start, span.end - span.start));
    }
    Ok(testo)
}

/// Toglie dal documento i byte `[start, end)` di ogni riga (dall'ultima alla prima); restituisce i byte tolti.
pub fn delete_block(doc: &mut Document, spans: &[RowSpan]) -> Result<u64, BlockError> {
    let mut tolti = 0u64;
    // dall'ultima riga alla prima: gli offset delle righe precedenti restano veri
    for span in spans.iter().rev() {
        if span.end > span.start {
            let quanti = span.end - span.start;
            doc.delete(span.start, quanti).map_err(BlockError::Edit)?;
            tolti = tolti.saturating_add(quanti);
        }
    }
    Ok(tolti)
}

/// Inserisce `testo` a `span.start` su una riga del blocco: tale e quale se la riga `reaches` la colonna `col_a`;
/// se non ci arriva, solo con `pad`, dopo `col_a - width` spazi nella codifica del documento che riportano la
/// riga alla colonna giusta (senza `pad` la riga resta intatta). `testo` non deve essere vuoto. Restituisce se
/// la riga e' stata modificata.
fn inserisci_riga(
    doc: &mut Document,
    span: &RowSpan,
    col_a: u64,
    testo: &[u8],
    pad: bool,
) -> Result<bool, BlockError> {
    if span.reaches {
        doc.insert(span.start, testo).map_err(BlockError::Edit)?;
        return Ok(true);
    }
    if !pad {
        return Ok(false);
    }
    // riga corta (width < col_a): prima gli spazi che mancano fino a col_a, ciascuno scritto
    // nella codifica del documento (b" " in UTF-8 e code page, l'unita' UTF-16 dello spazio)
    let spazi = usize::try_from(col_a - span.width).unwrap_or(usize::MAX);
    let spazio: &[u8] = match doc.newline() {
        Newline::Byte | Newline::Cr => b" ",
        Newline::Le16 => b" \0",
        Newline::Be16 => b"\0 ",
        Newline::Le32 => b" \0\0\0",
        Newline::Be32 => b"\0\0\0 ",
    };
    let capacita = spazi
        .saturating_mul(spazio.len())
        .saturating_add(testo.len());
    let mut dati = Vec::with_capacity(capacita);
    for _ in 0..spazi {
        dati.extend_from_slice(spazio);
    }
    dati.extend_from_slice(testo);
    doc.insert(span.start, &dati).map_err(BlockError::Edit)?;
    Ok(true)
}

/// Inserisce `text` in `span.start` di ogni riga che `reaches` (nel blocco di larghezza zero o dopo
/// `delete_block`: `start == end`); con `pad` anche sulle righe che non ci arrivano, dopo `col_a - width` spazi;
/// senza `pad` quelle righe restano intatte. `text` non deve contenere `\n` (se lo contiene: `Edit(...)` non serve,
/// si inserisce com'e'; la responsabilita' e' del chiamante). Restituisce quante righe sono state modificate.
pub fn insert_block(
    doc: &mut Document,
    spans: &[RowSpan],
    col_a: u64,
    text: &[u8],
    pad: bool,
) -> Result<u64, BlockError> {
    if text.is_empty() {
        return Ok(0);
    }
    let mut modificate = 0u64;
    // dall'ultima riga alla prima: gli offset delle righe precedenti restano veri
    for span in spans.iter().rev() {
        if inserisci_riga(doc, span, col_a, text, pad)? {
            modificate += 1;
        }
    }
    Ok(modificate)
}

/// Incolla `lines[i]` nella riga `i` del blocco, come `insert_block` (stesso `pad`). Se le righe da
/// incollare sono piu' delle righe di `spans`: `Err(NotEnoughRows)` e il documento resta com'era. Righe di `spans`
/// oltre `lines.len()` restano intatte. Restituisce quante righe sono state modificate.
pub fn paste_block(
    doc: &mut Document,
    spans: &[RowSpan],
    col_a: u64,
    lines: &[&[u8]],
    pad: bool,
) -> Result<u64, BlockError> {
    // il controllo viene prima di ogni modifica: in errore il documento resta com'era
    if lines.len() > spans.len() {
        return Err(BlockError::NotEnoughRows);
    }
    let mut modificate = 0u64;
    // dall'ultima riga alla prima, come insert_block; una riga da incollare vuota non fa nulla
    for (i, span) in spans.iter().enumerate().rev() {
        if i >= lines.len() || lines[i].is_empty() {
            continue;
        }
        if inserisci_riga(doc, span, col_a, lines[i], pad)? {
            modificate += 1;
        }
    }
    Ok(modificate)
}
