<table>
  <tr>
    <td valign="middle">
      <h1>Manto.RustPad</h1>
      <h3>Un visualizzatore di log per file che non stanno in RAM</h3>
      <p>
        <img src="https://img.shields.io/badge/APRE-ALL'ISTANTE-e3b341?style=for-the-badge" alt="apre all'istante" />
        <img src="https://img.shields.io/badge/TIMELINE-DEL%20FILE-e3b341?style=for-the-badge" alt="timeline del file" />
        <img src="https://img.shields.io/badge/SCORRE-FILE%20DA%20100%20GB-e3b341?style=for-the-badge" alt="file da 100 GB" />
        <img src="https://img.shields.io/badge/MODALITA'-COLONNA-e3b341?style=for-the-badge" alt="modalita' colonna" />
        <img src="https://img.shields.io/badge/MAI-TRUNCA-e3b341?style=for-the-badge" alt="mai trunca" />
      </p>
      <p>
        <a href="https://github.com/heavymanto/Manto.RustPad/actions/workflows/ci.yml"><img src="https://github.com/heavymanto/Manto.RustPad/actions/workflows/ci.yml/badge.svg" alt="CI" /></a>
        <img src="https://img.shields.io/badge/test-1538-brightgreen" alt="1538 test" />
        <img src="https://img.shields.io/badge/frontend-mrp--tui%20%C2%B7%20mrp--gui-blue" alt="mrp-tui e mrp-gui" />
        <img src="https://img.shields.io/badge/lingue-7-blue" alt="sette lingue" />
        <img src="https://img.shields.io/badge/licenza-AGPL--3.0-brightgreen" alt="AGPL-3.0" />
        <img src="https://img.shields.io/badge/stato-sviluppo%20iniziale-orange" alt="sviluppo iniziale" />
      </p>
      <p>
        <a href="docs/BENCHMARKS.md"><img src="https://img.shields.io/badge/%F0%9F%93%96-MISURE-007ec6?style=flat-square" alt="Misure" /></a>
        <a href="#quickstart"><img src="https://img.shields.io/badge/%F0%9F%94%A7-QUICKSTART-007ec6?style=flat-square" alt="Quickstart" /></a>
        <a href="README.md"><img src="https://img.shields.io/badge/%F0%9F%87%AC%F0%9F%87%A7-READ%20IN%20ENGLISH-007ec6?style=flat-square" alt="Read it in English" /></a>
      </p>
    </td>
  </tr>
</table>

---

## Abstract

**Apri un log da 100 GB in pochi secondi, utilizzando pochi mega di RAM. Scorri la sua cronologia con una timeline visuale. Individua immediatamente dove si addensano le criticità in un solo colpo d'occhio. Usa filtri avanzati per concentrarti solo su quello che interessa. Modifica il contenuto anche con la modalità a colonne avanzato.**

<p align="justify">
L'idea nasce da un fastidio che conosco bene, ossia esaminare log di grandi dimensione senza saturare la RAM, cercare i punti di interesse e all'occorrenza fare piccole modifiche. L'idea era di sfruttare la gestione dell memoria di Rust che, essendo senza GC (Garbage Collector), ha una gestione incredibilmente parsimoniosa. Scommessa vinta! FIno ad oggni suavo più tool, alcuni gestiscono bene la memoria ma sono viewer limitatissimi, altri hanno fuznioni avanzate ma usano molta ram, gli editor fanno ancora molta fatica. In nessun caso ho mai visto timeline visuali che mi dessero il colpo d'occhi sia sulla cronologia degli eventi che sulla densità delgli stessi. in un file enorme come "vedo" dove si sono verificati maggiormente gli errori. 
Così ho deciso di creare il mio tool 5 in 1, apre file onori, usa poca RAM, è velocissimo, da un colpo di colore per focalizzarsi su quello che serve. Scorri la crenologia degli eventi ed individua subito dove stanno i problemi. Tutto questo sia in modalità terminale che gui e su ben 3 OS. Il design è minimale, qui si va al sodo.
</p>

**Manto.RustPad** e' quel tool. Apre il file memory-mapped e in sola lettura, costruisce un indice sparso delle righe in background, e appena premi *Modifica* lo stesso file diventa editabile — nessun ricaricamento, nessuna copia. Le modifiche vivono in una piccola piece table accanto all'originale intatto; il salvataggio scrive in streaming un file nuovo e lo scambia in modo atomico.

## I quattro pilastri

Ognuno sta in piedi su un'idea ben nota — non e' invenzione, e' letteratura applicata:

