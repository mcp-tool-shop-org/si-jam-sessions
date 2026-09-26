//! Licence texts: the normalisation used to compare them, the closed list of texts that
//! admit, and the naming of refusals.
//!
//! **Admission is exact.** A licence text admits only if its normalised form is one of the
//! entries in [`admitted_class`]'s list. Nothing is admitted by resemblance.
//!
//! **Refusal naming is best effort.** A text that is not on the list is refused; the
//! phrases in [`restriction_in`] only choose which reason the refusal names. A text that
//! matches none of them is refused as [`LicenceRefusal::Unknown`]. So an incomplete phrase
//! list can mislabel a refusal but can never admit anything.
//!
//! **Restriction phrases are read everywhere.** Beyond naming a refused page licence,
//! [`restriction_in`] is applied to every other licence text the predicate reads: the
//! evidence quotes that hold the page licence and the terms, and each licence statement a
//! file makes about itself. A phrase found in any of them refuses the score by its class,
//! even when the page licence is on the admitted list. AI wording is the first class
//! looked for, and matches only as whole words. Every other class matches from the start
//! of a word, so a stem finds its inflections: `noncommercial` refuses `noncommercially`.
//! The rule is on [`REFUSAL_PHRASES`].
//!
//! **A text must affirm, not merely mention.** Where a text may hold the licence alongside
//! other text (a copyright markup, an evidence quote), it counts only if [`negates`] finds
//! no negating, limiting, lapsing or hedging word and no question mark.

use alloc::string::String;
use alloc::vec::Vec;

use crate::receipt::Restriction;

/// Why a licence refuses a score. Each refusal class of the licence predicate has one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LicenceRefusal {
    /// The licence is missing, unreadable, or not one this law version admits.
    Unknown,
    AllRightsReserved,
    NoRedistribution,
    ShareAlike,
    NonCommercial,
    NoDerivatives,
    /// The source's terms forbid or limit processing by, or training of, AI models.
    AiRestricted,
}

impl From<Restriction> for LicenceRefusal {
    fn from(r: Restriction) -> Self {
        match r {
            Restriction::AllRightsReserved => LicenceRefusal::AllRightsReserved,
            Restriction::NoRedistribution => LicenceRefusal::NoRedistribution,
            Restriction::ShareAlike => LicenceRefusal::ShareAlike,
            Restriction::NonCommercial => LicenceRefusal::NonCommercial,
            Restriction::NoDerivatives => LicenceRefusal::NoDerivatives,
            Restriction::AiRestricted => LicenceRefusal::AiRestricted,
        }
    }
}

/// The admitted licence classes. Each is its own tier; they never mix.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdmittedClass {
    PublicDomain,
    CcBy40,
}

/// The normalised texts that admit, and nothing else.
const ADMITTED: &[(&str, AdmittedClass)] = &[
    ("public domain", AdmittedClass::PublicDomain),
    ("creative commons attribution 4.0", AdmittedClass::CcBy40),
    (
        "creative commons attribution 4.0 international",
        AdmittedClass::CcBy40,
    ),
    ("cc by 4.0", AdmittedClass::CcBy40),
    ("cc-by-4.0", AdmittedClass::CcBy40),
];

/// How a refusal class's phrases are looked for in a normalised text. In both modes a
/// phrase must start a word: at the start of the text, or after a character that is not an
/// ASCII letter or digit.
#[derive(Clone, Copy)]
enum Matching {
    /// The phrase must also end a word (see [`contains_phrase`]). AI wording needs this:
    /// its short forms sit inside common words (`ai` in `domain` and `maintainer`) and start
    /// others (`ai` in `aim`, `air` and `aisle`).
    WholeWords,
    /// The phrase is a stem and may end inside a word (see [`contains_stem`]), so it finds
    /// the stem's inflections: `noncommercial` finds `noncommercially`, and `noderiv` finds
    /// `noderivs`, `noderivative` and `noderivatives`. A longer word can only add
    /// refusals, and over-refusal is the safe direction.
    WordStart,
}

