use actix_web::{HttpResponse, web};
use serde::Deserialize;
use tracing::instrument;

#[derive(Deserialize)]
pub struct Parameters {
    subscription_token: String,
}

/// so this will harbour the code for the /subscriptions_confirm end point
#[instrument(name = "Handling confirmation request", skip(parameters))]
pub async fn subscriptions_confirm(parameters: web::Query<Parameters>) -> HttpResponse {
    //lets start off with just returning 200
    //now we need to add in some kind of get the url and parse the token
    //then check we have the token change it in the db
    //and then return 200
    HttpResponse::Ok().finish()
}
