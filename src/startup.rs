use std::net::TcpListener;

use actix_web::{App, HttpRequest, HttpServer, Responder, dev::Server, web};
use sqlx::PgPool;
use tracing_actix_web::TracingLogger;

use crate::routes::{health_check, subscribe};

async fn greet(req: HttpRequest) -> impl Responder {
    let name = req.match_info().get("name").unwrap_or("World");
    format!("Hello {name}")
}

pub fn run(listener: TcpListener, connection_pool: PgPool) -> Result<Server, std::io::Error> {
    let conn = web::Data::new(connection_pool);

    let server = HttpServer::new(move || {
        App::new()
            .wrap(TracingLogger::default())
            .route("/", web::get().to(greet))
            .route("/health", web::get().to(health_check))
            .route("/subscribe", web::post().to(subscribe))
            .app_data(conn.clone())
    })
    .listen(listener)?
    .run();
    Ok(server)
}
