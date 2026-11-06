//! Il generatore di log sintetici di Manto.RustPad.
//!
//! Scrive un file di log finto, della dimensione che si vuole (da pochi KB a centinaia di GB), a flusso: la memoria usata e' di
//! qualche MB qualunque sia la dimensione. A parita' di `Config` (scenario, dimensione, seme, data di partenza, ritmo) i byte
//! sono SEMPRE gli stessi, su ogni macchina: nessun orologio, nessuna libreria di date o di numeri casuali, solo questo file.
//!
//! Gli scenari sono i casi di prova: ognuno ha un formato e una difficolta' diversi (vedi `Scenario`). Gli orari avanzano come
//! in un servizio vero: il ritmo cambia fra notte e giorno e fra feriali e festivi, e ogni tanto c'e' una raffica di errori
//! (visibile nella timeline).

use std::io::{self, Write};

// ---------- il generatore di numeri casuali ----------

/// SplitMix64: piccolo, veloce e uguale ovunque.
#[derive(Clone)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng(seed)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Un numero in `0..n` (`n` > 0).
    pub fn below(&mut self, n: u64) -> u64 {
        ((u128::from(self.next_u64()) * u128::from(n)) >> 64) as u64
    }

    /// Un numero in `[0, 1)`.
    pub fn unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    pub fn chance(&mut self, p: f64) -> bool {
        self.unit() < p
    }

    pub fn pick<'a, T>(&mut self, s: &'a [T]) -> &'a T {
        &s[self.below(s.len() as u64) as usize]
    }
}

// ---------- le date ----------

/// I giorni dal 1970-01-01 della data `anno-mese-giorno` (calendario gregoriano).
pub fn days_from_civil(anno: i64, mese: u32, giorno: u32) -> i64 {
    let y = if mese <= 2 { anno - 1 } else { anno };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (i64::from(mese) + 9) % 12;
    let doy = (153 * mp + 2) / 5 + i64::from(giorno) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// La data (anno, mese, giorno) di `z` giorni dal 1970-01-01.
pub fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (y + i64::from(m <= 2), m as u32, d as u32)
}

/// Legge `AAAA-MM-GG` (anni 1970..=2999): i giorni dal 1970-01-01.
pub fn parse_day(s: &str) -> Option<i64> {
    let mut it = s.split('-');
    let (a, m, g) = (it.next()?, it.next()?, it.next()?);
    if it.next().is_some() || a.len() != 4 || m.len() != 2 || g.len() != 2 {
        return None;
    }
    let (a, m, g): (i64, u32, u32) = (a.parse().ok()?, m.parse().ok()?, g.parse().ok()?);
    if !(1970..=2999).contains(&a) || !(1..=12).contains(&m) || g == 0 || g > days_in_month(a, m) {
        return None;
    }
    Some(days_from_civil(a, m, g))
}

fn days_in_month(anno: i64, mese: u32) -> u32 {
    match mese {
        2 if (anno % 4 == 0 && anno % 100 != 0) || anno % 400 == 0 => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

const MESI: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

// ---------- gli scenari ----------

/// I casi di prova.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scenario {
    Service,
    Bursts,
    Syslog,
    Access,
    Json,
    Epoch,
    Unordered,
    LongLines,
    Crlf,
    Unicode,
    Report,
    Text,
}

impl Scenario {
    pub const ALL: [Scenario; 12] = [
        Scenario::Service,
        Scenario::Bursts,
        Scenario::Syslog,
        Scenario::Access,
        Scenario::Json,
        Scenario::Epoch,
        Scenario::Unordered,
        Scenario::LongLines,
        Scenario::Crlf,
        Scenario::Unicode,
        Scenario::Report,
        Scenario::Text,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Scenario::Service => "service",
            Scenario::Bursts => "bursts",
            Scenario::Syslog => "syslog",
            Scenario::Access => "access",
            Scenario::Json => "json",
            Scenario::Epoch => "epoch",
            Scenario::Unordered => "unordered",
            Scenario::LongLines => "longlines",
            Scenario::Crlf => "crlf",
            Scenario::Unicode => "unicode",
            Scenario::Report => "report",
            Scenario::Text => "text",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Scenario::Service => "application log: `2026-11-04 10:17:11.756 +01:00 [INF] ...`, five levels, day/night rhythm, now and then an error storm, stack traces, a few very rare messages",
            Scenario::Bursts => "like `service` but quiet most of the time with a few long, violent error storms (the timeline shows them at a glance)",
            Scenario::Syslog => "syslog lines `Nov  4 10:17:11 host-07 sshd[1234]: ...` (no year, no levels), several hosts and programs",
            Scenario::Access => "web server access log `10.1.2.3 - - [04/Nov/2026:10:17:11 +0100] \"GET /x HTTP/1.1\" 200 512 ...`, no levels, 5xx storms",
            Scenario::Json => "one JSON object per line: `{\"ts\":\"2026-11-04T10:17:11.756Z\",\"level\":\"INFO\",...}`",
            Scenario::Epoch => "lines that start with a Unix time in milliseconds: `1793498400877 INFO ...`",
            Scenario::Unordered => "like `service` but 3% of the lines carry a time up to ten minutes in the past (late arrivals): times are not sorted",
            Scenario::LongLines => "like `service` but 1.5% of the lines carry a payload from 2 KB up to 200 KB on the same line",
            Scenario::Crlf => "like `service` with Windows line endings (CR LF)",
            Scenario::Unicode => "like `service` with messages and names in Italian, German, Russian, Japanese, Chinese, with accents and emoji",
            Scenario::Report => "log lines mixed with ASCII tables (`|` and `---`), like a backtest report: many lines without a time",
            Scenario::Text => "plain prose, no dates, no levels: the timeline must say there are no times",
        }
    }

    pub fn parse(s: &str) -> Option<Scenario> {
        Scenario::ALL.into_iter().find(|c| c.name() == s)
    }
}

