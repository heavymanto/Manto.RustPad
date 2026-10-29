//! Il terminale vero del binario `mrp` (tutto quello che non e' `--dump`): modalita' raw, schermo
//! alternativo e cattura del mouse; il ciclo disegna lo schermo, aspetta un evento (al piu' 50
//! millisecondi), lo consegna all'app e raccoglie i lavori in background. Il terminale si ripristina SEMPRE
//! all'uscita, anche dopo un errore o un panic: l'hook che si installa all'inizio ripristina e poi richiama
//! l'hook che c'era prima.

use crate::app::App;
use crate::ui;
use ratatui::backend::CrosstermBackend;
use ratatui::crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyEventKind,
};
use ratatui::crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::crossterm::ExecutableCommand;
use ratatui::Terminal;
use std::io::{self, Stdout};
use std::time::Duration;

/// Quanto si aspetta un evento prima di rifare il giro (il `tick` passa comunque: i lavori in background
/// non restano mai fermi).
const ATTESA_EVENTO: Duration = Duration::from_millis(50);

/// Il terminale su stdout.
type Schermo = Terminal<CrosstermBackend<Stdout>>;

/// Prepara il terminale, fa girare il ciclo e ripristina comunque: il ripristino sta fuori dal lavoro,
/// cosi' vale anche se qualcosa dentro e' andato storto (e un suo errore non nasconde quello originale).
pub fn run(mut app: App) -> io::Result<()> {
    installa_hook();
    let esito = dentro(&mut app);
    ripristina();
    esito
}

/// Raw, schermo alternativo, mouse, le dimensioni iniziali e il ciclo.
fn dentro(app: &mut App) -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    stdout.execute(EnterAlternateScreen)?;
    stdout.execute(EnableMouseCapture)?;
    let mut terminale = Terminal::new(CrosstermBackend::new(stdout))?;
    // le dimensioni iniziali dal terminale vero; poi i Resize le aggiornano
    let dimensione = terminale.size()?;
    app.resize(dimensione.width, dimensione.height);
    ciclo(&mut terminale, app)
}

/// Il ciclo: un disegno, un evento (se arriva entro `ATTESA_EVENTO`), un `tick`; si esce quando l'app lo
/// chiede.
fn ciclo(terminale: &mut Schermo, app: &mut App) -> io::Result<()> {
    while !app.should_quit() {
        terminale.draw(|f| ui::draw(f, app))?;
        if event::poll(ATTESA_EVENTO)? {
            match event::read()? {
                // solo le pressioni (e le ripetizioni): su Windows arrivano anche i rilasci
                Event::Key(t)
                    if t.kind == KeyEventKind::Press || t.kind == KeyEventKind::Repeat =>
                {
                    app.handle_key(t)
                }
                Event::Mouse(m) => app.handle_mouse(m),
                Event::Resize(w, h) => app.resize(w, h),
                _ => {}
            }
        }
        app.tick();
    }
    Ok(())
}

/// L'hook dei panic: ripristina il terminale e poi richiama l'hook che c'era prima (il messaggio del panic
/// lo stampa lui, sul terminale ripulito).
fn installa_hook() {
    let precedente = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        ripristina();
        precedente(info);
    }));
}

/// Rimette il terminale com'era: mouse, schermo alternativo, raw. Si fa il piu' possibile e gli errori si
/// buttano: qui non c'e' piu' nessuno a cui dirli (e non devono nascondere quello che sta salendo).
fn ripristina() {
    let _ = io::stdout().execute(DisableMouseCapture);
    let _ = io::stdout().execute(LeaveAlternateScreen);
    let _ = disable_raw_mode();
}
