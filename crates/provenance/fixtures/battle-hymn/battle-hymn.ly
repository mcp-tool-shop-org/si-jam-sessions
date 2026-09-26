\version "2.24.0"

% A fixture for the licence predicate's tests: the receipt beside this file records
% Battle Hymn of the Republic as this project's own CC0 engraving. The two notes below
% are a placeholder, not the arrangement.

\header {
  title = "Battle Hymn of the Republic"
  poet = "Words: Julia Ward Howe (1862)"
  composer = "Tune: anonymous (in print by 1859)"
  arranger = "Arrangement: si-jam-sessions"
  source = "Boston: Oliver Ditson & Co., 1862, plate 21454"
  copyright = "CC0 1.0"
  tagline = ##f
}

\score {
  \new Staff { \key bes \major \time 4/4 f'4 bes'2. }
  \layout { }
  \midi { \tempo 4 = 76 }
}
