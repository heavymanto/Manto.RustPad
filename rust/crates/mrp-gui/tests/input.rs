//! Tastiera, testo digitato, celle e rotella di egui verso l'`App` (E31). Funzioni pure: nessuna finestra. Attesi scritti
//! a mano da ARCH.

use eframe::egui::{Key, Modifiers, Pos2, Vec2};
use mrp_gui::input::{cella_sotto, celle_in, modificatori, rotella, tasto, testo};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

fn nessuno() -> Modifiers {
    Modifiers::NONE
}

fn ctrl() -> Modifiers {
    Modifiers {
        ctrl: true,
        command: true,
        ..Modifiers::NONE
    }
}

#[test]
fn navigation_and_editing_keys() {
    let casi = [
        (Key::ArrowUp, KeyCode::Up),
        (Key::ArrowDown, KeyCode::Down),
        (Key::ArrowLeft, KeyCode::Left),
        (Key::ArrowRight, KeyCode::Right),
        (Key::Home, KeyCode::Home),
        (Key::End, KeyCode::End),
        (Key::PageUp, KeyCode::PageUp),
        (Key::PageDown, KeyCode::PageDown),
        (Key::Insert, KeyCode::Insert),
        (Key::Delete, KeyCode::Delete),
        (Key::Backspace, KeyCode::Backspace),
        (Key::Enter, KeyCode::Enter),
        (Key::Escape, KeyCode::Esc),
        (Key::Tab, KeyCode::Tab),
    ];
    for (k, atteso) in casi {
        assert_eq!(
            tasto(k, nessuno()),
            Some(KeyEvent::new(atteso, KeyModifiers::NONE)),
            "{k:?}"
        );
    }
    // Maiusc+Tab e' BackTab, con Maiusc
    let shift = Modifiers {
        shift: true,
        ..Modifiers::NONE
    };
    assert_eq!(
        tasto(Key::Tab, shift),
        Some(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT))
    );
    // Maiusc+freccia resta una freccia con Maiusc (selezione)
    assert_eq!(
        tasto(Key::ArrowRight, shift),
        Some(KeyEvent::new(KeyCode::Right, KeyModifiers::SHIFT))
    );
}

#[test]
fn function_keys() {
    let tasti = [
        Key::F1,
        Key::F2,
        Key::F3,
        Key::F4,
        Key::F5,
        Key::F6,
        Key::F7,
        Key::F8,
        Key::F9,
        Key::F10,
        Key::F11,
        Key::F12,
    ];
    for (i, k) in tasti.iter().enumerate() {
        assert_eq!(
            tasto(*k, nessuno()),
            Some(KeyEvent::new(KeyCode::F(i as u8 + 1), KeyModifiers::NONE))
        );
    }
}

#[test]
fn letters_only_count_with_ctrl() {
    // da soli arrivano come testo digitato, non come tasti
    assert_eq!(tasto(Key::S, nessuno()), None);
    assert_eq!(tasto(Key::Num5, nessuno()), None);
    assert_eq!(tasto(Key::Space, nessuno()), None);
    // con Ctrl sono comandi, minuscoli
    assert_eq!(
        tasto(Key::S, ctrl()),
        Some(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL))
    );
    assert_eq!(
        tasto(Key::A, ctrl()),
        Some(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL))
    );
    // Ctrl+Maiusc+lettera: i due modificatori
    let cs = Modifiers {
        shift: true,
        ..ctrl()
    };
    assert_eq!(
        tasto(Key::Z, cs),
        Some(KeyEvent::new(
            KeyCode::Char('z'),
            KeyModifiers::CONTROL | KeyModifiers::SHIFT
        ))
    );
    // Ctrl+Alt e' AltGr: la lettera non e' un comando (il carattere arriva come testo)
    let altgr = Modifiers {
        alt: true,
        ..ctrl()
    };
    assert_eq!(tasto(Key::Q, altgr), None);
    // Alt da solo con una lettera: niente
    let alt = Modifiers {
        alt: true,
        ..Modifiers::NONE
    };
    assert_eq!(tasto(Key::F, alt), None);
}

#[test]
fn modifiers_map_and_command_counts_as_ctrl() {
    assert_eq!(modificatori(nessuno()), KeyModifiers::NONE);
    assert_eq!(modificatori(Modifiers::CTRL), KeyModifiers::CONTROL);
    assert_eq!(modificatori(Modifiers::COMMAND), KeyModifiers::CONTROL);
    assert_eq!(modificatori(Modifiers::ALT), KeyModifiers::ALT);
    assert_eq!(modificatori(Modifiers::SHIFT), KeyModifiers::SHIFT);
    assert_eq!(
        modificatori(Modifiers::CTRL | Modifiers::SHIFT),
        KeyModifiers::CONTROL | KeyModifiers::SHIFT
    );
}

