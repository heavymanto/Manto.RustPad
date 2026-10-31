//! La cartella della sessione: un file per scheda (`tab-N.mrps`, vedi `mrp_core::session`) e l'elenco (`manifest.txt`).
//! Si scrive tutto con file temporanei e rinomine, cosi' una scrittura interrotta lascia la sessione di prima. Qui solo
//! file e byte: cosa metterci e cosa farne lo decide il `Workspace`.

use mrp_core::session::{decode, decode_manifest, encode, encode_manifest, Manifest, TabRecord};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const ELENCO: &str = "manifest.txt";
const SCARTI: &str = "stale";

/// Scrive la sessione: i file delle schede, poi l'elenco, poi toglie i file di schede che non ci sono piu'.
pub fn scrivi(dir: &Path, schede: &[TabRecord], avanti: usize) -> io::Result<()> {
    fs::create_dir_all(dir)?;
    let mut nomi = Vec::with_capacity(schede.len());
    for (i, s) in schede.iter().enumerate() {
        let nome = format!("tab-{i}.mrps");
        scrivi_atomico(&dir.join(&nome), &encode(s))?;
        nomi.push(nome);
    }
    let elenco = Manifest {
        active: avanti,
        tabs: nomi.clone(),
    };
    scrivi_atomico(&dir.join(ELENCO), encode_manifest(&elenco).as_bytes())?;
    // i file delle schede che non sono piu' nell'elenco (e i temporanei rimasti da una scrittura interrotta)
    if let Ok(voci) = fs::read_dir(dir) {
        for v in voci.flatten() {
            let n = v.file_name().to_string_lossy().into_owned();
            let e_scheda = n.starts_with("tab-") && n.ends_with(".mrps");
            let e_temp = n.ends_with(".tmp");
            if (e_scheda && !nomi.contains(&n)) || e_temp {
                let _ = fs::remove_file(v.path());
            }
        }
    }
    Ok(())
}

/// Un file scritto per intero in un temporaneo accanto e rinominato sopra l'originale.
fn scrivi_atomico(percorso: &Path, dati: &[u8]) -> io::Result<()> {
    let mut tmp = percorso.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    fs::write(&tmp, dati)?;
    fs::rename(&tmp, percorso).inspect_err(|_| {
        let _ = fs::remove_file(&tmp);
    })
}

/// Cosa si e' letto di una scheda: il nome del suo file e il record, o il motivo per cui non si legge.
pub type SchedaLetta = (String, Result<TabRecord, String>);

/// Legge la sessione: quale scheda era avanti e le schede, nell'ordine dell'elenco. `None` se non c'e' una sessione (o
/// l'elenco non si legge: una sessione rovinata si ignora, non blocca l'avvio).
pub fn leggi(dir: &Path) -> Option<(usize, Vec<SchedaLetta>)> {
    let testo = fs::read_to_string(dir.join(ELENCO)).ok()?;
    let elenco = decode_manifest(&testo).ok()?;
    let schede = elenco
        .tabs
        .into_iter()
        .map(|nome| {
            let r = fs::read(dir.join(&nome))
                .map_err(|e| e.to_string())
                .and_then(|d| decode(&d).map_err(str::to_string));
            (nome, r)
        })
        .collect();
    Some((elenco.active, schede))
}

/// Mette da parte il file di una scheda che non si e' potuta ripristinare (cartella `stale`, nome con un numero che non
/// si ripete): restituisce il percorso dove sta ora, se ci e' riuscito.
pub fn metti_da_parte(dir: &Path, nome: &str) -> Option<PathBuf> {
    let scarti = dir.join(SCARTI);
    fs::create_dir_all(&scarti).ok()?;
    for n in 0u32.. {
        let dest = scarti.join(format!("{n}-{nome}"));
        if !dest.exists() {
            return fs::copy(dir.join(nome), &dest).ok().map(|_| dest);
        }
        if n > 10_000 {
            break;
        }
    }
    None
}

/// Svuota la sessione (nessuna scheda): un elenco vuoto e niente file di schede.
pub fn azzera(dir: &Path) -> io::Result<()> {
    scrivi(dir, &[], 0)
}
