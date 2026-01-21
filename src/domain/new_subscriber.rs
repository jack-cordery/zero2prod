use crate::domain::{SubscriberEmail, SubscriberName, SubscriberStatus};

pub struct NewSubscriber {
    pub email: SubscriberEmail,
    pub name: SubscriberName,
    pub status: SubscriberStatus,
}