#[test]
fn typed_text_becomes_char_keys_without_controls() {
    let ev = testo("aé日\u{1F600}");
    let attesi: Vec<char> = vec!['a', 'é', '日', '\u{1F600}'];
    assert_eq!(ev.len(), 4);
    for (e, c) in ev.iter().zip(attesi) {
        assert_eq!(*e, KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
    }
    // i controlli (Invio, Tab, Backspace, DEL) non sono testo
    assert!(testo("\r\n\t\u{8}\u{7f}").is_empty());
    assert_eq!(testo("a\nb").len(), 2);
    assert!(testo("").is_empty());
}

#[test]
fn the_cell_under_a_pixel() {
    let origine = Pos2::new(10.0, 20.0);
    let cella = Vec2::new(8.0, 16.0);
    assert_eq!(
        cella_sotto(Pos2::new(10.0, 20.0), origine, cella, 80, 24),
        Some((0, 0))
    );
    assert_eq!(
        cella_sotto(Pos2::new(17.9, 35.9), origine, cella, 80, 24),
        Some((0, 0))
    );
    assert_eq!(
        cella_sotto(Pos2::new(18.0, 36.0), origine, cella, 80, 24),
        Some((1, 1))
    );
    assert_eq!(
        cella_sotto(
            Pos2::new(10.0 + 79.0 * 8.0, 20.0 + 23.0 * 16.0),
            origine,
            cella,
            80,
            24
        ),
        Some((79, 23))
    );
    // fuori
    assert_eq!(
        cella_sotto(Pos2::new(9.9, 20.0), origine, cella, 80, 24),
        None
    );
    assert_eq!(
        cella_sotto(Pos2::new(10.0, 19.9), origine, cella, 80, 24),
        None
    );
    assert_eq!(
        cella_sotto(Pos2::new(10.0 + 80.0 * 8.0, 20.0), origine, cella, 80, 24),
        None
    );
    assert_eq!(
        cella_sotto(Pos2::new(10.0, 20.0 + 24.0 * 16.0), origine, cella, 80, 24),
        None
    );
    // cella degenere
    assert_eq!(
        cella_sotto(Pos2::new(11.0, 21.0), origine, Vec2::ZERO, 80, 24),
        None
    );
}

#[test]
fn how_many_cells_fit() {
    assert_eq!(celle_in(800.0, 8.0, 20), 100);
    assert_eq!(celle_in(807.9, 8.0, 20), 100);
    assert_eq!(celle_in(100.0, 8.0, 20), 20, "almeno il minimo");
    assert_eq!(celle_in(0.0, 8.0, 6), 6);
    assert_eq!(celle_in(-5.0, 8.0, 6), 6);
    assert_eq!(celle_in(f32::NAN, 8.0, 6), 6);
    assert_eq!(celle_in(800.0, 0.0, 6), 6);
    assert_eq!(celle_in(1e9, 1.0, 6), u16::MAX);
}

#[test]
fn wheel_pixels_become_rows_with_a_remainder() {
    // 16 px per riga: 40 px in su sono 2 righe e restano 8 px
    assert_eq!(rotella(0.0, 40.0, 16.0), (2, 0, 8.0));
    // il resto si riporta: 8 + 8 = una riga
    assert_eq!(rotella(8.0, 8.0, 16.0), (1, 0, 0.0));
    // in giu'
    assert_eq!(rotella(0.0, -40.0, 16.0), (0, 2, -8.0));
    assert_eq!(rotella(-8.0, -8.0, 16.0), (0, 1, 0.0));
    // meno di una riga: niente, tutto nel resto
    assert_eq!(rotella(0.0, 5.0, 16.0), (0, 0, 5.0));
    assert_eq!(rotella(0.0, -5.0, 16.0), (0, 0, -5.0));
    // cella degenere
    assert_eq!(rotella(3.0, 40.0, 0.0), (0, 0, 0.0));
}

#[test]
fn cjk_characters_are_recognised() {
    use mrp_gui::fonts::e_cjk;
    for c in ['日', '本', 'あ', 'ア', '한', '中', '！', '\u{20000}', '〒'] {
        assert!(e_cjk(c), "{c}");
    }
    for c in ['a', 'é', '€', 'Ж', ' ', '\u{1F600}', 'ß', '→'] {
        assert!(!e_cjk(c), "{c}");
    }
}

#[test]
fn each_asian_character_asks_for_its_script() {
    use mrp_gui::fonts::{scrittura_del_carattere, Scrittura};
    assert_eq!(scrittura_del_carattere('日'), Some(Scrittura::Giapponese));
    assert_eq!(scrittura_del_carattere('あ'), Some(Scrittura::Giapponese));
    assert_eq!(scrittura_del_carattere('ア'), Some(Scrittura::Giapponese));
    assert_eq!(scrittura_del_carattere('한'), Some(Scrittura::Coreano));
    assert_eq!(scrittura_del_carattere('ㄱ'), Some(Scrittura::Coreano));
    assert_eq!(scrittura_del_carattere('a'), None);
    assert_eq!(scrittura_del_carattere('é'), None);
}

#[test]
fn definitions_load_one_font_per_script_and_none_for_latin() {
    use mrp_gui::fonts::{definizioni, Scrittura};
    let (def, caricati) = definizioni(&[]);
    assert!(caricati.is_empty());
    let base = def.font_data.len();
    let (def2, caricati2) = definizioni(&[Scrittura::Nessuna]);
    assert!(caricati2.is_empty());
    assert_eq!(def2.font_data.len(), base);
    // con le scritture il numero di font cresce di quanti se ne sono trovati (dipende dal sistema, mai di piu')
    let (def3, caricati3) = definizioni(&[Scrittura::Giapponese, Scrittura::Coreano]);
    assert_eq!(def3.font_data.len(), base + caricati3.len());
    assert!(caricati3.len() <= 2);
}
