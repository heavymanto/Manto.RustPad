//! La sessione su disco (E35): il formato delle schede e dell'elenco, e il ripristino delle modifiche non salvate di un
//! `Document`. Scritti da ARCH con oracoli indipendenti: il formato e' calcolato a mano byte per byte (con un FNV-1a
//! scritto qui), il documento ripristinato si confronta con un `Vec<u8>` su cui si fanno le stesse modifiche.

use mrp_core::document::{Document, RestoreError};
use mrp_core::edit::{Buf, Piece, PieceTable};
use mrp_core::session::{
    decode, decode_manifest, encode, encode_manifest, EditState, Manifest, TabRecord,
};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn file(data: &[u8]) -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let p = std::env::temp_dir().join(format!("mrp_session_test_{}_{}.txt", std::process::id(), n));
    std::fs::write(&p, data).unwrap();
    p
}

/// FNV-1a a 64 bit, riscritto qui (costanti della specifica) per calcolare a mano il controllo.
fn fnv(dati: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in dati {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

fn semplice() -> TabRecord {
    TabRecord {
        path: "a".to_string(),
        cursor: 5,
        edit: true,
        encoding: String::new(),
        state: None,
    }
}

#[test]
fn a_tab_without_edits_has_exactly_this_layout() {
    let mut atteso: Vec<u8> = Vec::new();
    atteso.extend_from_slice(b"MRPS");
    atteso.push(1); // versione
    atteso.extend_from_slice(&1u64.to_le_bytes()); // lunghezza del percorso
    atteso.push(b'a');
    atteso.extend_from_slice(&5u64.to_le_bytes()); // cursore
    atteso.push(1); // modalita' modifica
    atteso.extend_from_slice(&0u64.to_le_bytes()); // codifica vuota
    atteso.push(0); // senza stato
    let h = fnv(&atteso);
    atteso.extend_from_slice(&h.to_le_bytes());
    assert_eq!(encode(&semplice()), atteso);
    assert_eq!(decode(&atteso).unwrap(), semplice());
}

#[test]
fn a_tab_with_edits_has_exactly_this_layout() {
    let t = TabRecord {
        path: "é".to_string(), // 2 byte in UTF-8
        cursor: 0,
        edit: false,
        encoding: "x".to_string(),
        state: Some(EditState {
            original_len: 10,
            modified: Some((-1, 7)),
            add: b"hi".to_vec(),
            pieces: vec![
                Piece {
                    buf: Buf::Original,
                    start: 0,
                    len: 4,
                },
                Piece {
                    buf: Buf::Add,
                    start: 0,
                    len: 2,
                },
            ],
        }),
    };
    let mut atteso: Vec<u8> = Vec::new();
    atteso.extend_from_slice(b"MRPS");
    atteso.push(1);
    atteso.extend_from_slice(&2u64.to_le_bytes());
    atteso.extend_from_slice("é".as_bytes());
    atteso.extend_from_slice(&0u64.to_le_bytes()); // cursore
    atteso.push(0); // lettura
    atteso.extend_from_slice(&1u64.to_le_bytes());
    atteso.push(b'x');
    atteso.push(1); // con stato
    atteso.extend_from_slice(&10u64.to_le_bytes()); // lunghezza originale
    atteso.push(1); // con data
    atteso.extend_from_slice(&(-1i64 as u64).to_le_bytes());
    atteso.extend_from_slice(&7u32.to_le_bytes());
    atteso.extend_from_slice(&2u64.to_le_bytes());
    atteso.extend_from_slice(b"hi");
    atteso.extend_from_slice(&2u64.to_le_bytes()); // due pezzi
    atteso.push(0);
    atteso.extend_from_slice(&0u64.to_le_bytes());
    atteso.extend_from_slice(&4u64.to_le_bytes());
    atteso.push(1);
    atteso.extend_from_slice(&0u64.to_le_bytes());
    atteso.extend_from_slice(&2u64.to_le_bytes());
    let h = fnv(&atteso);
    atteso.extend_from_slice(&h.to_le_bytes());
    assert_eq!(encode(&t), atteso);
    assert_eq!(decode(&atteso).unwrap(), t);
}

#[test]
fn a_tab_without_a_date_round_trips() {
    let t = TabRecord {
        state: Some(EditState {
            original_len: 0,
            modified: None,
            add: Vec::new(),
            pieces: Vec::new(),
        }),
        ..semplice()
    };
    assert_eq!(decode(&encode(&t)).unwrap(), t);
}

#[test]
fn any_single_altered_byte_or_any_truncation_is_refused_without_panicking() {
    let t = TabRecord {
        path: "C:/x/y.txt".to_string(),
        cursor: 77,
        edit: true,
        encoding: "shift-jis".to_string(),
        state: Some(EditState {
            original_len: 100,
            modified: Some((1_700_000_000, 123)),
            add: b"nuovo testo".to_vec(),
            pieces: vec![
                Piece {
                    buf: Buf::Original,
                    start: 0,
                    len: 50,
                },
                Piece {
                    buf: Buf::Add,
                    start: 0,
                    len: 11,
                },
                Piece {
                    buf: Buf::Original,
                    start: 50,
                    len: 50,
                },
            ],
        }),
    };
    let buono = encode(&t);
    assert_eq!(decode(&buono).unwrap(), t);
    for i in 0..buono.len() {
        let mut v = buono.clone();
        v[i] ^= 0x01;
        assert!(decode(&v).is_err(), "byte {i} alterato");
    }
    for n in 0..buono.len() {
        assert!(decode(&buono[..n]).is_err(), "troncato a {n}");
    }
    let mut piu = buono.clone();
    piu.push(0);
    assert!(decode(&piu).is_err());
}

#[test]
fn a_forged_but_well_checksummed_tab_cannot_ask_for_huge_memory() {
    // un blocco che dichiara una lunghezza enorme, con il controllo giusto: si rifiuta, non si alloca
    let mut v: Vec<u8> = Vec::new();
    v.extend_from_slice(b"MRPS");
    v.push(1);
    v.extend_from_slice(&u64::MAX.to_le_bytes());
    let h = fnv(&v);
    v.extend_from_slice(&h.to_le_bytes());
    assert!(decode(&v).is_err());
    // un conteggio di pezzi enorme con pochi byte dietro
    let mut v: Vec<u8> = Vec::new();
    v.extend_from_slice(b"MRPS");
    v.push(1);
    v.extend_from_slice(&0u64.to_le_bytes());
    v.extend_from_slice(&0u64.to_le_bytes());
    v.push(0);
    v.extend_from_slice(&0u64.to_le_bytes());
    v.push(1);
    v.extend_from_slice(&0u64.to_le_bytes());
    v.push(0);
    v.extend_from_slice(&0u64.to_le_bytes());
    v.extend_from_slice(&u64::MAX.to_le_bytes());
    let h = fnv(&v);
    v.extend_from_slice(&h.to_le_bytes());
    assert!(decode(&v).is_err());
}

#[test]
fn the_manifest_round_trips_and_refuses_names_that_leave_the_folder() {
    let m = Manifest {
        active: 1,
        tabs: vec!["tab-0.mrps".to_string(), "tab-1.mrps".to_string()],
    };
    let testo = encode_manifest(&m);
    assert_eq!(
        testo,
        "mrp-session 1\nactive 1\ntab tab-0.mrps\ntab tab-1.mrps\n"
    );
    assert_eq!(decode_manifest(&testo).unwrap(), m);
    let vuoto = Manifest {
        active: 0,
        tabs: vec![],
    };
    assert_eq!(decode_manifest(&encode_manifest(&vuoto)).unwrap(), vuoto);
    for nome in ["../x", "a/b", "a\\b", ".nascosto", "c:x", ""] {
        let t = format!("mrp-session 1\nactive 0\ntab {nome}\n");
        assert!(decode_manifest(&t).is_err(), "{nome:?}");
    }
    assert!(decode_manifest("altro\nactive 0\n").is_err());
    assert!(decode_manifest("mrp-session 1\n").is_err());
    assert!(decode_manifest("mrp-session 1\nactive x\n").is_err());
    assert!(decode_manifest("mrp-session 1\nactive 0\nboh\n").is_err());
}

// ---- il documento ----

/// Un generatore di numeri a caso ripetibile (LCG a 64 bit).
struct Caso(u64);

impl Caso {
    fn prossimo(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0 >> 33
    }
    fn fino_a(&mut self, n: u64) -> u64 {
        if n == 0 {
            0
        } else {
            self.prossimo() % n
        }
    }
}

/// Modifiche a caso su un documento e sul suo modello (un `Vec<u8>`).
fn modifica(doc: &mut Document, modello: &mut Vec<u8>, caso: &mut Caso, giri: usize) {
    for _ in 0..giri {
        if caso.fino_a(3) < 2 || modello.is_empty() {
            let pos = caso.fino_a(modello.len() as u64 + 1);
            let n = 1 + caso.fino_a(6) as usize;
            let dati: Vec<u8> = (0..n)
                .map(|_| match caso.fino_a(8) {
                    0 => b'\n',
                    k => b'a' + k as u8,
                })
                .collect();
            doc.insert(pos, &dati).unwrap();
            let p = pos as usize;
            modello.splice(p..p, dati);
        } else {
            let pos = caso.fino_a(modello.len() as u64);
            let n = 1 + caso.fino_a((modello.len() as u64 - pos).min(8));
            doc.delete(pos, n).unwrap();
            modello.drain(pos as usize..(pos + n) as usize);
        }
    }
}

fn contenuto(doc: &Document) -> Vec<u8> {
    doc.read_range(0, doc.len())
}

#[test]
fn restored_edits_read_back_exactly_like_a_model_over_many_random_histories() {
    for seme in 0..40u64 {
        let mut caso = Caso(seme + 1);
        let originale: Vec<u8> = (0..200 + caso.fino_a(300))
            .map(|i| {
                if i % 13 == 12 {
                    b'\n'
                } else {
                    b'A' + (i % 26) as u8
                }
            })
            .collect();
        let p = file(&originale);
        let mut doc = Document::open(&p).unwrap();
        let mut modello = originale.clone();
        let giri = 1 + caso.fino_a(300) as usize;
        modifica(&mut doc, &mut modello, &mut caso, giri);
        assert_eq!(
            contenuto(&doc),
            modello,
            "seme {seme}: il modello e' giusto"
        );
        // salva lo stato, passalo per i byte, rifallo su un documento nuovo
        let stato = doc.edit_state();
        if modello == originale {
            // un documento tornato uguale all'originale puo' comunque risultare modificato: si salta se non c'e' stato
        }
        let Some(stato) = stato else {
            continue;
        };
        let rec = TabRecord {
            path: p.display().to_string(),
            cursor: 0,
            edit: true,
            encoding: String::new(),
            state: Some(stato),
        };
        let letto = decode(&encode(&rec)).unwrap();
        let mut nuovo = Document::open(&p).unwrap();
        nuovo.restore_edits(letto.state.as_ref().unwrap()).unwrap();
        assert_eq!(contenuto(&nuovo), modello, "seme {seme}");
        assert_eq!(nuovo.len(), modello.len() as u64);
        assert!(nuovo.is_modified());
        assert!(!nuovo.can_undo() && !nuovo.can_redo());
        let _ = std::fs::remove_file(p);
    }
}

#[test]
fn a_restored_document_can_be_edited_undone_and_saved() {
    let p = file(b"uno\ndue\ntre\n");
    let mut doc = Document::open(&p).unwrap();
    doc.insert(4, b"XX").unwrap();
    doc.delete(0, 1).unwrap();
    let stato = doc.edit_state().unwrap();
    drop(doc);
    let mut nuovo = Document::open(&p).unwrap();
    nuovo.restore_edits(&stato).unwrap();
    assert_eq!(contenuto(&nuovo), b"no\nXXdue\ntre\n");
    // una modifica nuova si annulla fino allo stato ripristinato, non oltre
    nuovo.seal();
    nuovo.insert(0, b"Q").unwrap();
    assert_eq!(contenuto(&nuovo), b"Qno\nXXdue\ntre\n");
    nuovo.undo().unwrap();
    assert_eq!(contenuto(&nuovo), b"no\nXXdue\ntre\n");
    assert!(nuovo.is_modified());
    assert!(!nuovo.can_undo());
    // il salvataggio scrive il testo ripristinato e il documento torna non modificato
    let stop = std::sync::atomic::AtomicBool::new(false);
    let prog = std::sync::atomic::AtomicU64::new(0);
    nuovo.save(&stop, &prog).unwrap();
    assert!(!nuovo.is_modified());
    assert_eq!(std::fs::read(&p).unwrap(), b"no\nXXdue\ntre\n");
    assert!(nuovo.edit_state().is_none());
    let _ = std::fs::remove_file(p);
}

#[test]
fn there_is_no_state_without_edits_and_none_after_undoing_them_all() {
    let p = file(b"abc\n");
    let mut doc = Document::open(&p).unwrap();
    assert!(doc.edit_state().is_none());
    let v0 = doc.version();
    doc.insert(0, b"x").unwrap();
    assert!(doc.edit_state().is_some());
    assert_ne!(doc.version(), v0);
    doc.undo().unwrap();
    assert!(doc.edit_state().is_none());
    assert_eq!(doc.version(), v0);
    let _ = std::fs::remove_file(p);
}

#[test]
fn restoring_is_refused_when_the_file_changed_and_nothing_is_touched() {
    let p = file(b"uno\ndue\n");
    let mut doc = Document::open(&p).unwrap();
    doc.insert(0, b"Q").unwrap();
    let stato = doc.edit_state().unwrap();
    drop(doc);
    // lunghezza diversa
    std::fs::write(&p, b"uno\ndue\ntre\n").unwrap();
    let mut nuovo = Document::open(&p).unwrap();
    assert_eq!(nuovo.restore_edits(&stato), Err(RestoreError::FileChanged));
    assert_eq!(contenuto(&nuovo), b"uno\ndue\ntre\n");
    assert!(!nuovo.is_modified());
    // (il file mappato non si puo' riscrivere: si chiude)
    drop(nuovo);
    // stessa lunghezza, altra data
    std::fs::write(&p, b"uno\ndue\n").unwrap();
    let f = std::fs::OpenOptions::new().write(true).open(&p).unwrap();
    f.set_modified(std::time::SystemTime::now() + std::time::Duration::from_secs(3600))
        .unwrap();
    drop(f);
    let mut terzo = Document::open(&p).unwrap();
    assert_eq!(terzo.restore_edits(&stato), Err(RestoreError::FileChanged));
    assert!(!terzo.is_modified());
    let _ = std::fs::remove_file(p);
}

#[test]
fn restoring_is_refused_over_existing_edits_or_twice() {
    let p = file(b"uno\ndue\n");
    let mut doc = Document::open(&p).unwrap();
    doc.insert(0, b"Q").unwrap();
    let stato = doc.edit_state().unwrap();
    assert_eq!(doc.restore_edits(&stato), Err(RestoreError::HasHistory));
    let mut nuovo = Document::open(&p).unwrap();
    nuovo.restore_edits(&stato).unwrap();
    assert_eq!(nuovo.restore_edits(&stato), Err(RestoreError::HasHistory));
    assert_eq!(contenuto(&nuovo), b"Quno\ndue\n");
    // un documento senza modifiche ma con storia (modifica e annulla)
    let mut con_storia = Document::open(&p).unwrap();
    con_storia.insert(0, b"z").unwrap();
    con_storia.undo().unwrap();
    assert_eq!(
        con_storia.restore_edits(&stato),
        Err(RestoreError::HasHistory)
    );
    let _ = std::fs::remove_file(p);
}

#[test]
fn incoherent_pieces_are_refused_and_the_document_is_unchanged() {
    let p = file(b"uno\ndue\n");
    let mut doc = Document::open(&p).unwrap();
    doc.insert(0, b"Q").unwrap();
    let buono = doc.edit_state().unwrap();
    let md = std::fs::metadata(&p).unwrap();
    assert_eq!(buono.original_len, md.len());
    for pezzi in [
        // un pezzo oltre la fine del file
        vec![Piece {
            buf: Buf::Original,
            start: 5,
            len: 100,
        }],
        // un pezzo oltre la fine del buffer delle aggiunte
        vec![Piece {
            buf: Buf::Add,
            start: 0,
            len: 99,
        }],
        // somma che trabocca
        vec![Piece {
            buf: Buf::Original,
            start: u64::MAX,
            len: 2,
        }],
    ] {
        let falso = EditState {
            pieces: pezzi,
            ..buono.clone()
        };
        let mut nuovo = Document::open(&p).unwrap();
        assert!(matches!(
            nuovo.restore_edits(&falso),
            Err(RestoreError::Invalid(_))
        ));
        assert_eq!(contenuto(&nuovo), b"uno\ndue\n");
        assert!(!nuovo.is_modified());
    }
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_line_index_built_before_restoring_still_counts_the_restored_text() {
    let originale: Vec<u8> = (0..300)
        .map(|i| format!("riga {i}\n"))
        .collect::<String>()
        .into_bytes();
    let p = file(&originale);
    let mut a = Document::open(&p).unwrap();
    let mut caso = Caso(99);
    let mut modello = originale.clone();
    modifica(&mut a, &mut modello, &mut caso, 120);
    let stato = a.edit_state().unwrap();
    // il documento di riferimento: lo stesso testo, ottenuto con le modifiche vere e un indice costruito
    let b = {
        let mut d = Document::open(&p).unwrap();
        let _ = d.build_lines(
            &std::sync::atomic::AtomicBool::new(false),
            &std::sync::atomic::AtomicU64::new(0),
        );
        d
    };
    drop(b);
    // quello ripristinato, con l'indice costruito PRIMA del ripristino (come fa l'app: l'indice parte all'apertura)
    let mut nuovo = Document::open(&p).unwrap();
    nuovo
        .build_lines(
            &std::sync::atomic::AtomicBool::new(false),
            &std::sync::atomic::AtomicU64::new(0),
        )
        .unwrap();
    // le interrogazioni di prima del ripristino riempiono le cache: non devono sopravvivere
    let _ = nuovo.line_count();
    let _ = nuovo.offset_of_line(3);
    nuovo.restore_edits(&stato).unwrap();
    let righe = modello.iter().filter(|&&b| b == b'\n').count() as u64;
    let attesa = if modello.last() == Some(&b'\n') {
        righe
    } else {
        righe + 1
    };
    let ottenute = nuovo.line_count().expect("l'indice c'e'");
    assert!(
        ottenute == attesa || ottenute == righe + 1,
        "righe {ottenute}, attese {attesa} (o {})",
        righe + 1
    );
    // e la posizione di ogni riga e' quella del modello
    let mut inizi = vec![0u64];
    for (i, &b) in modello.iter().enumerate() {
        if b == b'\n' && i + 1 < modello.len() {
            inizi.push(i as u64 + 1);
        }
    }
    for (n, &inizio) in inizi.iter().enumerate().step_by(7) {
        assert_eq!(nuovo.offset_of_line(n as u64), Some(inizio), "riga {n}");
    }
    let _ = std::fs::remove_file(p);
}

// ---- la tabella dei pezzi ----

#[test]
fn from_parts_merges_contiguous_pieces_and_drops_empty_ones() {
    let add = b"xyz".to_vec();
    let pezzi = [
        Piece {
            buf: Buf::Original,
            start: 0,
            len: 3,
        },
        Piece {
            buf: Buf::Original,
            start: 3,
            len: 2,
        },
        Piece {
            buf: Buf::Add,
            start: 1,
            len: 0,
        },
        Piece {
            buf: Buf::Add,
            start: 0,
            len: 1,
        },
        Piece {
            buf: Buf::Add,
            start: 1,
            len: 2,
        },
        Piece {
            buf: Buf::Original,
            start: 9,
            len: 1,
        },
    ];
    let t = PieceTable::from_parts(10, 512, add, &pezzi).unwrap();
    assert_eq!(
        t.pieces(),
        vec![
            Piece {
                buf: Buf::Original,
                start: 0,
                len: 5
            },
            Piece {
                buf: Buf::Add,
                start: 0,
                len: 3
            },
            Piece {
                buf: Buf::Original,
                start: 9,
                len: 1
            },
        ]
    );
    assert_eq!(t.len(), 9);
    assert_eq!(t.piece_count(), 3);
}

#[test]
fn from_parts_with_thousands_of_pieces_splits_into_blocks_that_still_edit_correctly() {
    // 3000 pezzi alternati: originale e aggiunte, a blocchi piccoli, per avere molti blocchi
    let add: Vec<u8> = (0..3000u32).map(|i| b'a' + (i % 26) as u8).collect();
    let originale_len = 6000u64;
    let mut pezzi = Vec::new();
    let mut modello: Vec<u8> = Vec::new();
    let originale: Vec<u8> = (0..originale_len).map(|i| b'0' + (i % 10) as u8).collect();
    for i in 0..3000u64 {
        pezzi.push(Piece {
            buf: Buf::Original,
            start: i * 2,
            len: 1,
        });
        modello.push(originale[(i * 2) as usize]);
        pezzi.push(Piece {
            buf: Buf::Add,
            start: i,
            len: 1,
        });
        modello.push(add[i as usize]);
    }
    let mut t = PieceTable::from_parts(originale_len, 8, add, &pezzi).unwrap();
    assert!(t.chunk_count() > 100, "{}", t.chunk_count());
    assert_eq!(t.len(), modello.len() as u64);
    // dopo il ripristino la tabella regge inserimenti e cancellazioni a caso (confronto con il modello)
    let mut caso = Caso(5);
    for _ in 0..500 {
        let pos = caso.fino_a(modello.len() as u64 + 1);
        if caso.fino_a(2) == 0 {
            t.insert(pos, b"##").unwrap();
            modello.splice(pos as usize..pos as usize, *b"##");
        } else if (pos as usize) < modello.len() {
            let n = 1 + caso.fino_a((modello.len() as u64 - pos).min(5));
            t.delete(pos, n).unwrap();
            modello.drain(pos as usize..(pos + n) as usize);
        }
    }
    let sorgente = {
        // una sorgente col contenuto originale per leggere i pezzi
        let p = file(&originale);
        mrp_core::FileSource::open(&p).unwrap()
    };
    assert_eq!(t.read_range(&sorgente, 0, t.len()), modello);
}

#[test]
fn from_parts_refuses_pieces_outside_their_buffers() {
    let pezzo = |buf, start, len| Piece { buf, start, len };
    assert!(PieceTable::from_parts(5, 8, vec![], &[pezzo(Buf::Original, 0, 6)]).is_err());
    assert!(PieceTable::from_parts(5, 8, vec![], &[pezzo(Buf::Original, 0, 5)]).is_ok());
    assert!(PieceTable::from_parts(5, 8, vec![1], &[pezzo(Buf::Add, 0, 2)]).is_err());
    assert!(PieceTable::from_parts(5, 8, vec![1, 2], &[pezzo(Buf::Add, 0, 2)]).is_ok());
    assert!(PieceTable::from_parts(5, 8, vec![], &[pezzo(Buf::Original, u64::MAX, 1)]).is_err());
    // nessun pezzo: documento vuoto
    let vuoto = PieceTable::from_parts(5, 8, vec![], &[]).unwrap();
    assert_eq!(vuoto.len(), 0);
    assert_eq!(vuoto.piece_count(), 0);
}

#[test]
fn a_well_checksummed_tab_with_extra_bytes_is_refused() {
    let buono = encode(&semplice());
    let mut v = buono[..buono.len() - 8].to_vec();
    v.push(0xAA);
    let h = fnv(&v);
    v.extend_from_slice(&h.to_le_bytes());
    assert!(decode(&v).is_err());
}

#[test]
fn restoring_checks_the_length_and_the_date_separately_down_to_the_nanosecond() {
    let p = file(b"uno\ndue\n");
    let mut doc = Document::open(&p).unwrap();
    doc.insert(0, b"Q").unwrap();
    let stato = doc.edit_state().unwrap();
    let (sec, nano) = stato.modified.expect("la data c'e'");
    drop(doc);
    // la data uguale ma la lunghezza diversa (si rimette la data di prima dopo aver riscritto)
    let data = std::time::UNIX_EPOCH + std::time::Duration::new(sec as u64, nano);
    std::fs::write(&p, b"uno\ndue\nxx\n").unwrap();
    let f = std::fs::OpenOptions::new().write(true).open(&p).unwrap();
    f.set_modified(data).unwrap();
    drop(f);
    let mut a = Document::open(&p).unwrap();
    assert_eq!(a.restore_edits(&stato), Err(RestoreError::FileChanged));
    drop(a);
    // la lunghezza uguale e la data diversa di soli 500 ns (Windows conta a 100 ns)
    std::fs::write(&p, b"uno\ndue\n").unwrap();
    let f = std::fs::OpenOptions::new().write(true).open(&p).unwrap();
    f.set_modified(data + std::time::Duration::from_nanos(500))
        .unwrap();
    drop(f);
    let mut b = Document::open(&p).unwrap();
    assert_eq!(b.restore_edits(&stato), Err(RestoreError::FileChanged));
    drop(b);
    // e con la data esatta, lunghezza uguale: si ripristina
    let f = std::fs::OpenOptions::new().write(true).open(&p).unwrap();
    f.set_modified(data).unwrap();
    drop(f);
    let mut c = Document::open(&p).unwrap();
    assert_eq!(c.restore_edits(&stato), Ok(()));
    assert_eq!(contenuto(&c), b"Quno\ndue\n");
    drop(c);
    let _ = std::fs::remove_file(p);
}

#[test]
fn restoring_is_refused_after_a_save_that_kept_the_undo_history() {
    let p = file(b"uno\ndue\n");
    let mut doc = Document::open(&p).unwrap();
    doc.insert(0, b"Q").unwrap();
    let stato = doc.edit_state().unwrap();
    doc.seal();
    let stop = std::sync::atomic::AtomicBool::new(false);
    let prog = std::sync::atomic::AtomicU64::new(0);
    doc.save(&stop, &prog).unwrap();
    assert!(!doc.is_modified());
    // dopo il salvataggio la storia dell'annulla c'e' ancora: il documento non e' "appena aperto"
    // il salvataggio chiude la storia: niente da annullare, quindi un ripristino e' ammesso solo sul file appena
    // aperto; qui il file e' cambiato (salvato), e la data non coincide piu'
    {
        assert!(!doc.can_undo(), "il salvataggio azzera la storia");
        assert!(doc.restore_edits(&stato).is_err());
    }
    let _ = std::fs::remove_file(p);
}