/// Quello che serve per generare un file.
#[derive(Clone, Debug)]
pub struct Config {
    pub scenario: Scenario,
    /// La dimensione a cui ci si ferma: si chiude l'ultima voce (mai una riga a meta'), quindi il file e' di poco piu' grande.
    pub bytes: u64,
    pub seed: u64,
    /// Il giorno di partenza, in giorni dal 1970-01-01 (vedi `parse_day`).
    pub start_day: i64,
    /// Le righe al secondo in media (il ritmo vero cambia con l'ora e il giorno).
    pub rate: f64,
}

impl Config {
    /// Seme 1, partenza il 2026-01-01, 20 voci al secondo.
    pub fn new(scenario: Scenario, bytes: u64) -> Config {
        Config {
            scenario,
            bytes,
            seed: 1,
            start_day: days_from_civil(2026, 1, 1),
            rate: 20.0,
        }
    }
}

/// Cosa e' stato scritto.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    pub bytes: u64,
    pub lines: u64,
    pub entries: u64,
    pub first_ms: i64,
    pub last_ms: i64,
}

// ---------- i testi ----------

// livelli: 0 DBG, 1 INF, 2 WRN, 3 ERR, 4 FATAL
const DBG: &[&str] = &[
    "Cache hit for key %h (%m us)",
    "Cache miss for key %h, loading from %t",
    "Query on %t returned %i rows in %m ms",
    "Pool %s: %i active, %i idle connections",
    "Heartbeat from node %p seq=%I",
    "Token for %u refreshed, valid for %i s",
];
const INF: &[&str] = &[
    "Request GET /api/%t/%i completed in %m ms (status 200)",
    "Request POST /api/%t completed in %m ms (status 201)",
    "User %u logged in from %p",
    "User %u logged out",
    "Order %o created for %u, total %q EUR",
    "Order %o shipped via %s",
    "Payment %h accepted: %q EUR",
    "Scheduled job %s finished in %m ms, processed %i items",
    "Connection from %p accepted",
    "Config reloaded from %f",
    "Session %h opened for %u",
    "Session %h closed after %i s",
    "Backup %f completed: %I bytes",
];
const WRN: &[&str] = &[
    "Slow query on %t: %m ms (threshold 500 ms)",
    "Retry %i/5 for %s after timeout",
    "Disk usage on %f at %i%%",
    "Rate limit reached for %p, delaying %m ms",
    "Deprecated API /v1/%t called by %u",
    "Queue %s length %I above soft limit",
];
const ERR: &[&str] = &[
    "Connection to %p timed out after %m ms",
    "Connection to %p refused",
    "Failed to process order %o: %s unavailable",
    "Payment %h declined for %u: insufficient funds",
    "Deadlock detected on %t, transaction %h rolled back",
    "Cannot write %f: no space left on device",
];
/// Gli errori con una pila di chiamate sotto.
const ERR_TRACE: &[&str] = &[
    "Unhandled exception in %s: System.NullReferenceException: Object reference not set to an instance of an object",
    "Unhandled exception in %s: System.InvalidOperationException: Sequence contains no elements",
    "Unhandled exception in %s: System.TimeoutException: The operation has timed out",
];
const FATAL: &[&str] = &[
    "Out of memory in %s, shutting down",
    "Database %p unreachable, giving up after %i attempts",
];
/// I messaggi rarissimi: (livello, testo). Compaiono 1 volta su 2000: sono "l'ago" da trovare.
const RARI: &[(u8, &str)] = &[
    (3, "Checksum mismatch in block %I of %f"),
    (3, "TLS handshake failed with %p: certificate has expired"),
    (3, "Replica %p diverged at LSN %h"),
    (2, "Clock skew of %m ms detected against %p"),
    (2, "Unusual login for %u from %p (country change)"),
    (1, "Maintenance window started by %u"),
    (4, "Data corruption detected in %t"),
];
const STACK: &[&str] = &[
    "   at App.Services.OrderService.Process(Order order) in OrderService.cs:line %i",
    "   at App.Services.PaymentService.Charge(Payment p, CancellationToken ct) in PaymentService.cs:line %i",
    "   at App.Data.Repository`1.Find(Int32 id) in Repository.cs:line %i",
    "   at System.Threading.Tasks.Task.Execute()",
    "   at App.Web.Controllers.ApiController.Post(Request r) in ApiController.cs:line %i",
    "   at Microsoft.AspNetCore.Mvc.Infrastructure.ActionInvoker.InvokeAsync()",
    "   at System.Runtime.CompilerServices.TaskAwaiter.ThrowForNonSuccess(Task task)",
];
/// I messaggi internazionali dello scenario `unicode`: (livello, testo).
const UNICODE: &[(u8, &str)] = &[
    (1, "Utente %u ha effettuato l'accesso da %p"),
    (1, "ユーザー %u がログインしました (%p)"),
    (1, "用户 %u 已登录 (%p)"),
    (1, "Пользователь %u вошёл в систему с адреса %p"),
    (1, "Benutzer %u hat sich angemeldet – Größe %i MB"),
    (1, "Café order %o: total %q € ☕"),
    (1, "🚀 Deploy %h finished in %m ms"),
    (2, "Verbindung zu %p ist langsam: %m ms"),
    (2, "Pagamento %h in ritardo: è già passato più di un'ora"),
    (3, "接続エラー: %p がタイムアウトしました (%m ms)"),
    (3, "Ошибка: соединение с %p разорвано"),
    (
        3,
        "Fehler: Verbindung zu %p unterbrochen – Wiederholung %i/5",
    ),
];
const UTENTI: &[&str] = &[
    "alice", "bob", "carol", "dave", "erin", "frank", "grace", "heidi", "ivan", "judy", "mallory",
    "oscar", "peggy", "trent", "victor", "walter",
];
const UTENTI_UNICODE: &[&str] = &[
    "alice",
    "José",
    "Zoë",
    "李雷",
    "田中",
    "Müller",
    "Ørsted",
    "Никита",
    "Åsa",
    "François",
];
const TABELLE: &[&str] = &[
    "orders",
    "customers",
    "invoices",
    "products",
    "sessions",
    "payments",
    "shipments",
    "audit_log",
    "inventory",
    "tokens",
];
const SERVIZI: &[&str] = &[
    "orders",
    "billing",
    "auth",
    "gateway",
    "inventory",
    "mailer",
    "scheduler",
    "search",
    "reports",
    "payments",
];
const FILE: &[&str] = &[
    "/var/lib/app/data.db",
    "/var/log/app/app.log",
    "/etc/app/config.json",
    "/srv/backup/db.tar.gz",
    "/tmp/export.csv",
    "/data/shared/reports.xlsx",
];
const PAROLE: &[&str] = &[
    "the", "river", "morning", "quiet", "engine", "window", "paper", "garden", "slowly", "after",
    "before", "machine", "letter", "winter", "bridge", "number", "silver", "answer", "market",
    "small", "large", "story", "evening", "forest", "memory", "kitchen", "pattern", "signal",
    "travel", "simple", "reason", "gentle", "shadow", "planet", "harbor", "orange", "thread",
    "mirror", "candle", "ladder", "island", "season", "yellow", "whisper", "journey", "carpet",
    "bottle", "castle", "meadow", "pencil", "rocket", "violin", "anchor", "basket", "cactus",
    "dragon", "falcon", "helmet", "jungle",
];
const AGENTI: &[&str] = &[
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/118.0 Safari/537.36",
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 13_5) AppleWebKit/605.1.15 Version/16.5 Safari/605.1.15",
    "curl/8.4.0",
    "Mozilla/5.0 (X11; Linux x86_64; rv:109.0) Gecko/20100101 Firefox/118.0",
];

