# Manto.RustPad — Manuale

> **Stato: prima bozza.** Scritto finora: la panoramica del prodotto, le carte di apertura e navigazione, modi e salvataggio,
> tempo e filtri, e le note "sotto il cofano" per il salvataggio automatico, il salvataggio, l'indice delle righe, il tempo,
> le codifiche, la ricerca e le cache. Ancora da fare: le carte rimanenti (codifiche e colonne, dettagli delle schede,
> impostazioni, temi, aiuto). Le funzionalita' sono congelate per la prima release. Questo documento descrive cio' che il
> prodotto fa oggi.

## 0. Come si legge questo documento

Manto.RustPad e' un visualizzatore/editor per file che non stanno in RAM, con due facce sullo stesso motore: **`mrp-tui`**, l'editor
da terminale (server, SSH), e **`mrp-gui`**, la finestra del desktop. Entrambi disegnano la stessa griglia di testo e condividono
ogni tasto, tranne dove un capitolo dice diversamente.

Ogni funzionalita' ha una **carta** con gli stessi campi:

| Campo | Significato |
|---|---|
| **Ingresso** | Come la raggiungi: tasto, menu', click. *Terminale* e *Finestra* quando differiscono. |
| **Cosa fa** | Il comportamento in parole semplici. |
| **Risposta attesa** | Cosa l'interfaccia deve mostrare o fare. |
| **Non deve** | Cosa non deve mai succedere (perdita di dati, blocchi, un modo sbagliato...). |
| **Limiti e casi limite** | File grandi, permessi mancanti, modifiche non salvate, schermi stretti. |
| **Test automatici** | I test automatici che gia' la coprono, o *nessuno*. |

I tasti sono scritti uguali per Windows, Linux e macOS (`Ctrl`, `Alt`). Nel terminale, `Alt+lettera` richiede un terminale che invii
Alt come prefisso di escape (la maggior parte lo fa).

---

## 1. Il prodotto in una pagina

**Tre modi di usare un file.** Si escludono a vicenda: esattamente uno e' acceso.

| Modo | Tasti | Cos'e' |
|---|---|---|
| **VIEW** | `Alt+V` | Sola lettura. Il predefinito. |
| **EDIT** | `Alt+E` | Modifica il file. Niente tocca il disco finche' non salvi. |
| **TAIL** | `Alt+T` | Sola lettura, e la vista segue la fine del file mentre cresce (`tail -f`). |

Il modo corrente e' sempre l'ultima parola della riga di stato (`VIEW`, `EDIT`, `TAIL`, con `COL` dopo in modalita' colonna) ed e'
acceso nel footer. Un click sulla parola, o `Alt+M`, apre un piccolo menu' dei tre modi. Nella finestra la barra degli strumenti ha
tre pulsanti adiacenti, uno acceso.

**Lo schermo del terminale**, dall'alto in basso:

1. *(solo con piu' di una scheda)* la barra delle schede;
2. il testo, con il margine dei numeri di riga a sinistra;
3. la **riga di stato**: posizione (`Ln 12  Col 3  byte 1234  7%`) o un messaggio a sinistra; a destra il lavoro in corso
   (`indexing 45%`), il nome del file (` *` dopo quando e' modificato), la dimensione, la codifica se non e' UTF-8, il fine riga se
   non e' LF, e il modo;
4. il **footer**: `Alt+V View  Alt+E Edit  Alt+T Tail  F1 Help  F2 Save  F3 Find  F4 Repl  F5 Go ...  F10 Quit`. Su un terminale
   stretto i tasti F meno usati cedono per primi (F6, F7, F8, F9, poi F5); i tre modi mai.
**Tasti standard, gli stessi nel terminale e nella finestra:** `F1` aiuto, `F2` salva, `F3` cerca (`Shift+F3` precedente), `F4`
sostituisci, `F5` vai a, `F6` tema, `F7` lingua, `F8` modalita' colonna, `F9` codifica, `F10` esci, `Esc` chiude una finestra.
`Ctrl+Z`/`Ctrl+Y` annulla/ripeti, `Ctrl+C`/`X`/`V` appunti, `Ctrl+A` seleziona tutto, `Ctrl+O` apri, `Ctrl+N` nuovo, `Ctrl+W` chiudi scheda.
L'aiuto ha due tab nella finestra (Tasti, e *Info* con la versione, la licenza e l'indirizzo GitHub come link vero) e, nel
terminale, le stesse tre righe chiudono la tabella dei tasti: versione, AGPL-3.0, codice.

![La finestra dell'aiuto: il tab Tasti](img/manual/h-aiuto.png)

**La finestra** ha un menu' (File, Modifica, Vista, Log, Aiuto), una barra degli strumenti (Nuovo, Apri, Salva | Annulla, Ripeti | Taglia, Copia, Incolla |
Cerca, Sostituisci, Vai a | Timeline, Filtro tempo | VIEW, EDIT, TAIL, Modalita' colonna, Barra salti | Zoom), le schede, una scrollbar e la stessa

![La finestra delle impostazioni](img/manual/h-impostazioni.png)
riga di stato del terminale.

---

## 2. Le carte delle funzionalita'

### 2.1 Apertura e navigazione

#### N-01 Aprire un file dalla riga di comando

![Il log aperto nella finestra: il testo colorato, la riga di stato, il footer](img/manual/n01-apre.png)
- **Ingresso:** `mrp-tui FILE` / `mrp-gui FILE`. Senza file, torna l'ultima sessione (vedi C1); senza sessione, un "Untitled 1" vuoto.
- **Cosa fa:** mostra la prima schermata subito, qualunque sia la dimensione. Il file viene mappato, non letto; i numeri di riga si
  contano in background (vedi C3).
- **Risposta attesa:** prima schermata immediata, modo `VIEW`, posizione `Ln 1  Col 1  byte 0  0%`, nome e dimensione del file a destra.
  Mentre l'indice si costruisce: `indexing N%` a destra, `Ln ?` a sinistra, e il margine dei numeri di riga vuoto.
- **Non deve:** caricare il file intero; bloccare lo schermo durante l'indicizzazione; chiedere alcunche'.
- **Limiti e casi limite:** **su Windows, mentre un file e' aperto qui, altri programmi possono aggiungerci dati, rinominarlo o cancellarlo, ma non troncarlo** (aprirlo in scrittura con troncamento, o accorciarlo, fallisce con un errore di sistema: il file e' memory-mappato). Un servizio che azzera il suo log ricevera' quell'errore mentre qualcuno lo sta guardando; aggiungere, ruotare con un rename e cancellare vanno bene (testato). Un file mancante da' un errore e nessuna finestra; un file vuoto si apre con 0 righe; un file che e' una sola
  riga enorme scorre lo stesso (le righe si tagliano a 128 KiB); un file di 64 MiB o piu' mostra una barra di avanzamento dopo 2 s (N-08).
- **Test automatici:** si' (`app_view`, `cli`, `pausa_indice`, `utf16_core`...).