/// Phrases that name a refusal, in precedence order: the first class with a phrase in the
/// text is the reason given. Phrases are looked for in normalised text, as the class's
/// [`Matching`] says. The rule, closed:
/// - AI wording matches as whole words only.
/// - Every other class matches from the start of a word, and its phrases are stems.
///
/// A phrase that starts inside a word matches in neither mode: `piano derivatives` holds
/// `no deriv` only inside `piano`.
const REFUSAL_PHRASES: &[(LicenceRefusal, Matching, &[&str])] = &[
    (
        LicenceRefusal::AiRestricted,
        Matching::WholeWords,
        &[
            "ai",
            "a.i.",
            "genai",
            "artificial intelligence",
            "machine learning",
            "machine-learning",
            "deep learning",
            "neural network",
            "neural networks",
            "text and data mining",
            "text & data mining",
            "text-and-data mining",
            "data mining",
            "tdm",
            "llm",
            "llms",
            "language model",
            "language models",
            "large language model",
            "large language models",
            "model training",
            "training data",
            "to train",
        ],
    ),
    (
        LicenceRefusal::AllRightsReserved,
        Matching::WordStart,
        &["all rights reserved"],
    ),
    (
        LicenceRefusal::NoRedistribution,
        Matching::WordStart,
        &[
            "no redistribut",
            "not for redistribut",
            "may not be redistribut",
            "do not redistribut",
            "personal use only",
        ],
    ),
    (
        LicenceRefusal::NonCommercial,
        Matching::WordStart,
        &[
            "noncommercial",
            "non-commercial",
            "non commercial",
            "not for commercial",
            "no commercial",
            "cc by-nc",
            "cc-by-nc",
        ],
    ),
    (
        LicenceRefusal::NoDerivatives,
        Matching::WordStart,
        &["noderiv", "no-deriv", "no deriv", "cc by-nd", "cc-by-nd"],
    ),
    (
        LicenceRefusal::ShareAlike,
        Matching::WordStart,
        &[
            "sharealike",
            "share-alike",
            "share alike",
            "cc by-sa",
            "cc-by-sa",
        ],
    ),
];

/// Words that negate, limit, lapse or hedge what a text says, matched as whole words.
/// The list is broad on purpose: a text that trips it is refused, never admitted.
const NEGATING_WORDS: &[&str] = &[
    // Negation.
    "not",
    "no",
    "never",
    "non",
    "nor",
    "neither",
    "none",
    "nothing",
    "cannot",
    "without",
    // Exception and limitation.
    "except",
    "excepting",
    "excluding",
    "unless",
    "until",
    "only",
    "but",
    // Lapse.
    "formerly",
    "previously",
    "expired",
    "revoked",
    "withdrawn",
    // Hedging.
    "maybe",
    "perhaps",
    "possibly",
    "probably",
    "presumably",
    "disputed",
    "unconfirmed",
];

/// The minimal normalisation used for every licence comparison:
///
/// 1. leading and trailing ASCII whitespace is removed;
/// 2. each inner run of ASCII whitespace (space, tab, LF, FF, CR) becomes one space;
/// 3. ASCII letters are lower-cased.
///
/// Nothing else changes: no Unicode case folding, no punctuation or hyphen folding, and a
/// non-ASCII space such as U+00A0 stays what it is. It fails closed: `None` for text that
/// is empty after trimming or that contains any other control character, and every
/// caller refuses on `None`.
pub fn normalise(text: &str) -> Option<String> {
    let mut out = String::with_capacity(text.len());
    let mut pending_space = false;
    for c in text.chars() {
        if c.is_ascii_whitespace() {
            pending_space = !out.is_empty();
            continue;
        }
        if c.is_control() {
            return None;
        }
        if pending_space {
            out.push(' ');
            pending_space = false;
        }
        out.push(c.to_ascii_lowercase());
    }
    if out.is_empty() { None } else { Some(out) }
}

/// The admitted class for a normalised licence text, if the text is on the list.
pub fn admitted_class(normalised: &str) -> Option<AdmittedClass> {
    ADMITTED
        .iter()
        .find(|(text, _)| *text == normalised)
        .map(|&(_, class)| class)
}

/// The first refusal class, in precedence order, with a phrase in the normalised text,
/// each class matched as [`REFUSAL_PHRASES`] documents.
pub fn restriction_in(normalised: &str) -> Option<LicenceRefusal> {
    REFUSAL_PHRASES
        .iter()
        .find(|&&(_, matching, phrases)| {
            phrases.iter().any(|p| match matching {
                Matching::WholeWords => contains_phrase(normalised, p),
                Matching::WordStart => contains_stem(normalised, p),
            })
        })
        .map(|&(class, _, _)| class)
}

