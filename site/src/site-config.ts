import type { SiteConfig } from '@mcptoolshop/site-theme';

export const config: SiteConfig = {
  title: 'si-jam-sessions',
  description:
    "A music engine where an AI's playing can be graded exactly, replayed exactly, and trained on with a clean conscience.",
  logoBadge: 'SJ',
  brandName: 'si-jam-sessions',
  repoUrl: 'https://github.com/mcp-tool-shop-org/si-jam-sessions',
  footerText:
    'Code MIT. Arrangements CC0 1.0. Piano: Salamander Grand Piano V3 by Alexander Holm, CC BY 3.0. Built by <a href="https://mcp-tool-shop.github.io/" style="color:var(--color-muted);text-decoration:underline">MCP Tool Shop</a>',

  hero: {
    badge: 'Rust to WebAssembly · open source',
    headline: 'Every note graded,',
    headlineAccent: 'to the sample.',
    description:
      "A music engine where an AI's playing can be graded exactly, replayed exactly, and trained on with a clean conscience. The model proposes. A checker that is not the model admits the proposal or refuses it with a reason, and a deterministic law commits and hashes what it admits.",
    primaryCta: { href: '#listen', label: 'Listen to the Battle Hymn' },
    secondaryCta: { href: 'handbook/', label: 'Read the Handbook' },
    previews: [
      { label: 'Fetch the piano', code: 'cargo run -p host --release -- fetch-piano' },
      { label: 'Play', code: 'cargo run -p host --release -- play' },
      { label: 'Play along', code: 'cargo run -p host --release -- jam --midi 0' },
    ],
  },

  sections: [
    {
      kind: 'features',
      id: 'how',
      title: 'How it works',
      subtitle: 'A model may propose anything. Only the law decides what happened.',
      features: [
        {
          title: 'Integer time',
          desc: 'The law keeps time at 3,360 ticks per quarter note and steps a fixed quantum of 48 samples at 48 kHz, whether anyone plays or not. A plan that arrives late is refused; it never stalls the music.',
        },
        {
          title: 'The take is part of the law',
          desc: 'Every note you play is graded against the score to the sample, inside a gate of ±40 ms. "Late by 45 ms" is a fact the engine can prove, and the waveform is only presentation.',
        },
        {
          title: 'One hash everywhere',
          desc: 'The law is no_std Rust with no floating point, compiled to one pinned WebAssembly binary. The Entertainer’s golden hash is the same on x86_64 and ARM64 and under V8, SpiderMonkey and JavaScriptCore; the exemplars’ goldens are checked on both architectures.',
        },
        {
          title: 'Every song earns its place',
          desc: 'A score is admitted only with evidence: a composition in the public domain in the US and the EU, a free or self-engraved arrangement, the source edition and its year, and an in-file licence that matches its receipt.',
        },
        {
          title: 'Replays without a model',
          desc: 'The law is deterministic, so a recorded take replays to the same hash without calling a model. CI replays The Entertainer’s constructed take on every run.',
        },
        {
          title: 'Reviewed across families',
          desc: 'Before a code change merges, two models from families other than the author’s review it on Ollama Cloud, each blind to the other.',
        },
      ],
    },
    {
      kind: 'code-cards',
      id: 'try',
      title: 'Try it',
      subtitle:
        'You need Rust; the repository pins 1.98.1 and rustup installs it. Windows runs everything, though live input has not yet met a real MIDI keyboard; on Linux, CI builds, tests and renders, and live input is Windows-only for now.',
      cards: [
        {
          title: 'Get the grand piano once',
          code: 'git clone https://github.com/mcp-tool-shop-org/si-jam-sessions\ncd si-jam-sessions\ncargo run -p host --release -- fetch-piano  # 742 MB, checked by SHA-256',
        },
        {
          title: 'Listen',
          code: 'cargo run -p host --release -- play  # glm-5.3\ncargo run -p host --release -- play --piece battle-hymn-kimi-k3',
        },
        {
          title: 'Play along',
          code: 'cargo run -p host --release -- devices  # find your MIDI input\ncargo run -p host --release -- jam --midi 0',
        },
        {
          title: 'Reproduce a recording',
          code: 'cargo run -p host --release --locked -- render battle-hymn-glm-5.3.wav \\\n  --piece battle-hymn-glm-5.3 --voice piano\n# SHA-256 85b2d56723cacc6c87895a2784d2cd034dea1142e62de6d8a1a9d2867dad2eed',
        },
      ],
    },
    {
      kind: 'data-table',
      id: 'pins',
      title: 'Pinned',
      subtitle: 'Every claim on this page is a hash you can check.',
      columns: ['What', 'SHA-256'],
      rows: [
        ['The Entertainer: the golden take, graded', '66b59807261ff93086eed6b0c17cb4a7673e40937a83e0bd32db7ae25ef02946'],
        ['Battle Hymn, glm-5.3: the law’s snapshot', '1c0789b11d61914879dd39630b763f44d2955955d1d97cef42c695e0fdd6ad24'],
        ['Battle Hymn, kimi-k3: the law’s snapshot', '0f93de92b08ed6534a79462744035dcc553a91d8c05b3475750e83b440b89391'],
        ['Battle Hymn, glm-5.3: the WAV', '85b2d56723cacc6c87895a2784d2cd034dea1142e62de6d8a1a9d2867dad2eed'],
        ['Battle Hymn, kimi-k3: the WAV', 'bf49d310de5e94975b277cd1e7c57134388a0232d53bb059f12e7727baeba017'],
        ['The Salamander piano archive', 'b7760e168494cf095344e217b0af013fc449ad033abbbdf1c65211cf11dc038b'],
      ],
    },
    {
      kind: 'features',
      id: 'trust',
      title: 'Trust model',
      subtitle: 'What the code touches, and what it never does.',
      features: [
        {
          title: 'The law does no I/O',
          desc: 'CI checks that the WebAssembly module imports nothing at all.',
        },
        {
          title: 'One download, pinned',
          desc: 'Only fetch-piano uses the network: one archive from one fixed address, checked by SHA-256 before anything is unpacked, with links and escaping paths refused.',
        },
        {
          title: 'No telemetry',
          desc: 'Nothing is collected or sent. There are no accounts, and the code handles no credentials.',
        },
      ],
    },
  ],
};
