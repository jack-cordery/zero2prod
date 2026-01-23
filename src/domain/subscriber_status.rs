#[derive(Clone, Copy)]
pub enum SubscriberStatus {
    Pending,
    Confirmed,
}

impl From<SubscriberStatus> for String {
    fn from(status: SubscriberStatus) -> String {
        match status {
            SubscriberStatus::Pending => "pending_confirmation".into(),
            SubscriberStatus::Confirmed => "confirmed".into(),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_pending_converts_to_string_correctly() {
        let actual: String = SubscriberStatus::Pending.into();
        let expected: String = "pending_confirmation".into();
        assert_eq!(
            actual, expected,
            "testing Status::Pending converts to {}.",
            expected
        );
    }

    #[test]
    fn test_confirmed_converts_to_string_correctly() {
        let actual: String = SubscriberStatus::Confirmed.into();
        let expected: String = "confirmed".into();
        assert_eq!(
            actual, expected,
            "testing Status::Confirmed converts to {}",
            expected
        );
    }
}
