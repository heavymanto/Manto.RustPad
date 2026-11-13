# Manto.RustPad — Architettura

[English](ARCHITECTURE.md) · **Italiano**

Come funziona dentro Manto.RustPad: le scelte, i meccanismi, i limiti. Le costanti vengono dal sorgente e sono controllate dai test dove indicato. Per come si usa il programma, il manuale e' [docs/MANUAL.it.md](docs/MANUAL.it.md).

---
## Salvataggio automatico e sessione: come sopravvive il lavoro non salvato

**Cos'e'.** Una piccola cartella di file che ricorda le schede aperte e, per ogni scheda con modifiche non salvate, quelle modifiche. E'
quello che ti lascia premere `F10` senza domande e ritrovare tutto alla prossima partenza.

**Cosa si custodisce, e cosa no.** Per scheda: il percorso del file, il cursore, il modo, il nome della codifica e — solo se modificato — le
modifiche stesse: il buffer dei byte inseriti e la lista dei pezzi che dice come si intrecciano con l'originale. **Il file originale non
viene mai copiato**: un log da 100 GB con tre modifiche ha una sessione di pochi kilobyte. La storia degli annullamenti non si custodisce.

**Dove.**

| Sistema | Cartella |
|---|---|
| Windows | `%APPDATA%\mrp\session\` |
| Linux / macOS | `~/.config/mrp/session/` |
| Portatile (una cartella `lang` accanto ai programmi) | `<cartella dei programmi>/session/` |

Il terminale e la finestra usano la stessa cartella (e lo stesso lock). Contenuto: `manifest.txt` (un elenco di testo: `mrp-session 1`,
`active N`, un `tab NOME` per scheda), `tab-0.mrps`, `tab-1.mrps`... (binario, little endian, magic `MRPS`, versione 1, chiuso da un
checksum FNV-1a a 64 bit), `lock`, `untitled/` (i file vuoti di lavoro che reggono i documenti "Untitled N"), `stale/` (cio' che non
si e' potuto ripristinare).

**Quando si scrive.** Mentre lavori, al piu' una volta ogni **5 secondi**, controllato una volta al secondo, e solo se qualcosa e' cambiato
(una modifica, un movimento del cursore, una scheda aperta o chiusa, un'altra scheda davanti). Ogni file passa per un nome temporaneo e
viene rinominato, quindi una scrittura interrotta lascia la sessione precedente intatta. Un crash duro puo' quindi perdere fino a ~6 secondi
di lavoro. All'uscita la sessione si scrive subito. Mentre un documento si salva, la scrittura della sessione salta e si riprova un attimo dopo.

**Quando torna.** Partendo senza file: ogni scheda torna, modifiche comprese, con la stessa scheda davanti. Partendo con
file: quelli si aprono prima e le vecchie schede *con modifiche non salvate* si aggiungono dietro (una scheda pulita dello stesso file viene
sostituita da quella ripristinata). La riga di stato dice "restored N tab(s) with unsaved changes".

**Cosa puo' andare storto.**

| Situazione | Cosa succede |
|---|---|
| Il file e' cambiato su disco | La scheda si ripristina lo stesso; al primo salvataggio il conflitto emerge (vedi [*Modifica e salvataggio*](#modifica-e-salvataggio-piece-table-e-scrittura-sicura)). |
| Il file e' stato cancellato | La scheda si salta; con modifiche, la copia va in `stale/` e un avviso la nomina; una scheda pulita sparisce in silenzio. |
| Un file di scheda e' corrotto | Il checksum o la decodifica fallisce: quella scheda si salta, il suo file va in `stale/`, la riga di stato lo dice. |
| `manifest.txt` e' corrotto | Tutta la sessione si ignora e si apre "Untitled 1". Nessun crash. |
| Disco pieno / senza permessi | La scrittura periodica riprova piu' tardi; all'uscita, se la sessione non si puo' scrivere, l'editor chiede scheda per scheda. |
| Due istanze insieme | La seconda non prende il lock e gira senza sessione (le sue schede non sono protette). |

Solo il primo avviso arriva alla riga di stato; con piu' avvisi, il resto sta dentro `stale/`. `stale/` non viene mai svuotato dal programma.
Cancellare tutta la cartella dimentica solo schede, cursori e modifiche non salvate; i file dell'utente non si toccano mai.

**Da sapere.** Anche i documenti senza nome sopravvivono ai riavvii (stanno su file di lavoro in `untitled/`; salvarne uno con un nome vero
lo trasforma in una scheda normale e toglie il file di lavoro). Un file dell'utente che per caso sta in una cartella chiamata `untitled` non
viene mai scambiato per un file di lavoro. Le esecuzioni di sviluppo con `--dump`, o con `MRP_NO_PROFILE=1`, non tengono sessione.

## Modifica e salvataggio: piece table e scrittura sicura

**Modificare senza copiare.** Un documento e' una mappatura in sola lettura del file piu' una **piece table**: un elenco ordinato di pezzi, ognuno
dei quali punta nel file originale oppure in un piccolo buffer solo-aggiunto che raccoglie tutto quello che digiti. La memoria dipende dalla
dimensione delle modifiche, non del file. La tabella si tiene in blocchi di al piu' 512 pezzi, quindi una modifica tocca solo i blocchi che
attraversa. I pezzi vicini che diventano contigui si fondono.

**Annulla.** Ogni modifica effettiva si registra come una giunzione minima (quali pezzi sostituiti da quali). Annulla rilegge le giunzioni
all'indietro, ripeti in avanti; nessuna copia di testo viene fatta, quindi annullare costa quasi niente anche su un file enorme. Il flag
"modificato" e' un numero di versione che non si ripete mai: annullare fino allo stato salvato lo azzera.

**Salvare.** L'originale non si scrive mai sul posto. Un salvataggio:

1. rifiuta se il file su disco non ha piu' la lunghezza e la data di modifica registrate all'apertura (qualcun altro l'ha cambiato);
2. crea un file temporaneo **nella stessa cartella**, chiamato `.<nome>.mrp-tmp-<pid>-<n>`;
3. copia i pezzi in ordine, in blocchi da **4 MiB** (anche la granularita' della barra di avanzamento e dell'annullamento);
4. scarica il temporaneo su disco, copia i permessi dell'originale (non il proprietario);
5. lo rinomina sull'originale — l'unico passo atomico. Su Windows rinominare su un file mappato funziona mentre troncarlo no, ed e' per questo
   che il salvataggio e' un rename;
6. riapre il file nuovo, collassa la tabella in un pezzo solo, svuota la storia degli annullamenti e ricostruisce l'indice delle righe in background.

Il salvataggio gira in un thread che porta con se' il documento e **lo restituisce sempre**, anche in errore: un salvataggio fallito non
perde mai le modifiche. `Esc` nel dialogo alza un flag controllato prima di ogni blocco e prima del rename; dopo il rename il salvataggio
e' concluso.

**Cosa puo' andare storto.**

| Situazione | Cosa succede |
|---|---|
| Il file e' cambiato o sparito dall'apertura | "the file on disk changed after opening": niente scritto, le modifiche restano. |
| Cartella non scrivibile, disco pieno, rename rifiutato | Un errore col messaggio di sistema; il temporaneo si cancella; l'originale intatto; il documento resta modificato. |
| Annullato | Il temporaneo si cancella, "save cancelled", il documento com'era. |
| Il file viene accorciato da un altro *mentre* si copia | Il salvataggio fallisce con un errore di I/O; originale intatto. |
| L'antivirus tiene il temporaneo bloccato | Il temporaneo puo' restare; l'originale e' salvo. |
| Rename fatto ma la riapertura fallisce | I dati sono salvi su disco, ma la scheda diventa "chiusa" e riporta l'errore (raro). |

Un documento modificato non viene seguito in TAIL (mescolerebbe due contenuti). Salvare in una codifica diversa passa da un passo di
conversione col suo file temporaneo (`<nome>.<pid>.<n>.mrp-conv`) nella cartella di destinazione.

## Indice delle righe e navigazione senza leggere il file

**L'idea.** L'apertura e' immediata perche' nulla si legge in anticipo: il file e' memory-mappato e lo schermo chiede solo i byte che
mostra. I numeri di riga servono l'indice, che si costruisce in background.

**Un indice sparso.** Non custodisce ogni riga. Ricorda l'offset in byte di un inizio riga ogni **256 righe**, piu' uno per blocco da
**4 MiB**. Qualunque domanda — quante righe in tutto, quale riga tiene questo byte, dove comincia la riga N — parte dal punto ricordato
piu' vicino e conta gli a-capo solo nel breve resto che manca. Un log enorme costa megabyte di indice, non una voce per riga, e un file
che e' una riga sola riceve comunque risposta con lavoro limitato.

**Come si costruisce.** Un thread in background apre un suo handle in sola lettura sul file. Un gruppo di lavoratori (uno per thread di CPU)
prende i blocchi da 4 MiB in ordine, ognuno letto in segmenti da **256 KiB** in un buffer riusato — letture posizionate esplicite, non page
fault, quindi la memoria del processo non cresce col file — e una fusione sequenziale trasforma i conteggi per blocco in numeri di riga
globali. Su file di **64 MiB o piu'** un thread di precaricamento legge avanti a passi di 8 MiB, al piu' 256 MiB davanti ai lavoratori,
cosi' il disco vede richieste contigue.

**Si toglie di mezzo.** Quando salti (trascinamento della scrollbar, vai a, `Alt+PgSu/PgGiu`, `Ctrl+Home/Fine`) l'indicizzatore mette in
pausa le sue letture su disco e riprende da solo **400 ms** dopo l'ultimo salto; le frecce e la digitazione non lo mettono in pausa (un test
su un file da 8 GiB lo controlla). L'uscita, lo spegnimento dell'indice o un lavoro nuovo lo annullano fra un blocco e l'altro.

**Finche' non e' pronto.** I numeri di riga sono ignoti: il margine e' vuoto, la riga di stato mostra `Ln ?`, e vai a una *riga* dice di
aspettare; vai a per byte (`@`) o percentuale (`%`) funziona subito. Senza indice, il movimento cammina le **righe visive**: una riga visiva
e' una riga vera o un pezzo da 128 KiB di una riga troppo lunga, quindi una riga unica da 1 GB non blocca mai lo schermo.

**Quando riparte.** Dopo ogni salvataggio riuscito (il file e' cambiato), quando cambia la forma dell'a-capo (per esempio una codifica nuova)
e quando riaccendi l'indice. In TAIL un file cresciuto *estende* l'indice sui byte nuovi invece di ricostruirlo.
La forma dell'a-capo (un byte, `CR`, o un'unita' allineata da 2 o 4 byte per UTF-16/32) si rileva dai primi 64 KiB.

**Cosa puo' andare storto.** (Su Windows la mappatura impedisce anche agli altri programmi di troncare il file mentre e' aperto qui; aggiungere,
rinominare e cancellare si puo'.) Un file che il lavoro in background non riesce ad aprire dà "index failed" con l'errore di sistema e nessun
numero, ma tutto il resto continua. Un file accorciato durante l'indicizzazione fa rifare il blocco toccato attraverso la mappatura. Un file
troncato da un altro processo sotto una mappatura viva puo' alzare un errore di accesso dal sistema: rischio noto e accettato del memory
mapping.

## Tempo: formati, salto a un orario, intervallo, timeline

**L'idea.** Un log e' quasi sempre ordinato per tempo e il tempo sta in testa a ogni riga. Quindi il programma non ha mai bisogno di un indice
dei tempi: trova qualunque orario con una ricerca binaria sui byte del file, leggendo solo i primi cento byte di una manciata di righe. E'
per questo che le funzioni del tempo funzionano su un file di qualunque dimensione e non lo leggono mai tutto.

**Maschera del tempo.** Una *maschera* e' un modello che dice dove sta l'orario in una riga (`yyyy-MM-dd HH:mm:ss.SSS`, `HH:mm:ss`,
`epoch_ms`, syslog, Apache...). Si confronta solo nei primi caratteri della riga (0 per la maggior parte dei formati, 40 per quello Apache,
regolabile). Il valore letto e' un numero di millisecondi dal 1970-01-01, negativo prima, calcolato con un calendario gregoriano scritto a
mano: nessun fuso orario e nessuna libreria di date, quindi le date prima del 1970 e gli anni bisestili funzionano. I valori epoch con 9–10
cifre sono secondi, con 12–13 millisecondi. Un secondo intercalare `:60` si legge 59,5 s; una maschera senza anno usa il 2000.

**Rilevamento.** Una volta per scheda, la prima volta che qualcosa lo chiede, il programma prende un campione — fino a 1000 righe non vuote dai
primi 64 KiB e fino a 1000 dagli ultimi — e prova i 19 formati noti. Vince quello che legge piu' righe, se abbastanza sono d'accordo: un formato
completo di data e ora vuole 3 righe che tornano (o tutte), uno debole (solo ora, solo data, epoch) vuole la meta' delle righe non vuote, quindi
un numero di 10 cifre sbagliato non passa per un epoch. Il formato scelto l'ultima volta si prova per primo. I documenti UTF-16/UTF-32 non
si riconoscono mai.

**Vai a un orario e intervallo.** Entrambi bisettano l'intervallo di byte: prendono il mezzo, vanno all'inizio della sua riga, guardano la prima riga con un orario da li' (al piu' 4096
righe visive avanti, leggendo al piu' 512 byte ciascuna), e restringono. Le righe senza orario (una intestazione, uno stack trace) si saltano.
Questo assume che gli orari non tornino mai indietro; con un log che lo fa, le ricerche possono fermarsi sulla riga sbagliata. L'intervallo non e'
altro che due offset in byte, quindi toglierlo e' immediato.

**Timeline.** Una barra con una riga per periodo. Invece di scansionare, il programma trova ogni confine di periodo saltando in avanti dal
precedente a passi che raddoppiano (galoppo, da 4096 byte) e poi bisettando l'intervallo trovato. Il costo e' proporzionale al numero di
periodi (al piu' 20.000), non al numero di righe. Il passo e' 1 minuto, 10 minuti, 1 ora o 1 giorno; *Auto* prende il piu' lungo che dà ancora
almeno 12 periodi (due settimane di log danno giorni, una giornata dà ore). Una seconda passata, nel suo thread e leggendo pezzi da 1 MiB, conta
per ogni periodo quante righe *cominciano* con ciascun livello; il livello di una riga e' la sua prima parola di livello, trovata dallo stesso
scanner che colora la riga. La passata aspetta (cicli da 20 ms) mentre il file si indicizza ancora, cosi' i due non si contendono il disco. Le 12
celle di ogni barra si dividono fra i livelli in proporzione, ma un livello presente non scende mai a zero.

**Thread e annullamento.** Le tacche si costruiscono in un thread che apre un handle *suo* sul file; lo schermo fa il sondaggio del
risultato. Cambiare il passo, l'intervallo o la lunghezza del file, o spegnere la timeline, annulla i lavori in corso e butta i risultati parziali
(nessuna barra a meta'). Vai a un orario e l'intervallo girano nel thread principale e non si possono interrompere, il che va bene perche' sono
poche decine di piccole letture.

**Cosa si ricorda.** Il formato scelto diventa il predefinito provato per primo sui prossimi file; fino a 8 regole imparate *da un
esempio* si custodiscono e si ripropongono dove leggono qualcosa; entrambe stanno nel file delle impostazioni. Ogni scheda ha la sua maschera e il
suo intervallo.

**Cosa puo' andare storto.** Orari che tornano indietro (vedi sopra); un log le cui ultime 4096 righe visive non hanno orario riporta ultimo = primo; un
documento modificato o senza nome non ha timeline ("unavailable"); un intervallo sbagliato e' rifiutato col primo e l'ultimo orario veri del
file; UTF-16/32 non ha funzioni del tempo.

## Codifiche, fine riga, modalita' colonna

**L'idea.** Un documento e' sempre una sequenza di byte; la codifica e' solo la lente che dice come quei byte si leggono come caratteri
(schermo, cursore, larghezze di colonna) e come un carattere digitato diventa byte. Posizione, conteggi di riga e ricerche restano allineati
all'unita': 1 byte per UTF-8, le code page e le codifiche asiatiche multibyte, 2 per UTF-16, 4 per UTF-32.

**64 codifiche.** UTF-8, UTF-16 LE/BE, UTF-32 LE/BE, 53 code page a byte singolo (windows-125x, ISO-8859, pagine IBM/DOS, KOI8, cinque pagine
Mac, ASCII) e 6 codifiche asiatiche multibyte (Shift_JIS, EUC-JP, GBK, GB18030, Big5, EUC-KR). I nomi non distinguono maiuscole, `_` vale `-`, con
alias (`cp1252`, `latin1`, `437`, `cp932`...); un `utf-16` nudo e' rifiutato perche' ambiguo. Le codifiche a stato (ISO-2022-JP, HZ) non sono supportate.

**Rilevamento.** Senza `--encoding`, i primi 64 KiB decidono, in quest'ordine: un BOM UTF-32, un BOM UTF-16, poi (con almeno 8 byte) quale meta' di ogni unita'
tiene gli zero, poi qualunque byte NUL significa "binario" e UTF-8, poi la validita' UTF-8, altrimenti windows-1252. UTF-32 senza BOM non si indovina mai.
Il rilevamento e' un'ipotesi: **F9** rilegge gli stessi byte con un'altra codifica sul posto, senza toccare il file. Un byte che non e' un
carattere valido compare come un carattere U+FFFD per byte, quindi niente si nasconde.

**Il BOM.** Resta nei byte e sopravvive ai salvataggi; l'editor gli gira intorno (digitare all'inizio va dopo di lui, Backspace si ferma
a lui, Canc lo scavalca).

**Fine riga.** LF, CRLF o CR solo si rileva dallo stesso campione (CR solo se a byte singolo e il campione ha CR ma nessun LF; CRLF
se i CRLF superano gli LF nudi). Invio scrive esattamente quello stile, quindi un file CRLF non diventa mai misto. `--eol` forza quello che
scrive Invio senza riscrivere le righe che ci sono. I documenti nuovi prendono lo stile del sistema (CRLF su Windows).

**Cambiare codifica.** F9 su un documento la cui forma dell'a-capo cambierebbe (UTF-16/32 contro il resto) e' rifiutato mentre il documento e'
modificato o ha una storia di annullamenti; altrimenti l'indice si ricostruisce in background e la selezione e il risalto della ricerca si perdono.

**Salva con nome con conversione.** *Salva con nome* chiede la codifica di destinazione e una casella BOM. Stessa codifica e stesso BOM e' un salvataggio
normale; qualunque altra cosa converte tutto il contenuto corrente, modifiche non salvate comprese, in un thread, in blocchi da 1 MiB, in un file
temporaneo (`<nome>.<pid>.<n>.mrp-conv`) che viene rinominato sul bersaglio. Se un carattere non si puo' scrivere nella codifica di destinazione, niente
viene rinominato: il temporaneo si cancella, il bersaglio non si tocca, il cursore salta su quel carattere e il messaggio lo nomina (U+XXXX) col suo offset.

**Modalita' colonna (F8).** `Maiusc`+frecce o `Alt`+trascinamento disegnano un rettangolo sulle righe visive. Digitare, Tab, Backspace e Canc agiscono su ogni riga del blocco che
raggiunge la colonna; copia unisce le righe con l'a-capo del documento; incolla va riga per riga; Invio butta il blocco. Un carattere appartiene al
blocco se *comincia* dentro le colonne; il `\r` nascosto e il `\n` mai. Ogni azione di colonna e' un passo di annulla solo. Limiti: 200.000 righe,
64 MiB di testo copiato, un tetto di colonna virtuale a 4096. Un incolla che chiedesse piu' righe di quante ne ha il documento e' rifiutato per intero.

## Ricerca, sostituzione e filtro per righe

**Cercare sul documento modificato.** Una ricerca non gira sul file: prende una *fotografia* del documento modificato (una lettura condivisa
del file mappato piu' una copia della piece table) e gira in un thread, in blocchi da 1 MiB con sovrapposizione, controllando il flag di annullamento prima di ogni blocco.
La fotografia resta valida anche se modifichi o salvi mentre gira. Il testo semplice distinguendo le maiuscole si cerca come byte esatti nella codifica del documento,
quindi funziona in ogni codifica; glob, regex e senza-maiuscole si compilano con la crate `regex` (che non ha backtracking, quindi nessun modello puo' impallare la ricerca,
e un limite di 8 MiB sulla dimensione compilata) e funzionano solo su documenti UTF-8. I modelli confrontano per riga, come `grep`. Le righe piu' lunghe di 16 MiB si leggono a pezzi
(una corrispondenza attraverso un taglio puo' sfuggire). Le corrispondenze vuote si scartano.

**Mentre un lavoro gira** il programma ignora ogni tasto tranne `Esc`, che lo annulla.

**Sostituisci** e' in due fasi: il primo Invio conta (anteprima), il secondo applica. Tutti i cambi di una corsa sono un passo di annulla solo. Oltre le 2.000.000
di occorrenze si rifiuta ("too many"). In modalita' tutte-le-schede il workspace fa un lavoro per scheda e totalizza i conteggi, le schede con corrispondenze e quelle saltate.

**Il filtro per righe** scrive le righe che corrispondono, byte per byte, in un file temporaneo nella cartella temporanea di sistema (`mrp-filter-<pid>-<n>.log`),
lo apre come scheda di sola lettura e tiene, accanto, l'offset vero di ogni riga nell'originale (8 byte per riga, al piu' 20.000.000 di righe). E'
per questo che `Invio` su una riga arriva esattamente su quella riga dell'originale anche se l'originale e' stato chiuso nel frattempo. Il file temporaneo si cancella quando la sua
scheda si chiude o il programma esce; un filtro che non trova niente, fallisce o viene annullato non lascia file.

**Le stesse impostazioni di ricerca** (testo, modo, maiuscole) sono condivise da Cerca, Filtro e Sostituisci, e i prompt sono precompilati col testo dell'ultima volta.

## Cache: cosa si ricorda, e quando si butta

**Il principio.** Niente si rende persistente: ogni cache vive in RAM dentro il processo e sparisce quando esce (le uniche cose
scritte su disco sono il documento salvato, le impostazioni e la sessione). La piu' grande non e' nostra: il file e' memory-mappato, quindi le sue
pagine sono la cache del file del sistema operativo, condivisa e cacciabile. Le passate sequenziali lunghe (l'indice delle righe, il salvataggio) usano letture
posizionate esplicite invece di page fault perche', su un disco che gira, una lettura sequenziale grande misurata correva a 149 MB/s contro 28–39 MB/s
dei fault.

**Perche' le modifiche restano economiche.** La tabella delle modifiche e' divisa in blocchi di al piu' 512 pezzi, e ogni blocco porta un'*etichetta* da un
contatore di processo che cambia ogni volta che i suoi pezzi cambiano e non si riusa mai: etichette uguali significano sempre contenuto uguale. E' quello che
lascia al programma ricordare i conteggi di riga dei blocchi gia' visti, e ricontare solo i blocchi toccati davvero. Un test
controlla che i numeri restino giusti modificando a mano i pezzi di un blocco, e nessuna conta mentre niente cambia.

**L'indice delle righe e' del file originale.** Resta valido attraverso le modifiche: la piece table fa da ponte sulla differenza. Quindi puoi digitare in
un log da 100 GB e i numeri di riga restano giusti senza reindicizzare.

| Cache | Contiene | Buttata quando | Peso |
|---|---|---|---|
| Indice delle righe | inizi sparsi delle righe del file originale (uno ogni 256 righe, piu' uno ogni 4 MiB) | un salvataggio, un cambio di codifica che cambia la forma dell'a-capo, l'indice spento, un indice nuovo installato; *esteso* sul posto in TAIL | 16 B ogni 256 righe + 16 B ogni 4 MiB: circa 60 MB per un miliardo di righe da 100 byte |
| Conteggio a-capo per pezzo | gli `\n` in ogni pezzo intero | con l'indice; svuotata oltre 2 × pezzi + 1024 | ~32 B per voce |
| Conteggio a-capo per blocco | gli `\n` in un blocco intero, per etichetta | con l'indice; potata alle etichette vive oltre 2 × blocchi + 1024 | 16 B per voce |
| Tabella dei prefissi | offset di inizio e a-capo prima di ogni blocco | **qualunque modifica, annulla o ripeti**; ricostruita al primo uso, in tempo proporzionale ai blocchi toccati | 16 B per blocco |
| Periodi della timeline | i confini dei periodi (orario, offset in byte) | cambio di lunghezza, intervallo o passo; timeline spenta; documento modificato | al piu' 20.000 × 16 B = 320 KB |
| Livelli della timeline | i conteggi dei quattro livelli per periodo | arrivano periodi nuovi; timeline spenta | al piu' ~640 KB |
| Formato del tempo | il formato riconosciuto, per scheda | riconosciuto una volta; sostituito da una scelta a mano (che spegne anche l'intervallo) | un modello |
| Aiuti del dialogo del tempo | conteggi delle corrispondenze, suggerimenti, il primo e l'ultimo orario del file | ogni volta che il dialogo si apre | poche centinaia di byte |
| Anteprima | fino a 3 coppie (riga, orario) | quando modello, distanza o lunghezza del documento cambiano | 3 righe |
| Righe disegnate | l'ultima schermata di testo e i suoi offset | sostituita a ogni disegno; riusata mentre un salvataggio tiene il documento | uno schermo |
| Campioni di avanzamento | (istante, byte) del lavoro in corso | il lavoro cambia o riparte; solo gli ultimi 3 s tenuti | pochi campioni |

**Come si legge la tabella.** Tutto quello che dipende dal testo *modificato* (la tabella dei prefissi) si ricostruisce dopo ogni modifica, ma solo per i
blocchi cambiati. Tutto quello che dipende dal *file originale* (l'indice e le cache dei conteggi) si butta solo quando il file stesso
cambia: un salvataggio, una forma dell'a-capo nuova, o l'indice spento. La potatura e' solo per dimensione: se la cache dei pezzi si svuota, la
prossima ricostruzione riconta tutto una volta.

**Da sapere.** Spegnere la timeline butta i suoi periodi; riaccenderla ricalcola gli stessi periodi. Un log con una
intestazione lunga senza orari riceve comunque il formato riconosciuto, perche' il campione comprende la coda. La preferenza della timeline e'
l'unica cosa "ricordata" di questo capitolo che arriva nel file delle impostazioni.