/// Le soglie (per mille) di DBG, INF, WRN, ERR (il resto e' FATAL): normale e in raffica.
const PESI_NORMALI: [u64; 4] = [60, 880, 960, 999];
const PESI_RAFFICA: [u64; 4] = [20, 270, 420, 990];

// ---------- scrittura dei numeri ----------

fn push_u(buf: &mut Vec<u8>, mut n: u64) {
    let mut t = [0u8; 20];
    let mut i = 20;
    loop {
        i -= 1;
        t[i] = b'0' + (n % 10) as u8;
        n /= 10;
        if n == 0 {
            break;
        }
    }
    buf.extend_from_slice(&t[i..]);
}

/// `n` in almeno `w` colonne (`w` <= 20), riempito a sinistra con `fill`.
fn push_pad(buf: &mut Vec<u8>, mut n: u64, w: usize, fill: u8) {
    let mut t = [fill; 20];
    let mut i = 20;
    loop {
        i -= 1;
        t[i] = b'0' + (n % 10) as u8;
        n /= 10;
        if n == 0 {
            break;
        }
    }
    let da = if 20 - i < w { 20 - w } else { i };
    buf.extend_from_slice(&t[da..]);
}

/// `2026-11-04 10:17:11.756` (con `sep` fra la data e l'ora).
fn put_data_ora(buf: &mut Vec<u8>, ms: i64, sep: u8) {
    let (g, resto) = (ms.div_euclid(GIORNO_MS), ms.rem_euclid(GIORNO_MS));
    let (a, m, d) = civil_from_days(g);
    push_pad(buf, a as u64, 4, b'0');
    buf.push(b'-');
    push_pad(buf, u64::from(m), 2, b'0');
    buf.push(b'-');
    push_pad(buf, u64::from(d), 2, b'0');
    buf.push(sep);
    put_ora(buf, resto, true);
}

