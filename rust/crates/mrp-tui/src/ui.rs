//! Il disegno del TUI: legge l'`App` e scrive su un `Frame` (layout in `docs/TEAM/DESIGN_TUI.md`, sezione 2).

use crate::app::App;
use ratatui::Frame;

/// Disegna tutto lo schermo e ricorda dove sta il cursore (`App::cursor_xy`).
pub fn draw(_f: &mut Frame, _app: &mut App) {
    todo!("E8")
}

/// Disegna su un terminale di prova `w` x `h` e restituisce le `h` righe di testo (una cella = un carattere, le celle
/// vuote sono spazi): serve ai test e alla modalita' `--dump` del binario.
pub fn render_lines(_app: &mut App, _w: u16, _h: u16) -> Vec<String> {
    todo!("E8")
}
