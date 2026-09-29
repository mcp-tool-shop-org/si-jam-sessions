<p align="center">
  <a href="README.ja.md">日本語</a> | <a href="README.zh.md">中文</a> | <a href="README.md">English</a> | <a href="README.fr.md">Français</a> | <a href="README.hi.md">हिन्दी</a> | <a href="README.it.md">Italiano</a> | <a href="README.pt-BR.md">Português (BR)</a>
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

**Un motor musical en el que la interpretación de una IA puede ser evaluada con precisión, reproducida exactamente y utilizada para su entrenamiento con total tranquilidad.**

Reproduzca una frase al ritmo, y el motor registrará cada nota de la interpretación:

- el momento en que se tocó, en muestras enteras;
- la nota de la partitura a la que corresponde.

Luego, evalúa cada nota en relación con la partitura. Dos máquinas que evalúan la misma interpretación coinciden en el último bit, porque
la ley de evaluación es un Rust determinista compilado en un único archivo binario de WebAssembly. La ley es determinista, por lo que una interpretación grabada se reproduce con el mismo hash sin necesidad de llamar a un modelo; CI la reproduce en cada ejecución.

`si-jam-sessions` es el equivalente de [`ai-jam-sessions`](https://github.com/mcp-tool-shop-org/ai-jam-sessions)
y un hermano de [`si-rpg-engine`](https://github.com/mcp-tool-shop-org/si-rpg-engine). El modelo propone. Un
verificador que no es el modelo acepta la propuesta o la rechaza con una razón, y la ley registra y genera el hash de
lo que acepta. El modelo nunca toca directamente la partitura.

## Escuchar

Dos arreglos de *Battle Hymn of the Republic* se reproducen uno al lado del otro en la
[página de inicio](https://mcp-tool-shop-org.github.io/si-jam-sessions/), con una vista de dónde difieren.

- **La fuente:** la transcripción de este proyecto de la edición anónima de Ditson de 1862.
- **Los arreglistas:** glm-5.3 y kimi-k3, cada uno trabajando a partir de esa fuente en Ollama Cloud.
- **La licencia:** ambos arreglos están dedicados a CC0 1.0. Cada uno superó la verificación de la licencia de la ley con sus
pruebas.
- **La grabación:** cada una se reprodujo a través de la ley en el
[Salamander Grand Piano V3](https://freepats.zenvoid.org/Piano/acoustic-grand-piano.html) de Alexander Holm
(CC BY 3.0).

| Arreglo | Duración | Notas que la ley registró | WAV sin comprimir (48 kHz, punto flotante de 32 bits) |
|---|---|---|---|
| glm-5.3 | 5:02 | 1,720 | [116 MB](https://github.com/mcp-tool-shop-org/si-jam-sessions/releases/download/v0.1.0/battle-hymn-glm-5.3.wav), SHA-256 `85b2d567…2eed` |
| kimi-k3 | 4:37 | 1,924 | [106 MB](https://github.com/mcp-tool-shop-org/si-jam-sessions/releases/download/v0.1.0/battle-hymn-kimi-k3.wav), SHA-256 `bf49d310…a017` |

La reproducción es determinista: una segunda reproducción produce los mismos bytes. El trabajo de piano de CI reprodujo cada ejemplo
completo en Linux, y ambos SHA-256 coinciden con la versión: lo mismo en Windows y Linux. Después de obtener el
piano una vez, este comando reproduce el primero:

```bash
cargo run -p host --release --locked -- render battle-hymn-glm-5.3.wav --piece battle-hymn-glm-5.3 --voice piano
```

Los hashes completos están en las [notas de la versión](https://github.com/mcp-tool-shop-org/si-jam-sessions/releases/tag/v0.1.0).

## Qué lo hace diferente

- **La interpretación es parte de la ley.** El tiempo, el tono y la velocidad son enteros en el estado con hash, por lo que "llegar con 45 ms de retraso" es un hecho que el motor puede demostrar. La forma de onda es solo una presentación.
- **El reloj nunca espera.** La ley avanza un quantum fijo, independientemente de si alguien toca o no. Un modelo planifica las frases
con antelación al horizonte de confirmación. Un plan tardío se rechaza, nunca se permite que detenga la música.
- **Cada canción se gana su lugar.** La música solo entra con pruebas:
- una composición de dominio público tanto en los EE. UU. como en la UE;
- un arreglo de dominio público, o uno que este proyecto haya grabado;
- la edición original y su año;
- una licencia en el archivo que coincida con su recepción.

Las fuentes desconocidas, de uso compartido, no comerciales y con restricciones de IA se rechazan. El material CC BY 4.0 se encuentra en su
propia categoría.
- **Los datos de entrenamiento serán una copia impresa de lo que la ley registró.** Aún no se ha creado ningún conjunto de datos. Cuando se cree uno,
cada fila se reproducirá con el mismo hash, las divisiones se realizarán por obra y se fijarán antes de que exista cualquier fila, y cada
resultado declarado indicará su poder estadístico.

## Pruébelo

Construya a partir del código fuente con Rust o ejecute el contenedor. El repositorio fija Rust 1.98.1 en `rust-toolchain.toml`,
y `rustup` lo instala en la primera compilación.

- **Windows 10 y 11** ejecutan todo, aunque aún no se ha probado la entrada en vivo con un teclado MIDI real.
- **Linux:** CI compila y prueba el host y lo reproduce con él. La reproducción a través de un dispositivo de audio de Linux no se ha probado, y la entrada en vivo es solo para Windows por ahora.
- **macOS** no se ha probado.
- **El contenedor** `ghcr.io/mcp-tool-shop-org/si-jam-sessions` ejecuta `render`, `notes`, `notices` y
`preview` sin una cadena de herramientas de Rust. La reproducción a través de un dispositivo es el mismo camino no probado de Linux.

La imagen se publica con cada etiqueta de versión. Contiene el host y las partituras, no las muestras de piano.

```bash
docker pull ghcr.io/mcp-tool-shop-org/si-jam-sessions:0.2.1
docker run --rm \
  -e HOST_UID="$(id -u)" -e HOST_GID="$(id -g)" \
  -v "$PWD:/out" \
  ghcr.io/mcp-tool-shop-org/si-jam-sessions:0.2.1 \
  notes /out/notes.json
```

En PowerShell, elimine las líneas uid y monte `${PWD}:/out`. La página del manual
[página del contenedor](https://mcp-tool-shop-org.github.io/si-jam-sessions/handbook/docker/) tiene la reproducción de piano
y el formulario de Windows. El paquete npm
[`@si-jam-sessions/si-jam-sessions`](https://www.npmjs.com/package/@si-jam-sessions/si-jam-sessions) es el
README, el registro de cambios y la licencia de esta versión, publicado por Trusted Publishing. La instalación no instala el
host. El host es el contenedor o la compilación que se muestra a continuación.

```bash
cargo run -p host --release -- devices        # list the audio outputs and MIDI inputs
cargo run -p host --release -- fetch-piano    # download and verify the grand piano once (742 MB)
cargo run -p host --release -- play           # the Battle Hymn as glm-5.3 arranged it
cargo run -p host --release -- play --piece battle-hymn-kimi-k3
cargo run -p host --release -- jam --midi 0   # play along; each note is graded as it lands
```

- `--piece` selecciona `battle-hymn-glm-5.3` (el valor predeterminado), `battle-hymn-kimi-k3` o `entertainer`, la pieza de prueba del primer
hito.
- `notes <out.json>` escribe las notas que la ley registra. La página de inicio se basa en estos archivos.
- `host help` enumera todos los comandos.

Utilice una salida cableada para `jam`. El retraso de una salida Bluetooth es mayor que los 100 ms que la ley permite para la entrega.

## Confíe en el modelo

- **Datos afectados:**
- los archivos de puntuación en `scores/`;
- las muestras de piano en una caché por usuario;
- las salidas de audio y las entradas MIDI que elija;
- los archivos que le indique que escriba.
- **Datos no afectados:** cualquier cosa fuera de esas rutas. No hay cuentas, credenciales ni telemetría.
- **El contenedor** es el mismo programa. Contiene el archivo binario y las puntuaciones, no el piano. Reduce los privilegios antes de que el host se ejecute: uid 10001, o `HOST_UID` cuando lo configure. Un directorio montado es la única
forma en que un archivo sale del contenedor.
- **Red:** solo `fetch-piano` la utiliza.
- Descarga un archivo de una dirección fija.
- Comprueba el archivo con un hash SHA-256 anclado antes de descomprimirlo.
- Rechaza los enlaces y las rutas que salen de su directorio.
- **Permisos:** una cuenta de usuario normal. No se necesitan derechos de administrador.
- **El programa** no realiza ninguna operación de entrada/salida. CI comprueba que su módulo WebAssembly no importe nada.

Para informar de una vulnerabilidad, consulte [`SECURITY.md`](SECURITY.md).

## Estado actual

- **La primera etapa está completa:** el programa, la ingestión y el origen, el hash dorado y el host. El
hash dorado es el mismo de forma nativa en x86_64 y ARM64, y como WebAssembly en V8, SpiderMonkey y
JavaScriptCore.
- **El diseño** está definido en [`docs/PHASE-0.md`](docs/PHASE-0.md).
- **La transición** en [`docs/HANDOFF.md`](docs/HANDOFF.md) cubre lo que está en curso, lo que se decidió y por qué,
y lo que viene a continuación.
- **Revisión:** antes de que se combine un cambio de código, dos modelos lo revisan en Ollama Cloud. Cada uno proviene de una familia diferente a la del autor, y cada uno desconoce la revisión del otro.

## Licencia

- **Código:** MIT (consulte [`LICENSE`](LICENSE)).
- **Los dos arreglos de Battle Hymn:** CC0 1.0.
- **Las muestras de piano:** CC BY 3.0 (Alexander Holm). `fetch-piano` las descarga y nunca
se incluyen.
- **Conjuntos de datos:** cada uno tiene su propia licencia en su propia tarjeta.

---

Creado por <a href="https://mcp-tool-shop.github.io/">MCP Tool Shop</a>