/// `10:17:11.756` (o senza i millisecondi): `resto` sono i millisecondi dall'inizio del giorno.
fn put_ora(buf: &mut Vec<u8>, resto: i64, millis: bool) {
    push_pad(buf, (resto / 3_600_000) as u64, 2, b'0');
    buf.push(b':');
    push_pad(buf, (resto / 60_000 % 60) as u64, 2, b'0');
    buf.push(b':');
    push_pad(buf, (resto / 1000 % 60) as u64, 2, b'0');
    if millis {
        buf.push(b'.');
        push_pad(buf, (resto % 1000) as u64, 3, b'0');
    }
}

// ---------- il generatore ----------

struct Gen {
    cfg: Config,
    rng: Rng,
    buf: Vec<u8>,
    t_ms: i64,
    burst_fino: i64,
    lines: u64,
    entries: u64,
    eol: &'static [u8],
}

const GIORNO_MS: i64 = 86_400_000;

impl Gen {
    fn new(cfg: &Config) -> Gen {
        Gen {
            cfg: cfg.clone(),
            rng: Rng::new(cfg.seed),
            buf: Vec::with_capacity(1 << 20),
            t_ms: cfg.start_day * GIORNO_MS,
            burst_fino: i64::MIN,
            lines: 0,
            entries: 0,
            eol: if cfg.scenario == Scenario::Crlf {
                b"\r\n"
            } else {
                b"\n"
            },
        }
    }

    fn in_raffica(&self) -> bool {
        self.t_ms < self.burst_fino
    }

    /// Fa avanzare l'orologio all'orario della voce che segue.
    fn avanza(&mut self) {
        let giorni = self.t_ms.div_euclid(GIORNO_MS);
        let ora = self.t_ms.rem_euclid(GIORNO_MS) as f64 / 3_600_000.0;
        // 1970-01-01 era un giovedi': lunedi = 0
        let sett = (giorni + 3).rem_euclid(7);
        // piu' vivace di giorno (picco alle 15, minimo alle 3), piu' calmo nel fine settimana; le medie si compensano
        let giorno = 0.2 + 0.8 * (0.5 - 0.5 * ((ora - 3.0) / 24.0 * std::f64::consts::TAU).cos());
        let settimana = if sett >= 5 { 0.6 } else { 1.0 };
        let mut ritmo = self.cfg.rate * (giorno / 0.6) * (settimana / 0.886);
        let (molt, ogni_s, durata_min) = match self.cfg.scenario {
            Scenario::Bursts => (10.0, 12.0 * 3600.0, (10, 90)),
            _ => (6.0, 3.0 * 86_400.0, (2, 30)),
        };
        if self.in_raffica() {
            ritmo *= molt;
        }
        let dt_ms = (-(1.0 - self.rng.unit()).ln() / ritmo * 1000.0) as i64;
        self.t_ms += dt_ms;
        if !self.in_raffica() && self.rng.chance(dt_ms as f64 / 1000.0 / ogni_s) {
            let minuti =
                durata_min.0 + self.rng.below((durata_min.1 - durata_min.0 + 1) as u64) as i64;
            self.burst_fino = self.t_ms + minuti * 60_000;
        }
    }

    /// Il livello di una voce: 0 DBG, 1 INF, 2 WRN, 3 ERR, 4 FATAL.
    fn livello(&mut self) -> u8 {
        let pesi = if self.in_raffica() {
            &PESI_RAFFICA
        } else {
            &PESI_NORMALI
        };
        let x = self.rng.below(1000);
        pesi.iter().position(|&s| x < s).unwrap_or(4) as u8
    }

    fn fine_riga(&mut self) {
        self.buf.extend_from_slice(self.eol);
        self.lines += 1;
    }

    // ----- i pezzi di una riga -----

    fn data_ora(&mut self, ms: i64, sep: u8) {
        put_data_ora(&mut self.buf, ms, sep);
    }

    fn ora(&mut self, resto: i64, millis: bool) {
        put_ora(&mut self.buf, resto, millis);
    }

