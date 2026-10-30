//! L'indice delle righe costruito con LETTURE ESPLICITE a segmenti di `READ_BYTES` (E21) invece che passando dalla
//! mappatura: stessi risultati di sempre, anche con gli a-capo esattamente sui confini dei segmenti e dei blocchi.
//! Oracolo ingenuo scritto qui, attesi mai calcolati dal codice.

use mrp_core::lines::READ_BYTES;
use mrp_core::{BuildOptions, FileSource, LineIndex};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

struct TempFile(PathBuf);

impl TempFile {
    fn new(data: &[u8]) -> TempFile {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let p =
            std::env::temp_dir().join(format!("mrp_idxread_test_{}_{}.txt", std::process::id(), n));
        std::fs::write(&p, data).unwrap();
        TempFile(p)
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
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
}

/// Gli inizi di riga: 0 e il byte dopo ogni `\n` che non e' l'ultimo del file.
fn starts(data: &[u8]) -> Vec<u64> {
    let mut v = Vec::new();
    if data.is_empty() {
        return v;
    }
    v.push(0);
    for (i, &b) in data.iter().enumerate() {
        if b == b'\n' && i + 1 < data.len() {
            v.push((i + 1) as u64);
        }
    }
    v
}

/// Quanti punti memorizza l'indice: per ogni blocco `[k*chunk, (k+1)*chunk)` gli inizi di riga che vi cadono,
/// numerati da 0 dentro il blocco; si tiene quello il cui numero locale e' multiplo di `every`.
fn expected_checkpoints(data: &[u8], chunk: usize, every: u64) -> usize {
    let st = starts(data);
    let mut tot = 0usize;
    let mut blocco_corrente = usize::MAX;
    let mut locale = 0u64;
    for &p in &st {
        let b = p as usize / chunk;
        if b != blocco_corrente {
            blocco_corrente = b;
            locale = 0;
        }
        if locale.is_multiple_of(every) {
            tot += 1;
        }
        locale += 1;
    }
    tot
}

fn check(data: &[u8], chunk: usize, every: u32) {
    let f = TempFile::new(data);
    let src = FileSource::open(&f.0).unwrap();
    let idx = LineIndex::build_with(
        &src,
        &BuildOptions {
            chunk_bytes: chunk,
            checkpoint_every: every,
        },
        &AtomicBool::new(false),
        &AtomicU64::new(0),
    )
    .unwrap();
    let st = starts(data);
    let ctx = format!("chunk={chunk} every={every} len={}", data.len());
    assert_eq!(idx.line_count(), st.len() as u64, "{ctx}");
    assert_eq!(
        idx.checkpoint_count(),
        expected_checkpoints(data, chunk, u64::from(every)),
        "{ctx}"
    );
    for (n, &p) in st.iter().enumerate() {
        assert_eq!(
            idx.offset_of_line(&src, n as u64),
            Some(p),
            "riga {n}; {ctx}"
        );
    }
    assert_eq!(idx.offset_of_line(&src, st.len() as u64), None, "{ctx}");
    // quanti `\n` prima di alcuni offset, fra cui quelli sui confini dei segmenti
    let mut offs: Vec<u64> = vec![0, data.len() as u64];
    for k in 1..=4u64 {
        for d in [-1i64, 0, 1] {
            let o = (k * READ_BYTES as u64) as i64 + d;
            if o >= 0 && (o as u64) <= data.len() as u64 {
                offs.push(o as u64);
            }
        }
    }
    for o in offs {
        let atteso = data[..o as usize].iter().filter(|&&b| b == b'\n').count() as u64;
        assert_eq!(
            idx.newlines_before(&src, o),
            Some(atteso),
            "offset {o}; {ctx}"
        );
    }
}

#[test]
fn the_segment_size_is_a_sane_constant() {
    const { assert!(READ_BYTES >= 4096) };
    // i buffer dei lavoratori devono restare piccoli
    const { assert!(READ_BYTES <= 4 * 1024 * 1024) };
}

#[test]
fn newlines_exactly_on_segment_and_block_boundaries() {
    let len = 3 * READ_BYTES + 17;
    let r = READ_BYTES;
    let casi: Vec<Vec<usize>> = vec![
        vec![],
        vec![0],
        vec![len - 1],
        vec![r - 1],
        vec![r],
        vec![r + 1],
        vec![r - 1, r],
        vec![2 * r - 1, 2 * r],
        vec![r - 1, 2 * r - 1, 3 * r - 1],
        vec![r, 2 * r, 3 * r],
        vec![0, r - 1, r, len - 1],
    ];
    for nl in casi {
        let mut data = vec![b'x'; len];
        for &p in &nl {
            data[p] = b'\n';
        }
        for chunk in [r - 1, r, r + 3, 2 * r, 4 * r, 4 * 1024 * 1024] {
            for every in [1u32, 3, 256] {
                check(&data, chunk, every);
            }
        }
    }
}

#[test]
fn random_lines_with_many_chunk_and_segment_sizes() {
    let mut rng = Lcg(0x1234_5678_9abc_def0);
    let mut data = Vec::new();
    while data.len() < 3 * READ_BYTES + 12345 {
        let n = (rng.next() % 300) as usize;
        data.extend(std::iter::repeat_n(b'a' + (rng.next() % 26) as u8, n));
        if !rng.next().is_multiple_of(11) {
            data.push(b'\n');
        }
        if rng.next().is_multiple_of(17) {
            data.push(b'\r');
            data.push(b'\n');
        }
    }
    // senza a-capo finale e con a-capo finale
    let mut senza = data.clone();
    while senza.last() == Some(&b'\n') {
        senza.pop();
    }
    let mut con = senza.clone();
    con.push(b'\n');
    for d in [&senza, &con] {
        for chunk in [
            64,
            1000,
            READ_BYTES - 1,
            READ_BYTES,
            3 * READ_BYTES + 5,
            4 * 1024 * 1024,
        ] {
            for every in [1u32, 7, 256] {
                check(d, chunk, every);
            }
        }
    }
}

#[test]
fn tiny_and_empty_files() {
    for data in [&b""[..], b"\n", b"a", b"a\n", b"\n\n", b"a\nb", b"\n\n\n\n"] {
        for chunk in [1, 2, 3, READ_BYTES, 4 * 1024 * 1024] {
            for every in [1u32, 2, 256] {
                check(data, chunk, every);
            }
        }
    }
}

#[test]
fn building_the_index_does_not_touch_the_mapping() {
    // E' lo scopo di E21: leggere con `read_at` in buffer piccoli, non passare dalle pagine mappate (il
    // working set del processo non deve crescere con la dimensione del file).
    let mut data = Vec::new();
    let mut rng = Lcg(77);
    while data.len() < 3 * READ_BYTES + 1000 {
        let n = (rng.next() % 200) as usize;
        data.extend(std::iter::repeat_n(b'q', n));
        data.push(b'\n');
    }
    for chunk in [64usize, READ_BYTES, 4 * 1024 * 1024] {
        let f = TempFile::new(&data);
        let src = FileSource::open(&f.0).unwrap();
        assert_eq!(src.map_accesses(), 0);
        let idx = LineIndex::build_with(
            &src,
            &BuildOptions {
                chunk_bytes: chunk,
                checkpoint_every: 8,
            },
            &AtomicBool::new(false),
            &AtomicU64::new(0),
        )
        .unwrap();
        assert_eq!(
            src.map_accesses(),
            0,
            "chunk {chunk}: la costruzione ha toccato la mappatura"
        );
        assert_eq!(idx.line_count(), starts(&data).len() as u64);
    }
}

#[test]
fn building_the_index_of_a_file_that_uses_the_prefetch_path_does_not_touch_the_mapping() {
    // oltre 64 MiB parte anche il thread di precaricamento
    let mut data = Vec::with_capacity(66 << 20);
    let riga = [b'z'; 99];
    while data.len() < (66 << 20) {
        data.extend_from_slice(&riga);
        data.push(b'\n');
    }
    let f = TempFile::new(&data);
    let src = FileSource::open(&f.0).unwrap();
    let idx = LineIndex::build(&src, &AtomicBool::new(false), &AtomicU64::new(0)).unwrap();
    assert_eq!(src.map_accesses(), 0);
    assert_eq!(idx.line_count(), (data.len() / 100) as u64);
    assert_eq!(idx.offset_of_line(&src, 123_456), Some(12_345_600));
}
