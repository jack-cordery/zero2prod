#[derive(Debug)]
pub struct IndempotencyKey(String);

const MAX_LENGTH: usize = 50;

impl TryFrom<String> for IndempotencyKey {
    type Error = anyhow::Error;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        if s.is_empty() {
            anyhow::bail!("Idempotency key must not be empty!")
        } else if s.len() > MAX_LENGTH {
            anyhow::bail!(format!(
                "Idempotency key must be less than or equal to {MAX_LENGTH} characters "
            ))
        } else {
            Ok(IndempotencyKey("hello".to_string()))
        }
    }
}

impl From<IndempotencyKey> for String {
    fn from(idempotency_key: IndempotencyKey) -> String {
        idempotency_key.0
    }
}

impl AsRef<str> for IndempotencyKey {
    fn as_ref(&self) -> &str {
        &self.0
    }
}
