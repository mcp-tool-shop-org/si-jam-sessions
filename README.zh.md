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

**一个音乐引擎，可以对人工智能的演奏进行精确的评分、精确的回放，并以放心的态度进行训练。**

将一段乐句与节拍对齐，引擎会记录该乐句中的每一个音符：

- 记录音符出现的时间，以整数采样为单位；
- 记录该音符在乐谱中的位置。

然后，它会根据乐谱对每个音符进行评分。两台机器对同一段乐句进行评分，结果会完全一致，因为
评分规则是确定性的 Rust 代码，编译成一个固定的 WebAssembly 二进制文件。由于规则是确定性的，因此记录的乐句在回放时会产生相同的哈希值，无需调用模型；CI 会在每次运行中回放一次。

`si-jam-sessions` 是 [`ai-jam-sessions`](https://github.com/mcp-tool-shop-org/ai-jam-sessions) 的对应物，
也是 [`si-rpg-engine`](https://github.com/mcp-tool-shop-org/si-rpg-engine) 的一个姊妹项目。模型提出建议。
一个不是模型的检查器会接受或拒绝该建议，并给出理由，然后规则会提交并哈希它所接受的内容。模型不会直接操作乐谱。

## 收听

两段《共和国战歌》的乐曲在 [登录页面](https://mcp-tool-shop-org.github.io/si-jam-sessions/) 上并排播放，以便查看它们之间的差异。

- **原始乐谱：** 这是该项目对 1862 年匿名出版的 Ditson 版本的乐谱进行的转录。
- **编曲者：** glm-5.3 和 kimi-k3，它们都使用该原始乐谱在 Ollama Cloud 上进行编曲。
- **许可：** 两段乐曲都采用 CC0 1.0 许可。它们都通过了规则的许可检查，并提供了相应的证据。
- **录音：** 它们都通过规则，使用 Alexander Holm 的 [Salamander Grand Piano V3](https://freepats.zenvoid.org/Piano/acoustic-grand-piano.html) 进行渲染（CC BY 3.0）。

| 编曲 | 长度 | 规则所提交的音符 | 未压缩的 WAV 文件（48 kHz，32 位浮点数） |
|---|---|---|---|
| glm-5.3 | 5:02 | 1,720 | [116 MB](https://github.com/mcp-tool-shop-org/si-jam-sessions/releases/download/v0.1.0/battle-hymn-glm-5.3.wav)，SHA-256 `85b2d567…2eed` |
| kimi-k3 | 4:37 | 1,924 | [106 MB](https://github.com/mcp-tool-shop-org/si-jam-sessions/releases/download/v0.1.0/battle-hymn-kimi-k3.wav)，SHA-256 `bf49d310…a017` |

渲染是确定性的：第二次渲染会产生相同的数据。CI 的钢琴任务在 Linux 上渲染了每个示例，并且两个 SHA-256 值都与发布版本相同：在 Windows 和 Linux 上也是如此。在首次获取钢琴后，此命令可以重现第一个示例：

```bash
cargo run -p host --release --locked -- render battle-hymn-glm-5.3.wav --piece battle-hymn-glm-5.3 --voice piano
```

完整的哈希值可以在 [发布说明](https://github.com/mcp-tool-shop-org/si-jam-sessions/releases/tag/v0.1.0) 中找到。

## 它与众不同之处

- **乐句是规则的一部分。** 时间、音高和力度都是哈希状态中的整数，因此“延迟 45 毫秒”是引擎可以证明的事实。波形只是呈现方式。
- **时钟不会等待。** 规则会以固定的时间间隔进行，无论是否有人演奏。模型会提前规划乐句，以应对提交时间范围。延迟的计划会被拒绝，绝不会被允许导致音乐停顿。
- **每首乐曲都必须证明其价值。** 音乐只有在提供证据的情况下才能进入：
- 一首在美国和欧盟都属于公共领域的乐曲；
- 一首属于公共领域的编曲，或者该项目创作的编曲；
- 原始乐谱及其年份；
- 一个与乐曲来源相匹配的内嵌许可。

未知的、共享许可、非商业用途和限制人工智能使用的乐曲将被拒绝。CC BY 4.0 许可的乐曲将位于其自己的归属层级中。
- **训练数据将是规则所提交内容的打印版本。** 尚未构建任何数据集。当构建数据集时，每一行都会回放至相同的哈希值，分割将按作品进行，并且在任何行存在之前都会固定，并且每个声明的结果都会说明其统计效力。

## 试用

使用 Rust 从源代码构建，或运行容器。该仓库在 `rust-toolchain.toml` 中固定了 Rust 1.98.1，
并且 `rustup` 会在首次构建时安装它。

- **Windows 10 和 11** 可以运行所有内容，但尚未尝试使用真实的 MIDI 键盘进行实时输入。
- **Linux：** CI 构建并测试主机，并使用它进行渲染。通过 Linux 音频设备进行播放尚未测试，并且实时输入目前仅适用于 Windows。
- **macOS** 尚未测试。
- **容器** `ghcr.io/mcp-tool-shop-org/si-jam-sessions` 运行 `render`、`notes`、`notices` 和
`preview`，无需 Rust 工具链。通过设备进行播放与未测试的 Linux 路径相同。

该镜像会与每个版本标签一起发布。它包含主机和乐谱，但不包含钢琴采样。

```bash
docker pull ghcr.io/mcp-tool-shop-org/si-jam-sessions:0.2.1
docker run --rm \
  -e HOST_UID="$(id -u)" -e HOST_GID="$(id -g)" \
  -v "$PWD:/out" \
  ghcr.io/mcp-tool-shop-org/si-jam-sessions:0.2.1 \
  notes /out/notes.json
```

在 PowerShell 上，删除 uid 行并挂载 `${PWD}:/out`。手册的
[容器页面](https://mcp-tool-shop-org.github.io/si-jam-sessions/handbook/docker/) 包含钢琴渲染和 Windows 表单。npm 包
[`@si-jam-sessions/si-jam-sessions`](https://www.npmjs.com/package/@si-jam-sessions/si-jam-sessions) 是此
版本发布的 README、变更日志和许可，由 Trusted Publishing 发布。安装它不会安装主机。主机是容器，或者是下面的构建。

```bash
cargo run -p host --release -- devices        # list the audio outputs and MIDI inputs
cargo run -p host --release -- fetch-piano    # download and verify the grand piano once (742 MB)
cargo run -p host --release -- play           # the Battle Hymn as glm-5.3 arranged it
cargo run -p host --release -- play --piece battle-hymn-kimi-k3
cargo run -p host --release -- jam --midi 0   # play along; each note is graded as it lands
```

- `--piece` 选择 `battle-hymn-glm-5.3`（默认值）、`battle-hymn-kimi-k3` 或 `entertainer`，即第一个
里程碑的测试乐曲。
- `notes <out.json>` 写入规则所提交的音符。登录页面从这些文件中获取数据。
- `host help` 列出每个命令。

对于 `jam`，请使用有线输出。蓝牙输出的延迟时间超过规则允许的 100 毫秒。

## 信任模型

- **涉及的数据：**
- `scores/` 中的乐谱文件；
- 每个用户缓存中的钢琴音效；
- 您选择的音频输出和 MIDI 输入；
- 您要求它写入的文件。
- **不涉及的数据：** 任何位于上述路径之外的数据。不涉及任何账户、凭据或遥测数据。
- **容器：** 实际上就是同一个程序。它包含二进制文件和乐谱，但不包含钢琴音效。在主机运行之前，它会降低权限：uid 为 10001，或者如果您设置了，则为 `HOST_UID`。挂载的目录是文件离开容器的唯一途径。
- **网络：** 只有 `fetch-piano` 会使用网络。
- 它从一个固定的地址下载一个压缩包。
- 在解压缩任何内容之前，它会检查压缩包是否与固定的 SHA-256 校验值匹配。
- 它会拒绝链接以及指向其目录之外的路径。
- **权限：** 普通用户账户。不需要管理员权限。
- **“法律”模块：** 完全不进行任何 I/O 操作。CI 检查确保其 WebAssembly 模块不导入任何内容。

如需报告漏洞，请参阅 [`SECURITY.md`](SECURITY.md)。

## 当前进展

- **第一个里程碑已完成：** “法律”模块、数据摄取和来源验证、黄金哈希值以及主机。黄金哈希值在 x86_64 和 ARM64 架构上以及在 V8、SpiderMonkey 和 JavaScriptCore 下的 WebAssembly 环境中都是相同的。
- **设计已确定：** 参见 [`docs/PHASE-0.md`](docs/PHASE-0.md)。
- **过渡：** [`docs/HANDOFF.md`](docs/HANDOFF.md) 涵盖了正在进行的工作、已做出的决定及其原因，以及下一步计划。
- **审查：** 在代码合并之前，两个模型会在 Ollama Cloud 上对其进行审查。每个模型都来自不同的家族，并且彼此不知道对方的审查结果。

## 许可

- **代码：** MIT（参见 [`LICENSE`](LICENSE)）。
- **两首《战歌》的改编版本：** CC0 1.0。
- **钢琴音效：** CC BY 3.0（Alexander Holm）。`fetch-piano` 会下载这些音效，并且它们不会被提交到代码库中。
- **数据集：** 每个数据集都有自己的许可证，并且许可证信息包含在其各自的卡片中。

---

由 <a href="https://mcp-tool-shop.github.io/">MCP Tool Shop</a> 构建。
