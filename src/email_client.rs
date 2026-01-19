use reqwest::{Client, Url};
use secrecy::{ExposeSecret, SecretString};

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
    base_url: Url,
    sender: SubscriberEmail,
    authorization_token: SecretString,
}

impl EmailClient {
    pub fn new(base_url: Url, sender: SubscriberEmail, authorization_token: SecretString) -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .expect("Failed to build cient");
        Self {
            client,
            base_url,
            sender,
            authorization_token,
        }
    }
    pub async fn send_email(
        &self,
        recipient: SubscriberEmail,
        subject: &str,
        text_content: &str,
        html_content: &str,
    ) -> Result<(), reqwest::Error> {
        let send_endpoint = self
            .base_url
            .join("email")
            .expect("Failed to join the base path and end point.");
        let body = SendEmailRequest {
            from: self.sender.as_ref(),
            to: recipient.as_ref(),
            subject,
            text_body: text_content,
            html_body: html_content,
        };
        self.client
            .post(send_endpoint)
            .header(
                "X-Postmark-Server-Token",
                self.authorization_token.expose_secret(),
            )
            .json(&body)
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }
}

#[derive(serde::Serialize)]
#[serde(rename_all = "PascalCase")]
struct SendEmailRequest<'a> {
    from: &'a str,
    to: &'a str,
    subject: &'a str,
    text_body: &'a str,
    html_body: &'a str,
}

#[cfg(test)]
mod test {
    use std::time::Duration;

    use claim::{assert_err, assert_ok};
    use fake::{
        Fake, Faker,
        faker::{
            internet::en::SafeEmail,
            lorem::en::{Paragraph, Sentence},
        },
    };
    use wiremock::{
        Mock, MockServer, Request, ResponseTemplate,
        matchers::{any, header, header_exists},
    };

    use super::*;

    struct SendEmailBody;

    impl wiremock::Match for SendEmailBody {
        fn matches(&self, request: &Request) -> bool {
            let request_body: Result<serde_json::Value, _> = request.body_json();
            match request_body {
                Ok(body) => {
                    dbg!(&body);
                    body.get("From").is_some()
                        && body.get("To").is_some()
                        && body.get("Subject").is_some()
                        && body.get("HtmlBody").is_some()
                        && body.get("TextBody").is_some()
                }
                Err(_) => false,
            }
        }
    }

    #[tokio::test]
    async fn send_email_sends_the_expected_request() {
        let mock_server = MockServer::start().await;
        Mock::given(header_exists("X-Postmark-Server-Token"))
            .and(header("Content-Type", "application/json"))
            .and(SendEmailBody)
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&mock_server)
            .await;

        // so now i want to create an email client and call send
        //
        let sender = SubscriberEmail::parse(SafeEmail().fake()).unwrap();
        let subject: String = Sentence(1..2).fake();
        let content: String = Paragraph(1..10).fake();
        let subscriber_email = SubscriberEmail::parse(SafeEmail().fake()).unwrap();

        let mock_url =
            Url::parse(&mock_server.uri()).expect("Failed to convert mock server to Url");
        let fake_auth_token: String = Faker.fake();
        let email_client = EmailClient::new(mock_url, sender, fake_auth_token.into());
        let _ = email_client
            .send_email(subscriber_email, &subject, &content, &content)
            .await;
        // Assert
        // Mock expectations are checked on drop
    }

    #[tokio::test]
    async fn send_email_succeeds_if_server_returns_200() {
        let mock_server = MockServer::builder().start().await;
        Mock::given(any())
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&mock_server)
            .await;

        let sender = SubscriberEmail::parse(SafeEmail().fake()).unwrap();
        let subject: String = Sentence(1..2).fake();
        let content: String = Paragraph(1..10).fake();
        let subscriber_email = SubscriberEmail::parse(SafeEmail().fake()).unwrap();

        let mock_url =
            Url::parse(&mock_server.uri()).expect("Failed to convert mock server to Url");
        let fake_auth_token: String = Faker.fake();
        let email_client = EmailClient::new(mock_url, sender, fake_auth_token.into());
        let response = email_client
            .send_email(subscriber_email, &subject, &content, &content)
            .await;
        assert_ok!(response);
    }

    #[tokio::test]
    async fn send_email_fails_if_server_returns_500() {
        let mock_server = MockServer::builder().start().await;
        Mock::given(any())
            .respond_with(ResponseTemplate::new(500))
            .expect(1)
            .mount(&mock_server)
            .await;

        let sender = SubscriberEmail::parse(SafeEmail().fake()).unwrap();
        let subject: String = Sentence(1..2).fake();
        let content: String = Paragraph(1..10).fake();
        let subscriber_email = SubscriberEmail::parse(SafeEmail().fake()).unwrap();

        let mock_url =
            Url::parse(&mock_server.uri()).expect("Failed to convert mock server to Url");
        let fake_auth_token: String = Faker.fake();
        let email_client = EmailClient::new(mock_url, sender, fake_auth_token.into());
        let response = email_client
            .send_email(subscriber_email, &subject, &content, &content)
            .await;
        assert_err!(response);
    }
    #[tokio::test]
    async fn send_email_fails_if_server_timesout() {
        let mock_server = MockServer::builder().start().await;
        let delay_duration = Duration::from_secs(3 * 60);
        let response = ResponseTemplate::new(200).set_delay(delay_duration);
        Mock::given(any())
            .respond_with(response)
            .expect(1)
            .mount(&mock_server)
            .await;

        let sender = SubscriberEmail::parse(SafeEmail().fake()).unwrap();
        let subject: String = Sentence(1..2).fake();
        let content: String = Paragraph(1..10).fake();
        let subscriber_email = SubscriberEmail::parse(SafeEmail().fake()).unwrap();

        let mock_url =
            Url::parse(&mock_server.uri()).expect("Failed to convert mock server to Url");
        let fake_auth_token: String = Faker.fake();
        let email_client = EmailClient::new(mock_url, sender, fake_auth_token.into());
        let response = email_client
            .send_email(subscriber_email, &subject, &content, &content)
            .await;
        assert_err!(response);
    }
}
