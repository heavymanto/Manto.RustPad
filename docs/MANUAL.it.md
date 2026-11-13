# Manto.RustPad — Manuale

[English](MANUAL.md) · **Italiano**

I tasti sono scritti uguali per Windows, Linux e macOS (`Ctrl`, `Alt`). Nel terminale, `Alt+lettera` richiede un terminale che invii
Alt come prefisso di escape (la maggior parte lo fa).

## Indice

- [Il prodotto in una pagina](#il-prodotto-in-una-pagina)
- **[Parte prima — Il terminale (TUI)](#parte-prima--il-terminale-tui)**
  - [Apertura e navigazione](#apertura-e-navigazione)
  - [Modi, modifica e salvataggio](#modi-modifica-e-salvataggio)
  - [Tempo, timeline e filtri](#tempo-timeline-e-filtri)
  - [Codifiche e fine riga](#codifiche-e-fine-riga)
- **[Parte seconda — La finestra (GUI)](#parte-seconda--la-finestra-gui)**
  - [Apertura e navigazione](#apertura-e-navigazione-1)
  - [Modi, modifica e salvataggio](#modi-modifica-e-salvataggio-1)
  - [Tempo, timeline e filtri](#tempo-timeline-e-filtri-1)
  - [Codifiche e fine riga](#codifiche-e-fine-riga-1)
- [Sotto il cofano](#sotto-il-cofano)

## Il prodotto in una pagina

**Tre modi di usare un file.** Si escludono a vicenda: esattamente uno e' acceso.

| Modo | Tasti | Cos'e' |
|---|---|---|
| **VIEW** | `Alt+V` | Sola lettura. Il predefinito. |
| **EDIT** | `Alt+E` | Modifica il file. Niente tocca il disco finche' non salvi. |
| **TAIL** | `Alt+T` | Sola lettura, e la vista segue la fine del file mentre cresce (`tail -f`). |

Il modo corrente e' sempre l'ultima parola della riga di stato (`VIEW`, `EDIT`, `TAIL`, con `COL` dopo in modalita' colonna) ed e'
acceso nel footer. Un click sulla parola, o `Alt+M`, apre un piccolo menu' dei tre modi.

**Tasti standard, gli stessi nel terminale e nella finestra:** `F1` aiuto, `F2` salva, `F3` cerca (`Shift+F3` precedente), `F4`
sostituisci, `F5` vai a, `F6` tema, `F7` lingua, `F8` modalita' colonna, `F9` codifica, `F10` esci, `Esc` chiude una finestra.
`Ctrl+Z`/`Ctrl+Y` annulla/ripeti, `Ctrl+C`/`X`/`V` appunti, `Ctrl+A` seleziona tutto, `Ctrl+O` apri, `Ctrl+N` nuovo, `Ctrl+W` chiudi scheda.
L'aiuto ha due tab nella finestra (Tasti, e *Info* con la versione, la licenza e l'indirizzo GitHub come link vero) e, nel
terminale, le stesse tre righe chiudono la tabella dei tasti: versione, AGPL-3.0, codice.

---

## Parte prima — Il terminale (TUI)

**Lo schermo del terminale**, dall'alto in basso:

1. *(solo con piu' di una scheda)* la barra delle schede;
2. il testo, con il margine dei numeri di riga a sinistra;
3. la **riga di stato**: posizione (`Ln 12  Col 3  byte 1234  7%`) o un messaggio a sinistra; a destra il lavoro in corso
   (`indexing 45%`), il nome del file (` *` dopo quando e' modificato), la dimensione, la codifica se non e' UTF-8, il fine riga se
   non e' LF, e il modo;
4. il **footer**: `Alt+V View  Alt+E Edit  Alt+T Tail  F1 Help  F2 Save  F3 Find  F4 Repl  F5 Go ...  F10 Quit`. Su un terminale
   stretto i tasti F meno usati cedono per primi (F6, F7, F8, F9, poi F5); i tre modi mai.

![Il terminale dell'aiuto: il tab Tasti](img/tui/h-aiuto.png)

### Apertura e navigazione

#### N-01 Aprire un file dalla riga di comando

![Il log aperto nel terminale: il testo colorato, la riga di stato, il footer](img/tui/n01-apre.png)
- **Tasti:** `mrp-tui FILE`. Senza file, torna l'ultima sessione (vedi [*Salvataggio automatico e sessione*](../ARCHITECTURE.it.md#salvataggio-automatico-e-sessione-come-sopravvive-il-lavoro-non-salvato)); senza sessione, un "Untitled 1" vuoto.
- **Cosa succede:** prima schermata immediata, modo `VIEW`, posizione `Ln 1  Col 1  byte 0  0%`, nome e dimensione del file a destra.
  Mentre l'indice si costruisce: `indexing N%` a destra, `Ln ?` a sinistra, e il margine dei numeri di riga vuoto.
- **Cosa non deve mai succedere:** caricare il file intero; bloccare lo schermo durante l'indicizzazione; chiedere alcunche'.
- **Limiti:** **su Windows, mentre un file e' aperto qui, altri programmi possono aggiungerci dati, rinominarlo o cancellarlo, ma non troncarlo** (aprirlo in scrittura con troncamento, o accorciarlo, fallisce con un errore di sistema: il file e' memory-mappato). Un servizio che azzera il suo log ricevera' quell'errore mentre qualcuno lo sta guardando; aggiungere, ruotare con un rename e cancellare vanno bene (testato). Un file mancante da' un errore e nessuna finestra; un file vuoto si apre con 0 righe; un file che e' una sola
  riga enorme scorre lo stesso (le righe si tagliano a 128 KiB); un file di 64 MiB o piu' mostra una barra di avanzamento dopo 2 s (N-08).

#### N-02 Aprire un altro file, documento nuovo

- **Tasti:** `Ctrl+O`: un prompt "percorso del file da aprire", Invio conferma, Esc annulla.
  `Ctrl+N` apre un documento vuoto in una scheda nuova, in modalita' EDIT.
- **Cosa succede:** compare una scheda nuova ed e' attiva; aprire un file gia' aperto non lo duplica.
- **Cosa non deve mai succedere:** sostituire una scheda che ha modifiche non salvate.
- **Limiti:** aprire mentre la scheda attiva ha modifiche non salvate conserva quelle modifiche (si usa una scheda nuova).

#### N-03 Muoversi
- **Tasti:** frecce, `PgSu`/`PgGiu`, `Home`/`Fine`, `Ctrl+Home`/`Ctrl+Fine`, rotella del mouse.
- **Cosa succede:** la posizione nella riga di stato segue; la vista scorre quando il cursore esce dallo schermo.
- **Cosa non deve mai succedere:** leggere il file intero per muoversi; bloccarsi su una riga lunghissima.
- **La barra orizzontale (aggiunta l'11/11/2026):** una riga piu' larga della finestra mostra una barra sotto il testo (una riga,
  solo quando serve: una riga piu' larga della finestra, o gia' scorreta) col pollice che dice dov'e' la finestra sulla riga piu'
  larga visibile — prima, l'unico modo di scorrere era camminarci col cursore. Clicca o trascina il pollice (segue il puntatore
  per come lo si e' afferrato; un clic sulla pista lo centra li'): la finestra si muove e il cursore resta dov'e' (il prossimo
  movimento del cursore riporta la vista su di lui). `«` e `»`
  segnalano ancora i lati tagliati di una riga; per una riga oltre il limite di lettura il mondo della barra e' un limite
  inferiore onesto che cresce scorrendo.
- **Limiti:** su un file ancora in indicizzazione, i salti funzionano per byte e percentuale, non per riga (N-04); saltare
  mette in pausa le letture su disco dell'indicizzatore per 400 ms ([*Indice delle righe*](../ARCHITECTURE.it.md#indice-delle-righe-e-navigazione-senza-leggere-il-file)).

#### N-04 Vai a (`F5` / `Ctrl+G`)

![Il prompt Vai a](img/tui/n04-vai.png)
- **Tasti:** `F5` o `Ctrl+G`. Un prompt: "numero di riga, @byte, %percentuale o ~orario".
- **Cosa succede:** il cursore va li' e il bersaglio sta verso il mezzo dello schermo *(osservato: `500` -> `Ln 500`,
  `%50` -> `49%`, `@1000000` -> `byte 1000000`)*. `0` -> "not a valid position: 0"; una riga oltre la fine -> "line N is beyond the end (M lines)".
- **Cosa non deve mai succedere:** muovere il cursore su un bersaglio non valido; fermarsi dentro un carattere multibyte (torna al suo inizio).
- **Limiti:** la `@` di `@byte` si digita con AltGr su molte tastiere (italiana, tedesca, spagnola, francese...): funziona
  anche nel terminale su Windows (vedi M-03). Per riga serve l'indice (mentre si costruisce: un messaggio dice di aspettare e niente si muove);
  `@` e `%` non serve l'indice; `~orario` serve un formato del tempo riconosciuto (vedi [*Tempo*](../ARCHITECTURE.it.md#tempo-formati-salto-a-un-orario-intervallo-timeline)).

#### N-05 Salti di pagine e barra dei salti

![La barra dei salti: Home, Fine e i pulsanti delle pagine](img/tui/n05-salti.png)
- **Tasti:** `Alt+PgSu`/`Alt+PgGiu` saltano 10 pagine (`Maiusc` x100, `Ctrl` x1000); `Ctrl+B` mostra una barra con *Home*, *Fine* e i pulsanti
  dei passi di pagina.
- **Cosa succede:** la vista arriva dove il passo dice; la riga di stato mostra la nuova posizione.
- **Cosa non deve mai succedere:** bloccarsi su un file enorme.

#### N-06 Numeri di riga e indice delle righe
- **Tasti:** `Alt+N` mostra/nasconde i numeri di riga; `Alt+X` accende/spegne l'indice delle righe.
- **Cosa succede:** spegnere l'indice chiede prima "SPEGNERE L'INDICE DELLE RIGHE?", elencando cosa si perde (numeri di riga, vai a una riga,
  il totale delle righe, i salti veloci di pagina, il conteggio delle righe di una selezione) e cosa continua a funzionare; Invio conferma e i numeri
  spariscono (`Ln ?`), Esc annulla. Riaccenderlo fa ripartire il conteggio da zero senza chiedere.
- **Cosa non deve mai succedere:** cambiare il file.

#### N-07 Schede
- **Tasti:** `Ctrl+PgGiu`/`Ctrl+PgSu`, `Alt+←`/`Alt+→`, `Alt+1`..`9`, `Ctrl+W` chiude.
- **Cosa succede:** la scheda attiva cambia subito; chiudere una scheda con modifiche non salvate chiede prima.
- **Cosa non deve mai succedere:** chiudere in silenzio una scheda modificata senza sessione (con la sessione le modifiche restano, vedi [*Salvataggio automatico e sessione*](../ARCHITECTURE.it.md#salvataggio-automatico-e-sessione-come-sopravvive-il-lavoro-non-salvato)).

#### N-08 Avanzamento dei lavori lunghi sui file grandi
- **Tasti:** nessuno: parte da solo per indicizzazione, ricerca e salvataggio.
- **Cosa succede:** da 60 colonne `indexing 61% #######.....`; da 90 anche `0:01 left`; da 140 anche
  `3.45 GiB / 5.59 GiB  1.67 GiB/s`. Sotto i 2 s, o sotto i 64 MiB, solo la percentuale. La barra e' a spettro
  (temi a colori) *(osservato nei codici di escape)*.
- **Cosa non deve mai succedere:** bloccare lo schermo; mostrare una stima assurda (resta in silenzio quando il lavoro e' in pausa o non ha ancora una velocita').
- **Limiti:** il tempo che resta si nasconde sopra le 99 ore.

### Modi, modifica e salvataggio

#### M-01 I tre modi
- **Tasti:** `Alt+V` VIEW, `Alt+E` EDIT, `Alt+T` TAIL; `F4` *non* e' un modo (e' Sostituisci).

- **Cosa succede:** l'ultima parola della riga di stato cambia; il footer accende quella corrente; un messaggio spiega il cambio
  ("edit mode: the file is not touched until you save", "TAIL: following the end of the file", "view mode").
- **Cosa non deve mai succedere:** stare in due modi insieme; avviare TAIL su un documento modificato (vedi M-08).

#### M-02 Il menu dei modi

![Il menu dei tre modi](img/tui/m02-modi.png)
- **Tasti:** `Alt+M`, o un clic sulla parola del modo nella riga di stato.
- **Cosa succede:** Su/Giu muovono, Invio sceglie, Esc chiude.
- **Cosa non deve mai succedere:** cambiare modo senza Invio.

#### M-03 Digitare e modificare
- **Tasti:** in EDIT: caratteri, `Invio`, `Tab`, `Backspace`, `Canc`; selezione con `Maiusc`+frecce o il mouse; `Ctrl+C`/`X`/`V`.
- **Cosa succede:** la riga di stato mostra ` *` dopo il nome del file; il testo cambia subito.
- **AltGr (Windows, corretto l'8/11/2026):** sulle tastiere che usano AltGr (italiana, tedesca, spagnola, francese, portoghese...) i caratteri
  `@ [ ] { } ~ # \` \\ |` ed `€` arrivano al terminale come Ctrl+Alt+carattere; il programma li riporta in testo piano (Ctrl+Alt+lettera o cifra,
  Alt+tasto e Ctrl+tasto restano scorciatoie). Funziona in ogni campo di testo (modifica, Cerca, Sostituisci, Filtri, Vai a `@byte`). Prima della correzione
  venivano scartati: `a@b{c}[d]` dava `abcd`.
- **Cosa non deve mai succedere:** toccare il file su disco prima di un salvataggio. In VIEW, digitare e' rifiutato col suggerimento **"read-only: press Alt+E to edit"**.

#### M-04 Annulla e ripeti
- **Tasti:** `Ctrl+Z`, `Ctrl+Y` (`Ctrl+Maiusc+Z`).
- **Cosa succede:** il testo torna/avanza a passi interi.
- **Cosa non deve mai succedere:** funzionare in VIEW (rifiutato col suggerimento di digitare, "read-only: press Alt+E to edit"); sopravvivere a un salvataggio (dopo un salvataggio la storia e' vuota).
- **Limiti:** la storia non si tiene nella sessione: una scheda ripristinata si puo' salvare ma non annullare ([*Salvataggio automatico e sessione*](../ARCHITECTURE.it.md#salvataggio-automatico-e-sessione-come-sopravvive-il-lavoro-non-salvato)).

#### M-05 Salva e Salva con nome
- **Tasti:** `F2` / `Ctrl+S`; `Ctrl+Maiusc+S` Salva con nome (chiede il nome, poi la codifica). Entrambi funzionano anche in VIEW quando ci sono modifiche.
- **Cosa succede:** il messaggio "saved: NOME" e nessun ` *`. Su un file grande un dialogo con barra di avanzamento, Esc annulla *(osservato: `saved: ux_copy.log`)*.
- **Cosa non deve mai succedere:** lasciare un file scritto a meta', mai, qualunque l'interruzione; sovrascrivere un file che un altro programma ha cambiato da quando l'hai aperto.
- **Limiti:** niente da salvare -> il messaggio "nothing to save"; file cambiato su disco -> rifiutato col motivo, le modifiche restano;
  cartella di sola lettura o disco pieno -> un errore, l'originale intatto, nessun temporaneo lasciato.

#### M-06 Scarta le modifiche (`Ctrl+R`)
- **Tasti:** `Ctrl+R` (o `Alt+R`).
- **Cosa succede:** una domanda che nomina il file; Invio/S/Y conferma e la scheda si riapre in VIEW vicino alla vecchia posizione; Esc/N tiene le modifiche.
- **Cosa non deve mai succedere:** toccare il file su disco. Con niente da scartare: il messaggio **"no unsaved changes to discard"** e nessun dialogo.

#### M-07 Uscire con modifiche non salvate
- **Tasti:** `F10` / `Ctrl+Q`.
- **Cosa succede (senza sessione):** una finestra "UNSAVED CHANGES": `S` Salva ed esci, `D` Scarta le modifiche, `C` Annulla.
  "Salva ed esci" esce solo se il salvataggio e' riuscito.
- **Cosa non deve mai succedere:** perdere modifiche in silenzio.

#### M-08 Tail (segue la fine)

- **Tasti:** `Alt+T`.
- **Cosa succede:** `TAIL` nella riga di stato e "TAIL: following the end of the file".
- **Cosa non deve mai succedere:** partire su un documento con modifiche non salvate: resta dov'e' e dice **"save or discard the changes (F2, Ctrl+R) before TAIL"**.
- **Limiti:** da EDIT senza modifiche passa direttamente a TAIL; un file che si accorcia smette di seguire, con un messaggio.

### Tempo, timeline e filtri

#### T-01 Formato dell'orario delle righe (`Alt+O`)

![Il terminale Tempo: il formato delle righe](img/tui/t01-formato.png)
- **Tasti:** `Alt+O`; automatico al primo uso.
- **Cosa succede:** una finestra "TIME FORMAT" col formato, entro quanti caratteri dall'inizio cercare, i formati
  noti con un esempio, e un'anteprima delle prime righe come `testo => 2026-01-01 00:00:28`. Tutto cio' che dipende
  dal tempo (orario in riga di stato, timeline, vai a un orario, intervallo, colori) segue il formato scelto.
- **Cosa non deve mai succedere:** leggere il file intero (il rilevamento usa al piu' 64 KiB di testa e 64 KiB di coda); cambiare il file.
  Un token sbagliato (`yyyyy`) dà un errore "bad token" e lascia il formato com'era.
- **Limiti:** i file UTF-16/UTF-32 non vengono mai riconosciuti, quindi non hanno funzioni del tempo. Le maschere senza anno (syslog)
  usano l'anno 2000. Orari che tornano indietro rompono l'assunzione dietro le ricerche (vedi [*Tempo*](../ARCHITECTURE.it.md#tempo-formati-salto-a-un-orario-intervallo-timeline)).

#### T-02 Vai a un orario (`F5`, poi `~orario`)
- **Tasti:** il prompt di vai a con un `~` davanti all'orario: `~03:20`, `~2026-10-31 05:00:00`.
- **Cosa succede:** il cursore e la vista arrivano li' *(osservato: `~03:20` su un log con orari dalle 00:00 arriva alla riga delle 03:22:55, `Ln 283`)*.
  Un bersaglio prima di tutto resta sulla prima riga.
- **Cosa non deve mai succedere:** leggere il file linearmente; fermarsi su una riga con un orario piu' piccolo.
- **Limiti:** serve un formato riconosciuto e orari che non tornano mai indietro; le righe senza orario si saltano.

#### T-03 Intervallo di tempo (`Alt+I`)

![Il terminale Tempo col calendario del campo Dal](img/tui/t03-intervallo.png)
- **Tasti:** `Alt+I`.
- **Cosa succede:** le righe fuori non si vedono; la riga di stato dice l'intervallo; la timeline lo mostra.
- **Cosa non deve mai succedere:** cambiare il file; confondersi con la finestra di tempo della scheda (sono due cose: quella e' un filtro, questa limita la vista della scheda).
- **Limiti:** un intervallo scritto a mano con orari che il file non copre viene rifiutato dicendo il primo e l'ultimo orario
  del file.

#### T-04 Timeline (`Alt+B`)

![La timeline: il file nel tempo, accanto alla scrollbar](img/tui/t04-timeline.png)
- **Tasti:** `Alt+B` (la scelta si ricorda).
- **Cosa non deve mai succedere:** leggere il file intero per ogni periodo (galoppa in avanti e bisetta); bloccare lo schermo (si costruisce in un thread in background
  che apre una sua copia del file); lavorare su un documento modificato (dice "unavailable").
- **Limiti:** servono almeno 40 colonne di testo; al piu' 20.000 periodi (forzare 1 minuto su un log di piu' giorni dà
  "too many buckets" col conteggio, in rosso e grassetto); i documenti UTF-16/32 non hanno timeline. Su un file senza orari la timeline
  si spegne da sola e la riga di stato dice perche', col suo consiglio (scegli un formato a mano): la preferenza non si tocca, il prossimo
  file con orari la riapre com'era.

#### T-05 Passo della timeline (`Alt+P`)
- **Tasti:** `Alt+P`, o un clic sull'intestazione della timeline (solo mentre e' accesa).
- **Cosa succede:** un menu' "TIMELINE: PERIOD": *Auto (scelto dalla durata del log)*, 1 minuto, 10 minuti, 1 ora, 1 giorno.
  Auto prende il piu' lungo fra giorno / ora / 10 minuti che dà ancora almeno 12 periodi, altrimenti il minuto.
- **La scelta e' della scheda, non un'abitudine:** non si ricorda fra i file — ogni file parte da Auto, e cambiare il formato
  dell'orario riporta ad Auto pure lui (le estremita' del file cambiano con quello). Un passo fine rimasto salvato rompeva i file piu' lunghi.
- **Cosa non deve mai succedere:** cambiare il file o l'intervallo.

#### T-06 Filtro dei livelli (`Alt+S`)
- **Tasti:** `Alt+S` (timeline accesa).
- **Cosa succede:** un menu' "TIMELINE: SHOW ONLY": *Tutte le righe*, *Solo gli errori*, *Errori e avvisi*, *Errori, avvisi e informazioni*.
  La scelta apre una scheda nuova di sola lettura col nome dei livelli.
- **Cosa non deve mai succedere:** cambiare il file o la scheda originali. Il confronto e' la parola intera del livello, maiuscole esatte.
- **Limiti:** rifiutato sui documenti non UTF-8, con un messaggio.

#### T-07 Colori dei log (`Alt+L`)

![Il log coi colori dei livelli accesi](img/tui/t07-colori.png)
- **Tasti:** `Alt+L`.
- **Cosa succede:** i colori compaiono subito e seguono il tema; gli orari sono colorati solo dove il formato corrente li legge.
- **Cosa non deve mai succedere:** rallentare lo scroll; colorare dentro una parola (il livello deve essere una parola intera).

#### F-01 Cerca (`F3` / `Ctrl+F`)

![Il prompt Cerca](img/tui/f01-cerca.png)
- **Tasti:** `Ctrl+F` apre il prompt (precompilato col testo dell'ultima volta); `F3` ripete in avanti, `Maiusc+F3` indietro (o apre il prompt se non c'e' ancora una ricerca).
  Con una selezione, `F3` cerca la selezione stessa, letterale e distinguendo le maiuscole, senza prompt; la selezione che e' la corrispondenza appena trovata
  non conta — `F3` continua la ricerca precedente (una regex, una senza maiuscole) come prima.
- **Cosa succede:** un prompt "text to search for (exact bytes)" con `mode: text | match case  (Alt+M, Alt+C)`; il cursore
  va alla prossima corrispondenza; oltre la fine ricomincia da capo con un avviso. Su un file grande una barra dopo 2 s ed `Esc` annulla ("cancelled", cursore fermo).
- **Cosa non deve mai succedere:** bloccarsi su un file enorme; impallarsi su qualunque espressione regolare (il motore non ha backtracking e un limite di 8 MiB).
- **Limiti:** i modelli confrontano per riga (mai attraverso un a-capo); glob, regex e senza-maiuscole servono un documento UTF-8
  (altrimenti un messaggio lo dice); il testo semplice in una code page il cui ago non si puo' codificare e' semplicemente "not found"; le corrispondenze
  nelle codifiche asiatiche multibyte contano solo all'inizio di un carattere.

#### F-02 Sostituisci (`F4` / `Ctrl+H`)

![Il terminale Sostituisci](img/tui/f02-sostituisci.png)
- **Tasti:** `F4` o `Ctrl+H`.
- **Cosa succede:** digitare qualsiasi cosa azzera l'anteprima; dopo l'applicazione, la scheda e' modificata e **un** `Ctrl+Z` annulla
  tutta la corsa.
- **Cosa non deve mai succedere:** toccare i file su disco; applicare alcche' quando si annulla con `Esc`, o oltre le 2.000.000 di occorrenze ("too many").
- **Limiti:** tutte-le-schede salta le schede che non lo permettono (non UTF-8 con una regex, per esempio) e dice quante; le schede non si possono
  aprire o chiudere mentre gira.

#### F-03 Filtro per righe (`Alt+G`)

- **Tasti:** `Alt+G`. Nel prompt, `Alt+I` inverte (nasconde le righe che corrispondono).
- **Cosa succede:** la scheda nuova con le righe trovate e un messaggio come `297 lines: Enter on a line goes to it in the file`;
  `Invio` su una riga porta avanti il file originale (riaprendolo se chiuso) col cursore su quella riga precisa.
- **Cosa non deve mai succedere:** modificare l'originale. Modificare nella vista e' rifiutato ("a filtered view is read-only: edit the original file"). Senza corrispondenze:
  "no line matches", nessuna scheda e nessun temporaneo lasciato.
- **Limiti:** al piu' 20.000.000 righe per vista (si ferma e dice che e' stata tagliata); il file temporaneo `mrp-filter-<pid>-<n>.log` sta nella
  cartella temporanea di sistema e si cancella quando la scheda si chiude o il programma esce.

#### F-04 Check dei tracciati (`Alt+H`)

![Il check dei tracciati: l'istogramma spuntabile delle lunghezze delle righe](img/tui/f04-lunghezze.png)
- **Tasti:** `Alt+H`. Nell'istogramma `SPAZIO` spunta la lunghezza sotto il cursore, `A` inverte tutta la selezione, `Invio` applica, `Esc` annulla.
- **Cosa succede:** mentre la passata conta il dialogo dice la percentuale; poi la lista, scorrevole, risponde subito a `SPAZIO`/`A`: ogni riga dice conteggio, barra e percentuale sul totale, e in fondo il totale (`2,202 records · 3 lengths`); `Invio` apre la vista con un messaggio come
  `2 lines: Enter on a line goes to it in the file` (e `Invio` su una riga della vista porta a quella riga nell'originale, come in F-03).
- **Cosa non deve mai succedere:** modificare l'originale. Con niente spuntato: "no line matches", nessuna scheda. Senza righe: "nothing to count". `Esc` lascia tutto com'era, e la selezione abortita non si ricorda.
- **Limiti:** le lunghezze sono byte di contenuto: le righe CRLF contano senza il `\r`, l'ultima riga senza a-capo e' una riga, una riga vuota ha lunghezza 0. La passata vede le modifiche non salvate (legge la fotografia) e si annulla chiudendo (riparte alla prossima apertura). Nella finestra le caselle sono cliccabili e Inverti / OK / Annulla sono pulsanti.

### Codifiche e fine riga

#### E-01 Codifica: rilevamento e cambio (`F9`)

![Il menu delle codifiche](img/tui/e01-codifica.png)
- **Tasti:** `F9` apre l'elenco delle 64 codifiche con quella corrente evidenziata; sulla riga di comando `--encoding NOME`. Il rilevamento e' automatico all'apertura.
- **Cosa succede:** il testo si rilegge senza muoversi e la riga di stato nomina la codifica (niente per UTF-8 semplice). Osservato su 28 file di prova (UTF-8 con e senza
  BOM, UTF-16/32 in entrambi gli ordini, 14 code page a byte singolo, 7 pagine DOS e Mac, Shift-JIS, EUC-JP, GBK, GB18030, Big5, EUC-KR): con `--encoding` tutti e 28 letti
  esatti; col rilevamento, gli Unicode con BOM, UTF-8, UTF-16 senza BOM e windows-1252/ISO-8859-1 sono giusti, ogni altro file a 8 bit o asiatico si apre come
  windows-1252 (leggibile ma sbagliato, intatto), e **scegliere la codifica giusta in `F9` sistema il testo in tutti i 21 casi provati**.
- **Cosa non deve mai succedere:** cambiare il file; perdere o nascondere un byte non valido (compare un carattere di sostituzione per byte e si riscrive uguale).
- **Limiti:** il passaggio fra UTF-16/32 e le altre codifiche e' rifiutato mentre il documento e' modificato ("save before switching to/from UTF-16/32").
  La riga di stato non dice che la codifica e' stata *indovinata*.

#### E-02 Salva con nome in un'altra codifica (`Ctrl+Maiusc+S`)
- **Tasti:** `Ctrl+Maiusc+S`: prima il percorso, poi un elenco di codifiche (quella corrente evidenziata, `Tab` = scrivi un BOM, `Invio` salva).
- **Cosa succede:** "Saved NAME as ENCODING"; il documento diventa il file nuovo. Osservato: 17 conversioni da UTF-8 (UTF-16/32 in entrambi gli ordini con e senza
  BOM, code page ISO e Windows, KOI8-R, CP866, Shift-JIS, EUC-JP, GBK, Big5, EUC-KR) hanno dato **file esatti al byte** col BOM giusto e si rileggono corrette;
  cp1251 → UTF-8 → cp1251 ha dato il file identico. Un carattere che il bersaglio non puo' contenere ferma il salvataggio con un messaggio preciso
  ("Cannot write U+1F600 in windows-1252 (at byte 5): nothing was saved") e nessun file viene creato.
- **Cosa non deve mai succedere:** scrivere un file parziale; toccare la sorgente.
- **Limiti:** il campo del percorso e' precompilato con la cartella del file: scrivi solo il nome nuovo (scrivere un percorso intero ci si aggiunge e fallisce
  con l'errore grezzo di sistema 123).

#### E-03 La modifica mantiene la codifica e il fine riga
- **Tasti:** nessuno: e' automatico.
- **Cosa succede:** osservato: digitare `é`, `ж`, `あ`, `好`, `가` in file windows-1252, windows-1251, UTF-16, Shift-JIS, GBK ed EUC-KR e salvare ha dato esattamente i byte
  giusti; un BOM e i byte UTF-8 non validi restano; digitare un carattere che la codifica non puo' contenere e' rifiutato subito ("cannot write '日' in windows-1252") e niente
  viene inserito.
- **Cosa non deve mai succedere:** rompere un fine riga.
- **CRLF (corretto l'8/11/2026):** in un file CRLF (UTF-8, UTF-16 LE/BE, UTF-32) lo `\r\n` e' un fine riga solo: `Fine` e `Maiusc+Fine` si fermano prima del `\r`, digitare e
  `Invio` a fine riga vanno prima di lui, `Destra`/`Sinistra`/`Backspace`/`Canc` scavalcano la coppia come un carattere solo, e `Giu` da fine riga arriva a fine riga
  successiva. Prima della correzione il cursore poteva stare fra `\r` e `\n` e digitare li' salvava `ab\rX\ncd\r\n`.

---

## Parte seconda — La finestra (GUI)

**La finestra** ha un menu' (File, Modifica, Visualizza, Avanzate, Aiuto), una barra degli strumenti (Nuovo, Apri, Salva | Annulla,
Ripeti | Taglia, Copia, Incolla | Cerca, Sostituisci, Vai a | Timeline, Filtro tempo | VIEW, EDIT, TAIL, Modalta' colonna, Barra
salti | Zoom), le schede, una scrollbar e la stessa riga di stato del terminale. Nella barra degli strumenti i tre modi sono tre
pulsanti adiacenti, uno acceso. Il titolo della finestra dice il nome del file — ` *` quando e' modificato — e la versione.

![La finestra dell'aiuto: il tab Tasti](img/manual/h-aiuto.png)
![La finestra delle impostazioni](img/manual/h-impostazioni.png)

Le carte della parte prima valgono anche qui: stesso motore, stesse risposte, stessi limiti. Ogni carta che segue dice solo come si
raggiunge la funzione dalla finestra — menu', pulsante, dialogo — e cosa cambia rispetto al terminale.

### Apertura e navigazione

#### N-01 Aprire un file
![Il log aperto nella finestra: il testo colorato, la riga di stato](img/manual/n01-apre.png)
- **Dalla finestra:** File > Apri (il dialogo di sistema), oppure `mrp-gui FILE` dalla riga di comando.
- Come nel terminale: senza file torna l'ultima sessione, senza sessione un "Untitled 1" vuoto.

#### N-02 Aprire un altro file, documento nuovo
![Due file, due schede](img/manual/n02-schede.png)
- **Dalla finestra:** File > Apri e File > Nuovo; `Ctrl+O` e `Ctrl+N` come nel terminale. Le schede si scelgono con un clic.

#### N-03 Muoversi
- **Dalla finestra:** rotella del mouse, trascinamento della scrollbar, clic sulla timeline. `Maiusc`+rotella scorre in orizzontale, otto celle a scatto.
- La barra orizzontale sotto il testo funziona come nel terminale: clic e trascinamento del pollice.

#### N-04 Vai a
![Il dialogo Vai a](img/manual/n04-vai.png)
- **Dalla finestra:** Modifica > Vai a, o il pulsante nella barra. Accetta numero di riga, `@byte`, `%percentuale`, `~orario` come il prompt del terminale.

#### N-05 Salti di pagine e barra dei salti
![La barra dei salti](img/manual/n05-salti.png)
- **Dalla finestra:** la barra dei salti e' una striscia di pulsanti (*Home*, i passi di pagina, *Fine*); l'ingranaggio apre le impostazioni dei passi. `Alt+PgSu`/`Alt+PgGiu` e i modificatori (`Maiusc` x100, `Ctrl` x1000) sono gli stessi.

#### N-06 Numeri di riga e indice delle righe
- **Dalla finestra:** menu Visualizza > Mostra i numeri di riga / Mostra l'indice delle righe.

#### N-07 Schede
- **Dalla finestra:** un clic su una scheda la porta davanti; `Ctrl+W` chiude quella davanti; `Alt+1`..`9` e `Alt+freccia` come nel terminale.

#### N-08 Avanzamento dei lavori lunghi
- Automatico, come nel terminale: la riga di stato dice il lavoro in corso e quanto ne manca.

### Modi, modifica e salvataggio

#### M-01 I tre modi
![Il menu dei tre modi](img/manual/m02-modi.png)
- **Dalla finestra:** tre pulsanti adiacenti nella barra degli strumenti, uno acceso; le voci col segno di spunta nel menu Modifica. I tasti `Alt+V`/`Alt+E`/`Alt+T` sono gli stessi.

#### M-02 Il menu dei modi
- **Dalla finestra:** il menu' dei modi sta nel menu Modifica e nei tre pulsanti della barra.

#### M-03 Digitare e modificare
- Come nel terminale: in EDIT caratteri, `Invio`, `Tab`, `Backspace`, `Canc`; selezione con `Maiusc`+frecce o il mouse; `Ctrl+C`/`X`/`V`.

#### M-04 Annulla e ripeti
- **Dalla finestra:** menu Modifica / i pulsanti Annulla e Ripeti nella barra. `Ctrl+Z` e `Ctrl+Y` come nel terminale.

#### M-05 Salva e Salva con nome
- **Dalla finestra:** File > Salva / File > Salva con nome; il percorso si sceglie col dialogo di sistema. Su un file grande il salvataggio mostra una barra di avanzamento e si puo' annullare.

#### M-06 Scarta le modifiche
- **Dalla finestra:** File > Ripristina. La domanda di conferma e' quella del terminale.

#### M-07 Uscire con modifiche non salvate
- **Dalla finestra:** chiudi la finestra; il dialogo "UNSAVED CHANGES" offre Salva ed esci, Scarta le modifiche, Annulla.

#### M-08 Tail
![Modalita' Tail: la vista segue la fine del file](img/manual/m08-tail.png)
- **Dalla finestra:** il pulsante TAIL nella barra / menu Modifica. `Alt+T` come nel terminale.

### Tempo, timeline e filtri

#### T-01 Formato dell'orario
![La finestra Tempo: il formato delle righe](img/manual/t01-formato.png)
- **Dalla finestra:** menu Avanzate; la finestra Tempo ha il calendario per i campi e il riquadro "esempio" con l'anteprima delle prime righe.

#### T-02 Vai a un orario
- Come nel terminale: `F5` (o Modifica > Vai a) con `~orario` nel dialogo.

#### T-03 Intervallo di tempo
![La finestra Tempo col calendario del campo Dal](img/manual/t03-intervallo.png)
- **Dalla finestra:** Avanzate > Intervallo di tempo, o il pulsante dell'orologio nella barra (acceso mentre c'e' un intervallo). I campi Dal e Al hanno il calendario.

#### T-04 Timeline
![La timeline: il file nel tempo, accanto alla scrollbar](img/manual/t04-timeline.png)
- **Dalla finestra:** Visualizza > Mostra la timeline, o il pulsante nella barra. Un clic sulla timeline porta li'.

#### T-05 Passo della timeline
- Come nel terminale: un clic sull'intestazione della timeline (solo mentre e' accesa) apre il menu' del passo.

#### T-06 Filtro dei livelli
- **Dalla finestra:** le bandierine dei livelli nella finestra Filtri; la scheda di sola lettura che ne esce e' la stessa del terminale.

#### T-07 Colori dei log
![Il log coi colori dei livelli accesi](img/manual/t07-colori.png)
- **Dalla finestra:** Visualizza > Mostra i colori dei log. `Alt+L` come nel terminale.

#### F-01 Cerca
![La finestra Cerca](img/manual/f01-cerca.png)
- **Dalla finestra:** Modifica > Trova o `Ctrl+F`; `F3` e `Maiusc+F3` iterano come nel terminale.

#### F-02 Sostituisci
![La finestra Sostituisci](img/manual/f02-sostituisci.png)
- **Dalla finestra:** Modifica > Sostituisci o il pulsante nella barra. L'anteprima e l'annullamento globale sono gli stessi.

#### F-03 Filtro per righe
![Il menu contestuale su una selezione: filtra, nascondi, cerca](img/manual/f03-filtri.png)
- **Dalla finestra:** la finestra Filtri (Avanzate > Filtri…); il menu contestuale su una selezione offre filtra, nascondi, cerca.

#### F-04 Check dei tracciati
![Il check dei tracciati: l'istogramma spuntabile delle lunghezze delle righe](img/manual/f04-lunghezze.png)
- **Dalla finestra:** Avanzate > Check dei tracciati. Le caselle dell'istogramma si spuntano col clic; la griglia mostra casella, conteggio, barra e percentuale per ogni lunghezza, e il totale in fondo.

### Codifiche e fine riga

#### E-01 Codifica
![Il menu delle codifiche](img/manual/e01-codifica.png)
- **Dalla finestra:** Visualizza > Codifica… apre l'elenco delle 64 codifiche con quella corrente evidenziata.

#### E-02 Salva con nome in un'altra codifica
- Come nel terminale: `Ctrl+Maiusc+S`; il percorso si sceglie col dialogo di sistema, poi l'elenco delle codifiche (quella corrente evidenziata, la spunta per il BOM).

#### E-03 La modifica mantiene la codifica e il fine riga
- Automatico, uguale al terminale.

---


## Sotto il cofano

Come funziona dentro — la sessione, la piece table, l'indice delle righe, il tempo, le codifiche, la ricerca, la cache — sta in una pagina sua: **[ARCHITECTURE.it.md](../ARCHITECTURE.it.md)**, pensata per chi vuole capire il motore (o contribuirvi).
