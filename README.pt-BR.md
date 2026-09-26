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

Em seguida, ele avalia cada nota em relação à partitura. Duas máquinas que avaliam a mesma gravação concordam em todos os detalhes, porque
a lei de avaliação é um código Rust determinístico, compilado em um único arquivo WebAssembly. A lei é determinística, portanto, uma gravação
reproduz o mesmo hash sem chamar um modelo; o CI a reproduz em cada execução.

`si-jam-sessions` é o equivalente de [`ai-jam-sessions`](https://github.com/mcp-tool-shop-org/ai-jam-sessions)
e um projeto irmão de [`si-rpg-engine`](https://github.com/mcp-tool-shop-org/si-rpg-engine). O modelo faz uma proposta. Um
verificador, que não é o modelo, aceita ou rejeita a proposta com uma justificativa, e a lei registra e calcula o hash
do que é aceito. O modelo nunca toca diretamente na partitura.

## Ouça

Duas versões de *Battle Hymn of the Republic* são reproduzidas lado a lado na
[página inicial](https://mcp-tool-shop-org.github.io/si-jam-sessions/), mostrando onde elas diferem.

- **A fonte:** a transcrição deste projeto da edição anônima de 1862 da Ditson.
- **Os arranjadores:** glm-5.3 e kimi-k3, cada um trabalhando a partir dessa fonte no Ollama Cloud.
- **A licença:** ambos os arranjos são dedicados sob a licença CC0 1.0. Cada um passou no teste de licença da lei com suas
evidências.
- **A gravação:** cada uma foi renderizada através da lei no
[Salamander Grand Piano V3](https://freepats.zenvoid.org/Piano/acoustic-grand-piano.html) de Alexander Holm
(CC BY 3.0).

| Arranjo | Duração | Notas que a lei registrou | WAV não compactado (48 kHz, 32 bits float) |
|---|---|---|---|
| glm-5.3 | 5:02 | 1,720 | [116 MB](https://github.com/mcp-tool-shop-org/si-jam-sessions/releases/download/v0.1.0/battle-hymn-glm-5.3.wav), SHA-256 `85b2d567…2eed` |
| kimi-k3 | 4:37 | 1,924 | [106 MB](https://github.com/mcp-tool-shop-org/si-jam-sessions/releases/download/v0.1.0/battle-hymn-kimi-k3.wav), SHA-256 `bf49d310…a017` |

A renderização é determinística: uma segunda renderização produz os mesmos bytes. O trabalho de piano do CI
renderizou cada exemplar inteiro no Linux, e os dois SHA-256 são os da release: os mesmos no Windows e no Linux.
Após buscar o piano uma vez, este comando reproduz a primeira versão:

```bash
cargo run -p host --release --locked -- render battle-hymn-glm-5.3.wav --piece battle-hymn-glm-5.3 --voice piano
```

Os hashes completos estão nas [notas de lançamento](https://github.com/mcp-tool-shop-org/si-jam-sessions/releases/tag/v0.1.0).

## O que o torna diferente

- **A gravação faz parte da lei.** O tempo, a altura e a velocidade são inteiros no estado com hash, portanto, "atrasado em
45 ms" é um fato que o motor pode provar. A forma de onda é apenas uma apresentação.
- **O relógio nunca espera.** A lei avança em um quantum fixo, independentemente de alguém tocar ou não. Um modelo planeja frases
com antecedência em relação a um horizonte de compromisso. Um plano tardio é rejeitado, nunca permitido a atrasar a música.
- **Cada música conquista seu lugar.** A música entra apenas com evidências:
- uma composição de domínio público nos EUA e na UE;
- um arranjo de domínio público ou um que este projeto criou;
- a edição original e seu ano;
- uma licença no arquivo que corresponda ao seu recebimento.

Fontes desconhecidas, com licença de compartilhamento, não comerciais e restritas à IA são rejeitadas. O material CC BY 4.0 está em sua
própria camada com atribuição.
- **Os dados de treinamento serão uma impressão do que a lei registrou.** Nenhum conjunto de dados foi criado ainda. Quando um for,
cada linha reproduzirá o mesmo hash, as divisões serão por obra e fixadas antes que qualquer linha exista, e cada
resultado reivindicado indicará seu poder estatístico.

## Experimente

Você precisa do Rust. O repositório fixa a versão 1.98.1 em `rust-toolchain.toml`, e `rustup` a instala na
primeira construção.

- **Windows 10 e 11** executam tudo, embora a entrada ao vivo ainda não tenha sido testada com um teclado MIDI real.
- **Linux:** o CI constrói e testa o host e renderiza com ele. A reprodução por meio de um dispositivo de áudio Linux não foi testada, e a entrada ao vivo é apenas para Windows por enquanto.
- **macOS** não foi testado.

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

- **Dados acessados:**
- os arquivos de partitura em `scores/`;
- as amostras de piano em um cache por usuário;
- as saídas de áudio e as entradas MIDI que você escolher;
- os arquivos que você pedir para ele gravar.
- **Dados não acessados:** qualquer coisa fora desses caminhos. Não há contas, credenciais ou telemetria.
- **Rede:** apenas `fetch-piano` a usa.
- Ele baixa um arquivo de um endereço fixo.
- Ele verifica o arquivo em relação a um SHA-256 fixo antes de descompactar qualquer coisa.
- Ele rejeita links e caminhos que saem de seu diretório.
- **Permissões:** uma conta de usuário comum. Nada precisa de direitos de administrador.
- **A lei** não faz nenhuma operação de E/S. O CI verifica que seu módulo WebAssembly não importa nada.

Para relatar uma vulnerabilidade, consulte [`SECURITY.md`](SECURITY.md).

## Onde está

- **O primeiro marco foi concluído:** a lei, a ingestão e a proveniência, o hash dourado e o host. O hash dourado é o mesmo nativamente em x86_64 e ARM64, e também como WebAssembly no V8, SpiderMonkey e JavaScriptCore.
- **O projeto** está finalizado em [`docs/PHASE-0.md`](docs/PHASE-0.md).
- **A transferência** em [`docs/HANDOFF.md`](docs/HANDOFF.md) abrange o que está em andamento, o que foi decidido e por quê, e o que acontecerá a seguir.
- **Revisão:** antes que uma alteração de código seja integrada, dois modelos a revisam no Ollama Cloud. Cada um pertence a uma família diferente da do autor, e cada um desconhece a revisão do outro.

## Licença

- **Código:** MIT (veja [`LICENSE`](LICENSE)).
- **As duas versões de Battle Hymn:** CC0 1.0.
- **As amostras de piano:** CC BY 3.0 (Alexander Holm). `fetch-piano` as baixa, e elas nunca são adicionadas ao repositório.
- **Conjuntos de dados:** cada um tem sua própria licença em seu próprio arquivo.

---

Criado por <a href="https://mcp-tool-shop.github.io/">MCP Tool Shop</a>
