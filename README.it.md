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
        <a href="https://github.com/heavymanto/Manto.RustPad/releases"><img src="https://img.shields.io/github/v/release/heavymanto/Manto.RustPad?label=versione" alt="versione" /></a>
        <img src="https://img.shields.io/badge/test-1674-brightgreen" alt="1674 test" />
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

**Apri log e tracciati da 100 GB in pochi secondi, usando poche decine di MB di RAM.**

Un visualizzatore ed editor ultraveloce scritto in Rust — disponibile sia da terminale (**TUI**) che su desktop (**GUI**) per Windows, Linux e macOS.

| `mrp-tui` — nel terminale | `mrp-gui` — in una finestra |
|:---:|:---:|
| <a href="#due-frontend-un-motore"><img src="docs/img/tui-timeline.gif" alt="mrp-tui in movimento: il log, lo scroll, la timeline che si accende, i colori dei livelli" /></a> | <a href="#due-frontend-un-motore"><img src="docs/img/gui-demo.gif" alt="mrp-gui in movimento: il log colorato nella finestra, lo scroll" /></a> |

## Perche' esiste

Questo progetto nasce da un fastidio quotidiano: dover analizzare log enormi senza saturare la memoria del sistema, oppure scorrere grandi tracciati a record fissi per correggere al volo piccole anomalie.

Fino ad oggi bisognava destreggiarsi tra `less`, `grep`, editor esadecimali e script custom — soluzioni scomode che non aiutano a individuare i problemi a colpo d'occhio.

**Manto.RustPad** riunisce tutto in un'interfaccia essenziale, senza fronzoli: apri il file all'istante, isoli visivamente le anomalie e, se serve, modifichi sul posto.

## Sotto il cofano (Architettura)

Per garantire prestazioni elevate e sicurezza dei dati anche su file giganteschi, **Manto.RustPad** si basa su scelte architetturali precise:

1. **Memory-mapped I/O e indicizzazione non bloccante.** Il file resta su disco in sola lettura; il kernel carica solo le pagine richieste a schermo. L'indice delle righe viene costruito in background e si mette automaticamente in pausa quando l'utente naviga, evitando contese sul disco.
2. **Salvataggio atomico e piece table.** Le modifiche sono gestite tramite *piece table* (la stessa struttura dati di VS Code). Al salvataggio, il nuovo file viene scritto accanto all'originale, sincronizzato su disco e sostituito tramite `rename` atomico — il protocollo stile SQLite — per prevenire corruzioni.
3. **Ricerca binaria sulla timeline.** Gli intervalli temporali della timeline vengono calcolati tramite ricerca binaria direttamente sul file, senza scansionarlo linearmente.
4. **Offset a 64 bit.** Superati i limiti, il tool restituisce errori espliciti anziche' troncare silenziosamente i dati.
5. **Cross-platform nativo (TUI + GUI).** Scritto in Rust senza garbage collector: gira in finestra sul desktop o via SSH direttamente nel terminale.

## Funzionalita' principali

### Navigazione ed esplorazione istantanea

- **Zero attese su file enormi:** apertura immediata indipendentemente dalla dimensione del file (testato oltre i 4 GiB e fino a centinaia di GB).
- **Spostamenti rapidi:** scroll fluido, salti di pagina scalabili (`Alt+PgSu/PgGiu'`: 10 pagine, `Shift` 100, `Ctrl` 1000) e salto diretto a un numero di riga (`Ctrl+G`) o a un orario.
- **Monitoraggio live:** modalita' *follow* stile `tail -f` (`Alt+T`) per seguire i log in tempo reale.
- **Piu' schede e sessioni persistenti:** piu' file insieme; dopo un crash o una chiusura accidentale, schede, cursori e modifiche non salvate vengono ripristinate (su un file da 100 GB la sessione pesa pochi KB).

### Analisi visiva a colpo d'occhio

- **Colori preattentivi (`Alt+L`):** livelli `ERROR`, `WARN`, `INFO`, `DEBUG`, date e orari colorati per essere intercettati dall'occhio prima ancora di leggere la riga.
- **Timeline proporzionale (`Alt+B`):** una mappa laterale suddivisa per intervalli temporali (minuto, dieci minuti, ora, giorno). L'altezza di ogni blocco riflette il volume delle righe e la distribuzione dei livelli: i picchi di errore saltano fuori prima ancora di arrivarci con lo scroll.
- **Check dei tracciati (`Alt+H`):** conta i byte di ogni riga (a-capo escluso) e genera un istogramma interattivo con le percentuali. I formati dominanti stanno in alto, le righe fuori misura in fondo: spuntando le lunghezze desiderate filtri la vista all'istante. Ideale per scovare record corrotti nei file a tracciato fisso.
- **Filtri stile `grep` (`Alt+G`):** isola rapidamente solo le righe che ti interessano.

