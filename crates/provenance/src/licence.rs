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
//! looked for. Phrases are matched on a text's words, so separators do not matter
//! (`CC BY NC` reads as `cc-by-nc`). A phrase must be whole words, except a few stems
//! that match from the start of a word and so find their inflections: `noncommercial`
//! refuses `noncommercially`, and `neural net` refuses `neural nets`. A phrase is a stem
//! only where no plain word runs on from it, so no stem refuses plain text. The rule is on
//! [`REFUSAL_PHRASES`].
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

/// A refusal class and the phrases that name it, written as words (see [`as_words`]).
struct Phrases {
    class: LicenceRefusal,
    /// Phrases that must be whole words (see [`contains_phrase`]).
    whole: &'static [&'static str],
    /// Stems: phrases that match from the start of a word and may end inside one (see
    /// [`contains_stem`]).
    stems: &'static [&'static str],
}

/// The phrases that name a refusal, in precedence order: the first class with a phrase in
/// the text is the reason given. The rule, closed:
/// - **Words.** A text is read as its words (see [`as_words`]): every run of characters
///   other than ASCII letters and digits is one separator. So `cc-by-nc`, `cc by nc` and
///   `CC_BY_NC` read alike, and every phrase here is written in that form (`a.i.` is
///   `a i`).
/// - **Whole words by default.** A phrase must match whole words. AI wording's short
///   tokens need this most: they sit inside common words (`ai` in `domain`) or start them
///   (`aim`, `air`, `tdma`).
/// - **Stems where no plain word runs on from them.** A stem matches from the start of a
///   word and may end inside one, so it finds its inflections: `noncommercial` finds
///   `noncommercially`, and `neural net` finds `neural nets` and `neural networks`. A
///   phrase is a stem only if every word that runs on from it is still the restriction it
///   names. Otherwise its forms are listed as whole words: `model train` would refuse
///   `model trains`, and `cc by sa` would refuse `cc by sarah`.
/// - **Never from inside a word.** A phrase that starts inside a word matches nothing:
///   `piano derivatives` holds `no deriv` only inside `piano`.
const REFUSAL_PHRASES: &[Phrases] = &[
    Phrases {
        class: LicenceRefusal::AiRestricted,
        whole: &[
            "ai",
            "a i",
            "genai",
            "tdm",
            "llm",
            "llms",
            "deep learning",
            "data mining",
            "data mine",
            "data mined",
            "data miner",
            "data miners",
            "language model",
            "language models",
            "language modelling",
            "language modeling",
            "model training",
            "training data",
            "training dataset",
            "training datasets",
            "training model",
            "training models",
            "to train",
        ],
        stems: &["artificial intelligen", "machine learn", "neural net"],
    },
    Phrases {
        class: LicenceRefusal::AllRightsReserved,
        whole: &["all rights reserved"],
        stems: &[],
    },
    Phrases {
        class: LicenceRefusal::NoRedistribution,
        whole: &["personal use only"],
        stems: &[
            "no redistribut",
            "not for redistribut",
            "may not be redistribut",
            "do not redistribut",
        ],
    },
    Phrases {
        class: LicenceRefusal::NonCommercial,
        whole: &["not for commercial", "no commercial", "cc by nc"],
        stems: &["noncommercial", "non commercial"],
    },
    Phrases {
        class: LicenceRefusal::NoDerivatives,
        whole: &["no derivative", "no derivatives", "no derivs", "cc by nd"],
        stems: &["noderiv"],
    },
    Phrases {
        class: LicenceRefusal::ShareAlike,
        whole: &["sharealike", "share alike", "cc by sa"],
        stems: &[],
    },
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

/// The first refusal class, in precedence order, with a phrase in the text's words, each
/// phrase matched as [`REFUSAL_PHRASES`] documents.
pub fn restriction_in(text: &str) -> Option<LicenceRefusal> {
    let words = as_words(text);
    REFUSAL_PHRASES
        .iter()
        .find(|p| {
            p.whole.iter().any(|w| contains_phrase(&words, w))
                || p.stems.iter().any(|s| contains_stem(&words, s))
        })
        .map(|p| p.class)
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

/// A text as its words: each run of characters that are not ASCII letters or digits
/// becomes one space, the ends are trimmed, and ASCII letters are lower-cased. Restriction
/// phrases are matched on this form, so a separator is a separator whatever it is: a
/// hyphen, an underscore, a slash, a full stop, a no-break space or a Unicode hyphen.
pub fn as_words(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut gap = false;
    for c in text.chars() {
        if c.is_ascii_alphanumeric() {
            if gap && !out.is_empty() {
                out.push(' ');
            }
            gap = false;
            out.push(c.to_ascii_lowercase());
        } else {
            gap = true;
        }
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

    /// A restriction's inflections are refused by its class: through a stem where no plain
    /// word runs on from it, and through its forms listed as whole words elsewhere. The
    /// first two texts are the markups found admitted as public domain at `fead502`,
    /// normalised.
    #[test]
    fn inflected_restriction_wording_is_refused() {
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

    /// AI wording's short tokens keep whole-word matching. They sit inside common words
    /// (`ai` in `domain`, `maintainer` and `said`) and start others (`aim`, `air`, `aisle`,
    /// `tdma`), and none of those is AI wording.
    #[test]
    fn domain_does_not_trip_the_ai_class() {
        for text in [
            "domain",
            "public domain",
            "placed in the public domain by the typesetter",
            "maintainer: chris sawer",
            "maintainer",
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

    /// Separators between words do not matter. The second external review found
    /// "Placed in the public domain (CC BY ND)" admitted at `390336b`, where the CC forms
    /// were listed with hyphens only.
    #[test]
    fn separators_between_words_do_not_matter() {
        use LicenceRefusal::*;
        let cases = [
            ("placed in the public domain (cc by nd)", NoDerivatives),
            ("placed in the public domain (cc by nc)", NonCommercial),
            ("placed in the public domain (cc by sa)", ShareAlike),
            // The combined forms are named by precedence, as `cc-by-nc-nd` is.
            ("cc by nc nd 4.0", NonCommercial),
            ("cc by nc sa 4.0", NonCommercial),
            ("cc-by nc", NonCommercial),
            ("cc_by_nd", NoDerivatives),
            ("cc/by/sa", ShareAlike),
            ("non\u{2010}commercial", NonCommercial),
            ("share\u{a0}alike", ShareAlike),
            ("all-rights-reserved", AllRightsReserved),
            ("no-redistribution", NoRedistribution),
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

    /// AI wording is refused in its listed forms, and through its three stems
    /// (`artificial intelligen`, `machine learn`, `neural net`) in their inflections. The
    /// second external review found "no neural nets" and "not for training models" passing
    /// at `390336b`, where AI wording was a shorter list of whole words.
    #[test]
    fn ai_wording_in_its_listed_forms_is_refused() {
        let cases = [
            "no neural nets",
            "not for training models",
            "neural networks",
            "neural-net",
            "machine learned",
            "deep-learning",
            "language modelling",
            "large-language-models",
            "text-and-data-mining",
            "data-mined",
            "artificial-intelligence",
            "a. i.",
        ];
        let wrong: Vec<String> = cases
            .iter()
            .filter(|text| restriction_in(text) != Some(LicenceRefusal::AiRestricted))
            .map(|text| alloc::format!("{text:?} gives {:?}", restriction_in(text)))
            .collect();
        assert!(
            wrong.is_empty(),
            "{} of {} wrong: {wrong:#?}",
            wrong.len(),
            cases.len()
        );
    }

    /// No stem refuses plain text. Each text runs a restriction phrase on into an ordinary
    /// word, and none of them is a restriction. At `f4038cc`, stems such as `model train`,
    /// `deep learn` and `cc by sa` refused them.
    #[test]
    fn plain_text_that_runs_a_phrase_on_is_not_refused() {
        let plain = [
            "public domain",
            "domain",
            "dedicated to model trains and their builders",
            "written for trainees",
            "written to trainees",
            "for deep learners of the piano",
            "a data minefield",
            "a notation language modelled on lilypond",
            "piano training modelled on czerny",
            "the training database",
            "prepared for the cc by sarah",
            "no commercially published edition exists",
            "no derivation from the autograph",
            "a noncommittal reply",
        ];
        let refused: Vec<String> = plain
            .iter()
            .filter(|text| restriction_in(text).is_some())
            .map(|text| alloc::format!("{text:?} gives {:?}", restriction_in(text)))
            .collect();
        assert!(
            refused.is_empty(),
            "{} of {} refused: {refused:#?}",
            refused.len(),
            plain.len()
        );
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

    /// A phrase with a separator other than one space could never match a text's words,
    /// so every phrase must already be written as words.
    #[test]
    fn every_phrase_is_written_as_words() {
        for p in REFUSAL_PHRASES {
            for phrase in p.whole.iter().chain(p.stems) {
                assert!(!phrase.is_empty(), "{:?}", p.class);
                assert_eq!(as_words(phrase), *phrase, "{:?}", p.class);
            }
        }
    }

    #[test]
    fn words_fold_every_separator() {
        assert_eq!(as_words("CC-BY_NC/ND 4.0"), "cc by nc nd 4 0");
        assert_eq!(as_words("  (CC BY ND)  "), "cc by nd");
        assert_eq!(as_words("a.i."), "a i");
        assert_eq!(as_words("non\u{2010}commercial"), "non commercial");
        assert_eq!(as_words("share\u{a0}alike"), "share alike");
        assert_eq!(as_words("public domain"), "public domain");
        assert_eq!(as_words("\u{e9}t\u{e9}"), "t");
        assert_eq!(as_words("--"), "");
        assert_eq!(as_words(""), "");
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
