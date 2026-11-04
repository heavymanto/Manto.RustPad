//! L'icona del progetto dentro l'eseguibile: l'oggetto COFF che `build.rs` passa al linker (`build_support/icona.rs`). Scritti da
//! ARCH: si rileggono i byte a mano, seguendo il formato dei file COFF e delle risorse di Windows (tipo, nome, lingua), senza usare il
//! codice che li ha scritti. La prova sull'eseguibile vero e' a parte (`inspect_pe_resources`).

include!("../../../build_support/icona.rs");

const PNG: &[u8] = include_bytes!("../../../assets/icon.png");

fn u16_at(d: &[u8], i: usize) -> u16 {
    u16::from_le_bytes([d[i], d[i + 1]])
}

fn u32_at(d: &[u8], i: usize) -> u32 {
    u32::from_le_bytes([d[i], d[i + 1], d[i + 2], d[i + 3]])
}

/// Le voci con identificatore della directory a `dove` nella sezione: `(id, destinazione)`.
fn voci(sezione: &[u8], dove: usize) -> Vec<(u32, u32)> {
    assert_eq!(u16_at(sezione, dove + 12), 0, "nessuna voce con nome");
    let n = usize::from(u16_at(sezione, dove + 14));
    (0..n)
        .map(|i| {
            (
                u32_at(sezione, dove + 16 + 8 * i),
                u32_at(sezione, dove + 20 + 8 * i),
            )
        })
        .collect()
}

/// Segue tipo -> nome -> lingua fino alla voce dati; ritorna la voce (dove sta nella sezione), i byte e la dimensione.
fn risorsa(sezione: &[u8], tipo: u32, nome: u32) -> (usize, u32, u32) {
    let alto = 0x8000_0000u32;
    let t = voci(sezione, 0);
    let (_, d) = t.iter().find(|(id, _)| *id == tipo).expect("il tipo");
    assert!(d & alto != 0);
    let n = voci(sezione, (d & !alto) as usize);
    let (_, d) = n.iter().find(|(id, _)| *id == nome).expect("il nome");
    assert!(d & alto != 0);
    let l = voci(sezione, (d & !alto) as usize);
    assert_eq!(l.len(), 1);
    assert_eq!(l[0].0, 0x0409, "inglese");
    assert!(l[0].1 & alto == 0, "una voce dati, non una directory");
    let voce = l[0].1 as usize;
    (voce, u32_at(sezione, voce), u32_at(sezione, voce + 4))
}

#[test]
fn the_object_is_a_coff_file_with_one_resource_section() {
    let o = oggetto_coff(PNG, 0x8664, 3);
    assert_eq!(u16_at(&o, 0), 0x8664, "x86-64");
    assert_eq!(u16_at(&o, 2), 1, "una sezione");
    let simboli = u32_at(&o, 8) as usize;
    assert_eq!(u32_at(&o, 12), 2, "il simbolo e la sua voce ausiliaria");
    assert_eq!(&o[20..28], b".rsrc\0\0\0");
    let (dimensione, grezzi, rilocazioni) = (
        u32_at(&o, 36) as usize,
        u32_at(&o, 40) as usize,
        u32_at(&o, 44) as usize,
    );
    assert_eq!(u16_at(&o, 52), 2, "due rilocazioni");
    assert_eq!(u32_at(&o, 56), 0x4030_0040, "dati, in lettura, a 4 byte");
    assert_eq!(dimensione % 4, 0);
    // la sezione, le rilocazioni e i simboli stanno uno dopo l'altro, e il file finisce con la tabella delle stringhe vuota
    assert_eq!(grezzi, 60);
    assert_eq!(rilocazioni, grezzi + dimensione);
    assert_eq!(simboli, rilocazioni + 20);
    assert_eq!(o.len(), simboli + 36 + 4);
    assert_eq!(u32_at(&o, o.len() - 4), 4);
    // il simbolo: il nome della sezione, sezione 1, classe STATIC con una voce ausiliaria che dice la lunghezza
    assert_eq!(&o[simboli..simboli + 8], b".rsrc\0\0\0");
    assert_eq!(u16_at(&o, simboli + 12), 1);
    assert_eq!((o[simboli + 16], o[simboli + 17]), (3, 1));
    assert_eq!(u32_at(&o, simboli + 18) as usize, dimensione);
}

