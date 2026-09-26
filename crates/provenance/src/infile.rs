//! Reading the licence a file states about itself.
//!
//! **LilyPond.** Every assignment to `license` or `copyright`, wherever it appears outside
//! comments, strings and embedded Scheme, is a statement:
//!
//! - `license = "..."` (or a Scheme string `#"..."`) is a [`StatementField::LilypondLicense`];
//! - `copyright = "..."` is a [`StatementField::LilypondCopyright`];
//! - `copyright = \markup { ... }` is a [`StatementField::LilypondCopyrightMarkup`] whose
//!   text is the markup's LilyPond string literals, in order, joined by one space, with
//!   ASCII whitespace runs collapsed and the ends trimmed. Scheme data inside the markup
//!   (font names, URLs, numbers) is not text and is skipped;
//! - `copyright = ##f` states nothing.
//!
//! Any other value for either field (a variable, a markup for `license`, a markup without
//! braces) is unreadable, and an unreadable file is refused.
//!
//! **SMF.** Every copyright meta event (`FF 02`) is a [`StatementField::SmfCopyright`].
//! Every other text meta event (text, track name, instrument, lyric, marker, cue point,
//! program, device) whose text mentions copyright or a licence, or names a restriction
//! such as a ban on AI use (see [`smf_marker`]), is a [`StatementField::SmfText`], so a
//! notice written into the wrong event is still read.
//! Only a plain SMF is read: the bytes open with one `MThd` chunk of length 6, and every
//! later chunk is an `MTrk`. midly would unwrap a RIFF (RMID) file and skip unknown chunks
//! unread, even with `strict`, and either could hold a licence notice this reader never
//! sees, so both are unreadable. A timecode division is unreadable too, and is refused
//! before midly sees it: midly 0.5.3 negates the division's high byte as an `i8`, which
//! overflows for `0x80` and panics under overflow checks. The rest, including a ragged end,
//! is parsed with midly's `strict` feature; a file it rejects is unreadable. It is read as
//! `Smf::parse` reads it, minus the one floating-point computation `Smf::parse` makes (see
//! [`read_tracks`]), because this crate runs inside the law's wasm and the law's binary
//! carries no floating point.
//!
//! Statements are returned in file order, each with its text verbatim (the markup text is
//! the one exception, as described). They must be UTF-8.

use alloc::string::String;
use alloc::vec::Vec;

use midly::{Format, Header, MetaMessage, TrackEvent, TrackEventKind};

use crate::licence;
use crate::receipt::{Media, Statement, StatementField};

/// Why a file's licence statements could not be read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unreadable {
    NotUtf8,
    UnterminatedString,
    UnterminatedComment,
    UnterminatedScheme,
    UnterminatedMarkup,
    /// A string escape other than `\"`, `\\`, `\n` or `\t` inside a licence value.
    BadEscape,
    /// A `license` or `copyright` value this reader does not accept.
    UnreadableValue,
    /// Not a plain SMF (see the module documentation). `offset` is where the offending
    /// chunk starts: 0 when the bytes do not open with a header.
    NotPlainSmf {
        offset: usize,
    },
    /// A timecode (SMPTE) division.
    TimecodeDivision,
    /// midly rejected the SMF. The message is midly's.
    Smf(&'static str),
    /// The SMF header declares a different number of tracks than the file holds.
    TrackCountMismatch {
        declared: usize,
        found: usize,
    },
    /// A format-0 (single-track) SMF that holds other than one track.
    SingleTrackFormat {
        tracks: usize,
    },
    /// An SMF licence statement that is not UTF-8.
    SmfTextNotUtf8,
}

/// Every licence statement the file makes about itself, in file order.
pub fn statements(media: Media, bytes: &[u8]) -> Result<Vec<Statement>, Unreadable> {
    match media {
        Media::Lilypond => lilypond(bytes),
        Media::Smf => smf(bytes),
    }
}

