use anyhow::anyhow;
use std::{
    fmt::{Debug, Display},
    ops::Deref,
};
use uuid::Uuid;

use actix_web::{
    HttpMessage,
    body::MessageBody,
    dev::{ServiceRequest, ServiceResponse},
    error::{ErrorInternalServerError, InternalError},
    middleware::Next,
};
use tracing::instrument;

use crate::{session_state::TypedSession, utils::see_other};

#[derive(Debug, Clone)]
pub struct UserId(Uuid);

impl Deref for UserId {
    type Target = Uuid;

    fn deref(&self) -> &Self::Target {
        self.0.as_ref()
    }
}

#[instrument(name = "checking user privilege", skip(req, next, session))]
pub async fn admin_protection(
    session: TypedSession,
    mut req: ServiceRequest,
    next: Next<impl MessageBody>,
) -> Result<ServiceResponse<impl MessageBody>, actix_web::Error> {
    if let Some(user_id) = session.get_user_id().map_err(e500)? {
        let (request, _) = req.parts_mut();
        request.extensions_mut().insert(UserId(user_id));
        return next.call(req).await;
    } else {
        Err(
            InternalError::from_response(anyhow!("User is not logged in"), see_other("/login"))
                .into(),
        )
    }
}

fn e500<T>(error: T) -> actix_web::Error
where
    T: Debug + Display + 'static,
{
    ErrorInternalServerError(error)
}
