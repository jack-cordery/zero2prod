use argon2::{
    Argon2, Params, PasswordHasher,
    password_hash::{SaltString, rand_core::OsRng},
};
use base64::{Engine, prelude::BASE64_URL_SAFE_NO_PAD};
use fake::{
    Fake,
    faker::{
        internet::{ar_sa::Password, raw::SafeEmail},
        name::raw::{Name, NameWithTitle},
    },
    locales::EN,
};
use jsonwebtoken::{EncodingKey, Header, encode};
use linkify::LinkFinder;
use reqwest::{Client, Response, redirect};
use rsa::{RsaPrivateKey, pkcs8::DecodePrivateKey, traits::PublicKeyParts};
use serde::Serialize;
use serde_json::json;
use sqlx::{Connection, Executor, PgConnection, PgPool, postgres::PgPoolOptions};
use std::{sync::LazyLock, time::Duration};
use tokio::time::sleep;
use url::Url;
use uuid::Uuid;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path},
};
use zero2prod::{
    configuration::{DatabaseSettings, RateLimitSettings, get_configuration},
    email_client::EmailClient,
    idempotency::delete_expired_keys,
    issue_delivery_worker::{QueueState, process_email},
    routes::CallbackQuery,
    startup::{Application, get_connection_pool},
    telementry::{get_subscriber, init_subscriber},
};

static TRACING: LazyLock<()> = LazyLock::new(|| {
    let default_name = "test".to_string();
    let default_level = "info".to_string();

    if std::env::var("TEST_LOG").is_ok() {
        let subscriber = get_subscriber(default_name, default_level, std::io::stdout);
        init_subscriber(subscriber);
    } else {
        let subscriber = get_subscriber(default_name, default_level, std::io::sink);
        init_subscriber(subscriber);
    }
});

pub struct TestUser {
    pub username: String,
    pub password: String,
    pub user_id: Uuid,
}

impl TestUser {
    pub fn generate() -> Self {
        let name: String = NameWithTitle(EN).fake();
        let password: String = Password(1..10).fake();
        Self {
            username: name,
            password,
            user_id: Uuid::new_v4(),
        }
    }

    pub fn get_hash_password(&self) -> String {
        let salt = SaltString::generate(&mut OsRng);
        Argon2::new(
            argon2::Algorithm::Argon2id,
            argon2::Version::V0x13,
            Params::new(15000, 2, 1, None).expect("Failed to initialise Argon2 params"),
        )
        .hash_password(self.password.as_bytes(), &salt)
        .expect("Failed to hash password")
        .to_string()
    }
}

#[derive(PartialEq, Debug)]
pub struct ConfirmationLinks {
    pub html: reqwest::Url,
    pub text: reqwest::Url,
}

pub struct TestApp {
    pub address: String,
    pub connection_pool: PgPool,
    pub email_server: MockServer,
    pub oidc_server: MockServer,
    pub port: u16,
    pub test_user: TestUser,
    pub client: reqwest::Client,
    pub email_client: EmailClient,
    pub max_retries: u8,
}

impl TestApp {
    pub async fn insert_test_user(&self) {
        sqlx::query!(
            r#"INSERT INTO users VALUES ($1, $2, $3);"#,
            self.test_user.user_id,
            self.test_user.username,
            self.test_user.get_hash_password(),
        )
        .execute(&self.connection_pool)
        .await
        .expect("Failed to insert test user.");
    }

    pub async fn post_subsription(&self, body: String) -> Response {
        self.client
            .post(format!("{}/subscribe", &self.address))
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(body)
            .send()
            .await
            .expect("should return")
    }

    pub fn get_confirmation_links(&self, request: &wiremock::Request) -> ConfirmationLinks {
        let body: serde_json::Value = serde_json::from_slice(&request.body).unwrap();
        let get_link = |s: &str| {
            let links: Vec<_> = LinkFinder::new()
                .links(s)
                .filter(|l| *l.kind() == linkify::LinkKind::Url)
                .collect();
            assert_eq!(links.len(), 1);
            let mut link = Url::parse(links[0].as_str()).unwrap();
            assert_eq!(link.host_str().unwrap(), "127.0.0.1"); // ensure we aren't calling internet
            link.set_port(Some(self.port)).unwrap();
            link
        };

        let html_link = get_link(body["HtmlBody"].as_str().unwrap());
        let text_link = get_link(body["TextBody"].as_str().unwrap());
        ConfirmationLinks {
            html: html_link,
            text: text_link,
        }
    }

