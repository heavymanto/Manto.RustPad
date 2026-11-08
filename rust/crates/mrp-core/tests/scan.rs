//! Test della navigazione per righe/righe-visuali (`scan`). Scritti da ARCH prima dell'implementazione: gli attesi
//! vengono da cicli ingenui su un `Vec<u8>` che replica le modifiche, mai dal codice.

use mrp_core::newline::Newline;
use mrp_core::scan::{
    crlf_at, cursor_end, next_row, prev_row_start, row_end, row_start, Next, DEFAULT_MAX_SCAN,
};
use mrp_core::Document;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

// ---------- attrezzi ----------

static COUNTER: AtomicUsize = AtomicUsize::new(0);

struct TestDir(PathBuf);

impl TestDir {
    fn new() -> TestDir {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let p = std::env::temp_dir().join(format!("mrp_scan_test_{}_{}", std::process::id(), n));
        std::fs::create_dir_all(&p).unwrap();
        TestDir(p)
    }
    fn open(&self, data: &[u8]) -> Document {
        // un nome nuovo a ogni apertura: su Windows un file ancora mappato non si puo' troncare
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let p = self.0.join(format!("a{n}.txt"));
        std::fs::write(&p, data).unwrap();
        Document::open(&p).unwrap()
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0 >> 33
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

/// Byte pseudocasuali con un `\n` ogni `nl_every` in media.
fn texty(r: &mut Lcg, n: usize, nl_every: u64) -> Vec<u8> {
    (0..n)
        .map(|_| {
            if r.below(nl_every) == 0 {
                b'\n'
            } else if r.below(9) == 0 {
                0xff
            } else {
                b'a'
            }
        })
        .collect()
}

// ---------- oracoli ingenui ----------

fn o_next(m: &[u8], pos: u64, max: u64) -> Next {
    let len = m.len() as u64;
    if pos >= len {
        return Next::End;
    }
    let fine = len.min(pos.saturating_add(max));
    for i in pos..fine {
        if m[i as usize] == b'\n' {
            return if i + 1 < len {
                Next::Line(i + 1)
            } else {
                Next::End
            };
        }
    }
    if pos.saturating_add(max) < len {
        Next::Cut(pos + max)
    } else {
        Next::End
    }
}

fn o_row_end(m: &[u8], pos: u64, max: u64) -> u64 {
    let len = m.len() as u64;
    let fine = len.min(pos.saturating_add(max));
    for i in pos.min(len)..fine {
        if m[i as usize] == b'\n' {
            return i;
        }
    }
    fine.max(pos.min(len))
}

fn o_row_start(m: &[u8], pos: u64, max: u64) -> (u64, bool) {
    let pos = pos.min(m.len() as u64);
    let lo = pos.saturating_sub(max);
    for i in (lo..pos).rev() {
        if m[i as usize] == b'\n' {
            return (i + 1, true);
        }
    }
    if pos <= max {
        (0, true)
    } else {
        (pos - max, false)
    }
}

fn o_prev(m: &[u8], pos: u64, max: u64) -> Option<u64> {
    let pos = pos.min(m.len() as u64);
    if pos == 0 {
        None
    } else {
        Some(o_row_start(m, pos - 1, max).0)
    }
}

fn check_pos(d: &Document, m: &[u8], pos: u64, max: u64, what: &str) {
    assert_eq!(
        next_row(d, pos, max),
        o_next(m, pos, max),
        "{what}: next_row({pos}, {max})"
    );
    assert_eq!(
        row_start(d, pos, max),
        o_row_start(m, pos, max),
        "{what}: row_start({pos}, {max})"
    );
    assert_eq!(
        row_end(d, pos, max),
        o_row_end(m, pos, max),
        "{what}: row_end({pos}, {max})"
    );
    assert_eq!(
        prev_row_start(d, pos, max),
        o_prev(m, pos, max),
        "{what}: prev_row_start({pos}, {max})"
    );
}

// ---------- a mano ----------

#[test]
fn small_document_by_hand() {
    let dir = TestDir::new();
    let d = dir.open(b"ab\ncde\n\nf");
    // inizi di riga: 0, 3, 7, 8
    let m = DEFAULT_MAX_SCAN;
    assert_eq!(next_row(&d, 0, m), Next::Line(3));
    assert_eq!(next_row(&d, 3, m), Next::Line(7));
    assert_eq!(next_row(&d, 7, m), Next::Line(8));
    assert_eq!(next_row(&d, 8, m), Next::End);
    assert_eq!(next_row(&d, 99, m), Next::End);
    assert_eq!(row_start(&d, 0, m), (0, true));
    assert_eq!(row_start(&d, 2, m), (0, true)); // il \n appartiene alla riga che chiude
    assert_eq!(row_start(&d, 3, m), (3, true));
    assert_eq!(row_start(&d, 5, m), (3, true));
    assert_eq!(row_start(&d, 7, m), (7, true));
    assert_eq!(row_start(&d, 9, m), (8, true)); // pos == len
    assert_eq!(row_start(&d, 99, m), (8, true)); // oltre la fine: si ferma alla fine
                                                 // row_end: la posizione del \n che chiude la riga, o la fine
    assert_eq!(row_end(&d, 0, m), 2);
    assert_eq!(row_end(&d, 3, m), 6);
    assert_eq!(row_end(&d, 7, m), 7, "riga vuota");
    assert_eq!(
        row_end(&d, 8, m),
        9,
        "ultima riga senza a-capo: la fine del documento"
    );
    assert_eq!(row_end(&d, 99, m), 9);
    assert_eq!(row_end(&d, 3, 2), 5, "taglio dopo 2 byte");
    assert_eq!(prev_row_start(&d, 0, m), None);
    assert_eq!(prev_row_start(&d, 3, m), Some(0));
    assert_eq!(prev_row_start(&d, 8, m), Some(7));
    assert_eq!(prev_row_start(&d, 7, m), Some(3));
}

#[test]
fn trailing_newline_does_not_open_a_row() {
    let dir = TestDir::new();
    let d = dir.open(b"ab\n");
    assert_eq!(next_row(&d, 0, DEFAULT_MAX_SCAN), Next::End);
    let d = dir.open(b"");
    assert_eq!(next_row(&d, 0, DEFAULT_MAX_SCAN), Next::End);
    assert_eq!(row_start(&d, 0, DEFAULT_MAX_SCAN), (0, true));
    assert_eq!(prev_row_start(&d, 0, DEFAULT_MAX_SCAN), None);
}

#[test]
fn long_lines_are_cut_instead_of_scanned_for_ever() {
    let dir = TestDir::new();
    // una riga di 100 'a', poi \n, poi "b"
    let mut data = vec![b'a'; 100];
    data.push(b'\n');
    data.push(b'b');
    let d = dir.open(&data);
    // con 100 byte esaminati da 0 si vedono gli indici 0..99, tutti 'a' (il \n e' all'indice 100): taglio
    assert_eq!(next_row(&d, 0, 100), Next::Cut(100));
    assert_eq!(next_row(&d, 0, 50), Next::Cut(50));
    assert_eq!(next_row(&d, 50, 50), Next::Cut(100));
    assert_eq!(next_row(&d, 100, 50), Next::Line(101));
    // all'indietro dalla seconda riga
    assert_eq!(row_start(&d, 101, 1000), (101, true));
    assert_eq!(row_start(&d, 100, 30), (70, false));
    assert_eq!(row_start(&d, 100, 1000), (0, true));
    assert_eq!(prev_row_start(&d, 101, 30), Some(70));
}

#[test]
fn zero_max_scan_is_treated_as_one_so_it_always_progresses() {
    let dir = TestDir::new();
    let d = dir.open(
        b"ab
cd
",
    );
    assert_eq!(next_row(&d, 0, 0), next_row(&d, 0, 1));
    assert_eq!(next_row(&d, 0, 0), Next::Cut(1));
    assert_eq!(row_start(&d, 3, 0), row_start(&d, 3, 1));
    assert_eq!(prev_row_start(&d, 4, 0), prev_row_start(&d, 4, 1));
}

// ---------- casuali contro l'oracolo ----------

#[test]
fn every_position_on_small_edited_documents() {
    let dir = TestDir::new();
    for seed in 1..=8u64 {
        let mut r = Lcg(seed * 31);
        let n = r.below(400) as usize;
        let mut m = texty(&mut r, n, 6 + seed);
        let mut d = dir.open(&m);
        for step in 0..12 {
            // modifica casuale (anche vuota all'inizio: passo 0 = documento intatto)
            if step > 0 {
                if r.below(2) == 0 || m.is_empty() {
                    let pos = r.below(m.len() as u64 + 1) as usize;
                    let k = r.below(40) as usize;
                    let data = texty(&mut r, k, 5);
                    d.insert(pos as u64, &data).unwrap();
                    m.splice(pos..pos, data.iter().copied());
                } else {
                    let pos = r.below(m.len() as u64) as usize;
                    let len = r.below((m.len() - pos) as u64 + 1).min(r.below(60) + 1) as usize;
                    d.delete(pos as u64, len as u64).unwrap();
                    m.drain(pos..pos + len);
                }
            }
            for max in [1u64, 2, 7, 33, 1000, DEFAULT_MAX_SCAN] {
                for pos in 0..=(m.len() as u64 + 1) {
                    check_pos(&d, &m, pos, max, &format!("seed {seed} passo {step}"));
                }
            }
        }
    }
}

#[test]
fn big_document_crosses_the_internal_read_chunks() {
    // 600 KB con righe lunghe (un \n ogni ~20000 byte): le scansioni attraversano i blocchi di lettura interni
    let dir = TestDir::new();
    let mut r = Lcg(99);
    let mut m = texty(&mut r, 600_000, 20_000);
    let mut d = dir.open(&m);
    // e un po' di pezzi: inserimenti a caso
    for _ in 0..30 {
        let pos = r.below(m.len() as u64 + 1) as usize;
        let k = r.below(300) as usize;
        let data = texty(&mut r, k, 50);
        d.insert(pos as u64, &data).unwrap();
        m.splice(pos..pos, data.iter().copied());
    }
    for max in [100u64, 5_000, 70_000, 200_000, DEFAULT_MAX_SCAN] {
        for _ in 0..400 {
            let pos = r.below(m.len() as u64 + 1);
            check_pos(&d, &m, pos, max, "grande");
        }
        // gli estremi
        for pos in [0u64, 1, m.len() as u64 - 1, m.len() as u64] {
            check_pos(&d, &m, pos, max, "grande, estremi");
        }
    }
}

#[test]
fn walking_the_whole_document_row_by_row_visits_every_line_start() {
    // la camminata che fa il TUI: dalla cima, con next_row; e poi all'indietro con prev_row_start: stessi punti
    let dir = TestDir::new();
    let mut r = Lcg(7);
    let m = texty(&mut r, 5000, 12);
    let d = dir.open(&m);
    let mut starts = vec![0u64];
    for (i, &b) in m.iter().enumerate() {
        if b == b'\n' && i + 1 < m.len() {
            starts.push(i as u64 + 1);
        }
    }
    let mut avanti = vec![0u64];
    let mut p = 0u64;
    loop {
        match next_row(&d, p, DEFAULT_MAX_SCAN) {
            Next::Line(q) => {
                avanti.push(q);
                p = q;
            }
            Next::Cut(_) => panic!("nessun taglio con righe corte"),
            Next::End => break,
        }
    }
    assert_eq!(avanti, starts);
    let mut indietro = vec![p];
    while let Some(q) = prev_row_start(&d, p, DEFAULT_MAX_SCAN) {
        indietro.push(q);
        p = q;
    }
    indietro.reverse();
    assert_eq!(indietro, starts);
}

#[test]
fn cursor_end_stops_before_the_cr_of_a_crlf_pair() {
    let dir = TestDir::new();
    // UTF-8: `ab\r\ncd\r\nef` (l'ultima riga non ha a-capo)
    let d = dir.open(b"ab\r\ncd\r\nef");
    assert!(crlf_at(&d, 2) && crlf_at(&d, 6));
    assert!(!crlf_at(&d, 1) && !crlf_at(&d, 3) && !crlf_at(&d, 8));
    assert_eq!(row_end(&d, 0, DEFAULT_MAX_SCAN), 3);
    assert_eq!(cursor_end(&d, 0, DEFAULT_MAX_SCAN), 2);
    assert_eq!(cursor_end(&d, 4, DEFAULT_MAX_SCAN), 6);
    assert_eq!(cursor_end(&d, 8, DEFAULT_MAX_SCAN), 10);
    // una riga vuota (`\r\n` all'inizio) ha la fine al suo inizio, non prima
    let v = dir.open(b"\r\nab");
    assert_eq!(cursor_end(&v, 0, DEFAULT_MAX_SCAN), 0);
    // un `\r` solo, o un `\n` solo, non sono una coppia
    let l = dir.open(b"a\rb\nc\n");
    assert!(!crlf_at(&l, 1));
    assert_eq!(
        cursor_end(&l, 0, DEFAULT_MAX_SCAN),
        row_end(&l, 0, DEFAULT_MAX_SCAN)
    );
    assert_eq!(cursor_end(&l, 4, DEFAULT_MAX_SCAN), 5);
    // 16 bit little endian: `a \r \n b` e la stessa cosa a unita' di 2 byte
    let mut u = dir.open(&[b'a', 0, 0x0D, 0, 0x0A, 0, b'b', 0]);
    assert!(u.set_newline(Newline::Le16));
    assert!(crlf_at(&u, 2));
    assert!(!crlf_at(&u, 4));
    assert_eq!(row_end(&u, 0, DEFAULT_MAX_SCAN), 4);
    assert_eq!(cursor_end(&u, 0, DEFAULT_MAX_SCAN), 2);
}