fn smf(bytes: &[u8]) -> Result<Vec<Statement>, Unreadable> {
    let division = plain_smf(bytes)?;
    if division & 0x8000 != 0 {
        return Err(Unreadable::TimecodeDivision);
    }
    let (_, tracks) = read_tracks(bytes)?;
    let mut out = Vec::new();
    for track in &tracks {
        for event in track {
            let TrackEventKind::Meta(meta) = event.kind else {
                continue;
            };
            let (field, raw) = match meta {
                MetaMessage::Copyright(raw) => (StatementField::SmfCopyright, raw),
                MetaMessage::Text(raw)
                | MetaMessage::TrackName(raw)
                | MetaMessage::InstrumentName(raw)
                | MetaMessage::Lyric(raw)
                | MetaMessage::Marker(raw)
                | MetaMessage::CuePoint(raw)
                | MetaMessage::ProgramName(raw)
                | MetaMessage::DeviceName(raw)
                    if smf_marker(raw) =>
                {
                    (StatementField::SmfText, raw)
                }
                _ => continue,
            };
            let text = core::str::from_utf8(raw).map_err(|_| Unreadable::SmfTextNotUtf8)?;
            out.push(Statement {
                field,
                text: String::from(text),
            });
        }
    }
    Ok(out)
}

/// The tracks of an SMF, each a list of its events.
pub(crate) type Tracks<'a> = Vec<Vec<TrackEvent<'a>>>;

/// Reads an SMF as midly's `Smf::parse` does with `strict`, without its floating point.
///
/// `Smf::parse` is three steps:
/// 1. the lazy `midly::parse`;
/// 2. `EventIter::into_vec` on each track, which sizes its vector with
///    `(bytes as f32 * (1.0 / 3.0)) as usize`;
/// 3. two checks on the track count.
///
/// Step 2's capacity guess puts f32 instructions into any wasm that links it, and the
/// law's binary carries none. Iterating each `EventIter` instead reads the same events
/// with the same strict error. Step 3 is repeated here, in the same order and against the
/// same number: before any track is read, midly's track iterator reports the header's
/// declared count as its size hint, which is what `Smf::parse` compares with. The two
/// checks refuse by their own names. The ingest crate reads SMF the same way; `tests.rs`
/// compares this function with `Smf::parse` on the committed score and on truncations and
/// byte changes of it.
///
/// A timecode division must be refused before this is called: midly's header reader panics
/// on a division of `0x80xx` under overflow checks.
pub(crate) fn read_tracks(bytes: &[u8]) -> Result<(Header, Tracks<'_>), Unreadable> {
    fn refused(e: midly::Error) -> Unreadable {
        Unreadable::Smf(e.kind().message())
    }
    let (header, track_iter) = midly::parse(bytes).map_err(refused)?;
    let declared = track_iter.size_hint().0;
    let mut tracks = Vec::new();
    for track in track_iter {
        let events = track
            .map_err(refused)?
            .collect::<Result<Vec<TrackEvent<'_>>, midly::Error>>()
            .map_err(refused)?;
        tracks.push(events);
    }
    if tracks.len() != declared {
        return Err(Unreadable::TrackCountMismatch {
            declared,
            found: tracks.len(),
        });
    }
    if header.format == Format::SingleTrack && tracks.len() != 1 {
        return Err(Unreadable::SingleTrackFormat {
            tracks: tracks.len(),
        });
    }
    Ok((header, tracks))
}

/// Makes the layout checks midly does not make, and returns the header's division word:
/// the bytes open with one `MThd` chunk of length 6, and every later chunk is an `MTrk`.
/// A ragged end is left to midly's `strict` parser, which refuses it as malformed.
fn plain_smf(bytes: &[u8]) -> Result<u16, Unreadable> {
    let header = bytes
        .get(..14)
        .filter(|h| h[..4] == *b"MThd" && h[4..8] == 6u32.to_be_bytes())
        .ok_or(Unreadable::NotPlainSmf { offset: 0 })?;
    let division = u16::from_be_bytes([header[12], header[13]]);
    let mut at = 14;
    while let Some(head) = bytes.get(at..at + 8) {
        if head[..4] != *b"MTrk" {
            return Err(Unreadable::NotPlainSmf { offset: at });
        }
        let len = u32::from_be_bytes([head[4], head[5], head[6], head[7]]) as usize;
        match (at + 8).checked_add(len) {
            Some(end) if end <= bytes.len() => at = end,
            // A chunk that runs past the end: midly's strict parser refuses it.
            _ => break,
        }
    }
    Ok(division)
}