    /// Riempie un modello: `%i` intero, `%I` intero grande, `%m` millisecondi, `%p` indirizzo, `%h` esadecimale, `%u` utente,
    /// `%f` file, `%t` tabella, `%s` servizio, `%q` importo, `%d` giorni, `%o` ordine, `%%` una percentuale.
    fn riempi(&mut self, modello: &str) {
        let b = modello.as_bytes();
        let mut i = 0;
        while i < b.len() {
            if b[i] != b'%' || i + 1 >= b.len() {
                // il testo normale, a pezzi interi (anche multibyte)
                let fine = b[i..]
                    .iter()
                    .position(|&c| c == b'%')
                    .map_or(b.len(), |p| i + p.max(1));
                self.buf.extend_from_slice(&b[i..fine]);
                i = fine;
                continue;
            }
            match b[i + 1] {
                b'i' => {
                    let n = 1 + self.rng.below(99_999);
                    push_u(&mut self.buf, n);
                }
                b'I' => {
                    let n = 1 + self.rng.below(9_999_999);
                    push_u(&mut self.buf, n);
                }
                b'm' => {
                    let n = 1 + self.rng.below(4999);
                    push_u(&mut self.buf, n);
                }
                b'd' => {
                    let n = 1 + self.rng.below(90);
                    push_u(&mut self.buf, n);
                }
                b'p' => self.indirizzo(),
                b'h' => {
                    let n = self.rng.next_u64() as u32;
                    for k in (0..8).rev() {
                        let c = (n >> (k * 4)) & 0xF;
                        self.buf.push(b"0123456789abcdef"[c as usize]);
                    }
                }
                b'u' => {
                    let lista = if self.cfg.scenario == Scenario::Unicode {
                        UTENTI_UNICODE
                    } else {
                        UTENTI
                    };
                    let s = *self.rng.pick(lista);
                    self.buf.extend_from_slice(s.as_bytes());
                }
                b'f' => {
                    let s = *self.rng.pick(FILE);
                    self.buf.extend_from_slice(s.as_bytes());
                }
                b't' => {
                    let s = *self.rng.pick(TABELLE);
                    self.buf.extend_from_slice(s.as_bytes());
                }
                b's' => {
                    let s = *self.rng.pick(SERVIZI);
                    self.buf.extend_from_slice(s.as_bytes());
                }
                b'q' => {
                    let euro = 1 + self.rng.below(2999);
                    let cent = self.rng.below(100);
                    push_u(&mut self.buf, euro);
                    self.buf.push(b'.');
                    push_pad(&mut self.buf, cent, 2, b'0');
                }
                b'o' => {
                    self.buf.extend_from_slice(b"ORD-");
                    let n = 100_000 + self.rng.below(899_999);
                    push_u(&mut self.buf, n);
                }
                b'%' => self.buf.push(b'%'),
                altro => {
                    self.buf.push(b'%');
                    self.buf.push(altro);
                }
            }
            i += 2;
        }
    }

    fn indirizzo(&mut self) {
        let x = self.rng.next_u64();
        for (k, parte) in [
            10u64,
            (x >> 8) & 0xFF,
            (x >> 16) & 0xFF,
            1 + (x >> 24) % 254,
        ]
        .into_iter()
        .enumerate()
        {
            if k > 0 {
                self.buf.push(b'.');
            }
            push_u(&mut self.buf, parte);
        }
    }

    /// Il testo del messaggio di un livello (e se e' un errore con una pila sotto). Una voce su 2000 e' rarissima.
    fn messaggio(&mut self, livello: &mut u8) -> bool {
        if self.rng.chance(0.0005) {
            let (l, t) = *self.rng.pick(RARI);
            *livello = l;
            self.riempi(t);
            return false;
        }
        if self.cfg.scenario == Scenario::Unicode && self.rng.chance(0.4) {
            // solo i messaggi del livello estratto (o, se non ce n'e', quelli piu' vicini)
            let candidati: Vec<&str> = UNICODE
                .iter()
                .filter(|(l, _)| *l == (*livello).clamp(1, 3))
                .map(|(_, t)| *t)
                .collect();
            let t = *self.rng.pick(&candidati);
            self.riempi(t);
            return false;
        }
        let (lista, traccia) = match *livello {
            0 => (DBG, false),
            1 => (INF, false),
            2 => (WRN, false),
            3 if self.rng.chance(0.35) => (ERR_TRACE, true),
            3 => (ERR, false),
            _ => (FATAL, false),
        };
        let t = *self.rng.pick(lista);
        self.riempi(t);
        traccia
    }

    fn pila(&mut self) {
        let n = 3 + self.rng.below(10);
        for _ in 0..n {
            self.fine_riga();
            let t = *self.rng.pick(STACK);
            self.riempi(t);
        }
    }

    // ----- una voce per scenario -----

    fn voce(&mut self) {
        self.avanza();
        self.entries += 1;
        match self.cfg.scenario {
            Scenario::Service
            | Scenario::Bursts
            | Scenario::Crlf
            | Scenario::Unicode
            | Scenario::Unordered
            | Scenario::LongLines => self.voce_servizio(),
            Scenario::Report => {
                if self.rng.chance(1.0 / 12.0) {
                    self.rapporto();
                } else {
                    self.voce_servizio();
                }
            }
            Scenario::Syslog => self.voce_syslog(),
            Scenario::Access => self.voce_accesso(),
            Scenario::Json => self.voce_json(),
            Scenario::Epoch => self.voce_epoca(),
            Scenario::Text => self.voce_testo(),
        }
        self.fine_riga();
    }

