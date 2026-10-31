//! Sostituto minimo della crate `webbrowser` (solo `open`), per i collegamenti cliccabili di egui. La vera crate su
//! Windows collega `shlwapi`, che il toolchain `x86_64-pc-windows-gnu` di rustup non ha. Apre l'indirizzo con il
//! comando del sistema: `cmd /C start` su Windows, `open` su macOS, `xdg-open` altrove.

use std::io;
use std::process::{Command, Stdio};

/// Apre `url` con il browser predefinito. Si accettano solo indirizzi `http://`, `https://` e `mailto:`.
pub fn open(url: &str) -> io::Result<()> {
    let lecito = ["http://", "https://", "mailto:"]
        .iter()
        .any(|p| url.to_ascii_lowercase().starts_with(p));
    if !lecito || url.chars().any(|c| c.is_control() || c == '"' || c == '&' || c == '|' || c == '^') {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "indirizzo non ammesso",
        ));
    }
    let mut cmd = if cfg!(windows) {
        let mut c = Command::new("cmd");
        c.args(["/C", "start", ""]).arg(url);
        c
    } else if cfg!(target_os = "macos") {
        let mut c = Command::new("open");
        c.arg(url);
        c
    } else {
        let mut c = Command::new("xdg-open");
        c.arg(url);
        c
    };
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|_| ())
}
