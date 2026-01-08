use validator::ValidateEmail;
#[derive(Debug)]
pub struct SubscriberEmail(String);

impl SubscriberEmail {
    pub fn parse(email: String) -> Result<Self, String> {
        match ValidateEmail::validate_email(&email) {
            true => Ok(Self(email)),
            false => {
                Err(format!("{email} is not a valid email address for a subscriber").to_string())
            }
        }
    }
}

impl AsRef<str> for SubscriberEmail {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod test {
    use claim::{assert_err, assert_ok};

    use super::*;

    #[test]
    fn parse_returns_email_given_valid_email() {
        let email = SubscriberEmail::parse("jack@gmail.com".to_string());
        assert_ok!(&email);
        assert_eq!("jack@gmail.com", email.unwrap().as_ref());
    }

    #[test]
    fn parse_returns_err_given_email_without_at() {
        let email = SubscriberEmail::parse("jackgmail.com".to_string());
        assert_err!(email);
    }

    #[test]
    fn parse_returns_err_given_email_without_name() {
        let email = SubscriberEmail::parse("@gmail.com".to_string());
        assert_err!(email);
    }
    #[test]
    fn parse_returns_err_given_empty_string() {
        let email = "".to_string();
        assert_err!(SubscriberEmail::parse(email));
    }
}
