//! Le righe di servizio (l'uso, `--help`, gli errori prima che la finestra parta). Nella build di rilascio su Windows il
//! binario non ha una console propria — l'attributo `windows_subsystem` di `main.rs` evita la finestra nera accanto a
//! quella vera quando si apre un file da Explorer — quindi chi ha qualcosa da dire si aggancia alla console del
//! processo che l'ha lanciato, se c'e', e scrive li' (`CONOUT$`); senza console (un doppio click) la riga non va da
//! nessuna parte: la finestra e' l'interfaccia. Le build di sviluppo e gli altri sistemi stampano e basta.

#[cfg(all(windows, not(debug_assertions)))]
#[link(name = "kernel32")]
extern "system" {
    fn AttachConsole(dw_process_id: u32) -> i32;
}

/// Scrive `msg` come riga nella console, se ce n'e' una raggiungibile.
pub fn scrivi(msg: &str) {
    #[cfg(all(windows, not(debug_assertions)))]
    {
        // ATTACH_PARENT_PROCESS (u32::MAX): la console di chi ci ha lanciato, non una nuova
        unsafe {
            if AttachConsole(u32::MAX) != 0 {
                if let Ok(mut f) = std::fs::OpenOptions::new().write(true).open("CONOUT$") {
                    use std::io::Write;
                    let _ = writeln!(f, "{msg}");
                }
            }
        }
    }
    #[cfg(any(not(windows), debug_assertions))]
    eprintln!("{msg}");
}
