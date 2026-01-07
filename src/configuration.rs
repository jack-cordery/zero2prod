use std::env;

use config::Config;
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;

use serde_aux::field_attributes::deserialize_number_from_string;
use sqlx::postgres::PgConnectOptions;

#[derive(Deserialize)]
pub struct Settings {
    pub database: DatabaseSettings,
    pub application: ApplicationSettings,
}

#[derive(Deserialize)]
pub struct ApplicationSettings {
    pub host: String,
    #[serde(deserialize_with = "deserialize_number_from_string")]
    pub port: u16,
}

#[derive(Deserialize)]
pub struct DatabaseSettings {
    pub username: String,
    pub password: SecretString,
    pub database_name: String,
    #[serde(deserialize_with = "deserialize_number_from_string")]
    pub port: u16,
    pub host: String,
}

impl DatabaseSettings {
    pub fn get_connection_uri(&self) -> PgConnectOptions {
        self.get_connection_uri_without_db_name()
            .database(&self.database_name)
    }

    pub fn get_connection_uri_without_db_name(&self) -> PgConnectOptions {
        PgConnectOptions::new()
            .username(&self.username)
            .password(self.password.expose_secret())
            .host(&self.host)
            .port(self.port)
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
        .add_source(config::Environment::with_prefix("app").separator("__"))
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