    pub async fn post_newsletter<Body>(&self, body: &Body) -> Response
    where
        Body: Serialize,
    {
        self.client
            .post(format!("{}/admin/newsletter", &self.address))
            .form(body)
            .send()
            .await
            .expect("should return")
    }

    pub async fn get_newsletter(&self) -> Response {
        self.client
            .get(format!("{}/admin/newsletter", &self.address))
            .send()
            .await
            .expect("Failed to execute get request")
    }
    pub async fn get_newsletter_html(&self) -> String {
        self.client
            .get(format!("{}/admin/newsletter", &self.address))
            .send()
            .await
            .expect("Failed to execute get request")
            .text()
            .await
            .unwrap()
    }
    pub async fn post_login<Body>(&self, body: &Body) -> Response
    where
        Body: serde::Serialize,
    {
        self.client
            .post(format!("{}/login", &self.address))
            .form(body)
            .send()
            .await
            .expect("should return")
    }

    pub async fn get_login_html(&self) -> String {
        self.client
            .get(format!("{}/login", &self.address))
            .send()
            .await
            .expect("Failed to execute get request")
            .text()
            .await
            .unwrap()
    }
    pub async fn get_google_login(&self) -> Response {
        self.client
            .get(format!("{}/login/google", self.address))
            .send()
            .await
            .expect("Failed to execute get request")
    }

    pub async fn get_google_callback(&self, query: CallbackQuery) -> Response {
        let url = format!(
            "{}/login/callback?{}",
            self.address,
            serde_urlencoded::to_string(query).expect("invalid query")
        );
        self.client
            .get(url)
            .send()
            .await
            .expect("Failed to execute get request")
    }

    pub async fn get_admin_dashboard(&self) -> Response {
        self.client
            .get(format!("{}/admin/dashboard", self.address))
            .send()
            .await
            .expect("Failed to execute get request")
    }
    pub async fn get_change_password(&self) -> Response {
        self.client
            .get(format!("{}/admin/password", self.address))
            .send()
            .await
            .expect("Failed to execute get request")
    }

    pub async fn get_change_password_html(&self) -> String {
        self.client
            .get(format!("{}/admin/password", &self.address))
            .send()
            .await
            .expect("Failed to execute get request")
            .text()
            .await
            .unwrap()
    }

    pub async fn post_change_password<Body>(&self, body: &Body) -> Response
    where
        Body: Serialize,
    {
        self.client
            .post(format!("{}/admin/password", self.address))
            .form(body)
            .send()
            .await
            .expect("Failed to execute post request")
    }

    pub async fn post_logout(&self) -> Response {
        self.client
            .post(format!("{}/admin/logout", self.address))
            .send()
            .await
            .expect("Failed to execute logout")
    }

    pub async fn get_dashboard_html(&self) -> String {
        self.client
            .get(format!("{}/admin/dashboard", self.address))
            .send()
            .await
            .expect("Failed to get response")
            .text()
            .await
            .unwrap()
    }

    pub async fn get_issues_html(&self) -> String {
        self.client
            .get(format!("{}/admin/issues", &self.address))
            .send()
            .await
            .expect("Failed to execute get request")
            .text()
            .await
            .unwrap()
    }

    pub async fn dispatch_all_emails(&self) {
        loop {
            match process_email(&self.connection_pool, &self.email_client, &self.max_retries).await
            {
                Ok(QueueState::Empty) => {
                    break;
                }
                Ok(QueueState::Waiting(wait_time)) => {
                    sleep(Duration::from_secs_f64(wait_time)).await
                }
                Ok(QueueState::Ready(_)) => (),
                Err(_) => (),
            }
        }
    }