    fn etichetta(l: u8) -> &'static [u8] {
        match l {
            0 => b"[DBG]",
            1 => b"[INF]",
            2 => b"[WRN]",
            3 => b"[ERR]",
            _ => b"[FATAL]",
        }
    }

    /// Una riga `2026-11-04 10:17:11.756 +01:00 [INF] messaggio` (senza il fine riga finale).
    fn voce_servizio(&mut self) {
        let mut l = self.livello();
        let inizio = self.buf.len();
        // prima il messaggio: se e' uno dei rarissimi cambia anche il livello, che sta davanti
        let traccia = self.messaggio(&mut l);
        let msg: Vec<u8> = self.buf.split_off(inizio);
        let mut ms = self.t_ms;
        if self.cfg.scenario == Scenario::Unordered && self.rng.chance(0.03) {
            ms -= 1000 + self.rng.below(600_000) as i64;
        }
        self.data_ora(ms, b' ');
        self.buf.extend_from_slice(b" +01:00 ");
        self.buf.extend_from_slice(Self::etichetta(l));
        self.buf.push(b' ');
        self.buf.extend_from_slice(&msg);
        if self.cfg.scenario == Scenario::LongLines && self.rng.chance(0.015) {
            self.carico();
        }
        if traccia {
            self.pila();
        }
    }

    /// Un carico lungo sulla stessa riga: da 2 KB a 200 KB (scelto su scala logaritmica).
    fn carico(&mut self) {
        let n = (2048.0 * 100f64.powf(self.rng.unit())) as usize;
        self.buf.extend_from_slice(b" payload=");
        const ALFABETO: &[u8; 64] =
            b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut resta = n;
        while resta > 0 {
            let mut x = self.rng.next_u64();
            for _ in 0..resta.min(10) {
                self.buf.push(ALFABETO[(x & 63) as usize]);
                x >>= 6;
            }
            resta -= resta.min(10);
        }
    }

    /// `Nov  4 10:17:11 host-07 sshd[1234]: ...`
    fn voce_syslog(&mut self) {
        let ms = self.t_ms;
        let (g, resto) = (ms.div_euclid(GIORNO_MS), ms.rem_euclid(GIORNO_MS));
        let (_, m, d) = civil_from_days(g);
        self.buf.extend_from_slice(MESI[m as usize - 1].as_bytes());
        self.buf.push(b' ');
        push_pad(&mut self.buf, u64::from(d), 2, b' ');
        self.buf.push(b' ');
        self.ora(resto, false);
        self.buf.extend_from_slice(b" host-");
        let h = 1 + self.rng.below(12);
        push_pad(&mut self.buf, h, 2, b'0');
        self.buf.push(b' ');
        let t = if self.rng.chance(0.0005) {
            *self.rng.pick(&[
                "kernel: Out of memory: Killed process %i (%s)",
                "kernel: EXT4-fs error (device sda1): ext4_find_entry:%i: reading directory lblock 0",
            ])
        } else {
            const SYS: &[&str] = &[
                "sshd[%i]: Accepted publickey for %u from %p port %i ssh2",
                "sshd[%i]: Failed password for invalid user %u from %p port %i ssh2",
                "CRON[%i]: (%u) CMD (/usr/local/bin/%s --quiet)",
                "systemd[1]: Started Session %i of user %u.",
                "kernel: [%I.%i] eth0: link up, 1000Mbps",
                "nginx[%i]: upstream timed out (110: Connection timed out) while reading response header from upstream, client: %p",
                "postfix/smtp[%i]: %h: to=<%u@example.com>, relay=mail.example.com[%p]:25, delay=%m, status=sent (250 OK)",
                "dhclient[%i]: DHCPACK of %p from %p",
            ];
            // le righe piu' comuni (accessi, cron) piu' spesso delle altre
            let i = self.rng.below(12) as usize;
            SYS[[0, 0, 0, 1, 2, 2, 3, 3, 4, 5, 6, 7][i]]
        };
        self.riempi(t);
    }

    /// `10.1.2.3 - - [04/Nov/2026:10:17:11 +0100] "GET /x HTTP/1.1" 200 512 "-" "agente"`
    fn voce_accesso(&mut self) {
        self.indirizzo();
        self.buf.extend_from_slice(b" - - [");
        let (g, resto) = (
            self.t_ms.div_euclid(GIORNO_MS),
            self.t_ms.rem_euclid(GIORNO_MS),
        );
        let (a, m, d) = civil_from_days(g);
        push_pad(&mut self.buf, u64::from(d), 2, b'0');
        self.buf.push(b'/');
        self.buf.extend_from_slice(MESI[m as usize - 1].as_bytes());
        self.buf.push(b'/');
        push_pad(&mut self.buf, a as u64, 4, b'0');
        self.buf.push(b':');
        self.ora(resto, false);
        self.buf.extend_from_slice(b" +0100] \"");
        let metodo: &[u8] = self.rng.pick(&[
            &b"GET"[..],
            b"GET",
            b"GET",
            b"GET",
            b"POST",
            b"PUT",
            b"DELETE",
        ]);
        self.buf.extend_from_slice(metodo);
        self.buf.push(b' ');
        const PERCORSI: &[&str] = &[
            "/",
            "/login",
            "/health",
            "/api/orders/%i",
            "/api/users/%i",
            "/api/%t",
            "/static/app.%h.js",
            "/static/style.%h.css",
        ];
        let p = *self.rng.pick(PERCORSI);
        self.riempi(p);
        self.buf.extend_from_slice(b" HTTP/1.1\" ");
        let x = self.rng.below(1000);
        let stato: &[u8] = if self.in_raffica() && x < 400 {
            self.rng.pick(&[&b"500"[..], b"502", b"503"])
        } else if x < 860 {
            b"200"
        } else if x < 920 {
            b"304"
        } else if x < 960 {
            b"404"
        } else if x < 980 {
            b"301"
        } else if x < 996 {
            b"403"
        } else {
            b"500"
        };
        self.buf.extend_from_slice(stato);
        self.buf.push(b' ');
        let n = 200 + self.rng.below(1000) * self.rng.below(50);
        push_u(&mut self.buf, n);
        self.buf.extend_from_slice(b" \"-\" \"");
        let ua = *self.rng.pick(AGENTI);
        self.buf.extend_from_slice(ua.as_bytes());
        self.buf.push(b'"');
    }

    fn nome_livello(l: u8) -> &'static [u8] {
        match l {
            0 => b"DEBUG",
            1 => b"INFO",
            2 => b"WARN",
            3 => b"ERROR",
            _ => b"FATAL",
        }
    }

    /// `{"ts":"2026-11-04T10:17:11.756Z","level":"INFO","svc":"orders","msg":"...","ms":12}`
    fn voce_json(&mut self) {
        let mut l = self.livello();
        let inizio = self.buf.len();
        // niente pila di chiamate in JSON: la riga resta un oggetto solo
        let _ = self.messaggio(&mut l);
        let msg: Vec<u8> = self.buf.split_off(inizio);
        self.buf.extend_from_slice(b"{\"ts\":\"");
        self.data_ora(self.t_ms, b'T');
        self.buf.extend_from_slice(b"Z\",\"level\":\"");
        self.buf.extend_from_slice(Self::nome_livello(l));
        self.buf.extend_from_slice(b"\",\"svc\":\"");
        let svc = *self.rng.pick(SERVIZI);
        self.buf.extend_from_slice(svc.as_bytes());
        self.buf.extend_from_slice(b"\",\"msg\":\"");
        self.buf.extend_from_slice(&msg);
        self.buf.extend_from_slice(b"\",\"ms\":");
        let ms = 1 + self.rng.below(4999);
        push_u(&mut self.buf, ms);
        self.buf.push(b'}');
    }

    /// `1793498400877 INFO messaggio`
    fn voce_epoca(&mut self) {
        let mut l = self.livello();
        let inizio = self.buf.len();
        let _ = self.messaggio(&mut l);
        let msg: Vec<u8> = self.buf.split_off(inizio);
        push_u(&mut self.buf, self.t_ms as u64);
        self.buf.push(b' ');
        self.buf.extend_from_slice(Self::nome_livello(l));
        self.buf.push(b' ');
        self.buf.extend_from_slice(&msg);
    }

    /// Una riga di prosa: 8-20 parole; ogni tanto una riga vuota (fine paragrafo). Senza cifre.
    fn voce_testo(&mut self) {
        if self.rng.chance(0.08) {
            return;
        }
        let n = 8 + self.rng.below(13);
        for k in 0..n {
            if k > 0 {
                self.buf.push(b' ');
            }
            let p = *self.rng.pick(PAROLE);
            if k == 0 {
                self.buf.push(p.as_bytes()[0].to_ascii_uppercase());
                self.buf.extend_from_slice(&p.as_bytes()[1..]);
            } else {
                self.buf.extend_from_slice(p.as_bytes());
            }
        }
        self.buf.push(b'.');
    }

    /// Un blocco di tabella ASCII, come il rapporto di un backtest: la riga di titolo ha una data, le altre no.
    fn rapporto(&mut self) {
        const L: usize = 96;
        fn riga(buf: &mut Vec<u8>, testo: &[u8]) {
            buf.push(b'|');
            buf.extend_from_slice(testo);
            for _ in testo.len()..L - 2 {
                buf.push(b' ');
            }
            buf.push(b'|');
        }
        let mut t: Vec<u8> = Vec::new();
        self.buf.extend(std::iter::repeat_n(b'#', L));
        self.fine_riga();
        t.extend_from_slice(b" REPORT ");
        put_data_ora(&mut t, self.t_ms, b' ');
        t.extend_from_slice(b" - EXCHANGE - BTC_USDT - MARKET PRICE: ");
        push_u(&mut t, 20_000 + self.rng.below(40_000));
        t.push(b',');
        push_pad(&mut t, self.rng.below(100), 2, b'0');
        riga(&mut self.buf, &t);
        self.fine_riga();
        self.buf.extend(std::iter::repeat_n(b'#', L));
        self.fine_riga();
        riga(
            &mut self.buf,
            b" ACTION             | TREND         | PRICE        | VOLUME   | PROFIT USDT",
        );
        self.fine_riga();
        self.buf.extend(std::iter::repeat_n(b'-', L));
        for _ in 0..(3 + self.rng.below(4)) {
            self.fine_riga();
            let azione = *self.rng.pick(&[
                "OPEN NEW POSITION",
                "CLOSE ON GROUP SAFE",
                "REOPEN MEDIACY",
                "HOLD",
            ]);
            let tr = *self
                .rng
                .pick(&["UP --> UP", "DOWN --> DOWN", "NONE --> UP"]);
            let mut r: Vec<u8> = Vec::new();
            r.push(b' ');
            r.extend_from_slice(azione.as_bytes());
            while r.len() < 20 {
                r.push(b' ');
            }
            r.extend_from_slice(b"| ");
            r.extend_from_slice(tr.as_bytes());
            while r.len() < 36 {
                r.push(b' ');
            }
            r.extend_from_slice(b"| ");
            push_u(&mut r, 20_000 + self.rng.below(40_000));
            r.push(b',');
            push_pad(&mut r, self.rng.below(100), 2, b'0');
            while r.len() < 51 {
                r.push(b' ');
            }
            r.extend_from_slice(b"| 0,");
            push_pad(&mut r, self.rng.below(100), 2, b'0');
            while r.len() < 62 {
                r.push(b' ');
            }
            r.extend_from_slice(b"| ");
            r.push(if self.rng.chance(0.5) { b'+' } else { b'-' });
            push_u(&mut r, self.rng.below(500));
            r.push(b',');
            push_pad(&mut r, self.rng.below(100), 2, b'0');
            riga(&mut self.buf, &r);
        }
        self.fine_riga();
        self.buf.extend(std::iter::repeat_n(b'-', L));
    }
}

