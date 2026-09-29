<p align="center">
  <a href="README.ja.md">日本語</a> | <a href="README.zh.md">中文</a> | <a href="README.es.md">Español</a> | <a href="README.fr.md">Français</a> | <a href="README.hi.md">हिन्दी</a> | <a href="README.md">English</a> | <a href="README.pt-BR.md">Português (BR)</a>
</p>

<p align="center">
  <img src="https://raw.githubusercontent.com/mcp-tool-shop-org/brand/main/logos/si-jam-sessions/readme.png" alt="si-jam-sessions" width="400">
</p>

<p align="center">
  <a href="https://github.com/mcp-tool-shop-org/si-jam-sessions/actions/workflows/ci.yml"><img src="https://github.com/mcp-tool-shop-org/si-jam-sessions/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue.svg" alt="MIT License"></a>
  <a href="https://mcp-tool-shop-org.github.io/si-jam-sessions/"><img src="https://img.shields.io/badge/Landing_Page-live-blue" alt="Landing Page"></a>
</p>

# si-jam-sessions

**Un motore musicale in cui l'esecuzione di un'intelligenza artificiale può essere valutata con precisione, riprodotta esattamente e utilizzata per l'addestramento con la massima tranquillità.**

Esegui una frase a ritmo, e il motore registra ogni nota della traccia:

- quando è stata suonata, in campioni interi;
- quale nota dello spartito corrisponde.

Successivamente, valuta ogni nota rispetto allo spartito. Due macchine che valutano la stessa traccia concordano fino all'ultimo bit, perché
la legge di valutazione è un algoritmo deterministico in Rust, compilato in un singolo file WebAssembly. La legge è deterministica, quindi una traccia registrata viene riprodotta con lo stesso hash senza chiamare un modello; il sistema di integrazione continua (CI) la riproduce ad ogni esecuzione.

