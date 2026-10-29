//! Test di annulla/ripeti. Scritti da ARCH prima dell'implementazione: gli attesi vengono da un oracolo
//! ingenuo (copie intere del contenuto, su dati piccoli), mai dal codice.

use mrp_core::Document;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

// ---------- attrezzi ----------

static COUNTER: AtomicUsize = AtomicUsize::new(0);

struct TestDir(PathBuf);

impl TestDir {
    fn new() -> TestDir {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let p = std::env::temp_dir().join(format!("mrp_undo_test_{}_{}", std::process::id(), n));
        std::fs::create_dir_all(&p).unwrap();
        TestDir(p)
    }
    fn file(&self, nome: &str, data: &[u8]) -> PathBuf {
        let p = self.0.join(nome);
        std::fs::write(&p, data).unwrap();
        p
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn open(dir: &TestDir, data: &[u8]) -> Document {
    Document::open(&dir.file("a.txt", data)).unwrap()
}

fn whole(d: &Document) -> Vec<u8> {
    d.read_range(0, d.len())
}

fn flags() -> (AtomicBool, AtomicU64) {
    (AtomicBool::new(false), AtomicU64::new(0))
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

fn texty(r: &mut Lcg, n: usize) -> Vec<u8> {
    (0..n)
        .map(|_| match r.below(6) {
            0 => b'\n',
            1 => 0xff,
            2 | 3 => b'a',
            _ => b'b',
        })
        .collect()
}

// ---------- a mano ----------

#[test]
fn nothing_to_undo_or_redo_on_a_fresh_document() {
    let dir = TestDir::new();
    let mut d = open(&dir, b"hello");
    assert!(!d.can_undo());
    assert!(!d.can_redo());
    assert_eq!(d.undo(), None);
    assert_eq!(d.redo(), None);
    assert_eq!(whole(&d), b"hello");
    assert!(!d.is_modified());
}

#[test]
fn undo_and_redo_one_insert() {
    let dir = TestDir::new();
    let mut d = open(&dir, b"0123456789");
    d.insert(5, b"abc").unwrap();
    assert_eq!(whole(&d), b"01234abc56789");
    assert!(d.can_undo());
    assert!(!d.can_redo());
    assert_eq!(d.undo(), Some(5));
    assert_eq!(whole(&d), b"0123456789");
    assert!(!d.can_undo());
    assert!(d.can_redo());
    assert_eq!(d.redo(), Some(5));
    assert_eq!(whole(&d), b"01234abc56789");
    assert!(d.can_undo());
    assert!(!d.can_redo());
}

#[test]
fn undo_and_redo_one_delete_returns_its_position() {
    let dir = TestDir::new();
    let mut d = open(&dir, b"0123456789");
    d.delete(3, 4).unwrap();
    assert_eq!(whole(&d), b"012789");
    assert_eq!(d.undo(), Some(3));
    assert_eq!(whole(&d), b"0123456789");
    assert_eq!(d.redo(), Some(3));
    assert_eq!(whole(&d), b"012789");
}

#[test]
fn edits_without_a_seal_form_one_step() {
    let dir = TestDir::new();
    let mut d = open(&dir, b"0123456789");
    d.insert(10, b"x").unwrap(); // il primo della serie: la posizione che si restituisce
    d.insert(2, b"y").unwrap();
    d.delete(0, 1).unwrap();
    let dopo = whole(&d);
    assert_eq!(
        d.undo(),
        Some(10),
        "una sola annullata per tutta la serie; posizione = quella della prima modifica"
    );
    assert_eq!(whole(&d), b"0123456789");
    assert!(!d.can_undo());
    assert_eq!(d.redo(), Some(10));
    assert_eq!(whole(&d), dopo);
}

#[test]
fn seal_separates_the_steps() {
    let dir = TestDir::new();
    let mut d = open(&dir, b"abc");
    d.insert(3, b"1").unwrap();
    d.seal();
    d.insert(4, b"2").unwrap();
    d.seal();
    d.seal(); // ripetuto o a vuoto: nessun effetto
    d.insert(5, b"3").unwrap();
    assert_eq!(whole(&d), b"abc123");
    assert_eq!(d.undo(), Some(5)); // chiude da solo il passo aperto
    assert_eq!(whole(&d), b"abc12");
    assert_eq!(d.undo(), Some(4));
    assert_eq!(whole(&d), b"abc1");
    assert_eq!(d.undo(), Some(3));
    assert_eq!(whole(&d), b"abc");
    assert_eq!(d.undo(), None);
    assert_eq!(d.redo(), Some(3));
    assert_eq!(d.redo(), Some(4));
    assert_eq!(whole(&d), b"abc12");
    assert_eq!(d.redo(), Some(5));
    assert_eq!(d.redo(), None);
    assert_eq!(whole(&d), b"abc123");
}

#[test]
fn a_new_edit_after_undo_discards_the_redo_steps() {
    let dir = TestDir::new();
    let mut d = open(&dir, b"abc");
    d.insert(3, b"1").unwrap();
    d.seal();
    d.insert(4, b"2").unwrap();
    assert_eq!(d.undo(), Some(4));
    assert!(d.can_redo());
    d.insert(0, b"Z").unwrap();
    assert!(!d.can_redo(), "una modifica nuova butta via il ripeti");
    assert_eq!(whole(&d), b"Zabc1");
    assert_eq!(d.redo(), None);
}

#[test]
fn edits_that_change_nothing_or_fail_leave_no_step() {
    let dir = TestDir::new();
    let mut d = open(&dir, b"abc");
    d.insert(1, b"").unwrap();
    d.delete(1, 0).unwrap();
    assert!(d.insert(9, b"x").is_err());
    assert!(d.delete(2, 5).is_err());
    assert!(!d.can_undo());
    d.insert(1, b"X").unwrap();
    d.seal();
    d.insert(1, b"").unwrap(); // non apre un passo vuoto
    d.seal();
    assert_eq!(d.undo(), Some(1));
    assert_eq!(whole(&d), b"abc");
    assert!(!d.can_undo());
}

// ---------- modificato / non modificato ----------

#[test]
fn undoing_back_to_the_saved_state_is_not_modified() {
    let dir = TestDir::new();
    let mut d = open(&dir, b"abc");
    assert!(!d.is_modified());
    d.insert(3, b"1").unwrap();
    assert!(d.is_modified());
    d.undo();
    assert!(
        !d.is_modified(),
        "tornati allo stato del file: non modificato"
    );
    d.redo();
    assert!(d.is_modified());
    d.undo();
    d.insert(0, b"Q").unwrap();
    assert!(d.is_modified());
    d.undo();
    assert!(!d.is_modified());
}

#[test]
fn save_clears_the_history_and_the_document_is_clean() {
    let dir = TestDir::new();
    let mut d = open(&dir, b"abc");
    d.insert(3, b"1").unwrap();
    d.seal();
    d.insert(4, b"2").unwrap();
    d.undo();
    assert!(d.can_redo());
    d.insert(3, b"9").unwrap();
    let (c, p) = flags();
    d.save(&c, &p).unwrap();
    assert!(!d.is_modified());
    assert!(
        !d.can_undo(),
        "dopo il salvataggio la storia non vale piu' (i pezzi puntavano al vecchio file)"
    );
    assert!(!d.can_redo());
    assert_eq!(d.undo(), None);
    assert_eq!(whole(&d), b"abc91");
    // e si puo' ricominciare
    d.insert(0, b">").unwrap();
    assert!(d.is_modified());
    assert_eq!(d.undo(), Some(0));
    assert_eq!(whole(&d), b"abc91");
    assert!(!d.is_modified());
}

#[test]
fn a_failed_save_keeps_the_history() {
    let dir = TestDir::new();
    let mut d = open(&dir, b"abc");
    d.insert(3, b"1").unwrap();
    let (_, p) = flags();
    let c = AtomicBool::new(true); // annullato in partenza
    assert!(d.save(&c, &p).is_err());
    assert!(d.can_undo());
    assert!(d.is_modified());
    assert_eq!(d.undo(), Some(3));
    assert_eq!(whole(&d), b"abc");
}

#[test]
fn undo_after_edit_undo_edit_diverged_from_the_saved_state_stays_modified() {
    // la storia non contiene piu' lo stato "uguale al file": per annullare quanto basta non si torna a non-modificato
    // a meno che il contenuto sia davvero quello del file (qui lo e': si torna in fondo alla pila)
    let dir = TestDir::new();
    let mut d = open(&dir, b"abc");
    d.insert(3, b"1").unwrap();
    d.seal();
    d.insert(4, b"2").unwrap();
    d.seal();
    d.undo(); // abc1
    d.undo(); // abc
    assert!(!d.is_modified());
    d.redo(); // abc1
    assert!(d.is_modified());
    d.insert(0, b"Z").unwrap(); // butta via il ripeti: da qui il passo "abc2" non esiste piu'
    d.seal();
    d.undo(); // abc1
    assert!(d.is_modified());
    d.undo(); // abc
    assert!(!d.is_modified());
}

// ---------- la memoria segue le modifiche, non i dati ----------

#[test]
fn undoing_a_huge_delete_does_not_copy_the_data() {
    let dir = TestDir::new();
    let mut r = Lcg(5);
    let orig: Vec<u8> = (0..20 * 1024 * 1024)
        .map(|_| (r.next() & 0xff) as u8)
        .collect();
    let mut d = open(&dir, &orig);
    d.delete(100, 19 * 1024 * 1024).unwrap();
    assert_eq!(d.table().add_len(), 0, "cancellare non copia nulla");
    assert_eq!(d.undo(), Some(100));
    assert_eq!(
        d.table().add_len(),
        0,
        "annullare non copia i dati cancellati"
    );
    assert_eq!(d.len(), orig.len() as u64);
    assert_eq!(whole(&d), orig);
    assert_eq!(d.redo(), Some(100));
    assert_eq!(d.len(), orig.len() as u64 - 19 * 1024 * 1024);
    assert_eq!(d.table().pieces().len(), 2);
}

// ---------- casuale contro l'oracolo ----------

/// Oracolo: stato corrente + pile di copie intere. Ogni stato ha un identificativo: lo stato del file ha id 0,
/// ogni modifica che cambia qualcosa ne crea uno nuovo; "modificato" vuol dire "id diverso da 0" (come fa la
/// posizione nella storia, senza confrontare i contenuti).
struct Model {
    cur: Vec<u8>,
    id: u64,
    next_id: u64,
    undo: Vec<(Vec<u8>, u64)>,
    redo: Vec<(Vec<u8>, u64)>,
    open_start: Option<(Vec<u8>, u64)>,
}

impl Model {
    fn edit(&mut self, new: Vec<u8>) {
        if new == self.cur {
            return; // inserimento o cancellazione vuoti: il documento non apre un passo
        }
        if self.open_start.is_none() {
            self.open_start = Some((self.cur.clone(), self.id));
        }
        self.redo.clear();
        self.cur = new;
        self.id = self.next_id;
        self.next_id += 1;
    }
    fn seal(&mut self) {
        if let Some(s) = self.open_start.take() {
            self.undo.push(s);
        }
    }
    fn undo(&mut self) -> bool {
        self.seal();
        match self.undo.pop() {
            Some((prev, pid)) => {
                let cur = std::mem::replace(&mut self.cur, prev);
                let id = std::mem::replace(&mut self.id, pid);
                self.redo.push((cur, id));
                true
            }
            None => false,
        }
    }
    fn redo(&mut self) -> bool {
        self.seal();
        match self.redo.pop() {
            Some((next, nid)) => {
                let cur = std::mem::replace(&mut self.cur, next);
                let id = std::mem::replace(&mut self.id, nid);
                self.undo.push((cur, id));
                true
            }
            None => false,
        }
    }
}

#[test]
fn random_history_agrees_with_the_oracle() {
    let dir = TestDir::new();
    for seed in 1..=6u64 {
        let mut r = Lcg(seed * 7919);
        let n = r.below(300) as usize;
        let orig = texty(&mut r, n);
        let mut d = open(&dir, &orig);
        let mut m = Model {
            cur: orig.clone(),
            id: 0,
            next_id: 1,
            undo: Vec::new(),
            redo: Vec::new(),
            open_start: None,
        };
        for step in 0..300 {
            let what = format!("seed {seed} passo {step}");
            match r.below(10) {
                0..=3 => {
                    let pos = r.below(m.cur.len() as u64 + 1) as usize;
                    let k = r.below(8) as usize; // anche 0: inserimento vuoto
                    let data = texty(&mut r, k);
                    d.insert(pos as u64, &data).unwrap();
                    let mut nuovo = m.cur.clone();
                    nuovo.splice(pos..pos, data.iter().copied());
                    m.edit(nuovo);
                }
                4..=5 if !m.cur.is_empty() => {
                    let pos = r.below(m.cur.len() as u64) as usize;
                    let len = r.below((m.cur.len() - pos) as u64 + 1).min(r.below(20)) as usize;
                    d.delete(pos as u64, len as u64).unwrap();
                    let mut nuovo = m.cur.clone();
                    nuovo.drain(pos..pos + len);
                    m.edit(nuovo);
                }
                6 => {
                    d.seal();
                    m.seal();
                }
                7..=8 => {
                    let atteso = m.undo();
                    assert_eq!(d.undo().is_some(), atteso, "{what}: undo");
                }
                _ => {
                    let atteso = m.redo();
                    assert_eq!(d.redo().is_some(), atteso, "{what}: redo");
                }
            }
            assert_eq!(whole(&d), m.cur, "{what}: contenuto");
            assert_eq!(d.len(), m.cur.len() as u64, "{what}: len");
            assert_eq!(
                d.can_undo(),
                !m.undo.is_empty() || m.open_start.is_some(),
                "{what}: can_undo"
            );
            assert_eq!(d.can_redo(), !m.redo.is_empty(), "{what}: can_redo");
            assert_eq!(d.is_modified(), m.id != 0, "{what}: is_modified");
        }
        // alla fine si annulla tutto: si deve tornare al file
        while d.can_undo() {
            d.undo();
        }
        assert_eq!(whole(&d), orig, "seed {seed}: tutto annullato");
        assert!(!d.is_modified());
    }
}
