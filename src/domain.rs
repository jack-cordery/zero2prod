use unicode_segmentation::UnicodeSegmentation;

pub struct NewSubcsriber {
    pub email: String,
    pub name: SubscriberName,
}

pub struct SubscriberName(String);

impl SubscriberName {
    /// Returns an instance of SubscriberName if the parsed string is validated as true.
    /// That is to say that it is non-empty and is no larger than 256 graphemes.
    /// It panics otherwise.
    pub fn parse(s: String) -> Result<Self, String> {
        let is_empty_or_whitespace = s.trim().is_empty();

        let is_too_long = s.graphemes(true).count() > 256;

        let invalid_chars = ['/', '{', '}', '(', ')', '\\', '"'];
        let contains_invalid_chars = s.chars().any(|c| invalid_chars.contains(&c));
        if is_empty_or_whitespace || is_too_long || contains_invalid_chars {
            Err(format!("{s} is an invalid string for SubscriberName"))
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
