<table>
  <tr>
    <td valign="middle">
      <h1>Manto.RustPad</h1>
      <h3>L'editor per i file che non stanno in RAM</h3>
      <p><strong><em>Un editor di testo in Rust, con comandi familiari (non modale), la cui memoria non dipende dalla dimensione del file, che non tronca mai i dati in silenzio e che apre un file da molti gigabyte come un viewer, diventando modificabile a richiesta.</em></strong></p>
      <p>
        <img src="https://img.shields.io/badge/SI%20APRE-SUBITO-e3b341?style=for-the-badge" alt="si apre subito" />
        <img src="https://img.shields.io/badge/MEMORIA-NON%20DIPENDE%20DAL%20FILE-e3b341?style=for-the-badge" alt="memoria indipendente dalla dimensione del file" />
        <img src="https://img.shields.io/badge/NON-TRONCA%20MAI-e3b341?style=for-the-badge" alt="non tronca mai" />
      </p>
      <p>
        <a href="https://github.com/heavymanto/Manto.RustPad/actions/workflows/ci.yml"><img src="https://github.com/heavymanto/Manto.RustPad/actions/workflows/ci.yml/badge.svg" alt="CI" /></a>
        <img src="https://img.shields.io/badge/test-1133-brightgreen" alt="1133 test" />
        <img src="https://img.shields.io/badge/programmi-mrp--tui%20%C2%B7%20mrp--gui-blue" alt="mrp-tui e mrp-gui" />
        <img src="https://img.shields.io/badge/lingue-7-blue" alt="sette lingue" />
        <img src="https://img.shields.io/badge/licenza-AGPL--3.0-brightgreen" alt="AGPL-3.0" />
        <img src="https://img.shields.io/badge/stato-sviluppo%20iniziale-orange" alt="sviluppo iniziale" />
      </p>
      <p><sub>📖 <a href="docs/BENCHMARKS.md"><b>Le misure</b></a> · 🔧 <a href="#avvio-rapido"><b>Avvio rapido</b></a> · 🇬🇧 <a href="README.md"><b>Read it in English</b></a></sub></p>
    </td>
  </tr>
</table>

<div align="center">
<img src="docs/img/gui-light.png" alt="mrp-gui, la finestra: barra dei menu, barra degli strumenti, due schede, numeri di riga, barra di stato e scorrimento" width="80%" />
<br /><sub><b>mrp-gui</b> — la finestra: menu, barra degli strumenti, schede, barra di stato e scorrimento, con un log e un dump SQL aperti in due schede.</sub>
</div>

---

## L'idea

I file di testo enormi (log, dump SQL, esportazioni CSV, XML) sono routine, e gli strumenti si dividono in due campi:

- **Editor che caricano tutto in RAM.** Veloci e semplici, ma il file deve entrare in memoria. Abbiamo misurato
  **Notepad++ a 6,3 GB di RAM** per un file da 6 GB e **Microsoft Edit che tronca in silenzio un file da 6 GB a 4 GiB: salvando
  si sono persi 1,7 GB di dati, senza alcun avviso.**
- **Viewer e editor a flusso.** Memoria minima, ma il costo si sposta altrove: **LTFViewer** (43 MiB) legge tutto il file
  all'apertura (≈ 86 s, limitato dalla CPU); **UltraEdit** (19 MiB) scrive una copia temporanea completa e rilegge tutto il
  file a ogni salto a fine file. I viewer di log da terminale (lnav, logana, bvr, klogg) sono ottimi ma **di sola lettura**.

Non volevamo fare "l'ennesimo tool che esiste già". Abbiamo cercato uno strumento leggero in memoria, economico da
navigare **e** modificabile, e non l'abbiamo trovato ([indagine](docs/MARKET.md)). Così è nato **Manto.RustPad**: apre il file
mappato in memoria e in sola lettura, costruisce in background un indice righe "sparso", e appena premi *Modifica* lo stesso file
diventa modificabile, senza ricaricare e senza copie. Le tue modifiche stanno in una piccola *piece table* accanto all'originale
intatto; salvando, il programma scrive un file nuovo lì accanto e lo scambia in modo atomico.

