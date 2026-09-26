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

次に、各音符をスコアと比較して評価します。同じ演奏を2つのマシンで評価した場合、結果は完全に一致します。なぜなら、評価ルールは決定的なRustで記述され、単一のWebAssemblyバイナリにコンパイルされるからです。ルールは決定的なため、記録された演奏はモデルを呼び出すことなく、常に同じハッシュ値を返します。CIでは、毎回実行するたびにこれを再生します。

`si-jam-sessions`は、[`ai-jam-sessions`](https://github.com/mcp-tool-shop-org/ai-jam-sessions)の対となるものであり、[`si-rpg-engine`](https://github.com/mcp-tool-shop-org/si-rpg-engine)の兄弟プロジェクトです。モデルが提案を行い、モデル以外のチェッカーがその提案を受け入れるか、理由を付けて拒否します。ルールが受け入れたものは、コミットされ、ハッシュ化されます。モデルはスコアに直接触れることはありません。

## 試聴

「共和国賛歌」の2つのアレンジバージョンが、[ランディングページ](https://mcp-tool-shop-org.github.io/si-jam-sessions/)で並べて再生され、その違いを確認できます。

- **ソース:** このプロジェクトの、匿名で1862年に出版されたDitson版の楽譜。
- **編曲者:** glm-5.3とkimi-k3。それぞれがOllama Cloud上で、同じソースに基づいて編曲。
- **ライセンス:** どちらのアレンジもCC0 1.0で公開。それぞれが、ルールのライセンスチェックに合格し、証拠を提示。
- **録音:** それぞれが、[Salamander Grand Piano V3](https://freepats.zenvoid.org/Piano/acoustic-grand-piano.html)（Alexander Holmによる、CC BY 3.0）を使用して、ルールに基づいてレンダリングされました。

| アレンジ | 長さ | ルールがコミットした音符 | 非圧縮WAV（48 kHz、32ビット浮動小数点） |
|---|---|---|---|
| glm-5.3 | 5:02 | 1,720 | [116 MB](https://github.com/mcp-tool-shop-org/si-jam-sessions/releases/download/v0.1.0/battle-hymn-glm-5.3.wav)、SHA-256 `85b2d567…2eed` |
| kimi-k3 | 4:37 | 1,924 | [106 MB](https://github.com/mcp-tool-shop-org/si-jam-sessions/releases/download/v0.1.0/battle-hymn-kimi-k3.wav)、SHA-256 `bf49d310…a017` |

レンダリングは決定的な処理です。もう一度レンダリングしても、同じバイト列が得られます。CIのピアノジョブはLinuxで各実例を最後までレンダリングし、両方のSHA-256はリリースと一致しました。WindowsとLinuxで同じです。ピアノを一度フェッチした後、このコマンドで最初のレンダリングを再現できます。

```bash
cargo run -p host --release --locked -- render battle-hymn-glm-5.3.wav --piece battle-hymn-glm-5.3 --voice piano
```

完全なハッシュ値は、[リリースノート](https://github.com/mcp-tool-shop-org/si-jam-sessions/releases/tag/v0.1.0)に記載されています。

## このプロジェクトの特長

- **演奏はルールの不可欠な一部です。** タイミング、ピッチ、ベロシティはハッシュ化された状態の整数として保存されるため、「45ミリ秒遅れ」はエンジンが証明できる事実です。波形は単なる表現です。
- **クロックは決して待機しません。** ルールは、誰かが演奏するかどうかに関わらず、固定された量子でステップを進めます。モデルは、コミットの期限よりも先にフレーズを計画します。遅れた計画は拒否され、音楽が中断されることはありません。
- **すべての曲は、その価値を証明します。** 音楽は、証拠とともにのみ追加されます。
- 米国とEUの両方でパブリックドメインにある楽曲。
- パブリックドメインにある編曲、またはこのプロジェクトが作成した編曲。
- ソースとなる楽譜とその発行年。
- 楽譜に記載されたライセンスが、その楽譜の入手方法と一致すること。

不明、共有、非営利、AI制限のソースは拒否されます。CC BY 4.0の素材は、独自の属性が付与された階層に配置されます。
- **トレーニングデータは、ルールがコミットした内容の印刷物になります。** まだデータセットは作成されていません。作成された場合、各行は同じハッシュ値を返し、分割は、すべての行が存在する前に、作品ごとに固定され、すべての主張された結果は、その統計的有意性を明示します。

## 試してみてください

Rustが必要です。リポジトリは、`rust-toolchain.toml`でバージョン1.98.1を固定しており、`rustup`は最初のビルド時にこれをインストールします。

- **Windows 10および11**で、すべてを実行できますが、実際のMIDIキーボードを使用したライブ入力はまだ試されていません。
- **Linux:** CIでホストをビルドおよびテストし、レンダリングを行います。Linuxオーディオデバイスからの再生はテストされておらず、ライブ入力は現時点ではWindowsのみです。
- **macOS**はテストされていません。

```bash
cargo run -p host --release -- devices        # list the audio outputs and MIDI inputs
cargo run -p host --release -- fetch-piano    # download and verify the grand piano once (742 MB)
cargo run -p host --release -- play           # the Battle Hymn as glm-5.3 arranged it
cargo run -p host --release -- play --piece battle-hymn-kimi-k3
cargo run -p host --release -- jam --midi 0   # play along; each note is graded as it lands
```

- `--piece`は、`battle-hymn-glm-5.3`（デフォルト）、`battle-hymn-kimi-k3`、または`entertainer`（最初のマイルストーンのテストピース）を選択します。
- `notes <out.json>`は、ルールがコミットした音符を書き出します。ランディングページは、これらのファイルからデータを取得します。
- `host help`は、すべてのコマンドをリストします。

`jam`には、有線出力を使用してください。Bluetooth出力の遅延は、ルールが許容する100ミリ秒よりも長くなります。

## モデルへの信頼

- **アクセスされるデータ:**
- `scores/`にある楽譜ファイル。
- ユーザーごとのキャッシュにあるピアノのサンプル。
- 選択したオーディオ出力とMIDI入力。
- 書き出すように要求したファイル。
- **アクセスされないデータ:** 上記のパス以外のすべてのデータ。アカウント、認証情報、テレメトリはありません。
- **ネットワーク:** `fetch-piano`のみが使用します。
- 1つの固定アドレスから1つのアーカイブをダウンロードします。
- アーカイブをアンパックする前に、固定されたSHA-256と照合して検証します。
- リンクや、ディレクトリから外に出るパスは拒否します。
- **権限:** 通常のユーザーアカウント。管理者権限は必要ありません。
- **ルールは、I/Oを一切行いません。** CIは、WebAssemblyモジュールが何もインポートしないことを確認します。

脆弱性を報告するには、[`SECURITY.md`](SECURITY.md)を参照してください。

## 現状

- **最初のマイルストーンが完了しました。** 法令、データの取り込みと出所、ゴールデンハッシュ、およびホストです。ゴールデンハッシュは、x86_64とARM64、およびV8、SpiderMonkey、JavaScriptCoreの下のWebAssemblyで、ネイティブに同じです。
- **設計**は[`docs/PHASE-0.md`](docs/PHASE-0.md)で確定しました。
- **引き継ぎ**は[`docs/HANDOFF.md`](docs/HANDOFF.md)で行われ、現在進行中の作業、決定された内容とその理由、および今後の作業について説明します。
- **レビュー:** コードの変更をマージする前に、2つのモデルがOllama Cloudでレビューを行います。それぞれのモデルは、作成者とは異なる系列に属し、互いのレビュー内容は確認できません。

## ライセンス

- **コード:** MIT（[`LICENSE`](LICENSE)を参照）。
- **2つの「バトル・ヒムン」のアレンジ:** CC0 1.0。
- **ピアノのサンプル:** CC BY 3.0（アレクサンダー・ホルム）。`fetch-piano`がダウンロードし、リポジトリにはコミットされません。
- **データセット:** それぞれが独自のカードに独自のライセンスを記載しています。

---

<a href="https://mcp-tool-shop.github.io/">MCP Tool Shop</a>によって作成されました。