    pub async fn clean_idempotency_keys(&self) {
        delete_expired_keys(0, &self.connection_pool)
            .await
            .expect("Failed to run key deletion");
    }
    pub async fn create_unconfirmed_subscriber(&self) -> ConfirmationLinks {
        let _mock_guard = Mock::given(method("POST"))
            .and(path("/email"))
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .named("Create unconfirmed subscriber")
            .mount_as_scoped(&self.email_server)
            .await;

        let name: String = Name(EN).fake();
        let email: String = SafeEmail(EN).fake();

        let url_name = urlencoding::encode(&name);
        let url_email = urlencoding::encode(&email);

        self.post_subsription(format!("name={url_name}&email={url_email}"))
            .await
            .error_for_status()
            .unwrap();

        let recieved_request = self
            .email_server
            .received_requests()
            .await
            .unwrap()
            .pop()
            .unwrap();
        self.get_confirmation_links(&recieved_request)
    }

    pub async fn create_confirmed_subscriber(&self) {
        let confirmation_links = self.create_unconfirmed_subscriber().await;
        reqwest::get(confirmation_links.html)
            .await
            .unwrap()
            .error_for_status()
            .unwrap();
    }
}

pub fn assert_redirect_to(response: &Response, location: &str) {
    let response_location = response
        .headers()
        .get("LOCATION")
        .map(|h| h.to_str().expect("invalid string"))
        .expect("no location found");
    assert_eq!(303, response.status().as_u16());
    assert_eq!(location, response_location);
}

pub async fn spawn_app() -> TestApp {
    spawn_app_with_config(RateLimitSettings {
        rate_limit: 100,
        namespace: "prod".into(),
    })
    .await
}

pub const PRIVATE_KEY: &str = r#"
-----BEGIN PRIVATE KEY-----
MIIEvAIBADANBgkqhkiG9w0BAQEFAASCBKYwggSiAgEAAoIBAQDeOCq8BojmNcL7
s/waQL4E2JajDP2DQ1sP6ceiJv56esmtNT1Wa+jfg5HnsX9l/WWdTq6+cksxO2Xq
ftbrtneNrriO7/zeoIED7BRZ2ezGl8VZxKFRU0UTL2OWkNU5+GOE9+LRAE3oyxLL
Za3ou1/ccld04qRKYYLNhFEirVTNr3+W0E+pbe27Vg60Zh4BdG228e2wYSzTnHQG
tJFgIj24vp2DBVAmV5SQGTo+NKhpZBBt25n29Attx0bOETIsJejgfEAiGXjY6U4F
1mnLTzCIBkbEtHWPHCfE2Kuj515Xccmufg5JHdo3h8vickmh1FR9KtDvtZfxq5kj
qA68h6C3AgMBAAECggEABfZu2x23xamSoktZi+DJ2HpxTE24bbG8e0hUFXtDX8j0
qWOg0jVSCdFPdG6UUwnCFL78PFr3vonv+aNOpAOA4Lnb9OXmnJik7ZSDlUeeLVP8
NSTsCTEZTOL8IpmfRw9tqC84lFAURxdP2UpQqMqCT3l39EhyjRZhup7+yFXrTRuI
F/yj52FkApvrERQulSpIgMl0NNEwYN0W7xprpnx9iEOsS9U+IC8GRAOqTxPPFFTm
7EPgFkOT0+kPjBsyNeGtiPcQFNBVQ9AO0mb4Mtca0aII3P/kmr5LGTQzeSeETHun
adYRBVDhDwpgx3O9A/HcegsamyK48ad/S24x88D9zQKBgQD88oyayQyiANOp1JJR
eBTSmGjwz5eCZDfGztYW4hcn1399wSXmpi5USlNszvxwhbmooccuUXqbKPzS34f3
D26w6gWuy8nztk/IW9knxXY4eoTjFnTt1B1v421CeQgwYosPiScRaLWYN2JXW+HB
Yt8u7mbfahP4zR0jRKLgmjq8vQKBgQDg5q/jKt+Z54DppHfFD8MVnQyXhBHFgU3Z
qhi+7Yk6BOIpzSY9+7xK94jCC0NCbF3d6O8HP+jK90lUrkzqTgKj9hXATeEo9nFA
Aq1SEG48le8VWri1kV/XnfkbFejwqwbqQtKINmLRHbU9hYsBRJSRFNThsRyNG+cy
snmDvdv8gwKBgG3KYZk1ttQCg9ztNW1DL9aQ7MvJbzvbgBI86NQZ4m8arG3LDkZk
zysq77cEyLGWeZVmUuwZ1ZvPWJ23BG8KNcN4cGsEbW3pLgwLQeBvZvbwxwlCUBKC
xRwxnNUDb7iArVda8qgtyNR/BaJhcUXdQn4+YEyM4IpXjVQnkILorqIJAoGAa2sD
e1cQ8Wt3USDy67Z5kSsvxnaYHmOCEYKCyz6dGo8WjqyjpVtFNfFA6p2ChIlJ1CHb
ePT3dWnjJoURy59y92kkPnN0JaJ/uPkOW3HplRpv1R09t8s1ocCcKGmwlrK5XM6J
y/FeBU9RL49HM1XUN+9hNmLnpiY7qSVBkMDv/40CgYB1zUtBUJA0qVb6rbpZnAFI
+pAquJuEot0ZGQg/uDMSuo7OgWiBmE2hDCxJC8T6MTUb7jRySStr3k8Enfu+MAmD
NlhiCmNT1YpwYyC0m8GTwEIjlGF8WkxV0n+3wC96KtGFNbSPGDoCMd04UlyG7OAr
tC4uQIqx+L6rP9v0AYeEDA==
-----END PRIVATE KEY-----
"#;