## Misurato su un file da 6,0 GB (60 M righe)

| | Memoria | Apertura | Note |
|---|---|---|---|
| Microsoft Edit 1.2.1 | 4.100 MiB | 2,8 s | **tronca a 4 GiB, perde 1,7 GB salvando** |
| Notepad++ 8.8.1 | 6.288 MiB | 7 s a caldo / 26–30 s a freddo | tiene tutto in RAM |
| LTFViewer 5.2u | 43 MiB | interfaccia 0,5 s | scansione completa in background, ~86 s |
| UltraEdit-32 10.20.3 | 19 MiB | 7,5 s | scrive una copia temporanea da 5,7 GB; `Ctrl+Fine` rilegge il file (21,6 s) |
| **Manto.RustPad, motore** (solo la libreria) | 19–28 MiB | indice righe dell'intero file in 38 s a freddo / 1,3 s a caldo | esatto, senza troncamenti né copie; il tempo a freddo coincide con la velocità sequenziale del disco |

L'editor vero e proprio apre subito un file da 6 GB e tiene circa 13–35 MiB di memoria privata, anche durante il salvataggio.
La memoria cresce piano con il file (circa 3,7 MB per GB: ~410 MiB su un file sparso da 100 GiB, il cui working set resta sotto i 20 MiB).
**Una macchina, un disco meccanico, poche misure**: metodo, script e avvertenze in [docs/BENCHMARKS.md](docs/BENCHMARKS.md).

## Scaricalo

Niente da installare: estrai e avvia. Entrambi i programmi funzionano subito, in modo *portatile*: l'inglese è incorporato, le altre
sei lingue si trovano in `lang/` accanto ai programmi; impostazioni e sessione salvata restano nella stessa cartella. Li vuoi come comandi da qualunque cartella? Ogni pacchetto contiene `install.ps1` (Windows) o `install.sh` (Linux, macOS): copia `mrp-tui` e `mrp-gui` in `~/.local/bin` (e su Windows lo aggiunge al PATH utente) e le lingue nel tuo profilo.

