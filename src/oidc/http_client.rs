use reqwest::Client;

pub struct OidcHttpClient {
    pub client: reqwest::Client,
}

impl OidcHttpClient {
    pub fn new(timeout_duration: std::time::Duration) -> Self {
        Self {
            client: Client::builder()
                .timeout(timeout_duration)
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .expect("Failed to build cient"),
        }
    }
}