/// True if a normalised text negates, limits, lapses, hedges or questions what it says.
///
/// The rule, closed and documented: the text holds a question mark; or one of
/// [`NEGATING_WORDS`] as a whole word; or a contraction ending in *n't*, seen as a lone
/// `t` after a word ending in `n` (`isn't`, `can't`, `don't`, with either apostrophe).
/// Words are the runs of alphanumeric characters, so `non-public` holds the word `non`.
///
/// It applies where a text may hold the licence alongside other text: a copyright markup
/// must affirm the page licence, and an evidence quote that holds a licence or terms text
/// must not deny it. Over-refusal is the intended failure mode.
pub fn negates(normalised: &str) -> bool {
    if normalised.contains('?') {
        return true;
    }
    let words: Vec<&str> = normalised
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect();
    words.iter().enumerate().any(|(i, w)| {
        NEGATING_WORDS.contains(w) || (*w == "t" && i > 0 && words[i - 1].ends_with('n'))
    })
}

/// Lower-cases ASCII and collapses runs of ASCII whitespace to one space, trimming the
/// ends. Unlike [`normalise`] it never fails: it is for looking for phrases in text that
/// may hold control characters, never for comparing licences.
pub fn fold(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for word in text.split_ascii_whitespace() {
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(&word.to_ascii_lowercase());
    }
    out
}

/// The reason a normalised licence text that is not admitted is refused.
pub fn refusal_for(normalised: &str) -> LicenceRefusal {
    restriction_in(normalised).unwrap_or(LicenceRefusal::Unknown)
}

/// True if `phrase` occurs in `text` bounded on both sides by the text's ends or by a
/// character that is not an ASCII letter or digit. Both are normalised texts.
pub fn contains_phrase(text: &str, phrase: &str) -> bool {
    let bytes = text.as_bytes();
    let mut from = 0;
    while let Some(i) = text[from..].find(phrase) {
        let start = from + i;
        let end = start + phrase.len();
        let open = start == 0 || !bytes[start - 1].is_ascii_alphanumeric();
        let close = end == bytes.len() || !bytes[end].is_ascii_alphanumeric();
        if open && close {
            return true;
        }
        // Advance by one character, staying on a char boundary.
        from = start + text[start..].chars().next().map_or(1, char::len_utf8);
    }
    false
}