pub fn make_test_jwt(payload: serde_json::Value) -> String {
    encode(
        &Header::new(jsonwebtoken::Algorithm::RS256),
        &payload,
        &EncodingKey::from_rsa_pem(PRIVATE_KEY.as_bytes()).unwrap(),
    )
    .unwrap()
}

pub fn get_callback_query_from_login_response(response: Response) -> (CallbackQuery, String) {
    let location = response
        .headers()
        .get("LOCATION")
        .map(|h| h.to_str().expect("Invalid str"))
        .expect("No location found");
    let location = Url::parse(location).expect("Invalid url");
    let query_pairs = location.query_pairs();

    let mut nonce: Option<String> = None;
    let mut state: Option<String> = None;

    for (key, value) in query_pairs {
        match key.as_ref() {
            "nonce" => nonce = Some(value.to_string()),
            "state" => state = Some(value.to_string()),
            _ => {}
        }
    }

    let nonce = nonce.unwrap();
    let csrf = state.unwrap();

    (
        CallbackQuery::new(csrf, "some_code_to_exchange".into(), "some_scope".into()),
        nonce,
    )
}

async fn mount_openidconnect_end_points(oidc_uri: String, mock_server: &MockServer) {
    let private = RsaPrivateKey::from_pkcs8_pem(PRIVATE_KEY).unwrap();
    let public = private.to_public_key();

    let n = BASE64_URL_SAFE_NO_PAD.encode(public.n().to_bytes_be());
    let e = BASE64_URL_SAFE_NO_PAD.encode(public.e().to_bytes_be());

    Mock::given(method("GET"))
        .and(path("/certs"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
          "keys": [
            {
              "kty": "RSA",
              "kid": "test-key",
              "use": "sig",
              "alg": "RS256",
              "n": n,
              "e": e
            }
          ]
        })))
        .mount(mock_server)
        .await;

    Mock::given(method("GET"))
        .and(path("/.well-known/openid-configuration"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!(
        {
         "issuer": oidc_uri,
         "authorization_endpoint": format!("{oidc_uri}/auth"),
         "device_authorization_endpoint": format!("{oidc_uri}/auth"),
         "token_endpoint": format!("{oidc_uri}/token"),
         "userinfo_endpoint": "https://openidconnect.googleapis.com/v1/userinfo",
         "revocation_endpoint": "https://oauth2.googleapis.com/revoke",
         "jwks_uri": format!("{oidc_uri}/certs"),
         "response_types_supported": [
          "code",
          "token",
          "id_token",
          "code token",
          "code id_token",
          "token id_token",
          "code token id_token",
          "none"
         ],
         "response_modes_supported": [
          "query",
          "fragment",
          "form_post"
         ],
         "subject_types_supported": [
          "public"
         ],
         "id_token_signing_alg_values_supported": [
          "RS256"
         ],
         "scopes_supported": [
          "openid",
          "email",
          "profile"
         ],
         "token_endpoint_auth_methods_supported": [
          "client_secret_post",
          "client_secret_basic"
         ],
         "claims_supported": [
          "aud",
          "email",
          "email_verified",
          "exp",
          "family_name",
          "given_name",
          "iat",
          "iss",
          "name",
          "picture",
          "sub"
         ],
         "code_challenge_methods_supported": [
          "plain",
          "S256"
         ],
         "grant_types_supported": [
          "authorization_code",
          "refresh_token",
          "urn:ietf:params:oauth:grant-type:device_code",
          "urn:ietf:params:oauth:grant-type:jwt-bearer"
         ],
         "authorization_response_iss_parameter_supported": true
        }

                      )))
        .expect(1)
        .mount(mock_server)
        .await;
}

