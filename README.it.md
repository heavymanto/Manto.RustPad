# Manto.RustPad

### L'editor per i file che non stanno in RAM

[![CI](https://github.com/heavymanto/Manto.RustPad/actions/workflows/ci.yml/badge.svg)](https://github.com/heavymanto/Manto.RustPad/actions/workflows/ci.yml)
![licenza AGPL-3.0](https://img.shields.io/badge/licenza-AGPL--3.0-brightgreen)
![stato: sviluppo iniziale](https://img.shields.io/badge/stato-sviluppo%20iniziale-orange)

*Un editor di testo in Rust, con comandi familiari (non modale), la cui memoria non dipende dalla dimensione del file,
che non tronca mai i dati in silenzio e che apre un file da molti gigabyte come un viewer, diventando modificabile a richiesta.*

> **Stato: sviluppo iniziale — gira un primo editor da terminale (`mrp`).** File mappato in
> memoria, indice righe sparso, ricerca, livello di modifica a piece table con annulla/ripeti, salvataggio a prova di interruzione e interfaccia da terminale (lettura, modifica, ricerca, vai a, selezione e appunti, modalità colonna, temi, 7 lingue) sono realizzati e testati (356 test più 3 su file da 6 GB oltre 4 GiB). Finora è stato provato solo su Windows. Su un file da 6 GB si apre subito e tiene circa 13–35 MiB di memoria privata, anche durante il salvataggio (una macchina, un solo disco meccanico, poche misure: vedi [docs/BENCHMARKS.md](docs/BENCHMARKS.md)); Linux e macOS non sono provati. La prova che il vuoto è reale:
> [indagine di mercato](docs/MARKET.md) e [benchmark riproducibili](docs/BENCHMARKS.md) di quattro programmi su un file da 6 GB.
> 🇬🇧 [Read in English](README.md)

---

## Perché

I file di testo enormi (log, dump SQL, esportazioni CSV, XML) sono routine, e gli strumenti si dividono in due campi:

- **Editor che caricano tutto in RAM.** Veloci e semplici, ma il file deve entrare in memoria. Abbiamo misurato
  **Notepad++ a 6,3 GB di RAM** per un file da 6 GB e **Microsoft Edit che tronca in silenzio un file da 6 GB a 4 GiB: salvando
  si sono persi 1,7 GB di dati, senza alcun avviso.**
- **Viewer e editor a flusso.** Memoria minima, ma il costo si sposta altrove: **LTFViewer** (43 MiB) legge tutto il file
  all'apertura (≈ 86 s, limitato dalla CPU); **UltraEdit** (19 MiB) scrive una copia temporanea completa e rilegge tutto il
  file a ogni salto a fine file. I viewer di log da terminale (lnav, logana, bvr, klogg) sono ottimi ma **di sola lettura**.

Non volevamo fare "l'ennesimo tool che esiste già". Abbiamo cercato uno strumento leggero in memoria, economico da
navigare **e** modificabile, e non l'abbiamo trovato ([indagine](docs/MARKET.md)).

## Cosa sarà

- **Si apre come un viewer, si modifica quando lo chiedi.** Navigazione istantanea in sola lettura; premi *Edit* e lo
  stesso file diventa modificabile, senza ricaricarlo né copiarlo.
- **Memoria indipendente dalla dimensione del file**: obiettivo, decine di MiB.
- **Non perde mai dati.** Offset a 64 bit ovunque, test su file oltre 4 GiB fin dal primo giorno, errore esplicito invece di troncamento silenzioso.
- **Modalità colonna esplicita**, come tipo di selezione del core, condivisa da tutte le interfacce.
- **Codifiche rispettate**: UTF-8 e 33 code page a byte singolo (Windows-125x, ISO-8859-x, DOS 437/850/…, KOI8, Mac Roman) anche su file più grandi della RAM, scelte con `F9` o `--encoding`; UTF-16 è ancora da fare.
- **Operazioni a flusso su tutto il file** (sostituisci tutto, ordina, dividi), con avanzamento e annulla.
- **Multipiattaforma**: Windows, Linux, macOS.
- **Un solo core, più interfacce**: prima da terminale (TUI), poi GUI.

Non-obiettivi: analizzatore di log (esistono lnav, logana, klogg) e piattaforma a plugin.

## Misurato su un file da 6,0 GB (60 M righe)

| | Memoria | Apertura | Note |
|---|---|---|---|
| Microsoft Edit 1.2.1 | 4.100 MiB | 2,8 s | **tronca a 4 GiB, perde 1,7 GB al salvataggio** |
| Notepad++ 8.8.1 | 6.288 MiB | 7 s a caldo / 26–30 s a freddo | tiene tutto in RAM |
| LTFViewer 5.2u | 43 MiB | UI 0,5 s | scansione completa in background, ~86 s |
| UltraEdit-32 10.20.3 | 19 MiB | 7,5 s | scrive una copia temporanea da 5,7 GB; `Ctrl+End` rilegge il file (21,6 s) |
| **Core di Manto.RustPad** (solo libreria) | 19–28 MiB | indice righe di tutto il file in 38 s a freddo / 1,3 s a caldo | esatto, nessun taglio, nessuna copia; il tempo a freddo coincide con la velocità sequenziale del disco |

Dettagli, metodo e limiti: [docs/BENCHMARKS.md](docs/BENCHMARKS.md) (in inglese). Una sola macchina, per lo più una misura per cella.

## Tappe

1. **Core**: sorgente file mappato in memoria, indice righe sparso, ricerca annullabile (modalità viewer). *Fatto per la libreria: `rust/crates/mrp-core`.*
2. **Modifica**: piece table sopra il file originale *(fatto: `PieceTable`)*; salvataggio a flusso su file temporaneo con rinomina atomica *(fatto: `Document::save`)*; annulla/ripeti e indice righe che segue le modifiche *(fatto)*.
3. **Interfaccia TUI** (ratatui), tastiera classica, mouse come scorciatoia *(prima versione fatta: `mrp`, lettura/modifica/ricerca/vai a/temi/lingue; misurata una volta su un file da 6 GB; da fare: SSD, Linux/macOS)*.
4. **Modalità colonna** *(fatta nel core e in `mrp`: `F8`; non ancora provata con una tastiera vera)* e **codifiche** *(fatte le code page a byte singolo: `F9`, `--encoding NOME`, riconoscimento all'apertura; da fare UTF-16 e le codifiche multibyte dell'Asia orientale)*.
5. **Interfaccia GUI.**

Ogni tappa è verificata con la stessa suite di benchmark (`bench/`), così le affermazioni restano falsificabili.

## Provarlo (Windows)

```
cd rust
cargo build --release -p mrp-tui
.\install.ps1                  # facoltativo: mrp.exe -> %USERPROFILE%\.local\bin,
                               # lingue -> %APPDATA%\mrp\lang
mrp tuofile.txt                # F1 = aiuto, F4 = modifica, F10 = esci
```

`install.ps1` non tocca mai il `PATH` (ti avvisa soltanto se la cartella non c'è) né le impostazioni salvate.
Senza installare: `cargo run --release -p mrp-tui -- tuofile.txt`.

**Portatile:** `.\portable.ps1` (dopo la build) prepara una cartella con `mrp.exe` e `lang\`; si copia dove si vuole. Finché la cartella `lang` sta accanto all'exe, lingue e impostazioni salvate (`settings.txt`) restano in quella cartella e nel profilo non si scrive nulla.

## Licenza

Copyright (C) 2026 Francesco Mantovanelli.

Questo programma e' software libero: potete ridistribuirlo e/o modificarlo nei
termini della **GNU Affero General Public License v3.0** pubblicata dalla Free
Software Foundation — vedete [LICENSE](LICENSE). Per prodotti che non possono
essere distribuiti in AGPL e' disponibile, su richiesta, un canale di
licenziamento commerciale.

---

<div align="center">

**Autore** — Francesco Mantovanelli · **Progetto** — Manto.RustPad

</div>
