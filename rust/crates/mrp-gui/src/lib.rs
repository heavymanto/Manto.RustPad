//! Manto.RustPad GUI: libreria (finestra, disegno del testo, tastiera e mouse) usata dal binario `mrpg`.
//!
//! La GUI non rifa la logica dell'editor: apre un `mrp_tui::app::App` (la macchina a stati del TUI, con tutti i suoi
//! comandi, dialoghi, ricerca, modifica e codifiche) e ne disegna lo schermo come griglia di celle con egui
//! (`mrp_tui::cells`). Tastiera, incolla e rotella di egui diventano i tasti e gli eventi che l'`App` gia' conosce.

pub mod fonts;
pub mod gui;
pub mod input;
pub mod paint;
