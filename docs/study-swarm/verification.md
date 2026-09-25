# Verification record — phase-0-consult.dispatch.md

## Gate run 1 (2026-09-25): REFUSE, three false fabrications from the oracle's DOI registry

`roleos verify-citations --provider ollama` (prism 1.6.0, local mistral-small:24b lens, signed with the org key) returned `refuse`: 29 citations checked, 15 supported, 10 not_addressed, 4 unchecked. It dropped three DOIs because Crossref had no record of them.

All three are Zenodo DOIs, which DataCite registers and Crossref does not index. That is the same blind spot the protocol already records for a 10.5555 DOI. Each was checked against DataCite (`api.datacite.org/dois/<doi>`) on 2026-09-25:

| DOI | DataCite state | Year | Title | Creators |
|---|---|---|---|---|
| 10.5281/zenodo.15571083 | findable | 2025 | PDMX: A Large-Scale Public Domain MusicXML Dataset for Symbolic Music Processing | Long, Novack, McAuley, Berg-Kirkpatrick |
| 10.5281/zenodo.10265217 | findable | 2023 | Data Collection in Music Generation Training Sets: A Critical Analysis | Morreale, Sharma, Wei |
| 10.5281/zenodo.1416276 | findable | 2014 | What Is The Effect Of Audio Quality On The Robustness Of Mfccs And Chroma Features? | Urbano, Bogdanov, Herrera, Gómez |

Each record URL (`zenodo.org/records/<id>`) returned HTTP 200. The dispatch now cites those record URLs. The PDMX v9 record was also opened in the session and confirms the finding: 31,221 conflicts (12.29%), the public-facing value kept, and a 222,856-song conflict-free subset.

## Out-of-band retrieval by the advisor seat

These checks were done by Claude, the same family as the synthesiser. They are a retrieval check, not the family-different groundedness lens.

- arXiv:2502.21267, body §2.1, §2.2, §2.3.1, §4.3: 1/16-note frames; 4-beat lookahead; committed chords cannot change; most responses within 100 ms; users split on commit 0 vs 4. **Supports finding 29.**
- arXiv:2606.11886, body §IV-D Eq. 6, §V-B, §V-C: τtick = 60000 / (4·BPM) ms; feasibility is an inequality on the interval against modelled latency; base latency ≈100 ms LAN and ≈150 ms remote; over 60% of remote configurations violate real time. **Supports finding 30.**
- doi:10.1068/p6465: the publisher returned 403, so CCRMA's project page was used. It gives acceleration below 11.5 ms, increasingly severe deceleration above that, a linear fit and no threshold cliff, and cites Chafe, Cáceres & Gurevich 2010, Perception 39(7):982–992. **Supports finding 31.**
- arXiv:2510.22455, abstract: near ceiling on MIDI, accuracy drops on audio. **Supports finding 16.**
- arXiv:2509.16662, abstract: at least 38,134 of 178,561 files filtered in the most conservative setting. **Supports finding 18.**
- arXiv:2509.13658, abstract: detects exact replication at a granularity of at least one bar. **Supports finding 19.**
- creativecommons.org/using-cc-licensed-works-for-ai-training-2: the ShareAlike and attribution sentences, read verbatim. **Supports finding 10.**
- gesetze-im-internet.de/urhg/__70.html: 25 years. **Supports finding 2.**
- fourscoreandmore.org/openscore: CC0, about 1,500 Lieder and 200 quartets, no source editions named. **Supports finding 4.**

## Gate run 2 (2026-09-25): escalate, advisory, 0 fabricated

Same runner and lens as run 1. The only change: the three Zenodo works are now cited by their record URLs.

- **Verdict:** `escalate`. 26 citations checked: 15 supported, 10 not_addressed (the claim sits in the paper body), 1 unchecked (the arXiv oracle had a transient HTTP error on arXiv:2602.05064). 11 URL-only items went to out-of-band checking.
- **Receipt:**
  - prism: `prism-01m3cakqb0htswb2r60vek2rwj`
  - chain_sha256: `dba14b8bd1c361c41aa411a27cd54024c19d8c1e6b8346bf52afa69aab080ea4`
  - citations_sha256: `e7fd9a9c6d7d7862a8f72c447bdc2166e1c57665ec9c3e28e4801205bcba7310`
- **Files:**
  - run 1: `gate/run1-*`
  - run 2: `gate/run2-*`

## Still open

- **Body-level claims** that prism marked "retrieve full text", beyond the three above: findings 12, 13, 14, 17, 21, 28 and 33. The research seats report body-level support. No family-different lens has read those bodies yet.
- **URL-only citations:** 1, 3, 11, 35, 36, 37, 39–43. Prism cannot parse these; each was retrieved by a research seat, and none has been re-checked by a different family.
