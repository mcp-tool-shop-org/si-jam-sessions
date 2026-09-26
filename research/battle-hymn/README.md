# Battle Hymn of the Republic: the evidence and the reference

The exemplar that si-jam-sessions plays is this project's own arrangement of *Battle Hymn of the Republic*,
dedicated to the public domain under CC0. This folder holds what the arrangement and its receipt rest on. It is
research, not a score: nothing here is admitted by the law.

## Why the song is clear

- **The words.** First printed, unsigned, in *The Atlantic Monthly*, February 1862 (vol. IX, p. 145). Julia Ward
  Howe died in 1910.
- **The tune.** In print by 1859: the American Sunday-School Union's *Prayer-Meeting Tune-Book* credits it only
  to an arranger, "A. T.", and Jenks's *Devotional Melodies* prints it the same year. An 1858 printing is reported
  at second hand only.
- **Its composer is unknown.** The Library of Congress catalogues the song under its title, and its authority
  record cites Fuld: no claim to the tune can be sustained. The most-cited claimant is William Steffe (about
  1830–1890); Thomas Brigham Bishop (died 1905) and an 1861 Boston credit to Phillip Simonds are the others. The
  receipt records the tune as anonymous, with Steffe named in its notes.
- **Both jurisdictions.** Every publication involved is from 1858–1862, well before the United States cut-off.
  In the European Union every named claimant died by 1911, and an anonymous work's term runs 70 years from
  publication (Directive 2006/116/EC, Art. 1(3)), which ended around 1930.

## What is here

| File | What it is |
|---|---|
| `evidence-manifest.json` | 16 sources and 66 quotes, each with its URL, fetch time, byte count and SHA-256 |
| `battle-hymn-ditson-1862.abc` | A transcription of the reference edition: *Battle Hymn of the Republic*, Boston: Oliver Ditson & Co., 1862, plate 21454, anonymous (Library of Congress item 2023782802). Every bar names its page, system and position; lengths are exact sixteenths |
| `battle-hymn-ditson-1862.events.csv` | The same notes as onsets and durations in exact fractions, with MIDI numbers |
| `samples-manifest.json` | The survey of grand-piano sample libraries, with each licence quoted from its own page |
| `verify/` | Two independent checks of the transcription against the edition's scans, by vision models of two other families: kimi-k3 found no discrepancy; glm-5.3-flash confirmed every pitch and rhythm it could read and noted unprinted staccato dots in the left hand of bars 5–10 |

Two readings the scans leave open, neither of which the arrangement depends on: the octave of the A in the F7
chord at bar 19, beat 4, and whether the alto rises to E or repeats D at the end of bar 13.

## Rules for the arrangement

- It follows this 1862 edition and nothing later. Arrangements published after 1930 are still in copyright,
  so "the familiar version" is not a source.
- The piano that plays it is the Salamander Grand Piano V3 by Alexander Holm (CC BY 3.0), so every published
  recording of the exemplar carries that credit.
