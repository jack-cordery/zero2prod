use actix_web::{HttpResponse, http::header::ContentType};

pub async fn home() -> HttpResponse {
    let home_html = include_str!("home.html");

    HttpResponse::Ok()
        .content_type(ContentType::html())
        .body(home_html)
}
