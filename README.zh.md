<p align="center">
  <a href="README.ja.md">日本語</a> | <a href="README.md">English</a> | <a href="README.es.md">Español</a> | <a href="README.fr.md">Français</a> | <a href="README.hi.md">हिन्दी</a> | <a href="README.it.md">Italiano</a> | <a href="README.pt-BR.md">Português (BR)</a>
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

**一个音乐引擎，其中人工智能的演奏可以被精确地评分、精确地重放，并且可以放心地用于训练。**

将一段乐句与节拍对齐，引擎会记录该乐句中的每一个音符：

- 它何时演奏，以整数采样为单位；
- 它演奏了乐谱中的哪个音符。

然后，它会根据乐谱对每个音符进行评分。两台机器对同一段乐句进行评分时，结果会完全一致，因为
评分规则是确定性的 Rust 代码，编译成一个固定的 WebAssembly 二进制文件。该规则是确定性的，因此记录的
乐句在重放时会产生相同的哈希值，无需调用模型；CI 会在每次运行中重放一次。

`si-jam-sessions` 是 [`ai-jam-sessions`](https://github.com/mcp-tool-shop-org/ai-jam-sessions) 的对应物，
也是 [`si-rpg-engine`](https://github.com/mcp-tool-shop-org/si-rpg-engine) 的姊妹项目。模型提出建议。
一个不是模型的检查器会接受或拒绝该建议，并提供理由，然后该规则会确认并哈希
它所接受的内容。模型不会直接触及乐谱。

## 收听

两段《共和国战歌》的乐曲在
[登陆页面](https://mcp-tool-shop-org.github.io/si-jam-sessions/) 上并排播放，以便查看它们之间的差异。

- **原始乐谱：** 这是该项目对 1862 年匿名出版的 Ditson 版本的转录。
- **编曲者：** glm-5.3 和 kimi-k3，它们都基于该原始乐谱在 Ollama Cloud 上进行创作。
- **许可：** 这两段乐曲都采用 CC0 1.0 许可。它们都通过了该规则的许可检查，并提供了相应的证据。
- **录音：** 它们都通过该规则在
[Salamander Grand Piano V3](https://freepats.zenvoid.org/Piano/acoustic-grand-piano.html) 上进行了渲染，该钢琴由 Alexander Holm 创作（CC BY 3.0）。

| 编曲 | 长度 | 该规则确认的音符 | 未压缩的 WAV 文件（48 kHz，32 位浮点） |
|---|---|---|---|
| glm-5.3 | 5:02 | 1,720 | [116 MB](https://github.com/mcp-tool-shop-org/si-jam-sessions/releases/download/v0.1.0/battle-hymn-glm-5.3.wav)，SHA-256 `85b2d567…2eed` |
| kimi-k3 | 4:37 | 1,924 | [106 MB](https://github.com/mcp-tool-shop-org/si-jam-sessions/releases/download/v0.1.0/battle-hymn-kimi-k3.wav)，SHA-256 `bf49d310…a017` |

渲染是确定性的：在制作这些乐曲的 Windows 机器上，第二次渲染会产生相同的字节。
尚未对其他平台上的渲染进行比较。在首次获取钢琴后，此命令会重现第一个乐曲：

```bash
cargo run -p host --release --locked -- render battle-hymn-glm-5.3.wav --piece battle-hymn-glm-5.3 --voice piano
```

完整的哈希值可以在 [发布说明](https://github.com/mcp-tool-shop-org/si-jam-sessions/releases/tag/v0.1.0) 中找到。

## 它与众不同之处

- **乐句是规则的一部分。** 时间、音高和力度都是哈希状态中的整数，因此“延迟 45 毫秒”是引擎可以证明的事实。波形只是呈现方式。
- **时钟不会等待。** 无论是否有人演奏，该规则都会以固定的时间间隔进行。模型会提前规划乐句，以应对提交时间范围。延迟的计划会被拒绝，绝不会被允许导致音乐停顿。
- **每首乐曲都应得其应有的地位。** 音乐只能在提供证据的情况下才能进入：
- 一首在美国和欧盟都属于公共领域的乐曲；
- 一首属于公共领域的乐曲，或一首由本项目创作的乐曲；
- 原始乐谱及其年份；
- 一个与乐曲来源相匹配的内嵌许可。

未知的、共享许可、非商业用途和限制人工智能使用的乐曲将被拒绝。CC BY 4.0 许可的乐曲将位于其自己的归属层级中。
- **训练数据将是该规则确认的内容的打印版本。** 尚未构建任何数据集。当构建数据集时，
每一行都会重放为相同的哈希值，分割将按作品进行，并且在任何行存在之前都会固定，并且每个
声明的结果都会说明其统计效力。

## 试用

您需要 Rust。该仓库在 `rust-toolchain.toml` 中固定了版本 1.98.1，并且 `rustup` 会在第一次构建时安装它。

- **Windows 10 和 11** 可以运行所有内容，但尚未尝试使用真实的 MIDI 键盘进行实时输入。
- **Linux：** CI 会构建和测试主机，并使用它进行渲染。通过 Linux 音频设备进行播放尚未进行测试，并且实时输入目前仅适用于 Windows。
- **macOS** 尚未进行测试。

```bash
cargo run -p host --release -- devices        # list the audio outputs and MIDI inputs
cargo run -p host --release -- fetch-piano    # download and verify the grand piano once (742 MB)
cargo run -p host --release -- play           # the Battle Hymn as glm-5.3 arranged it
cargo run -p host --release -- play --piece battle-hymn-kimi-k3
cargo run -p host --release -- jam --midi 0   # play along; each note is graded as it lands
```

- `--piece` 选择 `battle-hymn-glm-5.3`（默认）、`battle-hymn-kimi-k3` 或 `entertainer`，即第一个
里程碑的测试乐曲。
- `notes <out.json>` 写入该规则确认的音符。登陆页面会从这些文件中获取数据。
- `host help` 列出所有命令。

对于 `jam`，请使用有线输出。蓝牙输出的延迟时间超过该规则允许的 100 毫秒。

## 信任模型

- **已触及的数据：**
- `scores/` 中的乐谱文件；
- 每个用户缓存中的钢琴采样；
- 您选择的音频输出和 MIDI 输入；
- 您要求它写入的文件。
- **未触及的数据：** 任何位于这些路径之外的数据。没有帐户、没有凭据，也没有遥测数据。
- **网络：** 只有 `fetch-piano` 使用网络。
- 它从一个固定的地址下载一个存档。
- 在解压缩任何内容之前，它会检查该存档是否与固定的 SHA-256 相匹配。
- 它拒绝链接和超出其目录范围的路径。
- **权限：** 普通用户帐户。不需要管理员权限。
- **该规则** 不进行任何 I/O 操作。CI 检查其 WebAssembly 模块是否没有导入任何内容。

要报告漏洞，请参阅 [`SECURITY.md`](SECURITY.md)。

## 当前状态

- **第一个里程碑已完成：**包括法律、数据摄取和来源、黄金哈希以及主机。黄金哈希在 x86_64 和 ARM64 架构上以及在 V8、SpiderMonkey 和 JavaScriptCore 下的 WebAssembly 环境中都是相同的。
- **设计**已确定，详情请参见 [`docs/PHASE-0.md`](docs/PHASE-0.md)。
- **交接**，详情请参见 [`docs/HANDOFF.md`](docs/HANDOFF.md)，涵盖了当前正在进行的工作、已做出的决定及其原因，以及下一步的计划。
- **审核：**在代码更改合并之前，两个模型会在 Ollama Cloud 上对其进行审核。这两个模型来自不同的家族，且彼此不知道对方的审核结果。

## 许可

- **代码：**MIT（参见 [`LICENSE`](LICENSE)）。
- **两首《战歌》的改编版本：**CC0 1.0。
- **钢琴采样：**CC BY 3.0（Alexander Holm）。`fetch-piano` 会下载这些采样，并且它们不会被提交到代码库中。
- **数据集：**每个数据集都有其自身的许可协议，具体信息请参见其各自的说明文档。

---

由 <a href="https://mcp-tool-shop.github.io/">MCP Tool Shop</a> 构建。
