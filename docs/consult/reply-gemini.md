VERDICT: revise
SENTENCE: The plan successfully isolates the deterministic score logic into a verifiable Rust core, but it falsely assumes a rigid integer-tick notation model and a sterile oscillator can validate an interactive "jam" experience.

Q1
ANSWER: If expressive timing (swing, rubato, and micro-timing) stays entirely in the host and outside the hash, the training rows fail to teach the model how to groove, push, or pull time. A jam is fundamentally about interactive rhythm and feel. If the model only sees integer ticks locked to a rigid grid, it learns to be a notation editor, not a musical partner, and will never understand human performance nuances. This is my own inference based on standard music systems architecture.
CHANGE IN THE LOCK: Revise "A score is integer ticks at one fixed pulses-per-quarter, normalized at ingest." to "A score is integer ticks at one fixed pulses-per-quarter with a committed deterministic tempo map or micro-timing offsets, normalized at ingest."
CONFIDENCE: high
BASIS: speculation

Q2
ANSWER: Engraving your own public-domain lead sheets does not produce a trainable corpus for harmony and jam generation. Ten pieces (mostly Joplin rags and two impressionist works) is catastrophically small; a preregistered run requires at least several thousand diverse pieces to learn valid musical probabilities. The public-domain shelf is simply too small without massive manual labor. Admitting CC-BY with a credit ledger makes the rows publishable beside an MIT/Apache crate. The obligation on the model card, based on [recall] standard CC-BY 4.0 terms, is that you must explicitly reproduce the attribution notices (creator names), copyright notices, and license URIs for every ingested CC-BY arrangement.
CHANGE IN THE LOCK: Revise "unknown, all-rights-reserved, no-redistribution, share-alike, and non-commercial are load refusals" to "unknown, all-rights-reserved, no-redistribution, share-alike, and non-commercial are load refusals; CC-BY is admitted into an attribution ledger."
CONFIDENCE: high
BASIS: [recall] standard CC-BY 4.0 license requirements.

Q3
ANSWER:
• ingesting a score: Hand-authored predicate. It deterministically verifies the SHA-256 against the authorized ledger and strictly parses structural integrity before ingest.
• placing notes: Hand-authored predicate. It enforces strict bounding rules like maximum polyphony, valid instrument ranges, and collision checks against the rhythmic grid.
• a chord label: Hand-authored predicate. It maps vertical pitch clusters to a deterministic dictionary of set-classes or standard triad/seventh definitions without requiring a neural network.
• a reharmonization that must not be a copy of the source: Hand-authored predicate. It enforces a strict Levenshtein or edit-distance constraint on the root progression to ensure the output diverges from the source.
• a teaching sentence that must not quietly rewrite the harmony: Do not admit this class yet. Deterministically checking semantic natural language constraints requires a large language model, which violates the strict no-model-in-replay rule of this architecture.
CHANGE IN THE LOCK: none (the lock's verbs already implicitly exclude teaching sentences, but we formally refuse the proposal class).
CONFIDENCE: high
BASIS: speculation

Q4
ANSWER: The oscillator is not enough to avoid the frozen-seat failure. A jam session requires rhythmic context and momentum to inspire a proposer or a human player. If the harness only plays a sterile, unvarying sine wave, there is no musical impulse for the user to react to, and the model has nothing worth responding to. The single missing sound that makes the golden hash premature is a metronome or a basic drum track. Without a time-keeping groove, a "jam" cannot start.
CHANGE IN THE LOCK: Revise "The host receives committed frames and may play them through an oscillator in slice 1" to "The host receives committed frames and must play them alongside an unhashed metronome or drum track in slice 1."
CONFIDENCE: medium
BASIS: speculation

Q5
ANSWER: Excluding the singer from v1 is necessary discipline, not a permanent refusal. The sibling's approach of aligning directly to the audio file violates the deterministic integer-tick score architecture and ruins replayability. However, to reattach the singer later as a pure socket without it becoming stateful, the exact temporal mapping must be known to the host. The single clock field that must exist in the first fixture is a deterministic tempo map (or a strict tick-to-microsecond translation) embedded in the committed events, allowing audio frames to align perfectly to score ticks.
CHANGE IN THE LOCK: Revise "Pitch, onset, duration, velocity, and voice are integers." to "Pitch, onset, duration, velocity, voice, and tick-to-microsecond tempo events are integers."
CONFIDENCE: high
BASIS: speculation

Q6
ANSWER: The assumption that testing a single re-fetched public-domain score against an admitted action proves the architecture is wrong. A jam session is inherently an interactive loop of call-and-response. Validating a single admitted action on a static, rigid Joplin rag proves only the plumbing (ABI and hashing). It completely ignores whether the checker can evaluate dynamic interplay or whether a human would actually engage with it. If you lock down the golden hash on a static test, you are building the foundation for a deterministic score database, not a jam session.
CHANGE IN THE LOCK: Revise "Proposed next slice: one re-fetched public-domain score, one admitted action, a golden hash native and under wasm, and an oscillator playback that is not hashed" to "Proposed next slice: a two-turn host loop with an admitted action, a golden hash native and under wasm, and a metronome playback that is not hashed."
CONFIDENCE: high
BASIS: speculation

SCAR: You are attempting to build an interactive "super-intelligence" jam session while strictly enforcing a deterministic, integer-tick, hash-based law that strips away the very essence of human performance (micro-timing and audio feel). You cannot have both a perfectly rigid law and a fluid, expressive musical partner without creating a massive disconnect between what the model learns and what the human hears.

DID NOT CHECK: The specific commercial grants of any MIDI crates, sample libraries, or singer models, as none were explicitly selected for evaluation in this phase-0 lock.