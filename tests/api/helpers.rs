use argon2::{
    Argon2, Params, PasswordHasher,
    password_hash::{SaltString, rand_core::OsRng},
};
use fake::{
    Fake,
    faker::{internet::ar_sa::Password, name::raw::NameWithTitle},
    locales::EN,
};
use linkify::LinkFinder;
use reqwest::{Client, Response, redirect};
use secrecy::ExposeSecret;
use sqlx::{Connection, Executor, PgConnection, PgPool};
use std::{collections::HashMap, sync::LazyLock};
use url::Url;
use uuid::Uuid;
use wiremock::MockServer;
use zero2prod::{
    authentication::Credentials,
    configuration::{DatabaseSettings, get_configuration},
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
    pub port: u16,
    pub test_user: TestUser,
    pub client: reqwest::Client,
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

    pub async fn post_newsletter(&self, body: serde_json::Value) -> Response {
        self.client
            .post(format!("{}/newsletters", &self.address))
            .basic_auth(&self.test_user.username, Some(&self.test_user.password))
            .json(&body)
            .send()
            .await
            .expect("should return")
    }

    pub async fn post_login(&self, credentials: &Credentials) -> Response {
        let mut params = HashMap::new();
        params.insert("username", credentials.username.to_owned());
        params.insert("password", credentials.password.expose_secret().into());

        self.client
            .post(format!("{}/login", &self.address))
            .form(&params)
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

    pub async fn post_change_password(
        &self,
        current_password: &str,
        new_password: &str,
        confirmation: &str,
    ) -> Response {
        let mut params = HashMap::new();
        params.insert("current_password", current_password);
        params.insert("new_password", new_password);
        params.insert("confirmation", confirmation);

        self.client
            .post(format!("{}/admin/password", self.address))
            .form(&params)
            .send()
            .await
            .expect("Failed to execute post request")
    }
}

pub fn assert_redirect_to(response: &Response, location: &str) {
    assert_eq!(303, response.status().as_u16());
    assert_eq!(
        Some(location),
        response
            .headers()
            .get("LOCATION")
            .map(|h| h.to_str().expect("invalid string"))
    );
}

pub async fn spawn_app() -> TestApp {
    LazyLock::force(&TRACING);

    let email_server = MockServer::start().await;

    let configuration = {
        let mut c = get_configuration().expect("Failed to read configuration");
        c.database.database_name = Uuid::new_v4().to_string();
        c.application.port = 0;
        c.email_client.base_url = email_server.uri();
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

    tokio::spawn(application.run_until_stopped());

    let test_app = TestApp {
        address,
        connection_pool,
        email_server,
        port,
        test_user: TestUser::generate(),
        client,
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

    let connection_pool = PgPool::connect_with(psql_connection_uri_with_db)
        .await
        .expect("Failed to connect to Postgres");

    sqlx::migrate!("./migrations")
        .run(&connection_pool)
        .await
        .expect("Failed to migrate database");

    connection_pool
}
