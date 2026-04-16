use std::net::TcpListener;

use actix_session::{SessionMiddleware, storage::RedisSessionStore};
use actix_web::{App, HttpServer, cookie::Key, dev::Server, middleware::from_fn, web};
use actix_web_flash_messages::{FlashMessagesFramework, storage::CookieMessageStore};
use secrecy::{ExposeSecret, SecretString};
use sqlx::{PgPool, Pool, Postgres, postgres::PgPoolOptions};
use tracing_actix_web::TracingLogger;
use url::Url;

use crate::{
    authentication::admin_protection,
    configuration::{DatabaseSettings, RateLimitSettings, Settings},
    email_client::EmailClient,
    issue_delivery_worker::Retries,
    rate_limit::rate_limit_protection,
    routes::{
        change_password, dashboard, health_check, home, issues, login, login_form, logout,
        newsletter_form, password_form, publish_newsletter, subscribe, subscriptions_confirm,
    },
};

pub struct ApplicationBaseUrl(pub String);

pub struct MaxRetries(pub Retries);

pub struct HmacSecret(pub SecretString);

#[allow(clippy::too_many_arguments)]
pub fn run(
    listener: TcpListener,
    connection_pool: PgPool,
    redis_client: redis::Client,
    email_client: EmailClient,
    application_base_url: String,
    max_retries: Retries,
    rate_limit: RateLimitSettings,
    valkey_session_store: RedisSessionStore,
    flash_secret: SecretString,
) -> Result<Server, std::io::Error> {
    let conn = web::Data::new(connection_pool);
    let redis_conn = web::Data::new(redis_client);
    let email_client = web::Data::new(email_client);
    let application_base_url = web::Data::new(ApplicationBaseUrl(application_base_url));
    let max_retries = web::Data::new(max_retries);
    let rate_limit = web::Data::new(rate_limit);

    let cookie_key =
        Key::from(&hex::decode(flash_secret.expose_secret()).expect("Invalid flash secret"));

    let message_store = CookieMessageStore::builder(cookie_key.clone()).build();
    let flash_framework = FlashMessagesFramework::builder(message_store).build();

    let server = HttpServer::new(move || {
        App::new()
            .wrap(TracingLogger::default())
            .wrap(flash_framework.clone())
            .wrap(from_fn(rate_limit_protection))
            .wrap(SessionMiddleware::new(
                valkey_session_store.clone(),
                cookie_key.clone(),
            ))
            .route("/", web::get().to(home))
            .route("/health", web::get().to(health_check))
            .route("/subscribe", web::post().to(subscribe))
            .route(
                "/subscriptions/confirm",
                web::get().to(subscriptions_confirm),
            )
            .route("/newsletter", web::post().to(publish_newsletter))
            .route("/login", web::get().to(login_form))
            .route("/login", web::post().to(login))
            .service(
                web::scope("/admin")
                    .wrap(from_fn(admin_protection))
                    .route("/dashboard", web::get().to(dashboard))
                    .route("/password", web::get().to(password_form))
                    .route("/password", web::post().to(change_password))
                    .route("/logout", web::post().to(logout))
                    .route("/newsletter", web::post().to(publish_newsletter))
                    .route("/newsletter", web::get().to(newsletter_form))
                    .route("/issues", web::get().to(issues)),
            )
            .app_data(conn.clone())
            .app_data(email_client.clone())
            .app_data(application_base_url.clone())
            .app_data(max_retries.clone())
            .app_data(redis_conn.clone())
            .app_data(rate_limit.clone())
    })
    .listen(listener)?
    .run();
    Ok(server)
}

pub struct Application {
    port: u16,
    server: Server,
}

impl Application {
    pub async fn build(configuration: &Settings) -> Result<Self, std::io::Error> {
        let connection = get_connection_pool(&configuration.database);

        let redis_client = redis::Client::open(configuration.valkey_uri.expose_secret())
            .expect("Failed to get redis client");

        let sender_email = configuration
            .email_client
            .sender()
            .expect("Invalid sender email in configuration");
        let timeout = configuration.email_client.timeout();
        let base_url =
            Url::parse(&configuration.email_client.base_url).expect("Invalid url in configuration");
        let email_client = EmailClient::new(
            base_url,
            sender_email,
            configuration.email_client.authorization_token.clone(),
            timeout,
        );

        let application_address = format!(
            "{}:{}",
            configuration.application.host, configuration.application.port
        );
        let application_base_url = configuration.application.base_url.clone();
        let listener = TcpListener::bind(application_address)?;
        let port = listener.local_addr().expect("should be valid").port();
        let flash_secret = configuration.application.flash_secret.clone();
        let max_retries = Retries::from_u8(configuration.application.max_retries);
        let rate_limit = configuration.application.rate_limit.clone();
        let valkey_store = RedisSessionStore::new(configuration.valkey_uri.expose_secret())
            .await
            .expect("Failed to connect to Valkey");
        let server = run(
            listener,
            connection,
            redis_client,
            email_client,
            application_base_url,
            max_retries,
            rate_limit,
            valkey_store,
            flash_secret,
        )?;

        Ok(Self { port, server })
    }
    pub fn port(&self) -> u16 {
        self.port
    }

    pub async fn run_until_stopped(self) -> Result<(), std::io::Error> {
        self.server.await?;
        Ok(())
    }
}

pub fn get_connection_pool(database_settings: &DatabaseSettings) -> Pool<Postgres> {
    let psql_connection_uri = database_settings.get_connection_uri();
    PgPoolOptions::new().connect_lazy_with(psql_connection_uri)
}
