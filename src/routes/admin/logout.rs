use actix_web::{HttpResponse, web::ReqData};
use tracing::instrument;

use crate::{authentication::UserId, session_state::TypedSession, utils::see_other};

#[instrument(name = "Handling logout", skip(session))]
pub async fn logout(user_id: ReqData<UserId>, session: TypedSession) -> HttpResponse {
    session.purge();
    see_other("/")
}
