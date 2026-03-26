use actix_web::{HttpResponse, http::header::LOCATION, web::ReqData};
use tracing::instrument;

use crate::{authentication::UserId, session_state::TypedSession};

#[instrument(name = "Handling logout", skip(session))]
pub async fn logout(user_id: ReqData<UserId>, session: TypedSession) -> HttpResponse {
    session.purge();
    HttpResponse::SeeOther()
        .insert_header((LOCATION, "/"))
        .finish()
}
