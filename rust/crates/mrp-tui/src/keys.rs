//! La lista di tasti dell'opzione `--keys` (la applica la modalita' `--dump` del binario): ogni voce,
//! separata dalle virgole e senza spazi in testa o in coda, diventa il `KeyEvent` che l'app riceverebbe dal
//! terminale. I nomi (`Up`, `PgDn`, `F1`...) non distinguono maiuscole e minuscole; i prefissi `Ctrl+`,
//! `Shift+` e `Alt+` si combinano fra loro; un carattere singolo si digita cosi' com'e' scritto (con `Ctrl+`
//! arriverebbe minuscolo e con `Shift+` maiuscolo, come dai tasti veri); `text:...` digita tutto quello che
//! segue il prefisso, fino alla virgola.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// La lista in eventi, nell'ordine scritto. `Err` col testo della voce che non si riconosce.
pub fn parse(lista: &str) -> Result<Vec<KeyEvent>, String> {
    let mut eventi = Vec::new();
    for voce in lista.split(',') {
        let voce = voce.trim();
        match voce_in_eventi(voce) {
            Some(ev) => eventi.extend(ev),
            None => return Err(voce.to_string()),
        }
    }
    Ok(eventi)
}

/// Una voce della lista: piu' di un evento solo per `text:...`; `None` se non si riconosce.
fn voce_in_eventi(voce: &str) -> Option<Vec<KeyEvent>> {
    if let Some(testo) = togli_prefisso(voce, "text:") {
        return Some(testo.chars().map(tasto_carattere).collect());
    }
    let mut modificatori = KeyModifiers::NONE;
    let mut resto = voce;
    while let Some((m, r)) = prefisso_modificatore(resto) {
        modificatori |= m;
        resto = r;
    }
    let code = nome_in_code(resto, modificatori)?;
    Some(vec![KeyEvent::new(code, modificatori)])
}

/// Il tasto di un carattere digitato, senza modificatori.
fn tasto_carattere(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)
}

/// Il prefisso `Ctrl+`, `Shift+` o `Alt+`, senza distinguere maiuscole e minuscole: i modificatori che
/// imposta e il resto del nome.
fn prefisso_modificatore(s: &str) -> Option<(KeyModifiers, &str)> {
    if let Some(resto) = togli_prefisso(s, "ctrl+") {
        return Some((KeyModifiers::CONTROL, resto));
    }
    if let Some(resto) = togli_prefisso(s, "shift+") {
        return Some((KeyModifiers::SHIFT, resto));
    }
    if let Some(resto) = togli_prefisso(s, "alt+") {
        return Some((KeyModifiers::ALT, resto));
    }
    None
}

/// Il nome di un tasto, o un carattere singolo, in un `KeyCode`. I modificatori servono al carattere: `Ctrl+`
/// lo vuole minuscolo e `Shift+` maiuscolo, come arrivano dai tasti veri.
fn nome_in_code(nome: &str, modificatori: KeyModifiers) -> Option<KeyCode> {
    let minuscolo = nome.to_ascii_lowercase();
    let code = match minuscolo.as_str() {
        "up" => KeyCode::Up,
        "down" => KeyCode::Down,
        "left" => KeyCode::Left,
        "right" => KeyCode::Right,
        "home" => KeyCode::Home,
        "end" => KeyCode::End,
        "pgup" => KeyCode::PageUp,
        "pgdn" => KeyCode::PageDown,
        "enter" => KeyCode::Enter,
        "tab" => KeyCode::Tab,
        "backspace" => KeyCode::Backspace,
        "delete" => KeyCode::Delete,
        "esc" => KeyCode::Esc,
        "space" => KeyCode::Char(' '),
        // F1..F12: la `f` e il numero
        _ if minuscolo.starts_with('f') && minuscolo.len() > 1 => return f_key(&minuscolo),
        // un carattere singolo si digita
        _ if nome.chars().count() == 1 => {
            let c = nome.chars().next()?;
            let c = if modificatori.contains(KeyModifiers::CONTROL) {
                c.to_ascii_lowercase()
            } else if modificatori.contains(KeyModifiers::SHIFT) {
                c.to_ascii_uppercase()
            } else {
                c
            };
            KeyCode::Char(c)
        }
        _ => return None,
    };
    Some(code)
}

/// `F1`..`F12` dal nome gia' in minuscolo: il numero dopo la `f`.
fn f_key(minuscolo: &str) -> Option<KeyCode> {
    let cifre = &minuscolo[1..];
    if !cifre.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let n = cifre.parse::<u8>().ok()?;
    (1..=12).contains(&n).then_some(KeyCode::F(n))
}

/// Il prefisso `prefisso`, senza distinguere maiuscole e minuscole: il resto della stringa, o `None`.
fn togli_prefisso<'a>(s: &'a str, prefisso: &str) -> Option<&'a str> {
    let n = prefisso.len();
    if s.get(..n)?.eq_ignore_ascii_case(prefisso) {
        Some(&s[n..])
    } else {
        None
    }
}
