use anyhow::anyhow;
use std::fmt::{Debug, Display};

use actix_web::{
    HttpResponse,
    body::MessageBody,
    dev::{ServiceRequest, ServiceResponse},
    error::{ErrorInternalServerError, InternalError},
    http::header::LOCATION,
    middleware::Next,
};
use tracing::instrument;

use crate::session_state::TypedSession;

#[instrument(name = "checking user privilege", skip(req, next, session))]
pub async fn admin_protection(
    session: TypedSession,
    req: ServiceRequest,
    next: Next<impl MessageBody>,
) -> Result<ServiceResponse<impl MessageBody>, actix_web::Error> {
    if session.get_user_id().map_err(e500)?.is_some() {
        return next.call(req).await;
    } else {
        Err(InternalError::from_response(
            anyhow!("User is not logged in"),
            HttpResponse::SeeOther()
                .insert_header((LOCATION, "/login"))
                .finish(),
        )
        .into())
    }
}

fn e500<T>(error: T) -> actix_web::Error
where
    T: Debug + Display + 'static,
{
    ErrorInternalServerError(error)
}