/// True if SMF text bytes are a licence statement. They mention copyright or a licence:
/// one of `copyright`, `(c)`, `licen`, `public domain`, `creative commons`,
/// `rights reserved`, `cc0` (ASCII, any case), or a copyright sign (U+00A9 in UTF-8, or
/// byte `A9` in text that is not UTF-8). Or they hold a restriction phrase
/// (`licence::restriction_in`, on the text's words), which is how a ban on AI use in a
/// text event is read.
pub fn smf_marker(raw: &[u8]) -> bool {
    const MARKERS: &[&[u8]] = &[
        b"copyright",
        b"(c)",
        b"licen",
        b"public domain",
        b"creative commons",
        b"rights reserved",
        b"cc0",
    ];
    let lower: Vec<u8> = raw.iter().map(u8::to_ascii_lowercase).collect();
    let ascii_hit = MARKERS
        .iter()
        .any(|m| lower.windows(m.len()).any(|w| w == *m));
    let sign = match core::str::from_utf8(raw) {
        Ok(text) => text.contains('\u{a9}'),
        Err(_) => raw.contains(&0xA9),
    };
    let restriction =
        licence::restriction_in(&licence::fold(&String::from_utf8_lossy(raw))).is_some();
    ascii_hit || sign || restriction
}

fn lilypond(bytes: &[u8]) -> Result<Vec<Statement>, Unreadable> {
    let text = core::str::from_utf8(bytes).map_err(|_| Unreadable::NotUtf8)?;
    let s = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < s.len() {
        match s[i] {
            b'%' => i = skip_comment(s, i)?,
            b'"' => i = string_end(s, i)?,
            b'#' | b'$' => i = skip_scheme(s, i + 1)?,
            b'\\' => i = skip_command(s, i),
            c if is_word(c) => {
                let start = i;
                while i < s.len() && is_word(s[i]) {
                    i += 1;
                }
                let field = match &s[start..i] {
                    b"license" => StatementField::LilypondLicense,
                    b"copyright" => StatementField::LilypondCopyright,
                    _ => continue,
                };
                let eq = skip_space(s, i)?;
                if s.get(eq) != Some(&b'=') || s.get(eq + 1) == Some(&b'=') {
                    continue;
                }
                let at = skip_space(s, eq + 1)?;
                let (statement, next) = licence_value(s, at, field)?;
                out.extend(statement);
                i = next;
            }
            _ => i += 1,
        }
    }
    Ok(out)
}

/// Letters, digits, `-`, `_` and every non-ASCII byte: what can border a LilyPond word.
fn is_word(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'-' || b >= 0x80
}

/// Reads the value of a `license` or `copyright` assignment starting at `at`.
fn licence_value(
    s: &[u8],
    at: usize,
    field: StatementField,
) -> Result<(Option<Statement>, usize), Unreadable> {
    let rest = &s[at..];
    if rest.starts_with(b"\"") || rest.starts_with(b"#\"") {
        let open = if rest[0] == b'#' { at + 1 } else { at };
        let end = string_end(s, open)?;
        let text = decode_string(&s[open + 1..end - 1])?;
        return Ok((Some(Statement { field, text }), end));
    }
    if field == StatementField::LilypondCopyright
        && rest.starts_with(b"##f")
        && rest.get(3).is_none_or(|&b| is_delimiter(b))
    {
        return Ok((None, at + 3));
    }
    if field == StatementField::LilypondCopyright
        && rest.starts_with(b"\\markup")
        && rest.get(7).is_none_or(|&b| !is_word(b))
    {
        let open = skip_space(s, at + 7)?;
        if s.get(open) != Some(&b'{') {
            return Err(Unreadable::UnreadableValue);
        }
        let (text, end) = markup_text(s, open)?;
        let statement = Statement {
            field: StatementField::LilypondCopyrightMarkup,
            text,
        };
        return Ok((Some(statement), end));
    }
    Err(Unreadable::UnreadableValue)
}