<p align="center">
  <img src="docs/img/manual/t04-timeline.png" alt="La timeline: una colonna di periodi alta quanto il volume delle righe, divisa per livello" />
</p>
<p align="center"><em>La timeline proporzionale (<code>Alt+B</code>): ogni periodo e' alto quanto il volume delle sue righe, diviso per livello.</em></p>

<p align="center">
  <img src="docs/img/gui-checkdata.png" alt="Il check dei tracciati nella finestra: l'istogramma spuntabile delle lunghezze con le percentuali" />
</p>
<p align="center"><em>Il check dei tracciati (<code>Alt+H</code>): l'istogramma spuntabile delle lunghezze con le percentuali — le righe fuori misura in fondo, sono sempre sospette!</em></p>

### Editing avanzato (senza ricaricare)

- **Modifica in-place (`F4`):** passa dalla sola lettura alla modifica senza ricaricare il file e senza copie.
- **Modalita' colonna (`F8` o `Alt`+trascinamento):** seleziona blocchi verticali per digitare, tabulare, copiare, tagliare o incollare contemporaneamente su piu' righe.
- **Trova e sostituisci (`Ctrl+F` / `Ctrl+H`):** testo semplice, *glob* e *regex* con anteprima delle modifiche e **annullamento globale** (un solo `Ctrl+Z` annulla l'intera sostituzione).
- **Supporto codifiche esteso:** UTF-8, UTF-16, UTF-32, 53 codifiche a byte singolo e set multi-byte dell'Asia orientale, anche su file piu' grandi della RAM.

Ogni funzione ha una carta nel manuale — ingresso, risposta attesa, cosa non deve fare: [docs/MANUAL.it.md](docs/MANUAL.it.md).

## Cosa NON e'

Per mantenere il tool veloce e focalizzato, alcune scelte sono intenzionali:

- **Non e' un log analyzer completo:** offre ausili visivi per la lettura e il debug rapido, non query engine o dashboard di aggregazione.
- **Non e' un IDE o una piattaforma a plugin:** niente estensioni di terze parti o configurazioni infinite; e' uno strumento pronto all'uso.

## Due frontend, un motore

Il motore e' uno, `mrp-core` (documenti, schede, ricerca, modifiche, sessioni), con due interfacce: **`mrp-tui`** per il terminale, pensato per i server e per SSH; **`mrp-gui`** per il desktop, con menu', barra degli strumenti, finestre di dialogo, appunti di sistema e drag-and-drop. Una funzionalita' aggiunta al motore arriva in entrambe.

| | `mrp-tui` | `mrp-gui` |
| --- | --- | --- |
| **Si avvia** | `mrp-tui file...` | `mrp-gui file...` |
| **Look** | sei temi da terminale | Blu (stile Total Commander), Debian, Scuro, Chiaro — o Automatico, che segue il sistema |
| **Timeline** | `Alt+B`, una colonna accanto al testo | lo stesso, o una barra accanto alla scrollbar |
| **Menu'** | i tasti funzione | menu' veri, barra strumenti, barra di stato, scrollbar |

## La prova coi tool noti

Un file da 6,0 GB (60 milioni di righe), una macchina, un hard disk:

| | Memoria | Apertura | Cosa succede |
|---|---:|---|---|
| L'editor che tutti usano | 6.288 MiB | 7 s a caldo / 26–30 s a freddo | tiene tutto in RAM |
| Quello del sistema operativo | 4.100 MiB | 2,8 s | **trunca a 4 GiB — perde 1,7 GB al salvataggio** |
| Il visualizzatore in streaming | 43 MiB | UI 0,5 s | scansione completa in background, ~86 s |
| Quello commerciale | 19 MiB | 7,5 s | scrive una copia temporanea di 5,7 GB; `Ctrl+Fine` rilegge il file |
| **Manto.RustPad** | **13–35 MiB** | **all'istante** | esatto, nessun troncamento, nessuna copia; l'indice si costruisce in background |

---

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

- Sviluppo iniziale: 1.674 test automatici, ma non ancora lunghe sessioni d'uso vero. Tieni una copia di quello che ti preme.
- Windows prima. L'editor da terminale e il core girano anche su Linux (Debian, WSL); la CI gira tutta la suite sui tre sistemi a ogni push.
- La finestra (mrp-gui) e' stata controllata con test automatici e schermate, non ancora con lungo uso vero di tastiera e mouse.
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

## Altri progetti collegati

- [Manto.FreeDisk](https://github.com/heavymanto/Manto.FreeDisk) — «Where did my disk space go?»: l'analizzatore di spazio disco in Rust da cui viene la separazione motore/frontend usata qui

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