pub async fn spawn_app_with_config(rate_limit_settings: RateLimitSettings) -> TestApp {
    LazyLock::force(&TRACING);

    let email_server = MockServer::start().await;

    let oidc_server = MockServer::start().await;
    mount_openidconnect_end_points(oidc_server.uri(), &oidc_server).await;

    let configuration = {
        let mut c = get_configuration().expect("Failed to read configuration");
        c.database.database_name = Uuid::new_v4().to_string();
        c.application.port = 0;
        c.email_client.base_url = email_server.uri();
        c.oidc.issuer_url = oidc_server.uri();
        c.application.rate_limit.rate_limit = rate_limit_settings.rate_limit;
        c.application.rate_limit.namespace = rate_limit_settings.namespace;
        c
    };

    let _ = configure_database(&configuration.database).await;

    let application = Application::build(&configuration)
        .await
        .expect("Failed to build application");
    let port = application.port();

    let connection_pool = get_connection_pool(&configuration.database);
    let address = format!("http://127.0.0.1:{}", application.port());

    let client = Client::builder()
        .redirect(redirect::Policy::none())
        .cookie_store(true)
        .build()
        .expect("should build");

    let base_url = Url::parse(&configuration.email_client.base_url).expect("Invalid base url");

    let email_client = EmailClient::new(
        base_url,
        configuration
            .email_client
            .sender()
            .expect("invalid sender email"),
        configuration.email_client.authorization_token.clone(),
        configuration.email_client.timeout(),
    );

    tokio::spawn(application.run_until_stopped());

    let test_app = TestApp {
        address,
        connection_pool,
        email_server,
        oidc_server,
        port,
        test_user: TestUser::generate(),
        client,
        email_client,
        max_retries: configuration.application.max_retries,
    };
    test_app.insert_test_user().await;
    test_app
}

async fn configure_database(db_settings: &DatabaseSettings) -> PgPool {
    let psql_connection_uri_without_db = db_settings.get_connection_uri_without_db_name();
    let psql_connection_uri_with_db = db_settings.get_connection_uri();

    let mut connection = PgConnection::connect_with(&psql_connection_uri_without_db)
        .await
        .expect("Failed to connect to Postgres");

    connection
        .execute(format!(r#"CREATE DATABASE "{}";"#, db_settings.database_name).as_str())
        .await
        .expect("Failed to create database");

    let connection_pool = PgPoolOptions::new()
        .connect_with(psql_connection_uri_with_db)
        .await
        .expect("Failed to connect to Postgres");

    sqlx::migrate!("./migrations")
        .run(&connection_pool)
        .await
        .expect("Failed to migrate database");

    connection_pool
}