/// The text of a braced markup starting at `open` (a `{`), and the index after its `}`.
fn markup_text(s: &[u8], open: usize) -> Result<(String, usize), Unreadable> {
    let mut parts: Vec<String> = Vec::new();
    let mut depth = 0usize;
    let mut i = open;
    while i < s.len() {
        match s[i] {
            b'{' => {
                depth += 1;
                i += 1;
            }
            b'}' => {
                depth -= 1;
                i += 1;
                if depth == 0 {
                    return Ok((collapse(&parts.join(" ")), i));
                }
            }
            b'"' => {
                let end = string_end(s, i)?;
                parts.push(decode_string(&s[i + 1..end - 1])?);
                i = end;
            }
            b'%' => i = skip_comment(s, i)?,
            b'#' | b'$' => i = skip_scheme(s, i + 1)?,
            b'\\' => i = skip_command(s, i),
            _ => i += 1,
        }
    }
    Err(Unreadable::UnterminatedMarkup)
}

/// Trims the ends and collapses inner runs of ASCII whitespace to one space.
fn collapse(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for word in text.split_ascii_whitespace() {
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(word);
    }
    out
}

/// Skips spaces and comments; returns the index of the next other byte.
fn skip_space(s: &[u8], mut i: usize) -> Result<usize, Unreadable> {
    while i < s.len() {
        match s[i] {
            b' ' | b'\t' | b'\n' | b'\r' | b'\x0c' => i += 1,
            b'%' => i = skip_comment(s, i)?,
            _ => break,
        }
    }
    Ok(i)
}

/// `s[i]` is `%`. Skips a `%{ ... %}` block comment or a line comment.
fn skip_comment(s: &[u8], i: usize) -> Result<usize, Unreadable> {
    if s.get(i + 1) == Some(&b'{') {
        let body = i + 2;
        match s[body..].windows(2).position(|w| w == b"%}") {
            Some(p) => Ok(body + p + 2),
            None => Err(Unreadable::UnterminatedComment),
        }
    } else {
        Ok(s[i..]
            .iter()
            .position(|&b| b == b'\n')
            .map_or(s.len(), |p| i + p + 1))
    }
}

/// `s[i]` is `\`. Skips the command name (or the single escaped byte).
fn skip_command(s: &[u8], i: usize) -> usize {
    let mut j = i + 1;
    while j < s.len() && s[j].is_ascii_alphabetic() {
        j += 1;
    }
    if j == i + 1 { (i + 2).min(s.len()) } else { j }
}

/// `s[open]` is `"`. Returns the index after the closing quote. A backslash always takes
/// the next byte with it.
fn string_end(s: &[u8], open: usize) -> Result<usize, Unreadable> {
    let mut i = open + 1;
    while i < s.len() {
        match s[i] {
            b'"' => return Ok(i + 1),
            b'\\' => i += 2,
            _ => i += 1,
        }
    }
    Err(Unreadable::UnterminatedString)
}

/// Decodes the inside of a string literal. Only `\"`, `\\`, `\n` and `\t` are accepted.
fn decode_string(raw: &[u8]) -> Result<String, Unreadable> {
    let mut out: Vec<u8> = Vec::with_capacity(raw.len());
    let mut i = 0;
    while i < raw.len() {
        if raw[i] == b'\\' {
            let escaped = match raw.get(i + 1) {
                Some(b'"') => b'"',
                Some(b'\\') => b'\\',
                Some(b'n') => b'\n',
                Some(b't') => b'\t',
                _ => return Err(Unreadable::BadEscape),
            };
            out.push(escaped);
            i += 2;
        } else {
            out.push(raw[i]);
            i += 1;
        }
    }
    // The input was UTF-8 and only ASCII escapes were replaced, so this cannot fail.
    String::from_utf8(out).map_err(|_| Unreadable::NotUtf8)
}

