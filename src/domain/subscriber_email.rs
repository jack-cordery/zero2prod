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
    use claim::assert_err;
    use fake::{Fake, faker::internet::en::SafeEmail};
    use quickcheck::Arbitrary;
    use rand::SeedableRng;
    use rand::rngs::StdRng;

    use super::*;

    #[derive(Debug, Clone)]
    struct SubscriberEmailFixture(pub String);

    impl Arbitrary for SubscriberEmailFixture {
        fn arbitrary(g: &mut quickcheck::Gen) -> Self {
            let mut rng = StdRng::seed_from_u64(u64::arbitrary(g));
            let email = SafeEmail().fake_with_rng(&mut rng);
            Self(email)
        }
    }

    #[quickcheck_macros::quickcheck]
    fn parse_returns_email_given_valid_email(email: SubscriberEmailFixture) -> bool {
        let subscriber_email = SubscriberEmail::parse(email.0.clone());
        subscriber_email.is_ok()
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
