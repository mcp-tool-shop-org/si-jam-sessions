<p align="center">
  <a href="README.ja.md">日本語</a> | <a href="README.zh.md">中文</a> | <a href="README.es.md">Español</a> | <a href="README.fr.md">Français</a> | <a href="README.hi.md">हिन्दी</a> | <a href="README.it.md">Italiano</a> | <a href="README.md">English</a>
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

**Um motor musical onde a execução de uma IA pode ser avaliada com precisão, reproduzida com exatidão e utilizada para treinamento com total tranquilidade.**

Toque uma frase no ritmo e o motor registra cada nota da gravação:

- quando ela foi tocada, em amostras inteiras;
- qual nota da partitura ela corresponde.

Em seguida, ele avalia cada nota em relação à partitura. Duas máquinas avaliando a mesma gravação concordam até o último bit, porque
a lei de avaliação é um código Rust determinístico, compilado em um único binário WebAssembly. A lei é determinística, portanto, uma gravação
reproduzida resulta no mesmo hash, sem a necessidade de chamar um modelo; o CI reproduz uma em cada execução.

`si-jam-sessions` é o equivalente de [`ai-jam-sessions`](https://github.com/mcp-tool-shop-org/ai-jam-sessions)
e um projeto irmão de [`si-rpg-engine`](https://github.com/mcp-tool-shop-org/si-rpg-engine). O modelo propõe. Um
verificador, que não é o modelo, aceita ou rejeita a proposta com uma justificativa, e a lei registra e calcula o hash
do que é aceito. O modelo nunca toca diretamente na partitura.

## Ouça

Duas versões de *Battle Hymn of the Republic* são reproduzidas lado a lado na
[página inicial](https://mcp-tool-shop-org.github.io/si-jam-sessions/), com uma visualização das diferenças entre elas.

- **A fonte:** a transcrição deste projeto da edição anônima de 1862 da Ditson.
- **Os arranjadores:** glm-5.3 e kimi-k3, cada um trabalhando a partir dessa fonte no Ollama Cloud.
- **A licença:** ambos os arranjos são dedicados sob a licença CC0 1.0. Cada um passou pela verificação da licença da lei com suas
respectivas evidências.
- **A gravação:** cada um foi renderizado através da lei no
[Salamander Grand Piano V3](https://freepats.zenvoid.org/Piano/acoustic-grand-piano.html) de Alexander Holm
(CC BY 3.0).

| Arranjo | Duração | Notas que a lei registrou | WAV não compactado (48 kHz, ponto flutuante de 32 bits) |
|---|---|---|---|
| glm-5.3 | 5:02 | 1,720 | [116 MB](https://github.com/mcp-tool-shop-org/si-jam-sessions/releases/download/v0.1.0/battle-hymn-glm-5.3.wav), SHA-256 `85b2d567…2eed` |
| kimi-k3 | 4:37 | 1,924 | [106 MB](https://github.com/mcp-tool-shop-org/si-jam-sessions/releases/download/v0.1.0/battle-hymn-kimi-k3.wav), SHA-256 `bf49d310…a017` |

A renderização é determinística: uma segunda renderização produz os mesmos bytes. O trabalho de piano do CI renderizou cada exemplar
completamente no Linux, e ambos os SHA-256 correspondem à versão: o mesmo no Windows e no Linux. Após buscar o
piano uma vez, este comando reproduz o primeiro:

```bash
cargo run -p host --release --locked -- render battle-hymn-glm-5.3.wav --piece battle-hymn-glm-5.3 --voice piano
```

Os hashes completos estão nas [notas da versão](https://github.com/mcp-tool-shop-org/si-jam-sessions/releases/tag/v0.1.0).

## O que o torna diferente

- **A gravação faz parte da lei.** O tempo, o tom e a velocidade são inteiros no estado com hash, portanto, "atrasado em
45 ms" é um fato que o motor pode provar. A forma de onda é apenas uma apresentação.
- **O relógio nunca espera.** A lei avança em um quantum fixo, independentemente de alguém tocar ou não. Um modelo planeja frases
com antecedência em relação a um horizonte de compromisso. Um plano tardio é rejeitado, nunca permitido a atrasar a música.
- **Cada música conquista seu lugar.** A música entra apenas com evidências:
- uma composição de domínio público tanto nos EUA quanto na UE;
- um arranjo de domínio público, ou um que este projeto criou;
- a edição de origem e seu ano;
- uma licença no arquivo que corresponda ao seu recebimento.

Fontes desconhecidas, de compartilhamento semelhante, não comerciais e com restrições de IA são rejeitadas. O material CC BY 4.0 está em sua
própria camada com atribuição.
- **Os dados de treinamento serão uma impressão do que a lei registrou.** Ainda não foi criado nenhum conjunto de dados. Quando for,
cada linha reproduzirá o mesmo hash, as divisões serão por obra e fixadas antes que qualquer linha exista, e cada
resultado reivindicado indicará seu poder estatístico.

## Experimente

Compile a partir do código-fonte com Rust ou execute o contêiner. O repositório define o Rust 1.98.1 em `rust-toolchain.toml`,
e `rustup` o instala na primeira compilação.

- **Windows 10 e 11** executam tudo, embora a entrada ao vivo ainda não tenha sido testada com um teclado MIDI real.
- **Linux:** o CI compila e testa o host e renderiza com ele. A reprodução por meio de um dispositivo de áudio Linux não foi testada, e a entrada ao vivo é apenas para Windows por enquanto.
- **macOS** não foi testado.
- **O contêiner** `ghcr.io/mcp-tool-shop-org/si-jam-sessions` executa `render`, `notes`, `notices` e
`preview` sem uma cadeia de ferramentas Rust. A reprodução por meio de um dispositivo é o mesmo caminho Linux não testado.

A imagem é publicada com cada tag de versão. Ela contém o host e as partituras, mas não as amostras de piano.

```bash
docker pull ghcr.io/mcp-tool-shop-org/si-jam-sessions:0.2.1
docker run --rm \
  -e HOST_UID="$(id -u)" -e HOST_GID="$(id -g)" \
  -v "$PWD:/out" \
  ghcr.io/mcp-tool-shop-org/si-jam-sessions:0.2.1 \
  notes /out/notes.json
```

No PowerShell, remova as linhas de uid e monte `${PWD}:/out`. A página do
[contêiner](https://mcp-tool-shop-org.github.io/si-jam-sessions/handbook/docker/) do manual tem a renderização do piano
e o formulário do Windows. O pacote npm
[`@si-jam-sessions/si-jam-sessions`](https://www.npmjs.com/package/@si-jam-sessions/si-jam-sessions) é o
README, o registro de alterações e a licença desta versão, publicados pelo Trusted Publishing. A instalação não instala o
host. O host é o contêiner ou a compilação abaixo.

```bash
cargo run -p host --release -- devices        # list the audio outputs and MIDI inputs
cargo run -p host --release -- fetch-piano    # download and verify the grand piano once (742 MB)
cargo run -p host --release -- play           # the Battle Hymn as glm-5.3 arranged it
cargo run -p host --release -- play --piece battle-hymn-kimi-k3
cargo run -p host --release -- jam --midi 0   # play along; each note is graded as it lands
```

- `--piece` seleciona `battle-hymn-glm-5.3` (o padrão), `battle-hymn-kimi-k3` ou `entertainer`, a peça de teste do primeiro
marco.
- `notes <out.json>` grava as notas que a lei registra. A página inicial extrai dados desses arquivos.
- `host help` lista todos os comandos.

Use uma saída com fio para `jam`. O atraso de uma saída Bluetooth é maior do que os 100 ms que a lei permite para a entrega.

## Confie no modelo

- **Dados afetados:**
- os arquivos de partitura em `scores/`;
- as amostras de piano em um cache por usuário;
- as saídas de áudio e as entradas MIDI que você escolher;
- os arquivos que você solicitar que sejam gravados.
- **Dados não afetados:** tudo o que estiver fora desses caminhos. Não há contas, credenciais ou telemetria.
- **O contêiner** é o mesmo programa. Ele contém o binário e as partituras, não o piano. Ele reduz os privilégios antes que o host seja executado: uid 10001 ou `HOST_UID`, quando você o definir. Um diretório montado é a única maneira de um arquivo sair do contêiner.
- **Rede:** apenas `fetch-piano` a utiliza.
- Ele baixa um arquivo de um endereço fixo.
- Ele verifica o arquivo em relação a um hash SHA-256 fixo antes de descompactar qualquer coisa.
- Ele rejeita links e caminhos que saem de seu diretório.
- **Permissões:** uma conta de usuário comum. Nada precisa de direitos de administrador.
- **O programa** não realiza nenhuma operação de entrada/saída. O CI verifica se seu módulo WebAssembly não importa nada.

Para relatar uma vulnerabilidade, consulte [`SECURITY.md`](SECURITY.md).

## Situação atual

- **O primeiro marco foi concluído:** o programa, a ingestão e a proveniência, o hash de referência e o host. O hash de referência é o mesmo nativamente em x86_64 e ARM64, e como WebAssembly no V8, SpiderMonkey e JavaScriptCore.
- **O projeto** está definido em [`docs/PHASE-0.md`](docs/PHASE-0.md).
- **A transição** em [`docs/HANDOFF.md`](docs/HANDOFF.md) abrange o que está em andamento, o que foi decidido e por quê, e o que vem a seguir.
- **Revisão:** antes que uma alteração de código seja mesclada, dois modelos a revisam no Ollama Cloud. Cada um vem de uma família diferente da do autor, e cada um não tem conhecimento da revisão do outro.

## Licença

- **Código:** MIT (consulte [`LICENSE`](LICENSE)).
- **As duas versões de "Battle Hymn":** CC0 1.0.
- **As amostras de piano:** CC BY 3.0 (Alexander Holm). `fetch-piano` as baixa, e elas nunca são incluídas.
- **Conjuntos de dados:** cada um carrega sua própria licença em seu próprio cartão.

---

Criado por <a href="https://mcp-tool-shop.github.io/">MCP Tool Shop</a>
