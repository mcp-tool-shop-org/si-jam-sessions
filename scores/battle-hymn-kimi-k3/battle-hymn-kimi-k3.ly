\version "2.24.0"
\language "english"

\header {
  title = "Battle Hymn of the Republic"
  subtitle = "for solo piano"
  poet = "Words: Julia Ward Howe (1862)"
  composer = "Tune: anonymous (in print by 1859)"
  arranger = "Arrangement: si-jam-sessions"
  copyright = "CC0 1.0"
  tagline = ##f
}

upper = {
  \key bf \major
  \time 4/4
  \tempo 4 = 76

  % Introduction (the edition's three bars, full piano)
  <bf d' f'>4.\f <g c' ef'>8 <f bf d'>8 <bf d' f'>8 <d' f' bf'>8. <ef' g' c''>16 |
  <f' bf' d''>2( <d' f' bf'>4) r8 <d' f' b'>8 |
  <ef' g' c''>4-. <ef' g' c''>4-. <d' f' bf'>4-. <c' ef' a'>4-. |

  % Verse 1: p, simple and chordal, close to the edition
  \partial 8 f'8\p |
  <d' f'>8 <d' f'>8 <d' f'>8. <c' ef'>16 <bf d'>8. <d' f'>16 <f' bf'>8. <f' c''>16 |
  <f' d''>8. <f' d''>16 <f' d''>8. <ef' c''>16 <d' bf'>4 r8 <d' bf'>16 <d' bf'>16 |
  <ef' g'>8. <ef' g'>16 <ef' g'>8. <f' a'>16 <f' bf'>8. <f' bf'>16 <f' a'>8. <ef' g'>16 |
  <d' f'>8. <ef' g'>16 <d' f'>8. <bf d'>16 <d' f'>4 r8 <d' f'>16 <d' f'>16 |
  <d' f'>8. <d' f'>16 <d' f'>8. <c' ef'>16 <bf d'>8. <d' f'>16 <f' bf'>8. <f' c''>16 |
  <f' d''>8. <f' d''>16 <f' d''>8. <ef' c''>16 <d' bf'>4 r8 <d' bf'>8 |
  <g' c''>4~ <g' c''>8. <g' c''>16 <f' bf'>4 <ef' a'>4 |
  << { bf'2 r2 } \\ { <bf d'>8 f'16 f'16 f'8 f'8 f'4 f'4 } >> |

  % Chorus 1: mp
  <d' f'>4.\mp <c' ef'>8 d'8. <d' f'>16 <d' bf'>8. <ef' c''>16 |
  <f' d''>2 <d' bf'>4 r4 |
  <ef' g'>8. <ef' g'>16 <ef' g'>8. <f' a'>16 <g' bf'>8. <g' bf'>16 <f' a'>8. <ef' g'>16 |
  <d' f'>4( d'4) <d' f'>2 |
  <d' f'>4. <c' ef'>8 d'8. <d' f'>16 <d' bf'>8. <d' c''>16 |
  <f' d''>2 <d' bf'>4 r8 <f' bf'>8 |
  <g' c''>4 <g' c''>4 <f' bf'>4 <ef' a'>4 |
  <d' bf'>2 r2 |

  % Verse 2: mp, flowing broken chords in the left hand
  \partial 8 <d' f'>16\mp <d' f'>16 |
  <bf d' f'>8 <d' f'>8 <d' f'>8. <c' ef'>16 <bf d'>8. <d' f'>16 <f' bf'>8. <g' c''>16 |
  <f' d''>8. <ef' c''>16 <f' d''>8. <ef' c''>16 <d' bf'>4 r8 <d' bf'>16 <d' bf'>16 |
  <ef' g'>8. <ef' g'>16 <ef' g'>8. <f' a'>16 <f' bf'>8. <f' bf'>16 <f' a'>8. <ef' g'>16 |
  <d' f'>8. <ef' g'>16 <d' f'>8. <bf d'>16 <d' f'>4 r8 <d' f'>16 <d' f'>16 |
  <d' f'>8. <d' f'>16 <d' f'>8. <c' ef'>16 <bf d'>8. <d' f'>16 <f' bf'>8. <f' c''>16 |
  <f' d''>8. <f' d''>16 <f' d''>8. <ef' c''>16 <d' bf'>4 r8 <d' bf'>8 |
  <ef' g' c''>4~ <ef' g' c''>8. <ef' g' c''>16 <d' f' bf'>4 <c' ef' a'>4 |
  <d' f' bf'>2 <d' f' bf'>4 r4 |

  % Chorus 2: mf
  <bf d' f'>4.\mf <c' ef'>8 <bf d'>8. <bf d' f'>16 <d' f' bf'>8. <ef' g' c''>16 |
  <f' bf' d''>2 <d' f' bf'>4 r4 |
  <bf ef' g'>8. <bf ef' g'>16 <bf ef' g'>8. <c' f' a'>16 <ef' g' bf'>8. <ef' g' bf'>16 <c' f' a'>8. <bf ef' g'>16 |
  <bf d' f'>4( <f bf d'>4) <bf d' f'>2 |
  <bf d' f'>4. <c' ef'>8 <bf d'>8. <bf d' f'>16 <d' f' bf'>8. <d' f' c''>16 |
  <f' bf' d''>2 <d' f' bf'>4 r8 <d' f' bf'>8 |
  <ef' g' c''>4 <ef' g' c''>4 <d' f' bf'>4 <c' ef' a'>4 |
  <d' f' bf'>2 r2 |

  % Verse 3: mf, the march, crisp drum figure
  \partial 8 <bf d' f'>16\mf <bf d' f'>16 |
  <bf d' f'>8 <bf d' f'>8 <bf d' f'>8. <g c' ef'>16 <f bf d'>8. <bf d' f'>16 <d' f' bf'>8. <f' c''>16 |
  <f' bf' d''>8. <ef' g' c''>16 <f' bf' d''>8. <ef' g' c''>16 <d' f' bf'>4 r8 <d' f' bf'>16 <d' f' bf'>16 |
  <bf ef' g'>8. <bf ef' g'>16 <bf ef' g'>8. <c' f' a'>16 <ef' g' bf'>8. <ef' g' bf'>16 <c' f' a'>8. <bf ef' g'>16 |
  <bf d' f'>8. <bf ef' g'>16 <bf d' f'>8. <f bf d'>16 <bf d' f'>4 r8 <bf d' f'>16 <bf d' f'>16 |
  <bf d' f'>8. <bf d' f'>16 <bf d' f'>8. <g c' ef'>16 <f bf d'>8. <bf d' f'>16 <d' f' bf'>8. <f' c''>16 |
  <f' bf' d''>8. <f' bf' d''>16 <f' bf' d''>8. <ef' g' c''>16 <d' f' bf'>4 r8 <d' f' bf'>8 |
  <ef' g' c''>4~ <ef' g' c''>8. <ef' g' c''>16 <d' f' bf'>4 <c' ef' a'>4 |
  << { bf'2\< r4 r4\! } \\ { <bf d'>8 f'16-. f'16-. f'8-. f'8-. f'4-. f'4-. } >> |

  % Chorus 3: f
  <bf d' f'>4.\f <c' ef'>8 <bf d'>8. <bf d' f'>16 <d' f' bf'>8. <ef' g' c''>16 |
  <f' bf' d''>2 <d' f' bf'>4 r4 |
  <bf ef' g'>8. <bf ef' g'>16 <bf ef' g'>8. <f' a'>16 <ef' g' bf'>8. <ef' g' bf'>16 <f' a'>8. <bf ef' g'>16 |
  <bf d' f'>4( <bf d'>4) <bf d' f'>2 |
  <bf d' f'>4. <c' ef'>8 <bf d'>8. <bf d' f'>16 <d' f' bf'>8. <d' f' c''>16 |
  <f' bf' d''>2 <d' f' bf'>4 r8 <d' f' bf'>8 |
  <g' c''>4 <g' c''>4 <f' bf'>4 <ef' a'>4 |
  <d' f' bf'>2 r2 |

  % Verse 4: mp, hushed chorale, melody in the tenor register
  \clef bass
  \partial 8 <bf, d f>16\mp <bf, d f>16 |
  <bf, d f>8 <bf, d f>8 <bf, d f>8. <g, c ef>16 <f, bf, d>8. <bf, d f>16 <d f bf>8. <f c'>16 |
  <f bf d'>8. <f c'>16 <f bf d'>8. <f c'>16 <d f bf>4 r8 <d f bf>16 <d f bf>16 |
  <bf, ef g>8. <bf, ef g>16 <bf, ef g>8. <c f a>16 <ef g bf>8. <ef g bf>16 <c f a>8. <bf, ef g>16 |
  <bf, d f>8. <bf, d g>16 <bf, d f>8. <f, bf, d>16 <bf, d f>4 r8 <bf, d f>16 <bf, d f>16 |
  <bf, d f>8. <bf, d f>16 <bf, d f>8. <g, c ef>16 <f, bf, d>8. <bf, d f>16 <d f bf>8. <f c'>16 |
  <f bf d'>8. <f bf d'>16 <f bf d'>8. <f c'>16 <d f bf>4 r8 <d f bf>8 |
  <ef g c'>4~ <ef g c'>8. <ef g c'>16 <d f bf>4 <c ef a>4 |
  <d f bf>1 |
  \clef treble

  % Chorus 4: f
  <bf d' f'>4.\f <g c' ef'>8 <f bf d'>8. <bf d' f'>16 <d' f' bf'>8. <ef' g' c''>16 |
  <f' bf' d''>2 <d' f' bf'>4 r4 |
  <bf ef' g'>8. <bf ef' g'>16 <bf ef' g'>8. <c' f' a'>16 <ef' g' bf'>8. <ef' g' bf'>16 <c' f' a'>8. <bf ef' g'>16 |
  <bf d' f'>4( <f bf d'>4) <bf d' f'>2 |
  <bf d' f'>4. <g c' ef'>8 <f bf d'>8. <bf d' f'>16 <d' f' bf'>8. <d' f' c''>16 |
  <f' bf' d''>2 <d' f' bf'>4 r8 <d' f' bf'>8 |
  <ef' g' c''>4 <ef' g' c''>4 <d' f' bf'>4 <c' ef' a'>4 |
  <d' f' bf'>2 r2 |

  % Verse 5: f to ff, melody in octaves over full chords
  \partial 8 <f' d'' f''>16\f <f' d'' f''>16 |
  <f' d'' f''>8 <f' d'' f''>8 <f' d'' f''>8. <ef' c'' ef''>16 <d' bf' d''>8. <f' d'' f''>16 <bf' f'' bf''>8. <c'' f'' c'''>16 |
  <d'' bf'' d'''>8. <c'' g'' c'''>16 <d'' bf'' d'''>8. <c'' g'' c'''>16 <bf' f'' bf''>4 r8 <bf' f'' bf''>16 <bf' f'' bf''>16 |
  <g' ef'' g''>8. <g' ef'' g''>16 <g' ef'' g''>8. <a' c'' a''>16 <bf' g'' bf''>8. <bf' g'' bf''>16 <a' ef'' a''>8. <g' ef'' g''>16 |
  <f' d'' f''>8. <g' ef'' g''>16 <f' d'' f''>8. <d' bf' d''>16 <f' d'' f''>4 r8 <f' d'' f''>16 <f' d'' f''>16 |
  <f' d'' f''>8.\< <f' d'' f''>16 <f' d'' f''>8. <ef' c'' ef''>16 <d' bf' d''>8. <f' d'' f''>16 <bf' f'' bf''>8. <c'' f'' c'''>16 |
  <d'' bf'' d'''>8. <d'' bf'' d'''>16 <d'' bf'' d'''>8. <c'' g'' c'''>16 <bf' f'' bf''>4 r8 <bf' f'' bf''>8 |
  <c'' g'' c'''>4~ <c'' g'' c'''>8. <c'' g'' c'''>16 <bf' d'' bf''>4 <a' ef'' a''>4 |
  <bf' f'' bf''>2 <bf' d'' f'' bf''>4 <bf' d'' f'' bf''>4 |

  % Chorus 5: ff
  <f' d'' f''>4.\ff <ef' c'' ef''>8 <d' bf' d''>8. <f' d'' f''>16 <bf' f'' bf''>8. <c'' g'' c'''>16 |
  <d'' bf'' d'''>2 <bf' f'' bf''>4 r4 |
  <g' ef'' g''>8. <g' ef'' g''>16 <g' ef'' g''>8. <a' c'' a''>16 <bf' g'' bf''>8. <bf' g'' bf''>16 <a' ef'' a''>8. <g' ef'' g''>16 |
  <f' d'' f''>4( <d' bf' d''>4) <f' d'' f''>2 |
  <f' d'' f''>4. <ef' c'' ef''>8 <d' bf' d''>8. <f' d'' f''>16 <bf' f'' bf''>8. <c'' f'' c'''>16 |
  <d'' bf'' d'''>2 <bf' f'' bf''>4 r8 <bf' f'' bf''>8 |
  <c'' g'' c'''>4 <c'' g'' c'''>4 <bf' d'' bf''>4 <a' ef'' a''>4 |
  <bf' f'' bf''>2 r2 |

  % Coda: final "Glory, hallelujah" cadence, broadened
  \tempo 4 = 70
  <c'' g'' c'''>4\ff <c'' g'' c'''>4 <bf' d'' bf''>4 <a' ef'' a''>4 |
  \tempo 4 = 60
  <bf' f'' bf''>2 <bf' d'' f'' bf''>2 |
  \tempo 4 = 50
  <f' bf' d'' f''>1\ff\fermata \bar "|."
}

lower = {
  \key bf \major
  \time 4/4
  \clef bass

  % Introduction
  <bf,, bf,>4 <f, bf, d>4 <bf,, bf,>4 <f, bf, d>4 |
  <bf,, bf,>4 <f, bf, d>4 <bf,, bf,>4 <f, bf, d>4 |
  <ef,, ef,>4 <g, c ef>4 <f,, f,>4 <f, a, c ef>4 |

  % Verse 1
  \partial 8 r8 |
  bf,4 <f, bf, d>4 bf,4 <f, bf, d>4 |
  bf,4 <f, bf, d>4 bf,4 <f, bf, d>4 |
  ef,4 <g, bf, ef>4 ef,4 <g, bf, ef>4 |
  bf,4 <f, bf, d>4 bf,4 <f, bf, d>4 |
  bf,4 <f, bf, d>4 bf,4 <f, bf, d>4 |
  bf,4 <f, bf, d>4 bf,4 <f, bf, d>4 |
  <ef,, ef,>4 <ef,, ef,>4 <f,, f,>4 <f,, f,>4 |
  bf,4 r4 r4 r4 |

  % Chorus 1
  <bf,, bf,>4. bf,8 bf,8. bf,16 bf,8. bf,16 |
  <bf,, bf,>2 bf,4 r4 |
  <ef,, ef,>8. ef,16 ef,8. ef,16 ef,8. ef,16 ef,8. ef,16 |
  <bf,, bf,>4 bf,4 bf,2 |
  <bf,, bf,>4. bf,8 bf,8. bf,16 bf,8. bf,16 |
  <bf,, bf,>2 bf,4 r8 bf,8 |
  <ef,, ef,>4 <ef,, ef,>4 <f,, f,>4 <f,, f,>4 |
  <bf,, bf,>2 r2 |

  % Verse 2: flowing broken chords
  \partial 8 r8 |
  bf,,8 f,8 bf,8 f8 bf,8 f,8 bf,8 f8 |
  bf,,8 f,8 bf,8 f8 bf,8 f,8 bf,8 f8 |
  ef,,8 g,8 ef8 g8 ef8 g,8 ef8 g8 |
  bf,,8 f,8 bf,8 f8 bf,8 f,8 bf,8 f8 |
  bf,,8 f,8 bf,8 f8 bf,8 f,8 bf,8 f8 |
  bf,,8 f,8 bf,8 f8 bf,8 f,8 bf,8 f8 |
  ef,8 g,8 ef8 g8 f,8 bf,8 f,8 a,8 |
  bf,,8 f,8 bf,8 f8 bf,8 f,8 bf,8 f8 |

  % Chorus 2
  <bf,, bf,>4. <f, bf, d>8 <bf,, bf,>8. <f, bf, d>16 <bf,, bf,>8. <f, bf, d>16 |
  <bf,, bf,>2 <f, bf, d>4 r4 |
  <ef,, ef,>8. <g, bf, ef>16 <ef,, ef,>8. <g, bf, ef>16 <ef,, ef,>8. <g, bf, ef>16 <ef,, ef,>8. <g, bf, ef>16 |
  <bf,, bf,>4 <f, bf, d>4 <bf,, bf,>2 |
  <bf,, bf,>4. <f, bf, d>8 <bf,, bf,>8. <f, bf, d>16 <bf,, bf,>8. <f, bf, d>16 |
  <bf,, bf,>2 <f, bf, d>4 r8 <bf,, bf,>8 |
  <ef,, ef,>4 <g, c ef>4 <f,, f,>4 <f, a, c ef>4 |
  <bf,, bf,>2 r2 |

  % Verse 3: the march drum figure
  \partial 8 bf,,16 bf,,16 |
  bf,,8 bf,16 bf,16 bf,,8 bf,8 bf,,4 bf,4 |
  bf,,8 bf,16 bf,16 bf,,8 bf,8 bf,,4 bf,4 |
  ef,,8 ef,16 ef,16 ef,,8 ef,8 ef,,4 ef,4 |
  bf,,8 bf,16 bf,16 bf,,8 bf,8 bf,,4 bf,4 |
  bf,,8 bf,16 bf,16 bf,,8 bf,8 bf,,4 bf,4 |
  bf,,8 bf,16 bf,16 bf,,8 bf,8 bf,,4 bf,4 |
  ef,,8 ef,16 ef,16 f,,8 f,8 f,,4 f,4 |
  bf,,8 bf,16 bf,16 bf,,8 bf,8 bf,,4 bf,,4 |

  % Chorus 3
  <bf,, bf,>4 <f, bf, d>4 <bf,, bf,>4 <f, bf, d>4 |
  <bf,, bf,>2 <f, bf, d>4 r4 |
  <ef,, ef,>4 <g, bf, ef>4 <ef,, ef,>4 <g, bf, ef>4 |
  <bf,, bf,>4 <f, bf, d>4 <bf,, bf,>2 |
  <bf,, bf,>4 <f, bf, d>4 <bf,, bf,>4 <f, bf, d>4 |
  <bf,, bf,>2 <f, bf, d>4 r8 <bf,, bf,>8 |
  <ef,, ef,>4 <g, c ef>4 <f,, f,>4 <f, a, c ef>4 |
  <bf,, bf,>2 r2 |

  % Verse 4: hushed chorale, deep sustained bass
  \partial 8 r8 |
  bf,,1 |
  bf,,1 |
  ef,,1 |
  bf,,1 |
  bf,,1 |
  bf,,1 |
  ef,,2 f,,2 |
  bf,,1 |

  % Chorus 4
  <bf,, bf,>4. <f, bf, d>8 <bf,, bf,>8. <f, bf, d>16 <bf,, bf,>8. <f, bf, d>16 |
  <bf,, bf,>2 <f, bf, d>4 r4 |
  <ef,, ef,>8. <g, bf, ef>16 <ef,, ef,>8. <g, bf, ef>16 <ef,, ef,>8. <g, bf, ef>16 <ef,, ef,>8. <g, bf, ef>16 |
  <bf,, bf,>4 <f, bf, d>4 <bf,, bf,>2 |
  <bf,, bf,>4. <f, bf, d>8 <bf,, bf,>8. <f, bf, d>16 <bf,, bf,>8. <f, bf, d>16 |
  <bf,, bf,>2 <f, bf, d>4 r8 <bf,, bf,>8 |
  <ef,, ef,>4 <g, c ef>4 <f,, f,>4 <f, a, c ef>4 |
  <bf,, bf,>8 bf,16 bf,16 bf,8 bf,8 bf,4 bf,4 |

  % Verse 5: moving bass with full chord stabs
  \partial 8 <bf,, bf,>8 |
  bf,,8 <f, bf, d>8 bf,8 <f, bf, d>8 f,8 <f, bf, d>8 bf,8 <f, bf, d>8 |
  bf,,8 <f, bf, d>8 bf,8 <f, bf, d>8 f,8 <f, bf, d>8 bf,8 <f, bf, d>8 |
  ef,,8 <g, bf, ef>8 ef,8 <g, bf, ef>8 bf,,8 <g, bf, ef>8 ef,8 <g, bf, ef>8 |
  bf,,8 <f, bf, d>8 bf,8 <f, bf, d>8 f,8 <f, bf, d>8 bf,8 <f, bf, d>8 |
  bf,,8 <f, bf, d>8 bf,8 <f, bf, d>8 f,8 <f, bf, d>8 bf,8 <f, bf, d>8 |
  bf,,8 <f, bf, d>8 bf,8 <f, bf, d>8 f,8 <f, bf, d>8 bf,8 <f, bf, d>8 |
  ef,,8 <g, c ef>8 ef,8 <g, c ef>8 f,,8 <f, bf, d>8 f,,8 <f, a, c ef>8 |
  bf,,8 <f, bf, d>8 bf,8 <f, bf, d>8 <bf,, bf,>4 <f, bf, d>4 |

  % Chorus 5
  <bf,, bf,>4. <f, bf, d>8 <bf,, bf,>8. <f, bf, d>16 <bf,, bf,>8. <f, bf, d>16 |
  <bf,, bf,>2 <f, bf, d>4 r4 |
  <ef,, ef,>8. <g, bf, ef>16 <ef,, ef,>8. <g, bf, ef>16 <ef,, ef,>8. <g, bf, ef>16 <ef,, ef,>8. <g, bf, ef>16 |
  <bf,, bf,>4 <f, bf, d>4 <bf,, bf,>2 |
  <bf,, bf,>4. <f, bf, d>8 <bf,, bf,>8. <f, bf, d>16 <bf,, bf,>8. <f, bf, d>16 |
  <bf,, bf,>2 <f, bf, d>4 r8 <bf,, bf,>8 |
  <ef,, ef,>4 <g, c ef>4 <f,, f,>4 <f, a, c ef>4 |
  <bf,, bf,>8 bf,,16 bf,,16 bf,,8 bf,,8 bf,,4 bf,,4 |

  % Coda
  <ef,, ef,>4 <g, c ef>4 <f,, f,>4 <f, a, c ef>4 |
  <bf,, bf,>4 <f, bf, d>4 <bf,, bf,>4 <f, bf, d>4 |
  <bf,, f, bf,>1\fermata \bar "|."
}

\score {
  \new PianoStaff <<
    \new Staff = "upper" { \upper }
    \new Staff = "lower" { \lower }
  >>
  \layout { }
  \midi { }
}
