use reqwest::Client;

use crate::domain::SubscriberEmail;

// ok so in here we will define how to send emails
// we will have some kind of email client struct
// that will have:
// - sender email
// that will have a send_email method that will
// take in
// - html content
// - text content
// - email to send to
// - subject
//
pub struct EmailClient {
    client: Client,
    base_url: String,
    sender: SubscriberEmail,
}

impl EmailClient {
    pub fn new(client: Client, base_url: String, sender: SubscriberEmail) -> Self {
        Self {
            client,
            base_url,
            sender,
        }
    }
    pub async fn send_email(
        &self,
        recipient: SubscriberEmail,
        subject: &str,
        html_content: &str,
        text_content: &str,
    ) -> Result<(), String> {
        todo!();
    }
}