#[test]
fn the_icon_group_and_the_png_are_where_the_tree_says() {
    let o = oggetto_coff(PNG, 0x8664, 3);
    let (grezzi, dimensione, rilocazioni) = (
        u32_at(&o, 40) as usize,
        u32_at(&o, 36) as usize,
        u32_at(&o, 44) as usize,
    );
    let sezione = &o[grezzi..grezzi + dimensione];
    // il gruppo (tipo 14, nome 1)
    let (voce_g, rva_g, len_g) = risorsa(sezione, 14, 1);
    let (voce_i, rva_i, len_i) = risorsa(sezione, 3, 1);
    // le rilocazioni puntano alle due voci dati, sul simbolo 0, di tipo ADDR32NB; in loco c'e' la distanza dei byte
    let mut rilocate = Vec::new();
    for k in 0..2 {
        let r = rilocazioni + 10 * k;
        let (dove, simbolo, tipo) = (u32_at(&o, r) as usize, u32_at(&o, r + 4), u16_at(&o, r + 8));
        assert_eq!((simbolo, tipo), (0, 3));
        rilocate.push(dove);
    }
    rilocate.sort_unstable();
    let mut voci_dati = vec![voce_g, voce_i];
    voci_dati.sort_unstable();
    assert_eq!(rilocate, voci_dati);
    // il gruppo: intestazione, una voce di 64 x 64 a 32 bit che rimanda all'icona 1 e dice quanto e' lunga
    assert_eq!(len_g, 20);
    let g = &sezione[rva_g as usize..rva_g as usize + 20];
    assert_eq!((u16_at(g, 0), u16_at(g, 2), u16_at(g, 4)), (0, 1, 1));
    assert_eq!((g[6], g[7], g[8], g[9]), (64, 64, 0, 0));
    assert_eq!((u16_at(g, 10), u16_at(g, 12)), (1, 32));
    assert_eq!(u32_at(g, 14) as usize, PNG.len());
    assert_eq!(u16_at(g, 18), 1, "l'icona 1");
    // l'icona: il PNG com'e', byte per byte
    assert_eq!(len_i as usize, PNG.len());
    assert_eq!(&sezione[rva_i as usize..rva_i as usize + PNG.len()], PNG);
}

#[test]
fn the_png_of_the_project_is_what_the_resource_says_it_is() {
    // 64 x 64, 8 bit, RGBA: l'intestazione e' quella che il gruppo ha copiato
    assert_eq!(&PNG[..8], b"\x89PNG\r\n\x1a\n");
    assert_eq!(&PNG[12..16], b"IHDR");
    assert_eq!(u32::from_be_bytes([PNG[16], PNG[17], PNG[18], PNG[19]]), 64);
    assert_eq!(u32::from_be_bytes([PNG[20], PNG[21], PNG[22], PNG[23]]), 64);
}

#[test]
fn arm64_uses_its_own_machine_and_relocation_and_a_non_png_gives_nothing() {
    let o = oggetto_coff(PNG, 0xAA64, 2);
    assert_eq!(u16_at(&o, 0), 0xAA64);
    let rilocazioni = u32_at(&o, 44) as usize;
    assert_eq!(u16_at(&o, rilocazioni + 8), 2);
    assert_eq!(u16_at(&o, rilocazioni + 18), 2);
    assert!(oggetto_coff(b"non e' un png", 0x8664, 3).is_empty());
    assert!(oggetto_coff(&PNG[..20], 0x8664, 3).is_empty());
    // una dimensione da 256 in su si scrive 0 nel gruppo
    let mut grande = PNG.to_vec();
    grande[16..20].copy_from_slice(&256u32.to_be_bytes());
    grande[20..24].copy_from_slice(&512u32.to_be_bytes());
    let o = oggetto_coff(&grande, 0x8664, 3);
    let (g, d) = (u32_at(&o, 40) as usize, u32_at(&o, 36) as usize);
    let sezione = &o[g..g + d];
    let (_, rva, _) = risorsa(sezione, 14, 1);
    assert_eq!(
        (sezione[rva as usize + 6], sezione[rva as usize + 7]),
        (0, 0)
    );
}

#[test]
fn the_window_icon_is_the_png_decoded_to_rgba() {
    let i = mrp_gui::icona::icona_finestra().expect("l'icona della finestra");
    assert_eq!((i.width, i.height), (64, 64));
    assert_eq!(i.rgba.len(), 64 * 64 * 4);
    // alcuni punti letti a parte (decodificando il PNG con un altro programma): angoli quasi opachi, il centro azzurro
    let punto = |x: usize, y: usize| -> [u8; 4] {
        let k = (y * 64 + x) * 4;
        [i.rgba[k], i.rgba[k + 1], i.rgba[k + 2], i.rgba[k + 3]]
    };
    assert_eq!(punto(0, 0), [12, 19, 35, 220]);
    assert_eq!(punto(63, 0), [24, 41, 61, 221]);
    assert_eq!(punto(32, 32), [61, 150, 205, 255]);
    assert_eq!(punto(10, 50), [0, 2, 14, 255]);
    assert_eq!(punto(63, 63), [11, 18, 34, 222]);
    // e il PNG incorporato e' lo stesso del progetto
    assert_eq!(mrp_gui::icona::PNG, PNG);
}
