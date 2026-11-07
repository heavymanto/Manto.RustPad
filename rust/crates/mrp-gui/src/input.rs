//! Dalla tastiera e dal mouse di egui all'`App` del TUI: funzioni pure, senza finestra, quindi provabili. L'`App`
//! vuole eventi di crossterm (`KeyEvent`, `MouseEvent`); egui ha i suoi (`Key`, `Modifiers`, il testo digitato a parte).
//! I caratteri stampabili arrivano dal testo digitato (rispettano la tastiera, AltGr e l'IME); qui si mappano solo i
//! tasti che non producono testo e le combinazioni con Ctrl.

use eframe::egui::{Key, Modifiers, Pos2, Vec2};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// I modificatori di egui in quelli di crossterm. Su macOS `command` vale come Ctrl.
pub fn modificatori(m: Modifiers) -> KeyModifiers {
    let mut r = KeyModifiers::NONE;
    if m.ctrl || m.command {
        r |= KeyModifiers::CONTROL;
    }
    if m.alt {
        r |= KeyModifiers::ALT;
    }
    if m.shift {
        r |= KeyModifiers::SHIFT;
    }
    r
}

/// Il tasto F`n` di egui, se e' F1..F12.
fn funzione(key: Key) -> Option<u8> {
    Some(match key {
        Key::F1 => 1,
        Key::F2 => 2,
        Key::F3 => 3,
        Key::F4 => 4,
        Key::F5 => 5,
        Key::F6 => 6,
        Key::F7 => 7,
        Key::F8 => 8,
        Key::F9 => 9,
        Key::F10 => 10,
        Key::F11 => 11,
        Key::F12 => 12,
        _ => return None,
    })
}

/// La lettera (minuscola) di un tasto lettera, o la cifra di un tasto cifra.
fn carattere(key: Key) -> Option<char> {
    let nome = key.name();
    let mut it = nome.chars();
    match (it.next(), it.next()) {
        (Some(c), None) if c.is_ascii_alphanumeric() => Some(c.to_ascii_lowercase()),
        _ => None,
    }
}

/// L'evento di tastiera per l'`App` dato un tasto premuto di egui, o `None` se non serve (un carattere normale arriva
/// come testo digitato). Le lettere e le cifre contano solo con Ctrl (senza Alt: Ctrl+Alt e' AltGr); le uniche eccezioni sono
/// Alt+W (i caratteri speciali), Alt+T (seguire la coda), Alt+E (modifica), Alt+L (colori dei log) e Alt+G (filtro) da soli, che sono comandi del registro.
pub fn tasto(key: Key, m: Modifiers) -> Option<KeyEvent> {
    let mods = modificatori(m);
    let code = match key {
        Key::ArrowUp => KeyCode::Up,
        Key::ArrowDown => KeyCode::Down,
        Key::ArrowLeft => KeyCode::Left,
        Key::ArrowRight => KeyCode::Right,
        Key::Home => KeyCode::Home,
        Key::End => KeyCode::End,
        Key::PageUp => KeyCode::PageUp,
        Key::PageDown => KeyCode::PageDown,
        Key::Insert => KeyCode::Insert,
        Key::Delete => KeyCode::Delete,
        Key::Backspace => KeyCode::Backspace,
        Key::Enter => KeyCode::Enter,
        Key::Escape => KeyCode::Esc,
        Key::Tab if m.shift => return Some(KeyEvent::new(KeyCode::BackTab, mods)),
        // i tasti standard della finestra: F4 = Sostituisci (come Ctrl+H), F12 = lettura / modifica (nel terminale e' F4)
        Key::F4 if mods == KeyModifiers::NONE => {
            return Some(KeyEvent::new(KeyCode::Char('h'), KeyModifiers::CONTROL))
        }
        Key::F12 if mods == KeyModifiers::NONE => {
            return Some(KeyEvent::new(KeyCode::F(4), KeyModifiers::NONE))
        }
        Key::Tab => KeyCode::Tab,
        k if funzione(k).is_some() => KeyCode::F(funzione(k)?),
        k => {
            let c = carattere(k)?;
            let alt_cmd = matches!(c, 'w' | 't' | 'e' | 'l' | 'g' | 'n' | 'v' | 'm')
                && m.alt
                && !m.ctrl
                && !m.command
                && !m.shift;
            if !alt_cmd && (!(m.ctrl || m.command) || m.alt) {
                return None;
            }
            KeyCode::Char(c)
        }
    };
    Some(KeyEvent::new(code, mods))
}

/// Gli eventi per un testo digitato: un tasto `Char` per ogni carattere stampabile (i controlli si scartano: Invio,
/// Tab e Backspace arrivano come tasti).
pub fn testo(s: &str) -> Vec<KeyEvent> {
    s.chars()
        .filter(|c| !c.is_control())
        .map(|c| KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE))
        .collect()
}

/// La cella (colonna, riga) sotto `pos`, con l'origine della griglia in `origine` e la cella grande `cella`;
/// `None` fuori dalla griglia di `colonne` x `righe` celle.
pub fn cella_sotto(
    pos: Pos2,
    origine: Pos2,
    cella: Vec2,
    colonne: u16,
    righe: u16,
) -> Option<(u16, u16)> {
    if cella.x <= 0.0 || cella.y <= 0.0 {
        return None;
    }
    let dx = pos.x - origine.x;
    let dy = pos.y - origine.y;
    if dx < 0.0 || dy < 0.0 {
        return None;
    }
    let (c, r) = ((dx / cella.x) as u32, (dy / cella.y) as u32);
    if c < u32::from(colonne) && r < u32::from(righe) {
        Some((c as u16, r as u16))
    } else {
        None
    }
}

/// La cella piu' vicina a `pos` dentro la griglia (per il trascinamento: fuori dalla griglia si resta sul bordo).
pub fn cella_vicina(pos: Pos2, origine: Pos2, cella: Vec2, colonne: u16, righe: u16) -> (u16, u16) {
    if cella.x <= 0.0 || cella.y <= 0.0 || colonne == 0 || righe == 0 {
        return (0, 0);
    }
    let c = ((pos.x - origine.x) / cella.x)
        .floor()
        .clamp(0.0, f32::from(colonne - 1));
    let r = ((pos.y - origine.y) / cella.y)
        .floor()
        .clamp(0.0, f32::from(righe - 1));
    (c as u16, r as u16)
}

/// Quante celle per quante ne stanno in `spazio` pixel: almeno `minimo`.
pub fn celle_in(spazio: f32, cella: f32, minimo: u16) -> u16 {
    if cella <= 0.0 || !spazio.is_finite() {
        return minimo;
    }
    ((spazio / cella).floor().max(0.0) as u32).clamp(u32::from(minimo), u32::from(u16::MAX)) as u16
}

/// Le righe di rotella da mandare all'`App` per uno spostamento di `delta` pixel (positivo: il contenuto scende,
/// cioe' si guarda in alto), con il resto da riportare al prossimo giro: `(righe_su, righe_giu, nuovo_resto)`.
pub fn rotella(resto: f32, delta: f32, altezza_cella: f32) -> (u32, u32, f32) {
    if altezza_cella <= 0.0 {
        return (0, 0, 0.0);
    }
    let totale = resto + delta;
    let righe = (totale / altezza_cella).trunc();
    let nuovo = totale - righe * altezza_cella;
    if righe > 0.0 {
        (righe as u32, 0, nuovo)
    } else {
        (0, (-righe) as u32, nuovo)
    }
}
