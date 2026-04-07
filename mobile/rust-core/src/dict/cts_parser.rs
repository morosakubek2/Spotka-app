// mobile/rust-core/src/dict/cts_parser.rs
// Compact Tag Sequence (CTS) Parser & Validator.
// Architecture: Language-Agnostic, UTF-8 Support, Strict Syntax Validation.
// Note: No built-in dictionaries. Dictionary loading is handled by 'loader.rs'.
// Year: 2026 | Rust Edition: 2024

use serde::{Serialize, Deserialize};
use std::fmt;

/// Status types for a single tag in the sequence.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TagStatus {
    Positive,   // No prefix (Main goal). Exactly one required.
    Negative,   // Prefix '0' (Exclusion).
    Mediating,  // Prefix '1' (Additional activity).
    Limiting,   // Prefix '2' (Condition/Missing item). Must follow '1'.
}

/// Represents a single parsed tag.
/// 'index' is optional and will be populated later by 'compressor.rs' if a dictionary match is found.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CtsTag {
    pub status: TagStatus,
    pub word: String, // Original word as entered (preserves case/UTF-8)
    pub index: Option<u8>, // Dynamic index (0-255) assigned during compression phase
}

/// Error codes returned by the parser.
/// These are keys to be translated by the UI layer via JSON dictionaries.
#[derive(Debug, Clone, PartialEq)]
pub enum CtsError {
    EmptyInput,
    SpaceInTag,
    InvalidStatusChar,
    ExactlyOnePositiveRequired,
    TooManyTags,
    LimitingWithoutMediating,
}

impl fmt::Display for CtsError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // Display the KEY, not the message. UI will translate.
        match self {
            CtsError::EmptyInput => write!(f, "ERR_CTS_EMPTY_INPUT"),
            CtsError::SpaceInTag => write!(f, "ERR_CTS_SPACE_IN_TAG"),
            CtsError::InvalidStatusChar => write!(f, "ERR_CTS_INVALID_STATUS_CHAR"),
            CtsError::ExactlyOnePositiveRequired => write!(f, "ERR_CTS_EXACTLY_ONE_POSITIVE"),
            CtsError::TooManyTags => write!(f, "ERR_CTS_TOO_MANY_TAGS"),
            CtsError::LimitingWithoutMediating => write!(f, "ERR_CTS_LIMITING_WITHOUT_MEDIATING"),
        }
    }
}

/// Parses a Compact Tag Sequence string into a vector of CtsTag structs.
/// Input example: "kino0alkohol1granie2pilko"
/// Returns: Result<Vec<CtsTag>, CtsError>
/// 
/// This function performs SYNTAX validation only. It does not check semantics or dictionary existence.
pub fn parse_cts(input: &str) -> Result<Vec<CtsTag>, CtsError> {
    if input.is_empty() {
        return Err(CtsError::EmptyInput);
    }

    let mut tags = Vec::new();
    let mut current_word = String::with_capacity(20);
    let mut current_status = TagStatus::Positive;
    let mut positive_count = 0;

    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];

        // Check for status prefixes (0, 1, 2)
        if c == '0' || c == '1' || c == '2' {
            // If we have a pending word, push it first
            if !current_word.is_empty() {
                validate_and_push_tag(&mut tags, current_word.clone(), current_status, &mut positive_count)?;
                current_word.clear();
            }

            // Set new status
            current_status = match c {
                '0' => TagStatus::Negative,
                '1' => TagStatus::Mediating,
                '2' => TagStatus::Limiting,
                _ => return Err(CtsError::InvalidStatusChar), 
            };
        } else {
            // Build the word
            if c.is_whitespace() {
                return Err(CtsError::SpaceInTag);
            }
            // Allow any non-whitespace, non-prefix character (supports full UTF-8)
            current_word.push(c);
        }
        i += 1;
    }

    // Push the last tag if exists
    if !current_word.is_empty() {
        validate_and_push_tag(&mut tags, current_word, current_status, &mut positive_count)?;
    } else if i > 0 && (chars[i-1] == '0' || chars[i-1] == '1' || chars[i-1] == '2') {
        // Edge case: String ends with a status digit but no word follows (e.g., "kino0")
        // Depending on strictness, this could be an error. Here we treat "kino0" as valid if "0" was part of previous logic?
        // Actually, "kino0" means "kino" (pos) then start of negative. If no word follows, it's incomplete.
        // But our loop logic handles "kino0" -> pushes "kino", sets status Negative, loop ends. 
        // current_word is empty. So nothing pushed. This is correct: "kino0" is just "kino" with a dangling flag?
        // Let's enforce that a status digit MUST be followed by a word.
        return Err(CtsError::EmptyInput); // Or a specific "ERR_TRAILING_STATUS"
    }

    // Final Validations
    if positive_count != 1 {
        return Err(CtsError::ExactlyOnePositiveRequired);
    }

    if tags.len() > 10 {
        return Err(CtsError::TooManyTags);
    }

    Ok(tags)
}

/// Helper function to validate individual tag rules before pushing.
fn validate_and_push_tag(
    tags: &mut Vec<CtsTag>,
    word: String,
    status: TagStatus,
    positive_count: &mut u32,
) -> Result<(), CtsError> {
    if status == TagStatus::Limiting {
        // Rule: '2' must immediately follow '1'
        if tags.is_empty() || tags.last().unwrap().status != TagStatus::Mediating {
            return Err(CtsError::LimitingWithoutMediating);
        }
    }

    if status == TagStatus::Positive {
        *positive_count += 1;
    }

    tags.push(CtsTag {
        status,
        word,
        index: None, // Will be assigned by Compressor based on external dictionary
    });

    Ok(())
}

/// Serializes a list of tags back to a CTS string.
/// Used when saving to DB or sending over P2P (before compression).
pub fn serialize_cts(tags: &[CtsTag]) -> String {
    let mut output = String::new();
    for tag in tags {
        match tag.status {
            TagStatus::Positive => {} // No prefix
            TagStatus::Negative => output.push('0'),
            TagStatus::Mediating => output.push('1'),
            TagStatus::Limiting => output.push('2'),
        }
        output.push_str(&tag.word);
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_simple() {
        let result = parse_cts("kino").unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].word, "kino");
        assert_eq!(result[0].status, TagStatus::Positive);
        assert!(result[0].index.is_none()); // Index not set by parser
    }

    #[test]
    fn test_valid_complex_utf8() {
        let input = "spacer1gra2pilka0deszcz";
        let result = parse_cts(input).unwrap();
        assert_eq!(result.len(), 4);
        assert_eq!(result[0].word, "spacer");
        assert_eq!(result[2].word, "pilka");
    }

    #[test]
    fn test_error_no_positive() {
        let result = parse_cts("0alkohol");
        assert_eq!(result.unwrap_err(), CtsError::ExactlyOnePositiveRequired);
    }

    #[test]
    fn test_error_two_positives() {
        let result = parse_cts("kinoplansza");
        assert_eq!(result.unwrap_err(), CtsError::ExactlyOnePositiveRequired);
    }

    #[test]
    fn test_error_limiting_orphan() {
        let result = parse_cts("kino2pilka");
        assert_eq!(result.unwrap_err(), CtsError::LimitingWithoutMediating);
    }
    
    #[test]
    fn test_error_space() {
        let result = parse_cts("kino film");
        assert_eq!(result.unwrap_err(), CtsError::SpaceInTag);
    }
}
