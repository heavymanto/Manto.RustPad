//! `mrp-loggen`: scrive un log sintetico della dimensione che si vuole (vedi la libreria per gli scenari).

use mrp_loggen::{generate, parse_day, parse_size, Config, Scenario};
use std::fs::OpenOptions;
use std::io::{self, BufWriter, IsTerminal, Write};
use std::process::ExitCode;
use std::time::Instant;

const USO: &str = "\
mrp-loggen - writes a synthetic log file of any size (same bytes for the same options, on every machine)

usage: mrp-loggen --size SIZE [options] FILE
       mrp-loggen --size SIZE [options] - > file        (standard output)
       mrp-loggen --list

  --size SIZE        where to stop: 500, 64K, 10MB, 2G, 1.5GiB, 100G (multiples of 1024). The last entry is
                     completed, so the file is a little larger. Required.
  --scenario NAME    the kind of log (default service); see --list
  --seed N           the random seed (default 1): same seed, same bytes
  --start YYYY-MM-DD the first day (default 2026-01-01)
  --rate N           average entries per second (default 20): the file spans more days when it is lower
  --force            overwrite FILE if it exists (otherwise it is refused)
  --list             the scenarios, one per line
  -h, --help         this text

Memory used: a few MB, whatever the size. To keep a compressed copy: mrp-loggen --size 1G - | gzip > log.gz";

fn errore(msg: &str) -> ExitCode {
    eprintln!("mrp-loggen: {msg}\nTry: mrp-loggen --help");
    ExitCode::from(2)
}

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let mut scenario = Scenario::Service;
    let mut bytes: Option<u64> = None;
    let mut seed = 1u64;
    let mut start = None;
    let mut rate = 20.0f64;
    let mut force = false;
    let mut file: Option<String> = None;
    while let Some(a) = args.next() {
        let mut valore = |nome: &str| args.next().ok_or_else(|| format!("{nome} needs a value"));
        let r: Result<(), String> = (|| {
            match a.as_str() {
                "-h" | "--help" => {
                    println!("{USO}");
                    std::process::exit(0);
                }
                "--list" => {
                    for s in Scenario::ALL {
                        println!("{:<10} {}", s.name(), s.description());
                    }
                    std::process::exit(0);
                }
                "--size" => {
                    let v = valore("--size")?;
                    bytes = Some(
                        parse_size(&v).ok_or(format!("{v:?} is not a size (try 500M or 2G)"))?,
                    );
                }
                "--scenario" => {
                    let v = valore("--scenario")?;
                    scenario = Scenario::parse(&v)
                        .ok_or(format!("unknown scenario {v:?} (see --list)"))?;
                }
                "--seed" => {
                    let v = valore("--seed")?;
                    seed = v.parse().map_err(|_| format!("{v:?} is not a number"))?;
                }
                "--start" => {
                    let v = valore("--start")?;
                    start = Some(
                        parse_day(&v)
                            .ok_or(format!("{v:?} is not a day (YYYY-MM-DD, from 1970)"))?,
                    );
                }
                "--rate" => {
                    let v = valore("--rate")?;
                    rate = v
                        .parse::<f64>()
                        .ok()
                        .filter(|r| r.is_finite() && *r > 0.0 && *r <= 1.0e6)
                        .ok_or(format!(
                            "{v:?} is not a rate (entries per second, 0 < N <= 1000000)"
                        ))?;
                }
                "--force" => force = true,
                s if s.starts_with('-') && s != "-" => return Err(format!("unknown option {s:?}")),
                _ => {
                    if file.replace(a.clone()).is_some() {
                        return Err("only one output file".to_string());
                    }
                }
            }
            Ok(())
        })();
        if let Err(e) = r {
            return errore(&e);
        }
    }
    let Some(bytes) = bytes else {
        return errore("--size is required");
    };
    let Some(file) = file else {
        return errore("the output file is missing (use - for the standard output)");
    };
    let mut cfg = Config::new(scenario, bytes);
    cfg.seed = seed;
    cfg.rate = rate;
    if let Some(d) = start {
        cfg.start_day = d;
    }

    let inizio = Instant::now();
    let tty = io::stderr().is_terminal();
    let mut progress = |scritti: u64| {
        if tty {
            let pct = (scritti as f64 / bytes.max(1) as f64 * 100.0).min(100.0);
            eprint!(
                "\r{pct:5.1}%  {:.2} GiB",
                scritti as f64 / (1u64 << 30) as f64
            );
        }
    };
    let esito = if file == "-" {
        let mut out = BufWriter::with_capacity(1 << 20, io::stdout().lock());
        generate(&cfg, &mut out, &mut progress)
    } else {
        let mut opts = OpenOptions::new();
        opts.write(true);
        if force {
            opts.create(true).truncate(true);
        } else {
            opts.create_new(true);
        }
        let mut f = match opts.open(&file) {
            Ok(f) => f,
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                return errore(&format!(
                    "{file} already exists (use --force to overwrite it)"
                ));
            }
            Err(e) => return errore(&format!("cannot create {file}: {e}")),
        };
        generate(&cfg, &mut f, &mut progress)
    };
    match esito {
        Ok(st) => {
            if tty {
                eprint!("\r{:60}\r", "");
            }
            let secondi = inizio.elapsed().as_secs_f64().max(0.001);
            let giorni = (st.last_ms - st.first_ms) as f64 / 86_400_000.0;
            let _ = writeln!(
                io::stderr(),
                "{} bytes, {} lines, {} entries, {:.1} days of log, {:.1} s ({:.0} MB/s)",
                st.bytes,
                st.lines,
                st.entries,
                giorni,
                secondi,
                st.bytes as f64 / 1.0e6 / secondi
            );
            ExitCode::SUCCESS
        }
        Err(e) => {
            if tty {
                eprintln!();
            }
            errore(&format!("writing failed: {e}"))
        }
    }
}
