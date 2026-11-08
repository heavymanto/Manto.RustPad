//! AltGr su Windows (V-02): crossterm riporta i caratteri della tastiera con AltGr (`@ [ ] { } ~ # \` ...) come Ctrl+Alt+carattere,
//! e il programma accetta testo solo senza Ctrl e Alt. `keys::altgr` li riporta a caratteri semplici; le vere combinazioni
//! (Ctrl+Alt+lettera, Alt+lettera, Ctrl+lettera, tasti non carattere) restano come sono. Scritti da ARCH prima della correzione.

use mrp_tui::keys::altgr;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

const ALTGR: KeyModifiers = KeyModifiers::CONTROL.union(KeyModifiers::ALT);

#[test]
fn altgr_characters_become_plain_text() {
    for c in "@[]{}~#`\\|€µ".chars() {
        let ev = altgr(KeyEvent::new(KeyCode::Char(c), ALTGR));
        assert_eq!(ev.code, KeyCode::Char(c), "{c}");
        assert_eq!(ev.modifiers, KeyModifiers::NONE, "{c}");
    }
}

#[test]
fn shift_is_kept_on_an_altgr_character() {
    let ev = altgr(KeyEvent::new(
        KeyCode::Char('@'),
        ALTGR | KeyModifiers::SHIFT,
    ));
    assert_eq!(ev.modifiers, KeyModifiers::SHIFT);
}

#[test]
fn real_shortcuts_are_left_alone() {
    let casi = [
        KeyEvent::new(KeyCode::Char('a'), ALTGR),
        KeyEvent::new(KeyCode::Char('5'), ALTGR),
        KeyEvent::new(KeyCode::Char('e'), KeyModifiers::ALT),
        KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL),
        KeyEvent::new(KeyCode::Char('@'), KeyModifiers::ALT),
        KeyEvent::new(KeyCode::Char('@'), KeyModifiers::CONTROL),
        KeyEvent::new(KeyCode::Char('@'), KeyModifiers::NONE),
        KeyEvent::new(KeyCode::Left, ALTGR),
        KeyEvent::new(KeyCode::F(4), KeyModifiers::ALT),
        KeyEvent::new(KeyCode::Enter, ALTGR),
    ];
    for ev in casi {
        assert_eq!(altgr(ev), ev, "{ev:?}");
    }
}