// ---------- l'entrata ----------

/// Scrive il file su `out`. `avanzamento` riceve i byte scritti finora ogni circa 32 MiB. Si ferma alla prima voce completa
/// dopo `cfg.bytes` (per `cfg.bytes` = 0 si scrive una voce sola).
pub fn generate<W: Write>(
    cfg: &Config,
    out: &mut W,
    avanzamento: &mut dyn FnMut(u64),
) -> io::Result<Stats> {
    let mut g = Gen::new(cfg);
    let mut scritti: u64 = 0;
    let mut ultimo_avanzamento: u64 = 0;
    let mut primo_ms = None;
    loop {
        g.voce();
        primo_ms.get_or_insert(g.t_ms);
        if g.buf.len() >= (1 << 18) || scritti + g.buf.len() as u64 >= cfg.bytes {
            out.write_all(&g.buf)?;
            scritti += g.buf.len() as u64;
            g.buf.clear();
            if scritti - ultimo_avanzamento >= (32 << 20) {
                ultimo_avanzamento = scritti;
                avanzamento(scritti);
            }
        }
        if scritti >= cfg.bytes {
            break;
        }
    }
    out.flush()?;
    Ok(Stats {
        bytes: scritti,
        lines: g.lines,
        entries: g.entries,
        first_ms: primo_ms.unwrap_or(g.t_ms),
        last_ms: g.t_ms,
    })
}

