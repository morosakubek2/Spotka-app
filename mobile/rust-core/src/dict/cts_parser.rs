// mobile/rust-core/src/dict/cts_parser.rs
// Compact Tag Sequence (CTS) Parser & Validator.
// Architecture: Language-Agnostic, UTF-8 Support, Strict Syntax Validation.
// Update: Allows MULTIPLE positive tags (Hybrid meetups), max total 10 tags.
// Year: 2026 | Rust Edition: 2024

use serde::{Serialize, Deserialize};
use std::fmt;

/// Status types for a single tag in the sequence.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TagStatus {
    Positive,   // No prefix. Can be multiple now (e.g., "kino" + "kaw").
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
    AtLeastOnePositiveRequired, // Changed from ExactlyOne...
    TooManyTags,
    LimitingWithoutMediating,
    TrailingStatusChar, // New error for cases like "kino0"
}

impl fmt::Display for CtsError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            CtsError::EmptyInput => write!(f, "ERR_CTS_EMPTY_INPUT"),
            CtsError::SpaceInTag => write!(f, "ERR_CTS_SPACE_IN_TAG"),
            CtsError::InvalidStatusChar => write!(f, "ERR_CTS_INVALID_STATUS_CHAR"),
            CtsError::AtLeastOnePositiveRequired => write!(f, "ERR_CTS_AT_LEAST_ONE_POSITIVE"),
            CtsError::TooManyTags => write!(f, "ERR_CTS_TOO_MANY_TAGS"),
            CtsError::LimitingWithoutMediating => write!(f, "ERR_CTS_LIMITING_WITHOUT_MEDIATING"),
            CtsError::TrailingStatusChar => write!(f, "ERR_CTS_TRAILING_STATUS"),
        }
    }
}

/// Parses a Compact Tag Sequence string into a vector of CtsTag structs.
/// Input example: "kino0alkohol1granie2pilko" OR "kinokawa" (hybrid)
/// Returns: Result<Vec<CtsTag>, CtsError>
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
            current_word.push(c);
        }
        i += 1;
    }

    // Handle trailing status character (e.g., "kino0" ends with '0' but no word after)
    if !current_word.is_empty() {
        validate_and_push_tag(&mut tags, current_word, current_status, &mut positive_count)?;
    } else if i > 0 {
        let last_char = chars[i-1];
        if last_char == '0' || last_char == '1' || last_char == '2' {
            return Err(CtsError::TrailingStatusChar);
        }
    }

    // Final Validations
    
    // RULE CHANGE: Allow multiple positive tags, but require AT LEAST ONE.
    if positive_count < 1 {
        return Err(CtsError::AtLeastOnePositiveRequired);
    }

    // Total limit still applies (max 10 tags of ANY type)
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
        index: None, 
    });

    Ok(())
}

/// Serializes a list of tags back to a CTS string.
pub fn serialize_cts(tags: &[CtsTag]) -> String {
    let mut output = String::new();
    for tag in tags {
        match tag.status {
            TagStatus::Positive => {} 
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
    fn test_valid_single_positive() {
        let result = parse_cts("kino").unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].status, TagStatus::Positive);
    }

    #[test]
    fn test_valid_multiple_positives_hybrid() {
        // NEW TEST: Hybrid meetup "Kino AND Coffee"
        let result = parse_cts("kinokawa").unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].word, "kino");
        assert_eq!(result[0].status, TagStatus::Positive);
        assert_eq!(result[1].word, "kawa");
        assert_eq!(result[1].status, TagStatus::Positive);
    }

    #[test]
    fn test_valid_complex_mixed() {
        // "Spacer AND Ball games" (2 pos), "NO rain" (1 neg), "WITH friends" (1 med)
        let input = "spacergra0deszcz1znajomi";
        let result = parse_cts(input).unwrap();
        assert_eq!(result.len(), 4);
        assert_eq!(result[0].status, TagStatus::Positive); // spacer
        assert_eq!(result[1].status, TagStatus::Positive); // gra
        assert_eq!(result[2].status, TagStatus::Negative); // deszcz
        assert_eq!(result[3].status, TagStatus::Mediating); // znajomi
    }

    #[test]
    fn test_error_no_positive() {
        let result = parse_cts("0alkohol");
        assert_eq!(result.unwrap_err(), CtsError::AtLeastOnePositiveRequired);
    }

    #[test]
    fn test_error_too_many_tags() {
        // 11 positive tags
        let input = "1234567890a"; 
        let result = parse_cts(input);
        assert_eq!(result.unwrap_err(), CtsError::TooManyTags);
    }

    #[test]
    fn test_error_trailing_status() {
        let result = parse_cts("kino0");
        assert_eq!(result.unwrap_err(), CtsError::TrailingStatusChar);
    }
}
