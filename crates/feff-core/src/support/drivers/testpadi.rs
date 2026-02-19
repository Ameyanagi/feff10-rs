#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
pub enum PadIntegerError {
    #[error("encoded value is empty")]
    Empty,
    #[error("encoded value must begin with '+' or '-'")]
    MissingSign,
    #[error("invalid PAD digit '{ch}' at position {index}")]
    InvalidDigit { ch: char, index: usize },
    #[error("PAD integer exceeds i32 range")]
    Overflow,
}

pub fn wrpadisc(value: i32) -> String {
    let sign = if value < 0 { '-' } else { '+' };
    let mut magnitude = i64::from(value).unsigned_abs();
    let mut digits = Vec::new();

    loop {
        let digit = u8::try_from(magnitude % 75).expect("base-75 digit should fit in u8");
        digits.push(char::from(digit + 48));
        magnitude /= 75;
        if magnitude == 0 {
            break;
        }
    }

    digits.reverse();
    let mut encoded = String::with_capacity(digits.len() + 1);
    encoded.push(sign);
    encoded.extend(digits);
    encoded
}

pub fn rdpadisc(encoded: &str) -> Result<i32, PadIntegerError> {
    let trimmed = encoded.trim();
    if trimmed.is_empty() {
        return Err(PadIntegerError::Empty);
    }

    let mut chars = trimmed.chars();
    let Some(sign_char) = chars.next() else {
        return Err(PadIntegerError::Empty);
    };

    let sign = match sign_char {
        '+' => 1_i64,
        '-' => -1_i64,
        _ => return Err(PadIntegerError::MissingSign),
    };

    let mut value = 0_i64;
    for (index, ch) in chars.enumerate() {
        let ascii = u32::from(ch);
        if !(48..=122).contains(&ascii) {
            return Err(PadIntegerError::InvalidDigit {
                ch,
                index: index + 2,
            });
        }

        let digit = i64::from(ascii - 48);
        value = value
            .checked_mul(75)
            .and_then(|acc| acc.checked_add(digit))
            .ok_or(PadIntegerError::Overflow)?;
    }

    let signed = value.checked_mul(sign).ok_or(PadIntegerError::Overflow)?;
    i32::try_from(signed).map_err(|_| PadIntegerError::Overflow)
}

#[cfg(test)]
mod tests {
    use super::{PadIntegerError, rdpadisc, wrpadisc};

    #[test]
    fn wrpadisc_matches_known_base75_example() {
        assert_eq!(wrpadisc(2_345), "+OD");
        assert_eq!(wrpadisc(-2_345), "-OD");
    }

    #[test]
    fn pad_integer_roundtrip_matches_legacy_codec() {
        let values = [0, 1, -1, 74, 75, 76, 2_345, -2_345, i32::MIN, i32::MAX];

        for value in values {
            let encoded = wrpadisc(value);
            let decoded = rdpadisc(&encoded).expect("roundtrip decode should succeed");
            assert_eq!(decoded, value);
        }
    }

    #[test]
    fn decoder_rejects_missing_sign() {
        let error = rdpadisc("OD").expect_err("decode should fail");
        assert_eq!(error, PadIntegerError::MissingSign);
    }

    #[test]
    fn decoder_rejects_non_pad_digits() {
        let error = rdpadisc("+!").expect_err("decode should fail");
        assert_eq!(error, PadIntegerError::InvalidDigit { ch: '!', index: 2 });
    }

    #[test]
    fn decoder_rejects_overflow_values() {
        let error = rdpadisc("+zzzzzzzzzz").expect_err("decode should overflow");
        assert_eq!(error, PadIntegerError::Overflow);
    }
}
