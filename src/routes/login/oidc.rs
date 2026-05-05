use actix_web::{HttpResponse, error::InternalError, web};
use anyhow::Context;
use openidconnect::{AuthorizationCode, CsrfToken};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use crate::{
    oidc::{OidcClient, OidcHttpClient},
    routes::{
        REDIRECT_SUCCESSFUL_LOGIN,
        login::post::{LoginError, login_redirect},
    },
    session_state::TypedSession,
    utils::see_other,
};

#[derive(Deserialize, Debug, Serialize)]
pub struct CallbackQuery {
    state: String,
    code: String,
    scope: String,
}

impl CallbackQuery {
    pub fn new(state: String, code: String, scope: String) -> Self {
        Self { state, code, scope }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct State {
    security_token: String,
    url: String,
}

impl State {
    pub fn new(security_token: String, url: String) -> Self {
        Self {
            security_token,
            url,
        }
    }
}

pub async fn initiate_google_login(
    client: web::Data<OidcClient>,
    redis_client: web::Data<redis::Client>,
) -> Result<HttpResponse, InternalError<LoginError>> {
    let client = client.as_ref();
    let mut redis_conn = redis_client
        .get_multiplexed_async_connection()
        .await
        .context("Failed to establish redis connection")
        .map_err(|e| {
            let error = LoginError::UnexpectedError(e);
            login_redirect(error)
        })?;
    let authorize_url = client
        .handle_login(&mut redis_conn)
        .await
        .map_err(login_redirect)?;
    Ok(see_other(authorize_url.as_str()))
}

pub async fn callback(
    query: web::Query<CallbackQuery>,
    openid_client: web::Data<OidcClient>,
    http_client: web::Data<OidcHttpClient>,
    session: TypedSession,
    pool: actix_web::web::Data<PgPool>,
    redis_client: web::Data<redis::Client>,
) -> Result<HttpResponse, InternalError<LoginError>> {
    let openid_client = &openid_client.as_ref();
    let mut redis_conn = redis_client
        .as_ref()
        .get_multiplexed_async_connection()
        .await
        .context("Failed to establish redis connection")
        .map_err(|e| {
            let error = LoginError::UnexpectedError(e);
            login_redirect(error)
        })?;

    let state: State = serde_urlencoded::from_str(&query.state)
        .context("misformed state")
        .map_err(|e| {
            let error = LoginError::UnexpectedError(e);
            login_redirect(error)
        })?;
    let access_code = AuthorizationCode::new(query.code.clone());

    let security_token = CsrfToken::new(state.security_token);

    openid_client
        .handle_callback(
            security_token,
            access_code,
            &http_client,
            &pool,
            session,
            &mut redis_conn,
        )
        .await
        .map_err(login_redirect)
        .inspect_err(|e| {
            dbg!(e);
        })?;

    Ok(see_other(REDIRECT_SUCCESSFUL_LOGIN))
}