/// Legge una dimensione: `500`, `64K`, `10MB`, `2G`, `1.5GiB`, `1T` (sempre multipli di 1024).
pub fn parse_size(s: &str) -> Option<u64> {
    let s = s.trim();
    let fine = s
        .find(|c: char| !(c.is_ascii_digit() || c == '.'))
        .unwrap_or(s.len());
    let (num, unita) = s.split_at(fine);
    let n: f64 = num.parse().ok()?;
    let mult: u64 = match unita.to_ascii_uppercase().as_str() {
        "" | "B" => 1,
        "K" | "KB" | "KIB" => 1 << 10,
        "M" | "MB" | "MIB" => 1 << 20,
        "G" | "GB" | "GIB" => 1 << 30,
        "T" | "TB" | "TIB" => 1 << 40,
        _ => return None,
    };
    let v = n * mult as f64;
    if !(0.0..1.0e16).contains(&v) {
        return None;
    }
    Some(v as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_round_trip_and_known_values() {
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(days_from_civil(2000, 3, 1), 11_017);
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(days_from_civil(2024, 2, 29)), (2024, 2, 29));
        for g in (-800..60_000).step_by(37) {
            let (a, m, d) = civil_from_days(g);
            assert_eq!(days_from_civil(a, m, d), g);
        }
    }

    #[test]
    fn day_parsing_is_strict() {
        assert_eq!(parse_day("2026-01-01"), Some(days_from_civil(2026, 1, 1)));
        assert_eq!(parse_day("2024-02-29"), Some(days_from_civil(2024, 2, 29)));
        for male in [
            "2026-02-29",
            "2026-13-01",
            "2026-00-10",
            "2026-1-1",
            "26-01-01",
            "1969-12-31",
            "2026-01-01-",
            "",
        ] {
            assert_eq!(parse_day(male), None, "{male:?}");
        }
    }

    #[test]
    fn sizes() {
        assert_eq!(parse_size("500"), Some(500));
        assert_eq!(parse_size("64k"), Some(64 << 10));
        assert_eq!(parse_size("10MB"), Some(10 << 20));
        assert_eq!(parse_size("1.5GiB"), Some(3 << 29));
        assert_eq!(parse_size("2T"), Some(2 << 40));
        assert_eq!(parse_size("x"), None);
        assert_eq!(parse_size("5 parsec"), None);
        assert_eq!(parse_size(""), None);
    }

    #[test]
    fn the_random_numbers_are_the_same_everywhere() {
        // valori di riferimento di SplitMix64 con seme 1
        let mut r = Rng::new(1);
        assert_eq!(r.next_u64(), 0x910a_2dec_8902_5cc1);
        assert_eq!(r.next_u64(), 0xbeeb_8da1_658e_ec67);
        let mut r = Rng::new(7);
        for _ in 0..1000 {
            assert!(r.below(10) < 10);
            let u = r.unit();
            assert!((0.0..1.0).contains(&u));
        }
    }

    #[test]
    fn padded_numbers() {
        let mut b = Vec::new();
        push_pad(&mut b, 7, 3, b'0');
        b.push(b'|');
        push_pad(&mut b, 12345, 3, b'0');
        b.push(b'|');
        push_pad(&mut b, 4, 2, b' ');
        b.push(b'|');
        push_u(&mut b, 0);
        assert_eq!(b, b"007|12345| 4|0");
    }
}
