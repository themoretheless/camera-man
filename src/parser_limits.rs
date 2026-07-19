use std::fmt;
use std::io::{self, Read};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParserLimits {
    pub max_input_bytes: usize,
    pub max_nesting_depth: usize,
    pub max_elements: usize,
    pub max_string_bytes: usize,
    pub max_allocation_bytes: usize,
}

pub const SCENE_PARSER_LIMITS: ParserLimits = ParserLimits {
    max_input_bytes: 1024 * 1024,
    max_nesting_depth: 64,
    max_elements: 64,
    max_string_bytes: 256,
    max_allocation_bytes: 2 * 1024 * 1024,
};

pub const PROFILE_PARSER_LIMITS: ParserLimits = ParserLimits {
    max_input_bytes: 4 * 1024 * 1024,
    max_nesting_depth: 64,
    max_elements: 256,
    max_string_bytes: 4 * 1024,
    max_allocation_bytes: 8 * 1024 * 1024,
};

pub const PREFERENCES_PARSER_LIMITS: ParserLimits = ParserLimits {
    max_input_bytes: 2 * 1024 * 1024,
    max_nesting_depth: 64,
    max_elements: 512,
    max_string_bytes: 4 * 1024,
    max_allocation_bytes: 4 * 1024 * 1024,
};

pub const BENCHMARK_REPORT_PARSER_LIMITS: ParserLimits = ParserLimits {
    max_input_bytes: 64 * 1024 * 1024,
    max_nesting_depth: 128,
    max_elements: 1_000_000,
    max_string_bytes: 64 * 1024,
    max_allocation_bytes: 128 * 1024 * 1024,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParserLimitError {
    InputBytes,
    AllocationBudget,
    NestingDepth,
    ElementCount,
    StringBytes,
}

impl fmt::Display for ParserLimitError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InputBytes => "parser input exceeds its byte limit",
            Self::AllocationBudget => "parser input exceeds its allocation budget",
            Self::NestingDepth => "structured input exceeds its nesting-depth limit",
            Self::ElementCount => "structured input exceeds its element-count limit",
            Self::StringBytes => "structured input contains an oversized string",
        })
    }
}

impl std::error::Error for ParserLimitError {}

pub fn validate_input_size(bytes: &[u8], limits: ParserLimits) -> Result<(), ParserLimitError> {
    if bytes.len() > limits.max_input_bytes {
        return Err(ParserLimitError::InputBytes);
    }
    if bytes.len().saturating_mul(2) > limits.max_allocation_bytes {
        return Err(ParserLimitError::AllocationBudget);
    }
    Ok(())
}

pub fn read_bounded(path: &Path, limits: ParserLimits) -> io::Result<Vec<u8>> {
    let file = std::fs::File::open(path)?;
    let metadata_len = file.metadata()?.len();
    if metadata_len > limits.max_input_bytes as u64 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            ParserLimitError::InputBytes,
        ));
    }
    let mut bytes = Vec::with_capacity(metadata_len as usize);
    file.take(limits.max_input_bytes as u64 + 1)
        .read_to_end(&mut bytes)?;
    validate_input_size(&bytes, limits)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    Ok(bytes)
}

/// Performs a conservative JSON envelope check before `serde_json` allocates.
/// Syntax and schema validation remain the structured parser's responsibility.
pub fn validate_json_envelope(bytes: &[u8], limits: ParserLimits) -> Result<(), ParserLimitError> {
    validate_input_size(bytes, limits)?;
    let mut depth = 0_usize;
    let mut in_string = false;
    let mut escaped = false;
    let mut string_bytes = 0_usize;
    let mut elements = 0_usize;
    for &byte in bytes {
        if in_string {
            if escaped {
                escaped = false;
                string_bytes = string_bytes.saturating_add(1);
            } else if byte == b'\\' {
                escaped = true;
                string_bytes = string_bytes.saturating_add(1);
            } else if byte == b'"' {
                in_string = false;
                string_bytes = 0;
            } else {
                string_bytes = string_bytes.saturating_add(1);
            }
            if string_bytes > limits.max_string_bytes {
                return Err(ParserLimitError::StringBytes);
            }
            continue;
        }
        match byte {
            b'"' => in_string = true,
            b'{' | b'[' => {
                depth = depth.saturating_add(1);
                elements = elements.saturating_add(1);
                if depth > limits.max_nesting_depth {
                    return Err(ParserLimitError::NestingDepth);
                }
                if elements > limits.max_elements {
                    return Err(ParserLimitError::ElementCount);
                }
            }
            b'}' | b']' => depth = depth.saturating_sub(1),
            b',' => {
                elements = elements.saturating_add(1);
                if elements > limits.max_elements {
                    return Err(ParserLimitError::ElementCount);
                }
            }
            _ => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_envelope_ignores_brackets_inside_escaped_strings() {
        let limits = ParserLimits {
            max_nesting_depth: 2,
            ..SCENE_PARSER_LIMITS
        };
        assert_eq!(
            validate_json_envelope(br#"{"text":"[\\\"]"}"#, limits),
            Ok(())
        );
        assert_eq!(
            validate_json_envelope(br#"[[[0]]]"#, limits),
            Err(ParserLimitError::NestingDepth)
        );
    }

    #[test]
    fn json_envelope_enforces_string_element_and_allocation_budgets() {
        let limits = ParserLimits {
            max_input_bytes: 128,
            max_nesting_depth: 4,
            max_elements: 2,
            max_string_bytes: 3,
            max_allocation_bytes: 256,
        };
        assert_eq!(
            validate_json_envelope(br#"["abcd"]"#, limits),
            Err(ParserLimitError::StringBytes)
        );
        assert_eq!(
            validate_json_envelope(b"[1,2,3]", limits),
            Err(ParserLimitError::ElementCount)
        );
        let tight_allocation = ParserLimits {
            max_allocation_bytes: 4,
            ..limits
        };
        assert_eq!(
            validate_json_envelope(b"[1]", tight_allocation),
            Err(ParserLimitError::AllocationBudget)
        );
    }
}