/// What ends a Scheme atom.
fn is_delimiter(b: u8) -> bool {
    matches!(
        b,
        b' ' | b'\t' | b'\n' | b'\r' | b'\x0c' | b'(' | b')' | b'"' | b';' | b'{' | b'}'
    )
}

/// Skips one Scheme datum starting at `i` (just after LilyPond's `#` or `$`). Quote
/// prefixes and `#| |#` comments before the datum are stepped over in a loop, so no input
/// can deepen the stack.
fn skip_scheme(s: &[u8], mut i: usize) -> Result<usize, Unreadable> {
    loop {
        match s.get(i) {
            None => return Err(Unreadable::UnterminatedScheme),
            Some(b'\'' | b'`' | b',') => {
                i += if s.get(i + 1) == Some(&b'@') { 2 } else { 1 };
            }
            Some(b'#') if s.get(i + 1) == Some(&b'|') => {
                i = skip_scheme_blank(s, skip_block_comment(s, i)?);
            }
            Some(b'"') => return string_end(s, i),
            Some(b'(') => return skip_list(s, i),
            Some(b'#') => {
                return match s.get(i + 1) {
                    Some(b'(') => skip_list(s, i + 1),
                    Some(b'\\') => {
                        // A character literal: the first character is taken whatever it is.
                        if i + 2 >= s.len() {
                            return Err(Unreadable::UnterminatedScheme);
                        }
                        Ok(atom_end(s, i + 3))
                    }
                    _ => Ok(atom_end(s, i + 1)),
                };
            }
            Some(&b) if is_delimiter(b) => return Err(Unreadable::UnterminatedScheme),
            Some(_) => return Ok(atom_end(s, i)),
        }
    }
}

/// Skips ASCII whitespace inside Scheme, where `%` is an ordinary character.
fn skip_scheme_blank(s: &[u8], mut i: usize) -> usize {
    while i < s.len() && matches!(s[i], b' ' | b'\t' | b'\n' | b'\r' | b'\x0c') {
        i += 1;
    }
    i
}

fn atom_end(s: &[u8], mut i: usize) -> usize {
    while i < s.len() && !is_delimiter(s[i]) {
        i += 1;
    }
    i
}

/// `s[i..]` starts with `#|`. Returns the index after the matching `|#`.
fn skip_block_comment(s: &[u8], i: usize) -> Result<usize, Unreadable> {
    match s[i + 2..].windows(2).position(|w| w == b"|#") {
        Some(p) => Ok(i + 2 + p + 2),
        None => Err(Unreadable::UnterminatedScheme),
    }
}

