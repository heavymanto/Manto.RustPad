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
        <img src="https://img.shields.io/badge/versione-0.1.6-blue" alt="versione 0.1.6" />
        <img src="https://img.shields.io/badge/test-1673-brightgreen" alt="1673 test" />
        <img src="https://img.shields.io/badge/frontend-mrp--tui%20%C2%B7%20mrp--gui-blue" alt="mrp-tui e mrp-gui" />
        <img src="https://img.shields.io/badge/lingue-7-blue" alt="sette lingue" />
        <img src="https://img.shields.io/badge/licenza-AGPL--3.0-brightgreen" alt="AGPL-3.0" />
        <img src="https://img.shields.io/badge/stato-sviluppo%20iniziale-orange" alt="sviluppo iniziale" />
      </p>
      <p>
        <a href="#quickstart"><img src="https://img.shields.io/badge/%F0%9F%94%A7-QUICKSTART-007ec6?style=flat-square" alt="Quickstart" /></a>
        <a href="README.md"><img src="https://img.shields.io/badge/%F0%9F%87%AC%F0%9F%87%A7-READ%20IN%20ENGLISH-007ec6?style=flat-square" alt="Read it in English" /></a>
      </p>
    </td>
  </tr>
</table>

---

## Abstract

<p align="justify"><strong>Apri un log da 100 GB in pochi secondi, con pochi mega di RAM. Scorri la sua cronologia con una timeline, colora i livelli, spunta le lunghezze dei record per isolare le righe fuori misura. E all'occorrenza modifichi, anche a colonne.</strong></p>

<p align="justify">
L'idea e' nata da un fastidio personale: analizzare log enormi senza saturare la RAM, beccare i punti critici e poterli correggere al volo. Fino ad oggi ero costretto a destreggiarmi tra piu' tool — una soluzione scomoda, piena di limiti e senza niente che ti aiutasse davvero a individuare le problematiche a colpo d'occhio.
</p>

<p align="justify">
Per risolverlo ho puntato su Rust: niente GC, prestazioni eccellenti e consumo di RAM ridotto all'osso. Ne e' uscito fuori un tool 6-in-1 che fa tutto:
</p>

- Gestisce file enormi con un consumo di RAM minimo.
- E' velocissimo.
- Usa i colori per focalizzare l'attenzione su quello che conta.
- Ha una timeline per muoversi tra gli eventi e scovare i guai.
- Include un istogramma spuntabile per isolare i record fuori misura.
- Il tutto disponibile sia da terminale, TUI, che in versione desktop GUI — cross-platform su 3 OS.

<p align="justify">Interfaccia essenziale, zero fronzoli.</p>

## I sei pilastri

Sei idee ben note, applicate fino in fondo:

1. **Il file sta su disco, non in RAM.** E' memory-mappato in sola lettura: il kernel consegna le pagine che l'editor chiede, quando le chiede (come `less`). L'indice delle righe si costruisce in background e si mette in pausa quando l'utente salta, per non contendere il disco. Su un file da 100 GB l'editor occupa decine di MiB.

2. **Il salvataggio e' atomico, la sessione e' un diario delle modifiche.** Il file nuovo viene scritto accanto al vecchio, scaricato su disco e scambiato con un rename: lo stesso protocollo dei commit di SQLite. La sessione registra solo le modifiche, quindi dopo un crash, o dopo aver chiuso senza salvare, schede, cursori e modifiche non salvate tornano al loro posto. La piece table e' la struttura descritta da Crowley per gli editor di testo nel 1998 e adottata da VS Code nel 2018.

3. **I livelli si vedono prima di leggerli.** ERROR rosso, WARN giallo, INFO verde, date e orari colorati: il colore preattentivo viene processato dall'occhio prima della lettura della riga (il lavoro di Healey sul preattentive processing). Un picco di errori in un log di milioni di righe si nota appena entra nello schermo.

4. **La timeline e' una mappa proporzionale del file.** Una colonna di periodi (minuto, dieci minuti, ora, giorno), ognuno alto quanto il suo volume di righe e diviso per livello: le zone dense si vedono prima di arrivarci con lo scroll. I periodi si trovano per ricerca binaria, senza leggere il file.

5. **Multi uso, multi piattaforma.** Si usa in un terminale (per esempio in SSH) senza bisogno dell'interfaccia grafica, oppure in finestra sul desktop. Gira su Windows, Linux e macOS.

6. **Il check dei tracciati trova le anomalie a colpo d'occhio.** `Alt+H` conta quante righe hanno ogni lunghezza (byte di contenuto, a-capo escluso) e le mette in un istogramma spuntabile: i formati dominanti prima, le righe fuori misura in fondo, dove l'occhio le becca. Spunta le lunghezze da vedere e la vista si rifà con solo quelle righe: in un file a record fissi le anomalie saltano fuori da sole.
## POC con i tool noti

Un file da 6,0 GB (60 milioni di righe), una macchina, un hard disk:

| | Memoria | Apertura | Cosa succede |
|---|---:|---|---|
| L'editor che tutti usano | 6.288 MiB | 7 s a caldo / 26–30 s a freddo | tiene tutto in RAM |
| Quello del sistema operativo | 4.100 MiB | 2,8 s | **trunca a 4 GiB — perde 1,7 GB al salvataggio** |
| Il visualizzatore in streaming | 43 MiB | UI 0,5 s | scansione completa in background, ~86 s |
| Quello commerciale | 19 MiB | 7,5 s | scrive una copia temporanea di 5,7 GB; `Ctrl+Fine` rilegge il file |
| **Manto.RustPad** | **13–35 MiB** | **all'istante** | esatto, nessun troncamento, nessuna copia; l'indice si costruisce in background |

