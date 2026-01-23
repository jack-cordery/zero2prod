use std::net::TcpListener;

use actix_web::{App, HttpRequest, HttpServer, Responder, dev::Server, web};
use sqlx::{PgPool, Pool, Postgres, postgres::PgPoolOptions};
use tracing_actix_web::TracingLogger;
use url::Url;

use crate::{
    configuration::{DatabaseSettings, Settings},
    email_client::EmailClient,
    routes::{health_check, subscribe, subscriptions_confirm},
};

async fn greet(req: HttpRequest) -> impl Responder {
    let name = req.match_info().get("name").unwrap_or("World");
    format!("Hello {name}")
}

pub struct ApplicationBaseUrl(pub String);

pub fn run(
    listener: TcpListener,
    connection_pool: PgPool,
    email_client: EmailClient,
    application_base_url: String,
) -> Result<Server, std::io::Error> {
    let conn = web::Data::new(connection_pool);
    let email_client = web::Data::new(email_client);
    let application_base_url = web::Data::new(ApplicationBaseUrl(application_base_url));

    let server = HttpServer::new(move || {
        App::new()
            .wrap(TracingLogger::default())
            .route("/", web::get().to(greet))
            .route("/health", web::get().to(health_check))
            .route("/subscribe", web::post().to(subscribe))
            .route(
                "/subscriptions/confirm",
                web::get().to(subscriptions_confirm),
            )
            .app_data(conn.clone())
            .app_data(email_client.clone())
            .app_data(application_base_url.clone())
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
        let server = run(listener, connection, email_client, application_base_url)?;

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
