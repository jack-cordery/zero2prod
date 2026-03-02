use actix_web::HttpResponse;

pub async fn home() -> HttpResponse {
    let home_html = include_str!("home.html");

    HttpResponse::Ok().body(home_html)
}