1. **Un file enorme si apre in secondi — e resta aperto.** Il file e' *memory-mappato*: il kernel consegna all'editor le pagine che chiede, quando le chiede (come fanno `less` e i grandi visualizzatori di log). L'indice delle righe si costruisce in background, e **si mette in pausa mentre salti** cosi' il disco resta tuo. Leggere 100 GB costa decine di MiB perche' nulla viene mai copiato.

2. **Nulla si perde mai — nemmeno cio' che non hai salvato, nemmeno con un crash.** Il salvataggio e' *atomico*: il file nuovo viene scritto intero accanto al vecchio, scaricato su disco, poi scambiato con un solo rename — lo stesso protocollo tutto-o-niente che SQLite usa per i suoi commit. E la sessione e' un diario delle *sole modifiche*: chiudi senza chiedere, uccidi il processo, manca la luce — riapri, e le schede, i cursori e **le modifiche non salvate** sono di nuovo li' (la piece table tiene l'originale intatto e le modifiche a parte: la struttura che Crowley ha descritto per gli editor di testo nel 1998 e che VS Code ha adottato nel 2018).

3. **I livelli, a colpo d'occhio.** ERROR in rosso, WARN in giallo, INFO in verde, date e orari con le loro tinte: il colore e' *preattentivo* — l'occhio lo processa prima di leggere una riga (il lavoro di Healey sul preattentive processing). Su un log con tempeste di errori, vedi la tempesta prima di trovarla.

4. **La timeline — unica nel suo genere.** Una colonna di periodi, ognuno lungo quanto il suo volume di righe, diviso per livello: una mappa proporzionale del file intero, dove i punti densi *sono* i problemi. Scorri, e il log si legge come una radiografia di se' stesso: dove le cose sono andate storte si vede prima di leggere una riga. I periodi si trovano per ricerca binaria, senza leggere il file.

5. **Multi uso, multi platfor - lo puoi usare in un termininale (es SSH) senza bisogno dell'interfaccia grafica. Gira su Win/Linux/Mac

## POC con tool "noti"

Su un file da 6,0 GB (60 milioni di righe), una macchina, un HDD — il metodo per intero in [docs/BENCHMARKS.md](docs/BENCHMARKS.md):

| | Memoria | Apertura | Cosa succede |
|---|---:|---|---|
| L'editor che tutti usano | 6.288 MiB | 7 s a caldo / 26–30 s a freddo | tiene tutto in RAM |
| Quello del sistema operativo | 4.100 MiB | 2,8 s | **trunca a 4 GiB — perde 1,7 GB al salvataggio** |
| Il visualizzatore in streaming | 43 MiB | UI 0,5 s | scansione completa in background, ~86 s |
| Quello commerciale | 19 MiB | 7,5 s | scrive una copia temporanea di 5,7 GB; `Ctrl+Fine` rilegge il file |
| **Manto.RustPad** | **13–35 MiB** | **all'istante** | esatto, nessun troncamento, nessuna copia — l'indice si riempie dietro di te |

La memoria cresce di circa 3,7 MB per GB di file — e non tiene mai il file. Un log da 100 GB su un hard disk si apre in 36 ms. *(Chi e' chi, con versioni e fonti: [docs/MARKET.md](docs/MARKET.md).)*

---

## Due frontend, un motore

Il motore e' uno (`mrp-core`: documenti, schede, ricerca, modifiche, sessioni) — e due modi di guardarlo: **`mrp-tui`** per qualunque terminale, pensato per i server e per SSH; **`mrp-gui`** per il desktop, con menu' veri, barra degli strumenti, finestre di dialogo, gli appunti di sistema e il drag-and-drop. Una funzionalita' aggiunta al motore arriva in entrambi.

| `mrp-tui` — nel terminale | `mrp-gui` — in una finestra |
|:---:|:---:|
| <a href="#due-frontend-un-motore"><img src="docs/img/tui-timeline.gif" alt="mrp-tui in movimento: il log, lo scroll, la timeline che si accende, i colori dei livelli" /></a> | <a href="#due-frontend-un-motore"><img src="docs/img/gui-demo.gif" alt="mrp-gui in movimento: il log colorato nella finestra, lo scroll" /></a> |

| | `mrp-tui` | `mrp-gui` |
| --- | --- | --- |
| **Si avvia** | `mrp-tui file...` | `mrp-gui file...` |
| **Look** | sei temi da terminale | Blu (stile Total Commander), Debian, Scuro, Chiaro — o Automatico, che segue il sistema |
| **Timeline** | `Alt+B`, una colonna accanto al testo | lo stesso, o una barra accanto alla scrollbar |
| **Menu'** | i tasti funzione | menu' veri, barra strumenti, barra di stato, scrollbar |

## In azione

`mrp-tui` su un log: il file si apre all'istante, lo scroll gira libero, poi la timeline si accende (`Alt+B`) — ogni periodo lungo quanto il suo volume di righe, diviso per livello (ERROR, WARN, INFO, DEBUG), le tempeste di errori visibili a colpo d'occhio — e arrivano i colori dei livelli (`Alt+L`):

<a href="#in-azione"><img src="docs/img/tui-timeline.gif" alt="mrp-tui in movimento: scroll, timeline, colori dei livelli" /></a>

E `mrp-gui`, lo stesso motore in una finestra — il log si apre, lo scroll gira, a colori:

<a href="#in-azione"><img src="docs/img/gui-demo.gif" alt="mrp-gui in movimento: il log nella finestra, lo scroll" /></a>

## Cosa fa

- **Un visualizzatore di log per file immensi.** Apre all'istante, scorre con rotella, scrollbar e salti di pagina (`Alt+PgSu/PgGiu'`: 10 pagine, con Shift 100, con Ctrl 1000). I numeri di riga si riempiono da un indice in background che **si mette in pausa mentre salti**. Vai a una riga (`Ctrl+G`) o a un orario, segui la coda come `tail -f` (`Alt+F`), filtra le righe come `grep` (`Alt+G`), colora i livelli (`Alt+L`).
- **Una timeline del file.** Una colonna di periodi — un minuto, dieci minuti, un'ora, un giorno — ognuno lungo quanto il suo volume di righe e diviso per livello, cosi' dove sono gli errori si vede a colpo d'occhio. La parte che stai leggendo e' marcata; un click o un trascinamento ti porta li'. I periodi si trovano per ricerca binaria **senza leggere il file**.
- **…e puoi modificarlo, con la modifica per colonne.** Premi `F4` e lo stesso file diventa editabile — nessun ricaricamento, nessuna copia. **La modalita' colonna (blocco)** (`F8`, o `Alt`+trascinamento col mouse): digitare, `Tab`, copia, taglia e incolla agiscono su tutte le righe del blocco in una volta, in un file di qualunque taglia.
- **Memoria indipendente dalla taglia del file.** Decine di MiB, qualunque sia il file. Mai il file intero in memoria.
- **Non perde mai dati.** Offset a 64 bit ovunque, test su file oltre i 4 GiB, errore rumoroso invece del troncamento silenzioso, salvataggio a prova di crash.
- **Schede, e chiudere non perde nulla.** Piu' file insieme; esci senza che ti venga chiesto — la volta dopo le schede, i cursori e **le modifiche non salvate** sono di nuovo li'. Si salvano solo le modifiche: su un file da 100 GB con poche correzioni, la sessione pesa pochi KB.
- **Trova e sostituisci con glob e regex** (`Ctrl+F` / `Ctrl+H`), con una **preview** prima di toccare qualunque cosa e un solo `Ctrl+Z` che annulla tutto.
- **Codifiche.** UTF-8/16/32, 53 tabelle a byte singolo, le multi-byte dell'Asia orientale — su file piu' grandi della RAM.

## Download

Niente da installare: estrai ed esegui — entrambi i programmi funzionano subito, in modalita' *portabile* (inglese incorporato, le altre sei lingue in `lang/` accanto ai programmi). Ogni pacchetto porta `install.ps1` (Windows) o `install.sh` (Linux, macOS) se li vuoi come comandi da qualunque cartella. Quello Windows accetta anche `-Associa`: mrp-gui finisce nell'elenco "Apri con" di `.txt` e `.log` e in Impostazioni > App predefinite, solo per il tuo utente (niente admin) — il default resta un click tuo, quello che Windows firma. Su Linux `./install.sh --associa` imposta davvero il default dei file di testo.

<div align="center">
<details>
<summary><h3>💚 ⬇️ DOWNLOAD ⬇️ 💚</h3></summary>
<br/>
<p align="center">
  <a href="#scaricalo"><img src="docs/img/star.png" alt="Stella" width="120" /></a>
  <br/><br/>
  <b>Noi ti doniamo il software. Tu donaci una stella.</b> ⭐<br/>
  <i>Le stelle fanno sognare — e per un progetto giovane sono la prova<br/>
  che serve. Sono il carburante di cio' che verra'.</i>
  <br/><br/>
  <a href="https://github.com/heavymanto/Manto.RustPad"><img src="https://img.shields.io/badge/%E2%AD%90-LASCIA%20UNA%20STELLA-e3b341?style=for-the-badge" alt="Lascia una stella" /></a>
  <br/>
  <a href="https://github.com/heavymanto/Manto.RustPad/stargazers"><img src="https://img.shields.io/github/stars/heavymanto/Manto.RustPad?style=flat-square&label=stelle" alt="stelle" /></a>
  <a href="https://github.com/heavymanto/Manto.RustPad/releases"><img src="https://img.shields.io/github/downloads/heavymanto/Manto.RustPad/total?style=flat-square&label=download" alt="download" /></a>
  <br/><br/>
  <a href="https://github.com/heavymanto/Manto.RustPad/releases/download/v0.1.3/manto-rustpad-0.1.3-win-x64.zip"><img src="https://img.shields.io/badge/%E2%AC%87-Windows%20x64-2ea44f?style=for-the-badge" alt="Windows x64" /></a>
  <a href="https://github.com/heavymanto/Manto.RustPad/releases/download/v0.1.3/manto-rustpad-0.1.3-linux-x64.tar.gz"><img src="https://img.shields.io/badge/%E2%AC%87-Linux%20x64-2ea44f?style=for-the-badge" alt="Linux x64" /></a>
  <a href="https://github.com/heavymanto/Manto.RustPad/releases/download/v0.1.3/manto-rustpad-0.1.3-macos-aarch64.tar.gz"><img src="https://img.shields.io/badge/%E2%AC%87-macOS-2ea44f?style=for-the-badge" alt="macOS" /></a>
</p>
</details>
</div>

Puoi anche compilarlo da fonte — due comandi, nel [Quickstart](#quickstart).

## Quickstart

Serve [Rust](https://rustup.rs) e un clone di questo repository.

```sh
cd rust
cargo build --release                 # mrp-tui e mrp-gui in target/release/
./target/release/mrp-tui tuofile.txt      # F1 = aiuto, F4 = modifica, F10 = esci
./target/release/mrp-gui tuofile.txt     # lo stesso, in una finestra
```

Nessun log sottomano? Creane uno, della taglia che vuoi, stessi byte su ogni macchina:

```sh
./target/release/mrp-loggen --size 2M --scenario bursts demo.log
```

## Limiti noti

- **Windows prima**; Sviluppato e testato principalmente su windows. l'editor da terminale e il core girano anche su Linux (Debian, WSL), le release sono state fatte in modo automatico via CI.
- Regex, glob e ignore-case funzionano solo su documenti UTF-8; la ricerca di testo semplice funziona in ogni codifica.
- Le misure vengono da una macchina sola e un HDD solo: [docs/BENCHMARKS.md](docs/BENCHMARKS.md).

## Sviluppo

```sh
cd rust
cargo test --workspace
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
```

## Struttura del repository

| Percorso | Contenuto |
|---|---|
| `rust/crates/mrp-core` | il motore: `FileSource`, `LineIndex`, `PieceTable`/`Document`, ricerca, sessioni, codifiche, lingue |
| `rust/crates/mrp-tui` | l'editor da terminale (ratatui) |
| `rust/crates/mrp-gui` | la finestra del desktop (egui) |
| `rust/crates/mrp-loggen` | log sintetici di qualunque taglia, stessi byte a parita' di seme |
| `docs/` | `MARKET.md` (il panorama, con le fonti), `BENCHMARKS.md` (misure, metodo) |
| `bench/` | script riproducibili e risultati grezzi |

## Documentazione

- [docs/MANUAL.md](docs/MANUAL.md) — il manuale delle funzionalità in dettaglio
- [CHANGELOG.md](CHANGELOG.md) — cosa e' cambiato
- [SECURITY.md](SECURITY.md) — come segnalare una vulnerabilita' in privato

## Contribuire

Vedi [CONTRIBUTING.md](CONTRIBUTING.md). In breve: gli offset sono a 64 bit, il file intero non entra mai in memoria, e ogni numero nei documenti e' stato misurato davvero. E' un progetto hobby — tutto viene letto, ma nessun tempo di risposta e' promesso.

## Licenza

Copyright (C) 2026 Francesco Mantovanelli.

Questo programma e' software libero: puoi ridistribuirlo e/o modificarlo nei
termini della **GNU Affero General Public License v3.0** come pubblicata dalla
Free Software Foundation — vedi [LICENSE](LICENSE). La licenza commerciale e'
disponibile su richiesta per prodotti che non possono viaggiare in AGPL.

---

<div align="center">

**Autore** — Francesco Mantovanelli · **Progetto** — Manto.RustPad

</div>