`si-jam-sessions` è l'equivalente di [`ai-jam-sessions`](https://github.com/mcp-tool-shop-org/ai-jam-sessions)
e un elemento affine di [`si-rpg-engine`](https://github.com/mcp-tool-shop-org/si-rpg-engine). Il modello propone. Un
verificatore, diverso dal modello, accetta o rifiuta la proposta con una motivazione, e la legge registra e calcola l'hash di
ciò che accetta. Il modello non tocca mai direttamente lo spartito.

## Ascolta

Due arrangiamenti di *Battle Hymn of the Republic* vengono riprodotti contemporaneamente sulla
[pagina principale](https://mcp-tool-shop-org.github.io/si-jam-sessions/), con una visualizzazione delle differenze.

- **La fonte:** la trascrizione di questo progetto dell'edizione anonima del 1862 di Ditson.
- **Gli arrangiatori:** glm-5.3 e kimi-k3, entrambi che lavorano sulla stessa fonte su Ollama Cloud.
- **La licenza:** entrambi gli arrangiamenti sono dedicati a CC0 1.0. Ognuno ha superato il controllo della licenza previsto dalla legge, fornendo le prove necessarie.
- **La registrazione:** ognuno è stato renderizzato tramite la legge sul
[Salamander Grand Piano V3](https://freepats.zenvoid.org/Piano/acoustic-grand-piano.html) di Alexander Holm
(CC BY 3.0).

| Arrangiamento | Durata | Note registrate dalla legge | WAV non compresso (48 kHz, 32 bit float) |
|---|---|---|---|
| glm-5.3 | 5:02 | 1,720 | [116 MB](https://github.com/mcp-tool-shop-org/si-jam-sessions/releases/download/v0.1.0/battle-hymn-glm-5.3.wav), SHA-256 `85b2d567…2eed` |
| kimi-k3 | 4:37 | 1,924 | [106 MB](https://github.com/mcp-tool-shop-org/si-jam-sessions/releases/download/v0.1.0/battle-hymn-kimi-k3.wav), SHA-256 `bf49d310…a017` |

Il rendering è deterministico: una seconda renderizzazione produce gli stessi byte. Il processo di rendering del piano del sistema CI ha renderizzato ogni esempio completo su Linux, e entrambi gli SHA-256 corrispondono alla versione pubblicata: lo stesso risultato si ottiene su Windows e Linux. Dopo aver scaricato il piano una volta, questo comando riproduce il primo:

```bash
cargo run -p host --release --locked -- render battle-hymn-glm-5.3.wav --piece battle-hymn-glm-5.3 --voice piano
```

Gli hash completi sono disponibili nelle [note di rilascio](https://github.com/mcp-tool-shop-org/si-jam-sessions/releases/tag/v0.1.0).

## Cosa lo rende diverso

- **La traccia fa parte della legge.** Il tempo, l'intonazione e la velocità sono numeri interi nello stato con hash, quindi "in ritardo di 45 ms" è un dato che il motore può dimostrare. La forma d'onda è solo una rappresentazione.
- **Il tempo non aspetta mai.** La legge esegue un passo quantistico fisso, indipendentemente dal fatto che qualcuno stia suonando o meno. Un modello pianifica le frasi in anticipo rispetto all'orizzonte di commit. Un piano tardivo viene rifiutato, e non gli è mai permesso di interrompere la musica.
- **Ogni brano si guadagna il suo posto.** La musica viene inserita solo con prove:
- una composizione di pubblico dominio sia negli Stati Uniti che nell'UE;
- un arrangiamento di pubblico dominio, o uno creato da questo progetto;
- l'edizione di origine e il suo anno;
- una licenza all'interno del file che corrisponda alla sua ricezione.

Le fonti sconosciute, con licenza share-alike, non commerciali e con restrizioni sull'IA vengono rifiutate. Il materiale CC BY 4.0 si trova in una categoria a sé stante.
- **I dati di addestramento saranno una stampa di ciò che la legge ha registrato.** Non è ancora stato creato alcun set di dati. Quando lo sarà, ogni riga riprodurrà lo stesso hash, le suddivisioni saranno per opera e definite prima che esista qualsiasi riga, e ogni risultato dichiarato indicherà la sua potenza statistica.

## Provalo

Compila dal codice sorgente con Rust, oppure esegui il container. Il repository definisce Rust 1.98.1 in `rust-toolchain.toml`,
e `rustup` lo installa alla prima compilazione.

- **Windows 10 e 11** eseguono tutto, anche se l'input in tempo reale non è ancora stato testato con una vera tastiera MIDI.
- **Linux:** il sistema CI compila e testa l'host e lo utilizza per il rendering. L'esecuzione tramite un dispositivo audio Linux non è stata testata, e l'input in tempo reale è disponibile solo su Windows per ora.
- **macOS** non è stato testato.
- **Il container** `ghcr.io/mcp-tool-shop-org/si-jam-sessions` esegue `render`, `notes`, `notices` e
`preview` senza una toolchain Rust. L'esecuzione tramite un dispositivo è la stessa procedura non testata di Linux.

L'immagine viene pubblicata con ogni tag di versione. Contiene l'host e gli spartiti, ma non i campioni del piano.

```bash
docker pull ghcr.io/mcp-tool-shop-org/si-jam-sessions:0.2.0
docker run --rm \
  -e HOST_UID="$(id -u)" -e HOST_GID="$(id -g)" \
  -v "$PWD:/out" \
  ghcr.io/mcp-tool-shop-org/si-jam-sessions:0.2.0 \
  notes /out/notes.json
```

Su PowerShell, elimina le righe uid e monta `${PWD}:/out`. La pagina dei container del manuale
[container page](https://mcp-tool-shop-org.github.io/si-jam-sessions/handbook/docker/) contiene il rendering del piano e il modulo di Windows. Il pacchetto npm
[`@si-jam-sessions/si-jam-sessions`](https://www.npmjs.com/package/@si-jam-sessions/si-jam-sessions) è il
README, il registro delle modifiche e la licenza di questa versione, pubblicati da Trusted Publishing. L'installazione non installa l'host. L'host è il container, o la compilazione descritta di seguito.

```bash
cargo run -p host --release -- devices        # list the audio outputs and MIDI inputs
cargo run -p host --release -- fetch-piano    # download and verify the grand piano once (742 MB)
cargo run -p host --release -- play           # the Battle Hymn as glm-5.3 arranged it
cargo run -p host --release -- play --piece battle-hymn-kimi-k3
cargo run -p host --release -- jam --midi 0   # play along; each note is graded as it lands
```

- `--piece` seleziona `battle-hymn-glm-5.3` (l'impostazione predefinita), `battle-hymn-kimi-k3` o `entertainer`, il brano di prova della prima milestone.
- `notes <out.json>` scrive le note che la legge registra. La pagina principale estrae i dati da questi file.
- `host help` elenca tutti i comandi.

Utilizza un'uscita cablata per `jam`. Il ritardo di un'uscita Bluetooth è superiore ai 100 ms consentiti dalla legge per la trasmissione.

## Fidati del modello

- **Dati interessati:**
- i file di partitura in `scores/`;
- i campioni di pianoforte in una cache per utente;
- gli output audio e gli input MIDI che si scelgono;
- i file di cui si richiede la scrittura.
- **Dati non interessati:** qualsiasi cosa al di fuori di questi percorsi. Non ci sono account, credenziali o dati di telemetria.
- **Il container** è lo stesso programma. Contiene il file eseguibile e le partiture, non il pianoforte. Riduce i privilegi prima che il sistema host venga eseguito: uid 10001, o `HOST_UID` quando lo si imposta. Una directory montata è l'unico modo in cui un file può uscire dal container.
- **Rete:** solo `fetch-piano` la utilizza.
- Scarica un archivio da un indirizzo fisso.
- Verifica l'archivio rispetto a un hash SHA-256 prima di estrarne il contenuto.
- Rifiuta i collegamenti e i percorsi che escono dalla sua directory.
- **Autorizzazioni:** un account utente standard. Non è necessario alcun diritto di amministratore.
- **Il programma** non esegue alcuna operazione di I/O. Il sistema di integrazione continua (CI) verifica che il suo modulo WebAssembly non importi nulla.

Per segnalare una vulnerabilità, consultare [`SECURITY.md`](SECURITY.md).

## Stato attuale

- **La prima fase è completa:** il programma, l'acquisizione e la provenienza dei dati, l'hash di riferimento e il sistema host. L'hash di riferimento è lo stesso nativamente su x86_64 e ARM64, e come WebAssembly sotto V8, SpiderMonkey e JavaScriptCore.
- **Il progetto** è definito in [`docs/PHASE-0.md`](docs/PHASE-0.md).
- **Il passaggio di consegne** in [`docs/HANDOFF.md`](docs/HANDOFF.md) illustra cosa è in corso, cosa è stato deciso e perché, e cosa succederà in seguito.
- **Revisione:** prima che una modifica del codice venga integrata, due modelli la esaminano su Ollama Cloud. Ognuno proviene da una famiglia diversa da quella dell'autore, e ognuno è all'oscuro della revisione dell'altro.

## Licenza

- **Codice:** MIT (vedere [`LICENSE`](LICENSE)).
- **Le due versioni di "Battle Hymn":** CC0 1.0.
- **I campioni di pianoforte:** CC BY 3.0 (Alexander Holm). `fetch-piano` li scarica e non vengono mai salvati in modo permanente.
- **Set di dati:** ciascuno ha la propria licenza sulla propria scheda.

---

Realizzato da <a href="https://mcp-tool-shop.github.io/">MCP Tool Shop</a>
