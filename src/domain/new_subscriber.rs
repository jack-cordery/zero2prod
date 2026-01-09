use crate::domain::{SubscriberEmail, SubscriberName};

pub struct NewSubcsriber {
    pub email: SubscriberEmail,
    pub name: SubscriberName,
}
