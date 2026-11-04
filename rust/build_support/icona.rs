// L'icona del progetto dentro l'eseguibile (Windows), senza strumenti esterni (niente windres ne' rc.exe): un oggetto COFF con la
// sola sezione `.rsrc` che il linker unisce al programma. Lo includono i `build.rs` di mrp-gui e mrp-tui (`include!`) e lo prova
// `mrp-gui/tests/icona.rs`, che rilegge i byte a mano.
//
// La sezione e' un albero di risorse a tre livelli (tipo, nome, lingua) con due risorse: RT_ICON 1 (il file PNG com'e': dalla
// Vista in poi un'icona puo' essere un PNG) e RT_GROUP_ICON 1 (la lista delle icone, che Esplora risorse legge per prima). Le
// voci dati portano l'indirizzo dei byte come RVA: due rilocazioni `ADDR32NB` sul simbolo della sezione, con in loco la distanza.

/// Tipo di risorsa: un'icona.
const RT_ICON: u32 = 3;
/// Tipo di risorsa: il gruppo di icone di un programma.
const RT_GROUP_ICON: u32 = 14;
/// Inglese (Stati Uniti): la lingua delle risorse.
const LINGUA: u32 = 0x0409;

/// Dove stanno le parti nella sezione (vedi `oggetto_coff`): l'albero e' fisso, i dati seguono.
const DIR_ICONA: u32 = 32;
const DIR_ICONA_LINGUE: u32 = 56;
const DIR_GRUPPO: u32 = 80;
const DIR_GRUPPO_LINGUE: u32 = 104;
const VOCE_ICONA: u32 = 128;
const VOCE_GRUPPO: u32 = 144;
const DATI_GRUPPO: u32 = 160;
/// 160 + 20 byte del gruppo.
const DATI_ICONA: u32 = 180;

fn u16le(v: &mut Vec<u8>, x: u16) {
    v.extend_from_slice(&x.to_le_bytes());
}

fn u32le(v: &mut Vec<u8>, x: u32) {
    v.extend_from_slice(&x.to_le_bytes());
}

/// Una directory di risorse con voci per identificatore: `(id, destinazione)`; una sottodirectory ha il bit alto acceso.
fn directory(v: &mut Vec<u8>, voci: &[(u32, u32)]) {
    u32le(v, 0); // caratteristiche
    u32le(v, 0); // data
    u16le(v, 0); // versione
    u16le(v, 0);
    u16le(v, 0); // voci con nome
    u16le(v, voci.len() as u16); // voci con identificatore
    for &(id, dove) in voci {
        u32le(v, id);
        u32le(v, dove);
    }
}