/// True if `stem` occurs in `text` at the start of a word: at the text's start, or after a
/// character that is not an ASCII letter or digit. Unlike [`contains_phrase`], it may end
/// inside a word, so `noncommercial` is found in `noncommercially`. Both are normalised
/// texts.
pub fn contains_stem(text: &str, stem: &str) -> bool {
    let bytes = text.as_bytes();
    let mut from = 0;
    while let Some(i) = text[from..].find(stem) {
        let start = from + i;
        if start == 0 || !bytes[start - 1].is_ascii_alphanumeric() {
            return true;
        }
        // Advance by one character, staying on a char boundary.
        from = start + text[start..].chars().next().map_or(1, char::len_utf8);
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalisation_is_minimal_and_fails_closed() {
        assert_eq!(normalise("Public Domain").as_deref(), Some("public domain"));
        assert_eq!(
            normalise("  Public \t\r\n Domain  ").as_deref(),
            Some("public domain")
        );
        // Nothing beyond ASCII whitespace and ASCII case changes.
        assert_eq!(normalise("PUBLIC-DOMAIN").as_deref(), Some("public-domain"));
        assert_eq!(
            normalise("Public\u{a0}Domain").as_deref(),
            Some("public\u{a0}domain")
        );
        // ASCII `T` is lower-cased; non-ASCII `É` is left alone.
        assert_eq!(normalise("ÉTÉ").as_deref(), Some("ÉtÉ"));
        // Fail closed.
        assert_eq!(normalise(""), None);
        assert_eq!(normalise(" \t\n "), None);
        assert_eq!(normalise("Public\0Domain"), None);
        assert_eq!(normalise("Public\u{b}Domain"), None);
        assert_eq!(normalise("Public\u{7f}Domain"), None);
        assert_eq!(normalise("Public\u{85}Domain"), None);
    }

    #[test]
    fn only_listed_texts_admit() {
        assert_eq!(
            admitted_class("public domain"),
            Some(AdmittedClass::PublicDomain)
        );
        assert_eq!(
            admitted_class("creative commons attribution 4.0"),
            Some(AdmittedClass::CcBy40)
        );
        assert_eq!(admitted_class("cc-by-4.0"), Some(AdmittedClass::CcBy40));
        for not in [
            "public-domain",
            "public domain dedication",
            "cc0",
            "cc0-1.0",
            "creative commons attribution 3.0",
            "creative commons attribution-sharealike 4.0",
            "public domain.",
            "not public domain",
        ] {
            assert_eq!(admitted_class(not), None, "{not}");
        }
    }

    #[test]
    fn refusals_are_named_by_precedence() {
        use LicenceRefusal::*;
        assert_eq!(
            refusal_for("creative commons attribution-sharealike 4.0"),
            ShareAlike
        );
        assert_eq!(refusal_for("cc-by-sa-3.0"), ShareAlike);
        assert_eq!(
            refusal_for("creative commons attribution-noncommercial-sharealike 4.0"),
            NonCommercial
        );
        assert_eq!(refusal_for("cc-by-nc-nd-4.0"), NonCommercial);
        assert_eq!(
            refusal_for("creative commons attribution-noderivatives 4.0"),
            NoDerivatives
        );
        assert_eq!(refusal_for("cc-by-nd-4.0"), NoDerivatives);
        assert_eq!(
            refusal_for("copyright 2019 a publisher. all rights reserved."),
            AllRightsReserved
        );
        assert_eq!(refusal_for("for personal use only"), NoRedistribution);
        assert_eq!(refusal_for("creative commons attribution 3.0"), Unknown);
        assert_eq!(refusal_for("cc0"), Unknown);
        assert_eq!(refusal_for("something else"), Unknown);
    }

    #[test]
    fn ai_wording_is_named_first() {
        use LicenceRefusal::*;
        for text in [
            "no ai training",
            "not for a.i. use",
            "no genai",
            "not for artificial intelligence",
            "may not be used for machine learning",
            "no machine-learning",
            "deep learning prohibited",
            "not to be fed to a neural network",
            "text and data mining reserved",
            "text & data mining reserved",
            "tdm reserved",
            "no data mining",
            "not for llms",
            "no large language model may read this",
            "not for model training",
            "not to be used as training data",
            "not to train anything",
            // AI wording outranks every other class.
            "cc-by-nc-sa-4.0; no ai training",
            "all rights reserved, including tdm",
        ] {
            assert_eq!(refusal_for(text), AiRestricted, "{text}");
        }
    }

    #[test]
    fn a_restriction_phrase_never_matches_from_inside_a_word() {
        // `ai` inside a word, and restriction words inside longer words, match nothing. Nor
        // does a stem that starts inside a word: `piano derivatives` holds `no deriv` only
        // inside `piano`, and `casino commercials` holds `no commercial` inside `casino`.
        for text in [
            "maintained by chris sawer",
            "said the typesetter",
            "raised in st. louis",
            "attribution",
            "trained ear",
            "commercially printed",
            "piano derivatives",
            "casino commercials",
        ] {
            assert_eq!(restriction_in(text), None, "{text}");
        }
        // The named forms still match at hyphens and punctuation.
        assert_eq!(
            restriction_in("attribution-noderivs 3.0"),
            Some(LicenceRefusal::NoDerivatives)
        );
        assert_eq!(
            restriction_in("cc-by-sa-4.0"),
            Some(LicenceRefusal::ShareAlike)
        );
    }

    /// Every class but AI wording matches from the start of a word, so a stem finds its
    /// inflections. The first two texts are the markups found admitted as public domain
    /// at `fead502`, normalised.
    #[test]
    fn restrictions_other_than_ai_match_from_the_start_of_a_word() {
        use LicenceRefusal::*;
        let cases = [
            ("public domain. free to use noncommercially.", NonCommercial),
            ("public domain, attribution-noderivative", NoDerivatives),
            ("noncommercially", NonCommercial),
            ("non-commercially", NonCommercial),
            ("non commercially", NonCommercial),
            ("non-commercial use", NonCommercial),
            // "NonCommercial", "NoDerivatives", "NoDerivative" and "ShareAlike", normalised.
            ("noncommercial", NonCommercial),
            ("noderivatives", NoDerivatives),
            ("noderivative", NoDerivatives),
            ("sharealike", ShareAlike),
            ("noderivs", NoDerivatives),
            ("no-derivs", NoDerivatives),
            ("no-derivative", NoDerivatives),
            ("no derivative", NoDerivatives),
            ("no derivs", NoDerivatives),
            ("share-alike", ShareAlike),
            ("share alike", ShareAlike),
            ("all rights reserved", AllRightsReserved),
            ("no redistribution", NoRedistribution),
            ("not for redistributing", NoRedistribution),
        ];
        let wrong: Vec<String> = cases
            .iter()
            .filter(|&&(text, class)| restriction_in(text) != Some(class))
            .map(|&(text, class)| {
                alloc::format!("{text:?} gives {:?}, not {class:?}", restriction_in(text))
            })
            .collect();
        assert!(
            wrong.is_empty(),
            "{} of {} wrong: {wrong:#?}",
            wrong.len(),
            cases.len()
        );
    }

    /// AI wording keeps whole-word matching. Its short forms sit inside common words (`ai`
    /// in `domain`, `maintainer` and `said`) and start others (`aim`, `air`, `aisle`), and
    /// none of those is AI wording.
    #[test]
    fn domain_does_not_trip_the_ai_class() {
        for text in [
            "domain",
            "public domain",
            "placed in the public domain by the typesetter",
            "maintainer: chris sawer",
            "said",
            "aim",
            "air",
            "aid",
            "aisle",
            "tdma",
            "trained",
            "training",
        ] {
            assert_eq!(restriction_in(text), None, "{text}");
        }
        for text in ["ai", "a.i.", "no ai training", "public domain; not for ai"] {
            assert_eq!(
                restriction_in(text),
                Some(LicenceRefusal::AiRestricted),
                "{text}"
            );
        }
    }

    #[test]
    fn negation_is_a_closed_word_rule() {
        for text in [
            "not in the public domain",
            "no longer public domain",
            "never placed in the public domain",
            "non-public domain",
            "public domain, except the fingering",
            "public domain in the us only",
            "public domain but not the edition",
            "formerly public domain",
            "possibly public domain",
            "public domain?",
            "it isn't public domain",
            "it isn\u{2019}t public domain",
            "we can't say",
            "it ain't so",
        ] {
            assert!(negates(text), "{text}");
        }
        for text in [
            // The Entertainer's own markup text, normalised.
            "mutopia project typeset using lilypond by placed in the public domain by the \
             typesetter free to distribute, modify, and perform",
            "copyright: public domain",
            "the contributor of this music has dedicated their contribution into the public \
             domain.",
            // Words that only contain a negating word are not negating words.
            "nothingness",
            "notation is public domain",
            "know",
            "button",
            "a t by itself",
        ] {
            assert!(!negates(text), "{text}");
        }
    }

    #[test]
    fn fold_never_fails() {
        assert_eq!(
            fold("  Not For\tAI\u{0}  Training "),
            "not for ai\u{0} training"
        );
        assert_eq!(fold(""), "");
    }

    #[test]
    fn phrases_match_on_word_boundaries() {
        let t = "placed in the public domain by the typesetter";
        assert!(contains_phrase(t, "public domain"));
        assert!(contains_phrase("public domain", "public domain"));
        assert!(contains_phrase("(public domain)", "public domain"));
        assert!(!contains_phrase("republic domains", "public domain"));
        assert!(!contains_phrase("publicdomain", "public domain"));
        assert!(contains_phrase("é public domain é", "public domain"));
    }

    #[test]
    fn stems_match_from_the_start_of_a_word() {
        assert!(contains_stem("noncommercially", "noncommercial"));
        assert!(contains_stem(
            "free to use noncommercially.",
            "noncommercial"
        ));
        assert!(contains_stem("attribution-noderivative", "noderiv"));
        assert!(contains_stem("(noderivs)", "noderiv"));
        assert!(contains_stem("noderiv", "noderiv"));
        // Only from the start of a word.
        assert!(!contains_stem("anoncommercial", "noncommercial"));
        assert!(!contains_stem("piano derivatives", "no deriv"));
        // A later occurrence at the start of a word is still found.
        assert!(contains_stem(
            "piano derivatives, no derivatives",
            "no deriv"
        ));
        assert!(!contains_stem("", "noderiv"));
    }

    #[test]
    fn every_restriction_maps_to_its_refusal() {
        for &r in Restriction::ALL {
            let refusal = LicenceRefusal::from(r);
            assert_ne!(refusal, LicenceRefusal::Unknown, "{r:?}");
        }
    }
}
