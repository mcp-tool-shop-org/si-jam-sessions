\version "2.24.0"

\header {
  title = "Battle Hymn of the Republic"
  subtitle = "for solo piano"
  poet = "Words: Julia Ward Howe (1862)"
  composer = "Tune: anonymous (in print by 1859)"
  arranger = "Arrangement: si-jam-sessions"
  copyright = "CC0 1.0"
  tagline = ##f
}

global = {
  \key bes \major
  \time 4/4
}

% ==================== INTRODUCTION: the edition's three bars, voiced for full piano ====================

introUpper = <<
  { f'4.\mf ees'8 d'8 f'8 bes'8. c''16 |
    d''2( bes'4) r8 b'8 |
    c''4-. c''4-. bes'4\> a'4\! | }
  \\
  { <f bes d'>4 <f bes d'>4 <f bes d'>4 <f bes d'>4 |
    <f bes d'>4 <f bes d'>4 <f bes d'>4 <f bes d'>4 |
    <g c'>4 <g c'>4 <bes d'>4 <c' ees'>4 | }
>>

introLower = {
  <bes,, bes,>4 <f, bes, d>4 <bes,, bes,>4 <f, bes, d>4 |
  <bes,, bes,>4 <f, bes, d>4 <bes,, bes,>4 <f, bes, d>4 |
  <ees,, ees,>4 <ees g c'>4 <f,, f,>4 <f c' ees'>4 |
}

% ==================== VERSE 1: piano, simple and chordal, close to the edition ====================

verseOneUpper = <<
  { bes'4\p r4 r4 r8 f'8 |
    f'8 f'8 f'8. ees'16 d'8. f'16 bes'8. c''16 |
    d''8. d''16 d''8. c''16 bes'4 r8 bes'16 bes'16 |
    g'8. g'16 g'8. a'16 bes'8. bes'16 a'8. g'16 |
    f'8. g'16 f'8. d'16 f'4 r8 f'16 f'16 |
    f'8. f'16 f'8. ees'16 d'8. f'16 bes'8. c''16 |
    d''8. d''16 d''8. c''16 bes'4 r8 bes'8 |
    c''4~ c''8. c''16 bes'4 a'4 |
    bes'2 r2 | }
  \\
  { r4 <f bes d'>4 <f bes d'>4 <f bes d'>4 |
    <f bes d'>4 <f bes d'>4 <f bes d'>4 <f bes d'>4 |
    <f bes d'>4 <f bes d'>4 <f bes d'>4 <f bes d'>4 |
    <g bes ees'>4 <g bes ees'>4 <g bes ees'>4 <g bes ees'>4 |
    <f bes d'>4 <f bes d'>4 <f bes d'>4 <f bes d'>4 |
    <f bes d'>4 <f bes d'>4 <f bes d'>4 <f bes d'>4 |
    <f bes d'>4 <f bes d'>4 <f bes d'>4 <f bes d'>4 |
    <g c'>4 <g c'>4 <f bes d'>4 <f c' ees'>4 |
    <bes d'>8 f'16 f'16 f'8 f'8 f'4 f'4 | }
>>

verseOneLower = {
  <bes d'>4 bes,4 bes,4 bes,4 |
  bes,4 bes,4 bes,4 bes,4 |
  bes,4 bes,4 bes,4 bes,4 |
  ees,4 ees,4 ees,4 ees,4 |
  bes,4 bes,4 bes,4 bes,4 |
  bes,4 bes,4 bes,4 bes,4 |
  bes,4 bes,4 bes,4 bes,4 |
  <ees, ees>4 <ees, ees>4 f,4 f,4 |
  bes,4 r4 r4 r4 |
}

% ==================== CHORUS 1: mezzo-piano ====================

chorusOneUpper = <<
  { f'4.\mp ees'8 d'8. f'16 bes'8. c''16 |
    d''2 bes'4 r4 |
    g'8. g'16 g'8. a'16 bes'8. bes'16 a'8. g'16 |
    f'4( d'4) f'2 |
    f'4. ees'8 d'8. f'16 bes'8. c''16 |
    d''2 bes'4 r8 bes'8 |
    c''4 c''4 bes'4 a'4 |
    bes'2 r2 | }
  \\
  { <f bes d'>4 <f bes d'>4 <f bes d'>4 <f bes d'>4 |
    <f bes d'>4 <f bes d'>4 <f bes d'>4 <f bes d'>4 |
    <g bes ees'>4 <g bes ees'>4 <g bes ees'>4 <g bes ees'>4 |
    <f bes d'>4 <f bes d'>4 <f bes d'>4 <f bes d'>4 |
    <f bes d'>4 <f bes d'>4 <f bes d'>4 <f bes d'>4 |
    <f bes d'>4 <f bes d'>4 <f bes d'>4 <f bes d'>4 |
    <g c' ees'>4 <g c' ees'>4 <f bes d'>4 <f c' ees'>4 |
    <d' f' bes'>2 r2 | }
>>

chorusOneLower = {
  bes,4 <d f bes>4 bes,4 <d f bes>4 |
  bes,4 <d f bes>4 bes,4 <d f bes>4 |
  ees,4 <ees g bes>4 ees,4 <ees g bes>4 |
  bes,4 <d f bes>4 bes,4 <d f bes>4 |
  bes,4 <d f bes>4 bes,4 <d f bes>4 |
  bes,4 <d f bes>4 bes,4 <d f bes>4 |
  ees4 ees,4 f,4 f,4 |
  bes,8 f,,16 f,,16 d,8 f,,8 bes,,2 |
}

% ==================== VERSE 2: mezzo-piano, flowing broken chords in the left hand ====================

verseTwoUpper = {
  s2\mp s4 s8 f'16 f'16 |
  f'8 f'8 f'8. ees'16 d'8. f'16 bes'8. c''16 |
  d''8. c''16 d''8. c''16 bes'4 r8 bes'16 bes'16 |
  g'8. g'16 g'8. a'16 bes'8. bes'16 a'8. g'16 |
  f'8. g'16 f'8. d'16 f'4 r8 f'16 f'16 |
  f'8. f'16 f'8. ees'16 d'8. f'16 bes'8. c''16 |
  d''8. d''16 d''8. c''16 bes'4 r8 bes'8 |
  c''4~ c''8. c''16 bes'4 a'4 |
  bes'2 r2 |
}

verseTwoLower = {
  bes,8 d8 f8 bes8 d'8 bes8 f8 d8 |
  bes,8 d8 f8 bes8 d'8 bes8 f8 d8 |
  bes,8 d8 f8 bes8 d'8 bes8 f8 d8 |
  ees,8 bes,8 ees8 g8 bes8 g8 ees8 bes,8 |
  bes,8 d8 f8 bes8 d'8 bes8 f8 d8 |
  bes,8 d8 f8 bes8 d'8 bes8 f8 d8 |
  bes,8 d8 f8 bes8 d'8 bes8 f8 d8 |
  ees,8 g,8 c8 ees8 f,8 bes,8 c8 ees8 |
  bes,8 d8 f8 bes8 d'8 bes8 f8 d8 |
}

% ==================== CHORUS 2: mezzo-forte ====================

chorusTwoUpper = <<
  { f'4.\mf ees'8 d'8. f'16 bes'8. c''16 |
    d''2 bes'4 r4 |
    g'8. g'16 g'8. a'16 bes'8. bes'16 a'8. g'16 |
    f'4( d'4) f'2 |
    f'4. ees'8 d'8. f'16 bes'8. c''16 |
    d''2 bes'4 r8 bes'8 |
    c''4 c''4 bes'4 a'4 |
    bes'2 r2 | }
  \\
  { <f bes d'>2 <f bes d'>2 |
    <f bes d'>2 <f bes d'>2 |
    <g bes ees'>2 <g bes ees'>2 |
    <f bes d'>2 <f bes d'>2 |
    <f bes d'>2 <f bes d'>2 |
    <f bes d'>2 <f bes d'>2 |
    <g c' ees'>4 <g c' ees'>4 <f bes d'>4 <f c' ees'>4 |
    <d' f' bes'>2 r2 | }
>>

chorusTwoLower = {
  bes,,8 bes,8 d8 f8 bes8 d'8 bes8 f8 |
  bes,,8 bes,8 d8 f8 bes8 d'8 bes8 f8 |
  ees,,8 ees,8 bes,8 ees8 g8 bes8 ees'8 bes8 |
  bes,,8 bes,8 d8 f8 bes8 d'8 bes8 f8 |
  bes,,8 bes,8 d8 f8 bes8 d'8 bes8 f8 |
  bes,,8 bes,8 d8 f8 bes8 d'8 bes8 f8 |
  ees,,8 g,8 c8 ees8 f,8 bes,8 c8 ees8 |
  bes,8 f,,16 f,,16 d,8 f,,8 bes,,2 |
}

% ==================== VERSE 3: mezzo-forte, the march, crisp ====================

verseThreeUpper = <<
  { s2\mf s4 s8 f'16 f'16 |
    f'8 f'8 f'8. ees'16 d'8. f'16 bes'8. c''16 |
    d''8. c''16 d''8. c''16 bes'4 r8 bes'16 bes'16 |
    g'8. g'16 g'8. a'16 bes'8. bes'16 a'8. g'16 |
    f'8. g'16 f'8. d'16 f'4 r8 f'16 f'16 |
    f'8. f'16 f'8. ees'16 d'8. f'16 bes'8. c''16 |
    d''8. d''16 d''8. c''16 bes'4 r8 bes'8 |
    c''4~ c''8. c''16 bes'4 a'4 |
    bes'2 r2 | }
  \\
  { <f bes d'>4-. <f bes d'>4-. <f bes d'>4-. <f bes d'>4-. |
    <f bes d'>4-. <f bes d'>4-. <f bes d'>4-. <f bes d'>4-. |
    <f bes d'>4-. <f bes d'>4-. <f bes d'>4-. <f bes d'>4-. |
    <g bes ees'>4-. <g bes ees'>4-. <g bes ees'>4-. <g bes ees'>4-. |
    <f bes d'>4-. <f bes d'>4-. <f bes d'>4-. <f bes d'>4-. |
    <f bes d'>4-. <f bes d'>4-. <f bes d'>4-. <f bes d'>4-. |
    <f bes d'>4-. <f bes d'>4-. <f bes d'>4-. <f bes d'>4-. |
    <g c'>4-. <g c'>4-. <f bes d'>4-. <f c' ees'>4-. |
    <bes d'>8 f'16 f'16 f'8 f'8 f'4 f'4 | }
>>

verseThreeLower = {
  bes,,4-. bes,4-. bes,,4-. bes,4-. |
  bes,,4-. bes,4-. bes,,4-. bes,4-. |
  bes,,4-. bes,4-. bes,,4-. bes,4-. |
  ees,,4-. ees,4-. ees,,4-. ees,4-. |
  bes,,4-. bes,4-. bes,,4-. bes,4-. |
  bes,,4-. bes,4-. bes,,4-. bes,4-. |
  bes,,4-. bes,4-. bes,,4-. bes,4-. |
  <ees, ees>4-. <ees, ees>4-. f,4-. f,4-. |
  bes,8 f,,16 f,,16 d,8 f,,8 bes,,2 |
}

% ==================== CHORUS 3: forte ====================

chorusThreeUpper = <<
  { f'4.\f ees'8 d'8. f'16 bes'8. c''16 |
    d''2 bes'4 r4 |
    g'8. g'16 g'8. a'16 bes'8. bes'16 a'8. g'16 |
    f'4( d'4) f'2 |
    f'4. ees'8 d'8. f'16 bes'8. c''16 |
    d''2 bes'4 r8 bes'8 |
    c''4 c''4 bes'4 a'4 |
    bes'2 r2 | }
  \\
  { <f bes d'>4 <f bes d'>4 <f bes d'>4 <f bes d'>4 |
    <f bes d'>4 <f bes d'>4 <f bes d'>4 <f bes d'>4 |
    <g bes ees'>4 <g bes ees'>4 <g bes ees'>4 <g bes ees'>4 |
    <f bes d'>4 <f bes d'>4 <f bes d'>4 <f bes d'>4 |
    <f bes d'>4 <f bes d'>4 <f bes d'>4 <f bes d'>4 |
    <f bes d'>4 <f bes d'>4 <f bes d'>4 <f bes d'>4 |
    <g c' ees'>4 <g c' ees'>4 <f bes d'>4 <f c' ees'>4 |
    <d' f' bes'>2 r2 | }
>>

chorusThreeLower = {
  <bes,, bes,>4 <bes, d f>4 <bes,, bes,>4 <bes, d f>4 |
  <bes,, bes,>4 <bes, d f>4 <bes,, bes,>4 <bes, d f>4 |
  <ees,, ees,>4 <ees g bes>4 <ees,, ees,>4 <ees g bes>4 |
  <bes,, bes,>4 <bes, d f>4 <bes,, bes,>4 <bes, d f>4 |
  <bes,, bes,>4 <bes, d f>4 <bes,, bes,>4 <bes, d f>4 |
  <bes,, bes,>4 <bes, d f>4 <bes,, bes,>4 <bes, d f>4 |
  <ees, ees>4 <ees, ees>4 <f, f>4 <f, f>4 |
  bes,8 f,,16 f,,16 d,8 f,,8 bes,,2 |
}

% ==================== VERSE 4: mezzo-piano, a hushed chorale in four parts, melody in the tenor register ====================

verseFourUpper = <<
  { f'1\mp |
    f'2 f'2 |
    bes'2 bes'2 |
    bes'2 bes'2 |
    f'2 f'2 |
    f'2 f'2 |
    bes'2 bes'2 |
    g'4 g'4 f'4 ees'4 |
    f'1 | }
  \\
  { d'1 |
    d'2 d'2 |
    f'2 f'2 |
    ees'2 ees'2 |
    d'2 d'2 |
    d'2 d'2 |
    f'2 f'2 |
    ees'4 ees'4 d'4 c'4 |
    d'1 | }
>>

verseFourLower = <<
  { r2 r4 r8 f16 f16 |
    f8 f8 f8. ees16 d8. f16 bes8. c'16 |
    d'8. c'16 d'8. c'16 bes4 r8 bes16 bes16 |
    g8. g16 g8. a16 bes8. bes16 a8. g16 |
    f8. g16 f8. d16 f4 r8 f16 f16 |
    f8. f16 f8. ees16 d8. f16 bes8. c'16 |
    d'8. d'16 d'8. c'16 bes4 r8 bes8 |
    c'4~ c'8. c'16 bes4 a4 |
    bes2 r2 | }
  \\
  { bes,1 |
    bes,1~ |
    bes,1 |
    ees1 |
    bes,1 |
    bes,1~ |
    bes,1 |
    ees4 ees4 f4 f4 |
    bes,1 | }
>>

% ==================== CHORUS 4: mezzo-forte, the edition's four vocal parts, melody in the soprano ====================

chorusFourUpper = <<
  { f'4.\mf ees'8 d'8. f'16 bes'8. c''16 |
    d''2 bes'4 r4 |
    g'8. g'16 g'8. a'16 bes'8. bes'16 a'8. g'16 |
    f'4( d'4) f'2 |
    f'4. ees'8 d'8. f'16 bes'8. c''16 |
    d''2 bes'4 r8 bes'8 |
    c''4 c''4 bes'4 a'4 |
    bes'2 r2 | }
  \\
  { d'4. c'8 d'8. d'16 d'8. ees'16 |
    f'2 d'4 r4 |
    ees'8. ees'16 ees'8. f'16 g'8. g'16 f'8. ees'16 |
    d'4 d'4 d'2 |
    d'4. c'8 d'8. d'16 d'8. d'16 |
    f'2 d'4 r8 f'8 |
    g'4 g'4 f'4 ees'4 |
    d'2 r2 | }
>>

chorusFourLower = <<
  { f2 f2 |
    f2 f2 |
    bes2 bes2 |
    f2 f2 |
    f2 f2 |
    f2 f2 |
    c'4 c'4 d'4 c'4 |
    f2 r2 | }
  \\
  { bes,4 bes,4 bes,4 bes,4 |
    bes,4 bes,4 bes,4 bes,4 |
    ees4 ees4 ees4 ees4 |
    bes,4 bes,4 bes,4 bes,4 |
    bes,4 bes,4 bes,4 bes,4 |
    bes,4 bes,4 bes,4 bes,4 |
    ees4 ees4 f4 f4 |
    bes,2 bes,,2 | }
>>

% ==================== VERSE 5: forte to fortissimo, the melody in octaves over full chords and a moving bass ====================

verseFiveUpper = {
  s2\f s4 s8 <f' f''>16 <f' f''>16 |
  <f' f''>8 <f' f''>8 <f' f''>8. <ees' ees''>16 <d' d''>8. <f' f''>16 <bes' bes''>8. <c'' c'''>16 |
  <d'' d'''>8. <c'' c'''>16 <d'' d'''>8. <c'' c'''>16 <bes' bes''>4 r8 <bes' bes''>16 <bes' bes''>16 |
  <g' g''>8. <g' g''>16 <g' g''>8. <a' a''>16 <bes' bes''>8. <bes' bes''>16 <a' a''>8. <g' g''>16 |
  <f' f''>8. <g' g''>16 <f' f''>8. <d' d''>16 <f' f''>4 r8 <f' f''>16 <f' f''>16 |
  <f' f''>8.\< <f' f''>16 <f' f''>8. <ees' ees''>16 <d' d''>8. <f' f''>16 <bes' bes''>8. <c'' c'''>16 |
  <d'' d'''>8. <d'' d'''>16 <d'' d'''>8. <c'' c'''>16 <bes' bes''>4 r8 <bes' bes''>8 |
  <c'' c'''>4~ <c'' c'''>8.\ff <c'' c'''>16 <bes' bes''>4 <a' a''>4 |
  <bes' bes''>2 r2 |
}

verseFiveLower = {
  <bes,, bes,>4 <bes, d f>4 <f, f>4 <bes, d f>4 |
  <bes,, bes,>4 <bes, d f>4 <f, f>4 <bes, d f>4 |
  <bes,, bes,>4 <bes, d f>4 <f, f>4 <bes, d f>4 |
  <ees,, ees,>4 <ees g bes>4 <bes,, bes,>4 <ees g bes>4 |
  <bes,, bes,>4 <bes, d f>4 <f, f>4 <bes, d f>4 |
  <bes,, bes,>4 <bes, d f>4 <f, f>4 <bes, d f>4 |
  <bes,, bes,>4 <bes, d f>4 <f, f>4 <bes, d f>4 |
  <ees, g,>4 <ees g>4 <f, bes, d>4 <f c' ees'>4 |
  <bes,, bes,>4 <bes, d f>4 <f, f>4 <bes, d f>4 |
}

% ==================== CHORUS 5: fortissimo ====================

chorusFiveUpper = {
  <f' f''>4.\ff <ees' ees''>8 <d' d''>8. <f' f''>16 <bes' bes''>8. <c'' c'''>16 |
  <d'' d'''>2 <bes' bes''>4 r4 |
  <g' g''>8. <g' g''>16 <g' g''>8. <a' a''>16 <bes' bes''>8. <bes' bes''>16 <a' a''>8. <g' g''>16 |
  <f' f''>4 <d' d''>4 <f' f''>2 |
  <f' f''>4. <ees' ees''>8 <d' d''>8. <f' f''>16 <bes' bes''>8. <c'' c'''>16 |
  <d'' d'''>2 <bes' bes''>4 r8 <bes' bes''>8 |
  <c'' c'''>4 <c'' c'''>4 <bes' bes''>4 <a' a''>4 |
  <bes' d'' bes''>2 r2 |
}

chorusFiveLower = {
  <bes,, bes,>4 <bes, d f bes>4 <f, f>4 <bes, d f bes>4 |
  <bes,, bes,>4 <bes, d f bes>4 <f, f>4 <bes, d f bes>4 |
  <ees,, ees,>4 <ees g bes ees'>4 <bes,, bes,>4 <ees g bes ees'>4 |
  <bes,, bes,>4 <bes, d f bes>4 <f, f>4 <bes, d f bes>4 |
  <bes,, bes,>4 <bes, d f bes>4 <f, f>4 <bes, d f bes>4 |
  <bes,, bes,>4 <bes, d f bes>4 <f, f>4 <bes, d f bes>4 |
  <ees, g, c>4 <ees g c'>4 <f, bes, d>4 <f c' ees'>4 |
  bes,8 f,,16 f,,16 d,8 f,,8 bes,,2 |
}

% ==================== CODA: a broadened "Glory, hallelujah" cadence, ff, then let ring ====================

codaUpper = <<
  { \tempo 4 = 66 f'4.\ff ees'8 d'8. f'16 bes'8. c''16 |
    \tempo 4 = 60 d''2 bes'4 r4 |
    \tempo 4 = 52 c''4 c''4 bes'4 a'4 |
    \tempo 4 = 46 bes'1~ |
    \tempo 4 = 42 bes'1 | }
  \\
  { <f bes d'>4 <f bes d'>4 <f bes d'>4 <f bes d'>4 |
    <f bes d'>4 <f bes d'>4 <f bes d'>4 <f bes d'>4 |
    <g c' ees'>4 <g c' ees'>4 <f bes d'>4 <f c' ees'>4 |
    <bes d' f'>1~ |
    <bes d' f'>1 | }
>>

codaLower = {
  <bes,, bes,>4 <bes, d f>4 <f, f>4 <bes, d f>4 |
  <bes,, bes,>4 <bes, d f>4 <f, f>4 <bes, d f>4 |
  <ees, g, c>4 <ees g c'>4 <f, bes, d>4 <f c' ees'>4 |
  <bes,, bes,>1~ |
  <bes,, bes,>1 |
}

% ==================== SCORE ====================

\score {
  \new PianoStaff <<
    \new Staff = "upper" {
      \global
      \tempo 4 = 76
      \introUpper
      \verseOneUpper
      \chorusOneUpper
      \verseTwoUpper
      \chorusTwoUpper
      \verseThreeUpper
      \chorusThreeUpper
      \verseFourUpper
      \chorusFourUpper
      \verseFiveUpper
      \chorusFiveUpper
      \codaUpper
    }
    \new Staff = "lower" {
      \global
      \clef bass
      \introLower
      \verseOneLower
      \chorusOneLower
      \verseTwoLower
      \chorusTwoLower
      \verseThreeLower
      \chorusThreeLower
      \verseFourLower
      \chorusFourLower
      \verseFiveLower
      \chorusFiveLower
      \codaLower
    }
  >>
  \layout { }
  \midi { }
}