[![Windows x64](https://img.shields.io/badge/%E2%AC%87-Windows%20x64-2ea44f?style=for-the-badge)](../../releases/latest)
[![Linux x64](https://img.shields.io/badge/%E2%AC%87-Linux%20x64-2ea44f?style=for-the-badge)](../../releases/latest)
[![macOS](https://img.shields.io/badge/%E2%AC%87-macOS-2ea44f?style=for-the-badge)](../../releases/latest)

I pacchetti (uno ZIP per Windows, un tar.gz per Linux e macOS, ciascuno con il suo SHA256) li costruisce il
[workflow di release](.github/workflows/release.yml) quando si pubblica il tag di una versione. Finché non esce la prima release,
compilalo dai sorgenti: bastano due comandi, vedi l'[Avvio rapido](#avvio-rapido).

---

## Due programmi, un solo motore

| `mrp-tui` — nel terminale | `mrp-gui` — in una finestra |
|:---:|:---:|
| ![mrp-tui: il dialogo Sostituisci con l'anteprima, nel terminale](docs/img/tui-replace.png) | ![mrp-gui: la finestra, tema scuro](docs/img/gui-dark.png) |

| | `mrp-tui` | `mrp-gui` |
| --- | --- | --- |
| **Cos'è** | un editor a tutto schermo per qualsiasi terminale (ratatui), pensato per server e SSH | un'applicazione desktop (egui): menu, barra degli strumenti, barra di stato, scorrimento e dialoghi veri, gli appunti di sistema, i caratteri CJK e il trascinamento dei file |
| **Avvio** | `mrp-tui file...` | `mrp-gui file...` (senza file riprende l'ultima sessione, o apre un "Senza nome 1" vuoto) |
| **Menu** | i tasti funzione, mostrati nel piede (`F1` = aiuto) | una barra dei menu (File, Modifica, Visualizza, Aiuto), una barra degli strumenti con le icone, il menu del tasto destro e i dialoghi nativi di apertura e salvataggio |
| **Schede** | una barra sulla prima riga, una scheda per file | una striscia con il pulsante di chiusura per ogni scheda |
| **Aspetto** | quattro temi da terminale | sei temi; segue l'impostazione chiaro / scuro del sistema finché non ne scegli uno |
| **Il mouse** | posiziona il cursore, seleziona (doppio clic: parola, triplo: riga), sceglie la scheda | lo stesso, e il clic centrale chiude una scheda |

Condividono **un solo motore**: la logica dell'editor (documenti, schede, ricerca, modifiche, sessioni) sta in un posto solo; `mrp-gui` disegna il testo come la stessa griglia di celle, ma la avvolge nei propri controlli nativi. Una funzione aggiunta al motore c'è in entrambi.

## Cosa fa

- **Si apre come un viewer, si modifica a richiesta.** Navigazione istantanea, mappata in memoria, in sola lettura; premi `F4` e lo stesso file diventa modificabile.
- **Memoria indipendente dalla dimensione del file.** Decine di MiB, qualunque sia il file. Mai l'intero file in memoria.
- **Non perde mai dati.** Offset a 64 bit ovunque, test su file oltre i 4 GiB, errore chiaro invece di troncamento silenzioso, salvataggio a prova di interruzione.
- **Schede.** Più file insieme, ciascuno con la propria ricerca, cronologia delle modifiche e cursore. `Ctrl+O` apre (un file già aperto torna solo in primo piano),
  `Ctrl+W` chiude (chiede se ci sono modifiche), `Ctrl+PgSu/PgGiù`, `Ctrl+Tab` / `Ctrl+Maiusc+Tab` e `Alt+←/→` cambiano scheda, `Alt+1..9` vanno a quella col numero.
- **Chiudere l'editor non fa perdere niente.** Esci con `F10`, `Ctrl+Q` o il pulsante di chiusura della finestra *senza che ti venga chiesto nulla*: alla volta dopo tornano
  le schede, i cursori e le **modifiche non salvate**. Si salvano solo le modifiche, mai una copia del file: su un file da 100 GB con poche modifiche la sessione pesa pochi KB.
  Se nel frattempo il file è cambiato sul disco, la scheda si apre pulita, lo dice, e la tua copia resta da parte.
- **Cerca e Sostituisci con glob e regex.** `Ctrl+F` / `F3` cercano; `Ctrl+H` sostituisce. Tre modi — **testo** semplice, **glob** (`*`, `?`, `[abc]`)
  e **regex** (con `$1` e `${nome}` nella sostituzione) — più maiuscole/minuscole e un ambito: questa scheda o **tutte le schede aperte**.
  Il primo `Invio` mostra l'**anteprima** (*"9 occorrenze in 2 scheda/e"*, senza toccare niente), il secondo applica, e un solo `Ctrl+Z` annulla tutto.
- **Modalità colonna (a blocchi)**, un tipo di selezione di prima classe nel motore (`F8`), anche col mouse: `Alt`+trascinamento disegna il blocco (e accende la modalità); con la modalità accesa basta trascinare.
- **Filtra le righe** (`Alt+G`, come `grep`): mostra solo le righe che corrispondono (testo, glob o regex, con o senza maiuscole; `Alt+I` mostra quelle che NON corrispondono) in una scheda nuova di sola lettura `[filtro] testo`. Gira in background su un file di qualsiasi dimensione (avanzamento nella riga di stato, `Esc` annulla), non tiene mai in memoria più di un blocco, e `Invio` su un risultato salta a quella riga nel file vero.
- **Colori dei log** (`Alt+L`, spenti di fabbrica): il livello di una riga (ERROR/FATAL in rosso, WARN in giallo, INFO in verde, DEBUG in grigio), le date e gli orari e gli indirizzi IPv4 hanno ciascuno il proprio colore in ogni tema; le righe di errore e di avviso si tingono per intero. Si trovano scorrendo a mano il solo testo visibile, quindi non costano nulla su un file enorme. ![colori dei log](docs/img/gui-log-colors.png)
- **Salta a un orario** (`Ctrl+G`, poi `~03:20` o `~2026-10-31 09:00`): in un log con gli orari che solo crescono, una ricerca binaria sui byte trova la prima riga da quell'orario in poi in poche decine di letture, senza indice, anche in un file di centinaia di GB. Le righe senza orario (tracce, intestazioni) si attraversano.
- **Segui la fine di un file** (`Alt+F`, come `tail -f`): un log che continua a crescere si segue fino alla sua nuova fine due volte al secondo, senza rileggere niente (il file si rimappa e i numeri di riga si allungano contando solo i byte nuovi). Salire con un tasto o con la rotella lo ferma; non si combina con la modifica.
- **Caratteri speciali visibili** (`Alt+W`): spazi come `·`, tabulazioni come `→`, fine riga come `¶`, caratteri di controllo come `¤`; le colonne e le posizioni del mouse non si spostano. Si salva con le impostazioni.
- **Onesto con le codifiche.** UTF-8, UTF-16 e UTF-32 (little e big endian, coppie surrogate comprese), 53 tabelle di caratteri a byte singolo
  (Windows-125x e 874, ISO-8859-x, DOS 437/850/855/…, KOI8, i set Mac, US-ASCII) e le codifiche asiatiche a più byte (Shift_JIS, EUC-JP,
  GBK, GB18030, Big5, EUC-KR), su file più grandi della RAM. Si scelgono con `F9` o `--encoding`, o si riconoscono all'apertura (quelle a più byte non si indovinano mai: si scelgono a mano).
- **Sei temi e sette lingue** (English, Deutsch, Español, Italiano, 日本語, Português do Brasil, 简体中文), che si cambiano al volo con `F6` / `F7`.

Fuori dagli obiettivi: un analizzatore di log (usa lnav, logana, klogg: gli aiuti per i log che ci sono qui — seguire, colori, salto a un orario, filtro — sono quelli che servono a un lettore, non un'analisi) e una piattaforma di plugin/IDE.

### I temi

| | |
|:---:|:---:|
| ![night](docs/img/gui-night.png) | ![debian](docs/img/gui-debian.png) |
| `night` | `debian` |
| ![console](docs/img/gui-console.png) | ![minimal](docs/img/gui-minimal.png) |
| `console` (il predefinito nel terminale) | `minimal` |
| ![dark](docs/img/gui-dark.png) | ![light](docs/img/gui-light.png) |
| `dark` (scuro) | `light` (chiaro) |

L'immagine del terminale in *Due programmi* mostra Sostituisci con la sua anteprima prima che cambi qualcosa (`Ctrl+H`, poi `Alt+M` per il modo, `Alt+C` per le maiuscole, `Alt+A` per l'ambito).
<sub>È generata dal testo dell'opzione `--dump` dell'editor, quindi è senza colori; il terminale vero usa quelli del tema.</sub>

---

## Avvio rapido

Servono [Rust](https://rustup.rs) (`cargo --version`) e una copia di questo repository.

```sh
cd rust
cargo build --release                 # mrp-tui e mrp-gui in target/release/
./target/release/mrp-tui ilmiofile.txt    # F1 = aiuto, F4 = modifica, F10 = esci
./target/release/mrp-gui ilmiofile.txt   # lo stesso, in una finestra
```

Windows (PowerShell): `.\install.ps1` in `rust\` è facoltativo: copia `mrp-tui.exe` in `%USERPROFILE%\.local\bin` e i file delle lingue in
`%APPDATA%\mrp\lang`; non tocca mai il `PATH` né le impostazioni salvate. `.\portable.ps1` (dopo la compilazione) crea una cartella con i programmi e `lang\`:
copiala dove vuoi, e finché `lang` sta accanto ai programmi, lingue, impostazioni (`settings.txt`) e sessione restano in quella cartella e nel tuo profilo non si scrive nulla.

Per provare le funzioni a più file: `mrp-tui a.txt b.txt c.txt`, poi `Ctrl+H`, scrivi un modello, `Tab`, scrivi la sostituzione, `Alt+A` (tutte le schede), `Invio`.

## Limiti noti

- **Sviluppo iniziale.** L'editor funziona ed è coperto da 1133 test automatici (più 3 su file da 6 GB oltre i 4 GiB, da lanciare a richiesta), ma non è stato
  usato a lungo da molte persone. Tieni una copia di ciò che conta.
- **Prima di tutto Windows.** È stato usato soprattutto su Windows. L'editor da terminale e tutto il motore (con i loro test) sono stati provati anche su **Linux** (Debian, sotto WSL);
  la finestra `mrp-gui` su Linux/macOS e macOS in generale non sono provati. La CI lancia i test su tutti e tre i sistemi a ogni push.
- **La finestra (`mrp-gui`)** è stata verificata con test automatici e schermate, non ancora con lunghe sessioni di tastiera e mouse veri.
- **Regex, glob e "ignora maiuscole" funzionano solo su documenti UTF-8** (si applicano riga per riga, come grep: un'occorrenza non attraversa mai una riga). La ricerca di testo semplice funziona in ogni codifica.
  In "tutte le schede" una scheda che non può usare la regex viene saltata, e il dialogo lo dice.
- Una sola Sostituzione gestisce fino a **2.000.000** di occorrenze; una riga più lunga di 16 MiB si legge a pezzi, quindi un'occorrenza a cavallo del taglio può sfuggire.
- **La chiusura senza domande è sempre attiva** quando esiste una cartella del profilo o portatile (non c'è ancora un interruttore per spegnerla).
- Le misure vengono da **una macchina e un disco meccanico**: vedi [docs/BENCHMARKS.md](docs/BENCHMARKS.md).

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
| `rust/crates/mrp-core` | Il motore (Rust): `FileSource`, `LineIndex`, `PieceTable`/`Document` (modifica, annulla, salvataggio), cerca e sostituisci (`pattern`), record di sessione (`session`), codifiche, lingue |
| `rust/crates/mrp-tui` | L'editor da terminale `mrp-tui` (ratatui): `App` (un documento), `Workspace` (le schede e la sessione), schermo, temi, impostazioni, riga di comando |
| `rust/crates/mrp-gui` | La finestra `mrp-gui` (egui): la barra dei menu, la striscia delle schede e la griglia di celle dell'editor da terminale |
| `rust/install.ps1`, `rust/portable.ps1` | Installazione locale su Windows; una cartella portatile |
| `docs/` | `MARKET.md` (il panorama degli strumenti esistenti, con le fonti), `BENCHMARKS.md` (misure, metodo, limiti) |
| `bench/` | Script riproducibili (Windows, Python 3.12, senza dipendenze) e risultati grezzi |
| `.github/workflows/` | `ci.yml` (test su tre sistemi, formato, clippy) e `release.yml` (i pacchetti) |

## Documentazione

- [docs/MARKET.md](docs/MARKET.md) — il panorama degli strumenti esistenti, con le fonti
- [docs/BENCHMARKS.md](docs/BENCHMARKS.md) — misure, metodo, limiti
- [CHANGELOG.md](CHANGELOG.md) — cosa è cambiato
- [SECURITY.md](SECURITY.md) — come segnalare una vulnerabilità in privato

## Contribuire

Vedi [CONTRIBUTING.md](CONTRIBUTING.md). In breve: gli offset sono a 64 bit, l'intero file non sta mai in memoria e ogni numero nella
documentazione è stato davvero misurato. Le issue sono aperte — tre modelli (bug, domanda, funzione) portano il contesto. Una nota sulle aspettative: è un
progetto per hobby, tutto viene letto ma non si promettono tempi di risposta.

## Progetto gemello

[Manto.FreeDisk](https://github.com/heavymanto/Manto.FreeDisk) — un analizzatore dello spazio su disco in Rust; la divisione motore/CLI/TUI usata lì è il modello di questo repository.

## Licenza

Copyright (C) 2026 Francesco Mantovanelli.

Questo programma è software libero: puoi ridistribuirlo e/o modificarlo secondo i
termini della **GNU Affero General Public License v3.0**, pubblicata dalla Free
Software Foundation — vedi [LICENSE](LICENSE). Una licenza commerciale è disponibile
su richiesta per i prodotti che non possono essere distribuiti con l'AGPL.

---

<div align="center">

**Autore** — Francesco Mantovanelli · **Progetto** — Manto.RustPad

</div>
