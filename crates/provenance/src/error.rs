//! Why a receipt could not be loaded. Loading checks structure only; a structurally sound
//! receipt that proves too little is refused later by the predicate, with a named reason.

use alloc::string::String;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReceiptError {
    /// The input is not in the accepted JSON subset. `offset` is a byte offset.
    Json {
        offset: usize,
        problem: JsonProblem,
    },
    /// A required key is missing.
    MissingKey {
        object: &'static str,
        key: &'static str,
    },
    /// A key the schema does not define. Unknown keys are refused, never ignored.
    UnknownKey {
        object: &'static str,
        key: String,
    },
    /// A value has the wrong JSON type, or is out of range, or is not in its vocabulary.
    BadValue {
        object: &'static str,
        key: &'static str,
    },
    UnsupportedSchema(u16),
    InvalidDate,
    FilesNotSorted,
    EvidenceNotSorted,
    RestrictionsNotSorted,
    /// A file or evidence name outside the plain-name alphabet.
    BadName,
    /// The canonical bytes are malformed. `offset` is a byte offset.
    Canonical {
        offset: usize,
        problem: CanonicalProblem,
    },
}

/// What is wrong with a JSON input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JsonProblem {
    TooLarge,
    NotUtf8,
    ByteOrderMark,
    UnexpectedEnd,
    UnexpectedByte,
    TrailingContent,
    DuplicateKey,
    NegativeNumber,
    NotAnInteger,
    LeadingZero,
    NumberTooLarge,
    BadEscape,
    LoneSurrogate,
    ControlCharacter,
    TooDeep,
}

/// What is wrong with canonical bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CanonicalProblem {
    BadMagic,
    UnsupportedVersion,
    UnexpectedEnd,
    BadTag,
    BadFlag,
    NotUtf8,
    TrailingBytes,
}