/// `s[open]` is `(`. Skips to after the matching `)`, stepping over strings, `;` comments,
/// `#| |#` comments and character literals.
fn skip_list(s: &[u8], open: usize) -> Result<usize, Unreadable> {
    let mut depth = 0usize;
    let mut i = open;
    while i < s.len() {
        match s[i] {
            b'(' => {
                depth += 1;
                i += 1;
            }
            b')' => {
                depth -= 1;
                i += 1;
                if depth == 0 {
                    return Ok(i);
                }
            }
            b'"' => i = string_end(s, i).map_err(|_| Unreadable::UnterminatedScheme)?,
            b';' => {
                i = s[i..]
                    .iter()
                    .position(|&b| b == b'\n')
                    .map_or(s.len(), |p| i + p + 1);
            }
            b'#' if s.get(i + 1) == Some(&b'|') => i = skip_block_comment(s, i)?,
            b'#' if s.get(i + 1) == Some(&b'\\') => i += 3,
            _ => i += 1,
        }
    }
    Err(Unreadable::UnterminatedScheme)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;
    // Tests may call `Smf::parse`: only the crate itself must stay free of its floating
    // point.
    use midly::Smf;

    fn st(field: StatementField, text: &str) -> Statement {
        Statement {
            field,
            text: String::from(text),
        }
    }

    fn ly(text: &str) -> Result<Vec<Statement>, Unreadable> {
        statements(Media::Lilypond, text.as_bytes())
    }

    use StatementField::{
        LilypondCopyright as Copyright, LilypondCopyrightMarkup as Markup,
        LilypondLicense as License,
    };

    #[test]
    fn reads_license_and_copyright_strings() {
        let src = "\\version \"2.19.32\"\n\\header {\n  title = \"T\"\n  license = \"Public Domain\"\n  copyright = \"Public Domain\"\n}\n";
        assert_eq!(
            ly(src).unwrap(),
            vec![st(License, "Public Domain"), st(Copyright, "Public Domain")]
        );
        assert_eq!(ly("license=#\"CC\"").unwrap(), vec![st(License, "CC")]);
        assert_eq!(
            ly("license = \"a \\\"b\\\" \\\\ c\"").unwrap(),
            vec![st(License, "a \"b\" \\ c")]
        );
    }

    #[test]
    fn ignores_comments_strings_scheme_and_other_words() {
        let src = concat!(
            "% license = \"line comment\"\n",
            "%{ license = \"block comment\" %}\n",
            "title = \"license = \\\"in a string\\\"\"\n",
            "#(define license \"scheme\")\n",
            "mutopialicense = \"x\"\n",
            "license-url = \"x\"\n",
            "\\license = \"a command, not a field\"\n",
            "licensee = \"x\"\n",
            "license == \"a comparison\"\n",
        );
        assert_eq!(ly(src).unwrap(), vec![]);
    }

    #[test]
    fn reads_a_markup_copyright_as_its_text() {
        let src = concat!(
            "copyright = \\markup {\\override #'(font-name . \"DejaVu Sans, Bold\") ",
            "\\with-url #\"http://example.org\" {\\abs-fontsize #9 \"Placed in the \" ",
            "\\char ##x2014 \"public domain \" } % \"not text\"\n",
            "\"by the typesetter\" }\n",
            "tagline = ##f\n",
        );
        assert_eq!(
            ly(src).unwrap(),
            vec![st(Markup, "Placed in the public domain by the typesetter")]
        );
    }

    #[test]
    fn copyright_false_states_nothing() {
        assert_eq!(ly("copyright = ##f\n").unwrap(), vec![]);
    }

    #[test]
    fn unreadable_values_refuse() {
        assert_eq!(
            ly("license = \\markup { \"PD\" }"),
            Err(Unreadable::UnreadableValue)
        );
        assert_eq!(
            ly("license = \\myLicence"),
            Err(Unreadable::UnreadableValue)
        );
        assert_eq!(
            ly("copyright = \\markup \\line"),
            Err(Unreadable::UnreadableValue)
        );
        assert_eq!(ly("license = ##f"), Err(Unreadable::UnreadableValue));
        assert_eq!(ly("copyright = #mycopy"), Err(Unreadable::UnreadableValue));
        assert_eq!(ly("license = \"a \\q\""), Err(Unreadable::BadEscape));
        assert_eq!(ly("license = \"open"), Err(Unreadable::UnterminatedString));
        assert_eq!(ly("%{ open"), Err(Unreadable::UnterminatedComment));
        assert_eq!(ly("#(open"), Err(Unreadable::UnterminatedScheme));
        assert_eq!(
            ly("copyright = \\markup { \"x\""),
            Err(Unreadable::UnterminatedMarkup)
        );
        assert_eq!(
            statements(Media::Lilypond, b"license = \"\xff\""),
            Err(Unreadable::NotUtf8)
        );
    }

    #[test]
    fn long_runs_of_scheme_prefixes_do_not_deepen_the_stack() {
        let quotes = alloc::format!("#{}x license = \"PD\"", "'".repeat(200_000));
        assert_eq!(ly(&quotes).unwrap(), vec![st(License, "PD")]);
        let comments = alloc::format!("#{}x license = \"PD\"", "#||# ".repeat(200_000));
        assert_eq!(ly(&comments).unwrap(), vec![st(License, "PD")]);
        // LilyPond `#`, a Scheme block comment, then the datum `%x`: inside Scheme `%` is
        // not a comment, so the licence after it is still read.
        assert_eq!(
            ly("##|c|# %x license = \"PD\"").unwrap(),
            vec![st(License, "PD")]
        );
    }

    #[test]
    fn scheme_character_literals_do_not_unbalance_lists() {
        let src = "#(list #\\( #\\) \"s\" ; comment ( \n #| ( |# 1)\nlicense = \"PD\"";
        assert_eq!(ly(src).unwrap(), vec![st(License, "PD")]);
    }

    fn smf_with_meta(metas: &[(u8, &[u8])]) -> Vec<u8> {
        let mut track = Vec::new();
        for &(kind, text) in metas {
            track.extend_from_slice(&[0x00, 0xFF, kind, u8::try_from(text.len()).unwrap()]);
            track.extend_from_slice(text);
        }
        track.extend_from_slice(&[
            0x00, 0x90, 60, 64, 0x60, 0x80, 60, 0, 0x00, 0xFF, 0x2F, 0x00,
        ]);
        let mut out = Vec::new();
        out.extend_from_slice(b"MThd\0\0\0\x06\0\0\0\x01\0\x60MTrk");
        out.extend_from_slice(&u32::try_from(track.len()).unwrap().to_be_bytes());
        out.extend_from_slice(&track);
        out
    }

    #[test]
    fn reads_smf_copyright_and_marked_text() {
        let file = smf_with_meta(&[
            (0x03, b"The Entertainer"),
            (0x01, b"creator: "),
            (0x02, b"Public Domain"),
            (0x01, b"(C) 1999 Someone"),
            (0x03, b"Licensed to nobody"),
        ]);
        assert_eq!(
            statements(Media::Smf, &file).unwrap(),
            vec![
                st(StatementField::SmfCopyright, "Public Domain"),
                st(StatementField::SmfText, "(C) 1999 Someone"),
                st(StatementField::SmfText, "Licensed to nobody"),
            ]
        );
        let none = smf_with_meta(&[(0x03, b"Caf\xc3\xa9"), (0x01, b"GNU LilyPond 2.19.32")]);
        assert_eq!(statements(Media::Smf, &none).unwrap(), vec![]);
    }

    #[test]
    fn smf_markers_and_encodings() {
        assert!(smf_marker(b"Copyright 2001"));
        assert!(smf_marker("\u{a9} 2001".as_bytes()));
        assert!(smf_marker(b"\xa9 2001"));
        assert!(!smf_marker("Café".as_bytes()));
        // A restriction phrase makes a text event a statement, AI wording included.
        assert!(smf_marker(b"Not for AI training"));
        assert!(smf_marker(b"NO   TDM"));
        assert!(smf_marker(b"Text and data mining reserved"));
        assert!(smf_marker(b"For personal use only"));
        // `ai` inside a word is not AI wording; the Entertainer's own events are not
        // statements.
        for text in [
            &b"Main theme"[..],
            b"Raised fourth",
            b"The Entertainer",
            b"creator: ",
            b"GNU LilyPond 2.19.32          ",
            b"up:",
            b"down:",
        ] {
            assert!(!smf_marker(text), "{text:?}");
        }
        let latin1 = smf_with_meta(&[(0x02, b"\xa9 2001")]);
        assert_eq!(
            statements(Media::Smf, &latin1),
            Err(Unreadable::SmfTextNotUtf8)
        );
    }

    /// Predicate version 3 admits CC0 1.0, so a text event that names CC0 states a
    /// licence: a CC0 notice written into the wrong event is still read.
    #[test]
    fn a_text_event_that_names_cc0_is_a_statement() {
        assert!(smf_marker(b"CC0 1.0"));
        assert!(smf_marker(b"Dedicated under cc0"));
        let file = smf_with_meta(&[(0x03, b"Piano"), (0x01, b"CC0 1.0")]);
        assert_eq!(
            statements(Media::Smf, &file).unwrap(),
            vec![st(StatementField::SmfText, "CC0 1.0")]
        );
    }

    #[test]
    fn only_a_plain_smf_is_read() {
        let inner = smf_with_meta(&[]);
        // A RIFF (RMID) wrapper, which midly would unwrap, ignoring any INFO chunk.
        let mut riff = b"RIFF".to_vec();
        riff.extend_from_slice(&u32::try_from(inner.len() + 12).unwrap().to_le_bytes());
        riff.extend_from_slice(b"RMIDdata");
        riff.extend_from_slice(&u32::try_from(inner.len()).unwrap().to_le_bytes());
        riff.extend_from_slice(&inner);
        assert!(Smf::parse(&riff).is_ok());
        assert_eq!(
            statements(Media::Smf, &riff),
            Err(Unreadable::NotPlainSmf { offset: 0 })
        );
        // An unknown chunk after the track, which midly would skip: here it holds a notice.
        let mut vendor = inner.clone();
        let at = vendor.len();
        vendor.extend_from_slice(b"XCPY\0\0\0\x13All rights reserved");
        assert!(Smf::parse(&vendor).is_ok());
        assert_eq!(
            statements(Media::Smf, &vendor),
            Err(Unreadable::NotPlainSmf { offset: at })
        );
        assert_eq!(
            statements(Media::Smf, b""),
            Err(Unreadable::NotPlainSmf { offset: 0 })
        );
    }

    #[test]
    fn a_timecode_division_is_unreadable_before_midly_can_panic() {
        let mut file = smf_with_meta(&[]);
        file[12] = 0x80;
        file[13] = 0x28;
        assert_eq!(
            statements(Media::Smf, &file),
            Err(Unreadable::TimecodeDivision)
        );
    }

    fn hex(text: &str) -> Vec<u8> {
        text.split_ascii_whitespace()
            .map(|h| u8::from_str_radix(h, 16).unwrap())
            .collect()
    }

    #[test]
    fn smf_that_strict_rejects_is_unreadable() {
        // The knowledge base's three malformed files (midi-notation-ingest lane, wave 5).
        // Without `strict` midly parses each as `Ok`. Cargo unifies features, so only the
        // strict half can run in this workspace.
        //
        // The track-count mismatch is refused by this reader's own count check:
        // `Smf::parse` counted tracks, and this reader no longer calls it, because its
        // capacity estimate is floating point. midly's `strict` chunk and event readers
        // still refuse the other two, as `Malformed`.
        assert_eq!(
            statements(
                Media::Smf,
                &hex(
                    "4D 54 68 64 00 00 00 06 00 01 00 02 00 60 4D 54 72 6B 00 00 00 04 00 FF 2F 00"
                )
            ),
            Err(Unreadable::TrackCountMismatch {
                declared: 2,
                found: 1
            })
        );
        let cases = [
            (
                "4D 54 68 64 00 00 00 06 00 00 00 01 00 60 4D 54 72 6B 00 00 00 10 00 FF",
                "invalid chunk",
            ),
            (
                "4D 54 68 64 00 00 00 06 00 00 00 01 00 60 4D 54 72 6B 00 00 00 05 00 FF 20 01 FF",
                "malformed event",
            ),
        ];
        for (bytes, message) in cases {
            assert_eq!(
                statements(Media::Smf, &hex(bytes)),
                Err(Unreadable::Smf(message))
            );
        }
        // A byte after the last chunk is left to strict, which refuses it.
        let mut trailing = smf_with_meta(&[]);
        trailing.push(0);
        assert_eq!(
            statements(Media::Smf, &trailing),
            Err(Unreadable::Smf("invalid chunk"))
        );
    }
}
