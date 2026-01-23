use std::env;
use std::time::Duration;

use config::Config;
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;

use serde_aux::field_attributes::deserialize_number_from_string;
use sqlx::ConnectOptions;
use sqlx::postgres::{PgConnectOptions, PgSslMode};

use crate::domain::SubscriberEmail;

#[derive(Deserialize)]
pub struct Settings {
    pub database: DatabaseSettings,
    pub application: ApplicationSettings,
    pub email_client: EmailClientSettings,
}

#[derive(Deserialize)]
pub struct EmailClientSettings {
    pub base_url: String,
    pub sender_email: String,
    pub authorization_token: SecretString,
    pub timeout_millis: u64,
}

impl EmailClientSettings {
    pub fn sender(&self) -> Result<SubscriberEmail, String> {
        SubscriberEmail::parse(self.sender_email.clone())
    }

    pub fn timeout(&self) -> std::time::Duration {
        Duration::from_millis(self.timeout_millis)
    }
}

#[derive(Deserialize)]
pub struct ApplicationSettings {
    pub host: String,
    #[serde(deserialize_with = "deserialize_number_from_string")]
    pub port: u16,
    pub base_url: String,
}

#[derive(Deserialize)]
pub struct DatabaseSettings {
    pub username: String,
    pub password: SecretString,
    pub database_name: String,
    #[serde(deserialize_with = "deserialize_number_from_string")]
    pub port: u16,
    pub host: String,
    pub require_ssl: bool,
}

impl DatabaseSettings {
    pub fn get_connection_uri(&self) -> PgConnectOptions {
        self.get_connection_uri_without_db_name()
            .database(&self.database_name)
            .log_statements(tracing::log::LevelFilter::Trace)
    }

    pub fn get_connection_uri_without_db_name(&self) -> PgConnectOptions {
        let ssl_mode = if self.require_ssl {
            PgSslMode::Require
        } else {
            PgSslMode::Prefer
        };
        PgConnectOptions::new()
            .username(&self.username)
            .password(self.password.expose_secret())
            .host(&self.host)
            .port(self.port)
            .ssl_mode(ssl_mode)
    }
}

pub fn get_configuration() -> Result<Settings, config::ConfigError> {
    let base_path = env::current_dir().expect("Failed to determine cd");
    let configuration_path = base_path.join("configuration");

    let environment: Environment = env::var("APP_ENVIRONMENT")
        .unwrap_or("local".into())
        .try_into()
        .expect("Failed to parse APP_ENVIRONMENT");
    let settings = Config::builder()
        .add_source(config::File::from(configuration_path.join("base")))
        .add_source(config::File::from(
            configuration_path.join(environment.as_str()),
        ))
        .add_source(
            config::Environment::with_prefix("app")
                .prefix_separator("_")
                .separator("__"),
        )
        .build()?;
    settings.try_deserialize::<Settings>()
}

pub enum Environment {
    Local,
    Production,
}

impl TryFrom<String> for Environment {
    type Error = String;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        match s.to_lowercase().as_str() {
            "local" => Ok(Self::Local),
            "production" => Ok(Self::Production),
            _ => Err(format!(
                "{s} is not a valid enviroment. Please use either `local` or `production`."
            )),
        }
    }
}

impl Environment {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Local => "local",
            Self::Production => "production",
        }
    }
}