/// L'oggetto COFF con l'icona `png` (un file PNG intero) per la macchina `macchina` (0x8664 x86-64, 0xAA64 ARM64) e il suo tipo di
/// rilocazione `rilocazione` (3 e 2: `ADDR32NB`). Ritorna un vettore vuoto se `png` non e' un PNG.
#[allow(dead_code)]
pub fn oggetto_coff(png: &[u8], macchina: u16, rilocazione: u16) -> Vec<u8> {
    if png.len() < 33 || png[..8] != [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a] {
        return Vec::new();
    }
    // la dimensione sta nell'intestazione IHDR; 0 vuol dire 256 (o piu')
    let larghezza = u32::from_be_bytes([png[16], png[17], png[18], png[19]]);
    let altezza = u32::from_be_bytes([png[20], png[21], png[22], png[23]]);
    let byte_dim = |d: u32| if d >= 256 { 0u8 } else { d as u8 };

    // la sezione: l'albero, le due voci dati, il gruppo di icone, il PNG
    let mut s: Vec<u8> = Vec::new();
    directory(
        &mut s,
        &[
            (RT_ICON, 0x8000_0000 | DIR_ICONA),
            (RT_GROUP_ICON, 0x8000_0000 | DIR_GRUPPO),
        ],
    );
    directory(&mut s, &[(1, 0x8000_0000 | DIR_ICONA_LINGUE)]);
    directory(&mut s, &[(LINGUA, VOCE_ICONA)]);
    directory(&mut s, &[(1, 0x8000_0000 | DIR_GRUPPO_LINGUE)]);
    directory(&mut s, &[(LINGUA, VOCE_GRUPPO)]);
    assert_eq!(s.len() as u32, VOCE_ICONA);
    // le voci dati: dove sono i byte (RVA, rilocata), quanti sono, pagina di codice, riservato
    u32le(&mut s, DATI_ICONA);
    u32le(&mut s, png.len() as u32);
    u32le(&mut s, 0);
    u32le(&mut s, 0);
    u32le(&mut s, DATI_GRUPPO);
    u32le(&mut s, 20);
    u32le(&mut s, 0);
    u32le(&mut s, 0);
    assert_eq!(s.len() as u32, DATI_GRUPPO);
    // il gruppo: intestazione (0, tipo 1 = icona, una sola) e la voce che rimanda all'icona 1
    u16le(&mut s, 0);
    u16le(&mut s, 1);
    u16le(&mut s, 1);
    s.push(byte_dim(larghezza));
    s.push(byte_dim(altezza));
    s.push(0); // colori
    s.push(0);
    u16le(&mut s, 1); // piani
    u16le(&mut s, 32); // bit per punto
    u32le(&mut s, png.len() as u32);
    u16le(&mut s, 1); // l'icona
    assert_eq!(s.len() as u32, DATI_ICONA);
    s.extend_from_slice(png);
    while !s.len().is_multiple_of(4) {
        s.push(0);
    }

    // il file COFF: intestazione, una sezione, i dati, due rilocazioni, la tabella dei simboli (il simbolo della sezione e la sua
    // voce ausiliaria), la tabella delle stringhe vuota
    let grezzi = 20 + 40;
    let rilocazioni = grezzi + s.len();
    let simboli = rilocazioni + 2 * 10;
    let mut o: Vec<u8> = Vec::new();
    u16le(&mut o, macchina);
    u16le(&mut o, 1);
    u32le(&mut o, 0);
    u32le(&mut o, simboli as u32);
    u32le(&mut o, 2); // simboli, voce ausiliaria compresa
    u16le(&mut o, 0);
    u16le(&mut o, 0);
    // l'intestazione della sezione
    o.extend_from_slice(b".rsrc\0\0\0");
    u32le(&mut o, 0);
    u32le(&mut o, 0);
    u32le(&mut o, s.len() as u32);
    u32le(&mut o, grezzi as u32);
    u32le(&mut o, rilocazioni as u32);
    u32le(&mut o, 0);
    u16le(&mut o, 2);
    u16le(&mut o, 0);
    // dati inizializzati, in sola lettura, allineati a 4 byte
    u32le(&mut o, 0x4030_0040);
    o.extend_from_slice(&s);
    for dove in [VOCE_ICONA, VOCE_GRUPPO] {
        u32le(&mut o, dove);
        u32le(&mut o, 0); // il simbolo 0: la sezione
        u16le(&mut o, rilocazione);
    }
    // il simbolo della sezione (classe STATIC) con la sua voce ausiliaria
    o.extend_from_slice(b".rsrc\0\0\0");
    u32le(&mut o, 0);
    u16le(&mut o, 1);
    u16le(&mut o, 0);
    o.push(3);
    o.push(1);
    u32le(&mut o, s.len() as u32);
    u16le(&mut o, 2);
    u16le(&mut o, 0);
    u32le(&mut o, 0);
    u16le(&mut o, 0);
    o.push(0);
    o.extend_from_slice(&[0, 0, 0]);
    u32le(&mut o, 4);
    o
}

/// Da chiamare dal `build.rs`: se si costruisce per Windows (x86-64 o ARM64) scrive l'oggetto con l'icona `png` in `OUT_DIR` e lo
/// passa al linker degli eseguibili. Altrove, o con un PNG che non c'e', non fa niente (l'eseguibile resta senza icona, non si rompe).
#[allow(dead_code)]
pub fn incorpora_icona(png: &str) {
    println!("cargo:rerun-if-changed={png}");
    println!("cargo:rerun-if-changed=build.rs");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let (macchina, rilocazione) = match std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() {
        Ok("x86_64") => (0x8664, 3),
        Ok("aarch64") => (0xAA64, 2),
        _ => return,
    };
    let Ok(dati) = std::fs::read(png) else {
        println!("cargo:warning=icona non trovata: {png}");
        return;
    };
    let oggetto = oggetto_coff(&dati, macchina, rilocazione);
    if oggetto.is_empty() {
        println!("cargo:warning=l'icona {png} non e' un PNG");
        return;
    }
    let Ok(out) = std::env::var("OUT_DIR") else {
        return;
    };
    let percorso = std::path::Path::new(&out).join("icona.o");
    if std::fs::write(&percorso, oggetto).is_ok() {
        println!("cargo:rustc-link-arg-bins={}", percorso.display());
    }
}
