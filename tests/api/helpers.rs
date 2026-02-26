use fake::{
    Fake,
    faker::{internet::ar_sa::Password, name::raw::NameWithTitle},
    locales::EN,
};
use linkify::LinkFinder;
use reqwest::{Client, Response};
use sqlx::{Connection, Executor, PgConnection, PgPool};
use std::sync::LazyLock;
use url::Url;
use uuid::Uuid;
use wiremock::MockServer;
use zero2prod::{
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
}

impl TestApp {
    pub async fn post_subsription(&self, body: String) -> Response {
        let client = reqwest::Client::new();
        client
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
        let (username, password) = get_test_user(&self.connection_pool).await;

        let client = Client::new();
        client
            .post(format!("{}/newsletters", &self.address))
            .basic_auth(username, Some(password))
            .json(&body)
            .send()
            .await
            .expect("should return")
    }
}

pub async fn add_test_user(pool: &PgPool) {
    let name: String = NameWithTitle(EN).fake();
    let password: String = Password(1..10).fake();

    let user_id: Uuid = Uuid::new_v4();
    sqlx::query!(
        r#"INSERT INTO users VALUES ($1, $2, $3);"#,
        user_id,
        name,
        password
    )
    .execute(pool)
    .await
    .expect("Failed to insert test user.");
}

pub async fn get_test_user(pool: &PgPool) -> (String, String) {
    let record = sqlx::query!(r#"SELECT username, password FROM users LIMIT 1;"#)
        .fetch_one(pool)
        .await
        .expect("Failed to query database for test user");
    (record.username, record.password)
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

    tokio::spawn(application.run_until_stopped());

    let test_app = TestApp {
        address,
        connection_pool,
        email_server,
        port,
    };
    add_test_user(&test_app.connection_pool).await;
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