---

## Due frontend, un motore

Il motore e' uno, `mrp-core` (documenti, schede, ricerca, modifiche, sessioni), con due interfacce: **`mrp-tui`** per il terminale, pensato per i server e per SSH; **`mrp-gui`** per il desktop, con menu', barra degli strumenti, finestre di dialogo, appunti di sistema e drag-and-drop. Una funzionalita' aggiunta al motore arriva in entrambe.

| `mrp-tui` — nel terminale | `mrp-gui` — in una finestra |
|:---:|:---:|
| <a href="#due-frontend-un-motore"><img src="docs/img/tui-timeline.gif" alt="mrp-tui in movimento: il log, lo scroll, la timeline che si accende, i colori dei livelli" /></a> | <a href="#due-frontend-un-motore"><img src="docs/img/gui-demo.gif" alt="mrp-gui in movimento: il log colorato nella finestra, lo scroll" /></a> |

| | `mrp-tui` | `mrp-gui` |
| --- | --- | --- |
| **Si avvia** | `mrp-tui file...` | `mrp-gui file...` |
| **Look** | sei temi da terminale | Blu (stile Total Commander), Debian, Scuro, Chiaro — o Automatico, che segue il sistema |
| **Timeline** | `Alt+B`, una colonna accanto al testo | lo stesso, o una barra accanto alla scrollbar |
| **Menu'** | i tasti funzione | menu' veri, barra strumenti, barra di stato, scrollbar |

## Funzioni

- Apertura all'istante di file di qualunque taglia; scroll con rotella, scrollbar e salti di pagina (`Alt+PgSu/PgGiu'`: 10 pagine, Shift 100, Ctrl 1000).
- Vai a una riga (`Ctrl+G`) o a un orario; coda del file come `tail -f` (`Alt+T`).
- Filtri stile `grep` (`Alt+G`), colori dei livelli (`Alt+L`), timeline (`Alt+B`), check dei tracciati (`Alt+H`).
- Modifica senza ricaricare (`F4`); modalita' colonna (`F8` o `Alt`+trascinamento): digitare, `Tab`, copia, taglia e incolla su tutte le righe del blocco.
- Trova e sostituisci (`Ctrl+F` / `Ctrl+H`) su testo, glob e regex, con preview e un solo `Ctrl+Z` che annulla tutto.
- Piu' schede insieme; la sessione ripristina schede, cursori e modifiche non salvate (su un file da 100 GB con poche correzioni pesa pochi KB).
- Codifiche: UTF-8/16/32, 53 tabelle a byte singolo, le multi-byte dell'Asia orientale, oltre la RAM.
- Offset a 64 bit, test oltre i 4 GiB, errore esplicito invece del troncamento silenzioso; la memoria resta decine di MiB qualunque sia la taglia.

Non e' un analizzatore di log (gli aiuti alla lettura sono quelli che servono a un visualizzatore) e non e' una piattaforma di plugin o un IDE.

## Download

Versione attuale: **0.1.6** (novembre 2026).

Niente installazione: estrai il pacchetto ed esegui. Entrambi i programmi funzionano subito in modalita' portabile (inglese incorporato, le altre sei lingue in `lang/` accanto ai programmi). Se li vuoi come comandi da qualunque cartella, c'e' `install.ps1` su Windows e `install.sh` su Linux e macOS. Su Windows, `install.ps1 -Associa` aggiunge mrp-gui all'elenco "Apri con" di `.txt` e `.log` e a Impostazioni > App predefinite, solo per l'utente corrente e senza admin: il default resta una scelta che firma Windows. Su Linux, `./install.sh --associa` imposta il default per i file di testo.

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
  <a href="https://github.com/heavymanto/Manto.RustPad/releases/download/v0.1.6/manto-rustpad-0.1.6-win-x64.zip"><img src="https://img.shields.io/badge/%E2%AC%87-Windows%20x64-2ea44f?style=for-the-badge" alt="Windows x64" /></a>
  <a href="https://github.com/heavymanto/Manto.RustPad/releases/download/v0.1.6/manto-rustpad-0.1.6-linux-x64.tar.gz"><img src="https://img.shields.io/badge/%E2%AC%87-Linux%20x64-2ea44f?style=for-the-badge" alt="Linux x64" /></a>
  <a href="https://github.com/heavymanto/Manto.RustPad/releases/download/v0.1.6/manto-rustpad-0.1.6-macos-aarch64.tar.gz"><img src="https://img.shields.io/badge/%E2%AC%87-macOS-2ea44f?style=for-the-badge" alt="macOS" /></a>
</p>
</details>
</div>

Puoi anche compilarlo dai sorgenti: due comandi, nel [Quickstart](#quickstart).

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

- Windows prima: sviluppato e testato principalmente su Windows. L'editor da terminale e il core girano anche su Linux (Debian, WSL); le release sono compilate in automatico dalla CI.
- Regex, glob e ignore-case funzionano solo su documenti UTF-8; la ricerca di testo semplice funziona in ogni codifica.
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
| `bench/` | script riproducibili e risultati grezzi |

## Documentazione

- [docs/MANUAL.it.md](docs/MANUAL.it.md) — il manuale delle funzionalita' in dettaglio, in italiano
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
