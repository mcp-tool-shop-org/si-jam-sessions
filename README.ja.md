<p align="center">
  <a href="README.md">English</a> | <a href="README.zh.md">中文</a> | <a href="README.es.md">Español</a> | <a href="README.fr.md">Français</a> | <a href="README.hi.md">हिन्दी</a> | <a href="README.it.md">Italiano</a> | <a href="README.pt-BR.md">Português (BR)</a>
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

**AIによる演奏を正確に評価し、正確に再生し、安心してトレーニングできる音楽エンジン。**

フレーズをビートに合わせて演奏すると、エンジンは演奏のすべての音符を記録します。

- 音符が鳴ったタイミング（整数サンプル単位）。
- スコアのどの音符に対応するか。

次に、エンジンは各音符をスコアと比較して評価します。同じ演奏を2つのマシンで評価した場合、結果は完全に一致します。なぜなら、評価のルールは決定的なRustで記述され、単一のWebAssemblyバイナリにコンパイルされるからです。ルールは決定的なため、記録された演奏はモデルを呼び出すことなく、常に同じハッシュ値を返します。CI（継続的インテグレーション）では、毎回このテストを実行します。

`si-jam-sessions`は、[`ai-jam-sessions`](https://github.com/mcp-tool-shop-org/ai-jam-sessions)の対となるものであり、[`si-rpg-engine`](https://github.com/mcp-tool-shop-org/si-rpg-engine)の兄弟プロジェクトです。モデルが提案を行い、モデル以外のチェッカーがその提案を承認または拒否し、その理由を提示します。そして、ルールが承認したものを記録し、ハッシュ化します。モデルはスコアに直接触れることはありません。

## 試聴

「共和国賛歌」の2つのアレンジバージョンが、[ランディングページ](https://mcp-tool-shop-org.github.io/si-jam-sessions/)で並行して再生され、その違いを確認できます。

- **ソース:** このプロジェクトで作成された、1862年に匿名で出版されたDitson版の楽譜。
- **アレンジ:** glm-5.3とkimi-k3。それぞれがOllama Cloud上で、同じソースに基づいてアレンジを作成。
- **ライセンス:** どちらのアレンジもCC0 1.0で公開。それぞれがルールのライセンスチェックに合格し、証拠を提示。
- **録音:** それぞれが、Alexander Holmによる[Salamander Grand Piano V3](https://freepats.zenvoid.org/Piano/acoustic-grand-piano.html)（CC BY 3.0）を使用して、ルールに基づいてレンダリングされました。

| アレンジ | 長さ | ルールが記録した音符 | 非圧縮WAV（48 kHz、32ビット浮動小数点） |
|---|---|---|---|
| glm-5.3 | 5:02 | 1,720 | [116 MB](https://github.com/mcp-tool-shop-org/si-jam-sessions/releases/download/v0.1.0/battle-hymn-glm-5.3.wav)、SHA-256 `85b2d567…2eed` |
| kimi-k3 | 4:37 | 1,924 | [106 MB](https://github.com/mcp-tool-shop-org/si-jam-sessions/releases/download/v0.1.0/battle-hymn-kimi-k3.wav)、SHA-256 `bf49d310…a017` |

レンダリングは決定的な処理です。2回レンダリングすると、同じバイト列が得られます。CIのピアノジョブは、各サンプルをLinux上で完全にレンダリングし、両方のSHA-256ハッシュがリリースと一致します。これは、WindowsとLinuxの両方で同じ結果が得られることを意味します。ピアノを一度フェッチした後、次のコマンドで最初のレンダリングを再現できます。

```bash
cargo run -p host --release --locked -- render battle-hymn-glm-5.3.wav --piece battle-hymn-glm-5.3 --voice piano
```

完全なハッシュ値は、[リリースノート](https://github.com/mcp-tool-shop-org/si-jam-sessions/releases/tag/v0.1.0)に記載されています。

## このプロジェクトの特長

- **演奏はルールの不可欠な一部です。** タイミング、ピッチ、ベロシティはハッシュ化された状態の整数として記録されるため、「45ミリ秒遅れ」はエンジンが証明できる事実です。波形は単なる表現です。
- **クロックは決して待機しません。** ルールは、誰かが演奏するかどうかに関係なく、固定された量子でステップを進めます。モデルは、コミットの期限よりも先にフレーズを計画します。遅れた計画は拒否され、音楽が中断されることはありません。
- **すべての曲は、その価値を証明します。** 音楽は、証拠とともにのみ追加されます。
- 米国とEUの両方でパブリックドメインにある楽曲。
- パブリックドメインにあるアレンジ、またはこのプロジェクトが作成したアレンジ。
- ソースとなる楽譜とその発行年。
- ファイル内に含まれるライセンスが、その楽譜の入手方法と一致すること。

不明、共有、非営利、AI制限付きのソースは拒否されます。CC BY 4.0の素材は、独自の属性が付与された層に配置されます。
- **トレーニングデータは、ルールが記録した内容のプリントアウトになります。** まだデータセットは作成されていません。作成された場合、各行は同じハッシュ値を返し、分割は事前に固定され、すべての行が存在する前に、各行は統計的な信頼性を示すことになります。

## 試してみてください

Rustを使用してソースからビルドするか、コンテナを実行します。リポジトリは、`rust-toolchain.toml`でRust 1.98.1を固定し、`rustup`が最初のビルド時にインストールします。

- **Windows 10および11**で、すべてを実行できますが、実際のMIDIキーボードを使用したライブ入力はまだ試されていません。
- **Linux:** CIはホストをビルドおよびテストし、それを使用してレンダリングを行います。Linuxオーディオデバイスを介した再生はテストされておらず、ライブ入力は現時点ではWindows専用です。
- **macOS**はテストされていません。
- **コンテナ** `ghcr.io/mcp-tool-shop-org/si-jam-sessions`は、Rustツールチェーンなしで、`render`、`notes`、`notices`、および`preview`を実行します。デバイスを介した再生は、テストされていないLinuxと同じ方法で行われます。

イメージは、各バージョンタグとともに公開されます。イメージには、ホストと楽譜が含まれますが、ピアノのサンプルは含まれません。

```bash
docker pull ghcr.io/mcp-tool-shop-org/si-jam-sessions:0.2.0
docker run --rm \
  -e HOST_UID="$(id -u)" -e HOST_GID="$(id -g)" \
  -v "$PWD:/out" \
  ghcr.io/mcp-tool-shop-org/si-jam-sessions:0.2.0 \
  notes /out/notes.json
```

PowerShellで、uid行を削除し、`${PWD}:/out`をマウントします。ハンドブックの[コンテナページ](https://mcp-tool-shop-org.github.io/si-jam-sessions/handbook/docker/)には、ピアノのレンダリングとWindowsフォームが含まれています。npmパッケージ[`@si-jam-sessions/si-jam-sessions`](https://www.npmjs.com/package/@si-jam-sessions/si-jam-sessions)は、このリリースのREADME、変更履歴、およびライセンスであり、Trusted Publishingによって公開されています。これをインストールしても、ホストはインストールされません。ホストは、コンテナまたは以下のビルドです。

```bash
cargo run -p host --release -- devices        # list the audio outputs and MIDI inputs
cargo run -p host --release -- fetch-piano    # download and verify the grand piano once (742 MB)
cargo run -p host --release -- play           # the Battle Hymn as glm-5.3 arranged it
cargo run -p host --release -- play --piece battle-hymn-kimi-k3
cargo run -p host --release -- jam --midi 0   # play along; each note is graded as it lands
```

- `--piece`は、`battle-hymn-glm-5.3`（デフォルト）、`battle-hymn-kimi-k3`、または`entertainer`（最初のマイルストーンのテストピース）を選択します。
- `notes <out.json>`は、ルールが記録した音符を書き出します。ランディングページは、これらのファイルからデータを取得します。
- `host help`は、すべてのコマンドをリストします。

`jam`には、有線出力を使用してください。Bluetooth出力の遅延は、ルールで許可されている100ミリ秒よりも長くなります。

## 信頼できるモデル

- **アクセスされるデータ:**
- `scores/` 内のスコアファイル
- ユーザーごとのキャッシュに保存されているピアノのサンプル
- 選択したオーディオ出力と MIDI 入力
- 書き込みを要求するファイル
- **アクセスされないデータ:** 上記のパス以外のすべてのデータ。アカウント、認証情報、テレメトリーは含まれません。
- **コンテナ:** 上記のプログラムそのものです。ピアノではなく、バイナリとスコアを保持します。ホストが実行される前に権限を制限します。uid は 10001、または設定した場合 `HOST_UID` になります。ファイルがコンテナから出力される唯一の方法は、マウントされたディレクトリを使用することです。
- **ネットワーク:** `fetch-piano` のみが使用します。
- 1つの固定アドレスから1つのアーカイブをダウンロードします。
- アーカイブを解凍する前に、固定された SHA-256 と照合して検証します。
- リンクや、自身のディレクトリから上位に移動するパスは拒否します。
- **権限:** 通常のユーザーアカウント。管理者権限は必要ありません。
- **プログラム:** 完全に I/O を行いません。CI は、WebAssembly モジュールが何もインポートしないことを確認します。

脆弱性を報告するには、[`SECURITY.md`](SECURITY.md) を参照してください。

## 現在の状況

- **最初のマイルストーンは完了しました:** プログラム、データの取り込みと出所、ゴールデンハッシュ、およびホスト。ゴールデンハッシュは、x86_64 と ARM64 のネイティブ環境、および V8、SpiderMonkey、JavaScriptCore の WebAssembly 環境で同じです。
- **設計:** [`docs/PHASE-0.md`](docs/PHASE-0.md) にて確定済み。
- **引き継ぎ:** [`docs/HANDOFF.md`](docs/HANDOFF.md) には、現在進行中の作業、決定された内容とその理由、および今後の作業について記載されています。
- **レビュー:** コードの変更をマージする前に、2つのモデルが Ollama Cloud でレビューを行います。それぞれのモデルは、作成者とは異なるグループに属し、互いのレビューを確認することはできません。

## ライセンス

- **コード:** MIT ( [`LICENSE`](LICENSE) を参照)。
- **2つの「バトル・ヒムン」のアレンジ:** CC0 1.0。
- **ピアノのサンプル:** CC BY 3.0 (Alexander Holm)。`fetch-piano` がダウンロードし、リポジトリにはコミットされません。
- **データセット:** それぞれに独自のライセンスが付属しており、それぞれのカードに記載されています。

---

<a href="https://mcp-tool-shop.github.io/">MCP Tool Shop</a> によって作成されました。
