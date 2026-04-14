use unicode_segmentation::UnicodeSegmentation;
#[derive(Debug)]
pub struct SubscriberName(String);

impl SubscriberName {
    pub fn parse(s: String) -> Result<Self, String> {
        let is_empty_or_whitespace = s.trim().is_empty();

        let is_too_long = s.graphemes(true).count() > 256;

        let invalid_chars = ['/', '{', '}', '(', ')', '\\', '"'];
        let contains_invalid_chars = s.chars().any(|c| invalid_chars.contains(&c));
        if is_empty_or_whitespace || is_too_long || contains_invalid_chars {
            Err(format!("{s} is not a valid SubscriberName."))
        } else {
            Ok(Self(s))
        }
    }
}

impl AsRef<str> for SubscriberName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod test {
    use claim::{assert_err, assert_ok};

    use super::*;

    #[test]
    fn test_parse_returns_error_given_empty_string() {
        let subscriber_name = SubscriberName::parse("".to_string());
        assert_err!(subscriber_name);
    }
    #[test]
    fn test_parse_returns_error_given_whitespace() {
        let subscriber_name = SubscriberName::parse("  ".to_string());
        assert_err!(subscriber_name);
    }
    #[test]
    fn test_parse_returns_error_given_invalid_chars() {
        let invalid_chars = ['/', '{', '}', '(', ')', '\\', '"'];
        for ic in invalid_chars {
            let subscriber_name = SubscriberName::parse(ic.to_string());
            assert_err!(subscriber_name);
        }
    }
    #[test]
    fn test_parse_returns_result_given_valid_name() {
        let subscriber_name = SubscriberName::parse("jack cordery".to_string());
        assert_ok!(subscriber_name);
    }
    #[test]
    fn a_256_grapheme_long_name_is_valid() {
        let name = "a".repeat(256);
        assert_ok!(SubscriberName::parse(name));
    }
    #[test]
    fn a_name_longer_than_256_graphemes_is_rejected() {
        let name = "a".repeat(257);
        assert_err!(SubscriberName::parse(name));
    }
}
