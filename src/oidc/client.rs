// in here we want to build the struct of a OidcClient

use anyhow::{Context, anyhow};
use openidconnect::{
    AuthorizationCode, CsrfToken, EndUserEmail, EndpointMaybeSet, EndpointNotSet, EndpointSet,
    IssuerUrl, Nonce, PkceCodeChallenge, PkceCodeVerifier, RedirectUrl, Scope,
    core::{CoreClient, CoreIdTokenVerifier, CoreProviderMetadata, CoreResponseType},
};
use redis::{
    AsyncTypedCommands, ExistenceCheck, SetExpiry, SetOptions, aio::MultiplexedConnection,
};
use sqlx::PgPool;
use url::Url;

use crate::{
    authentication::authorize_email, oidc::OidcHttpClient, routes::LoginError,
    session_state::TypedSession,
};

pub type CoreClientPlus = CoreClient<
    EndpointSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointMaybeSet,
    EndpointMaybeSet,
>;

pub struct OidcClient {
    pub client: CoreClientPlus,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct RedisValue {
    pkce_verifier: String,
    nonce: String,
}

impl OidcClient {
    const CSRF_NAMESPACE: &'static str = "csrf";
    const SCOPE_EMAIL: &'static str = "email";
    const SCOPE_PROFILE: &'static str = "profile";
    const SCOPE_OPENID: &'static str = "openid";
    const KEY_EXPIRY_SECONDS: &'static u64 = &600;

    pub async fn new(
        client_id: openidconnect::ClientId,
        client_secret: openidconnect::ClientSecret,
        issuer_url: IssuerUrl,
        http_client: &OidcHttpClient,
        redirect_url: RedirectUrl,
    ) -> Self {
        let provider_metadata =
            CoreProviderMetadata::discover_async(issuer_url, &http_client.client)
                .await
                .expect("Failed to retrieve dicsover oidc metadata");

        Self {
            client: CoreClient::from_provider_metadata(
                provider_metadata,
                client_id,
                Some(client_secret),
            )
            .set_redirect_uri(redirect_url),
        }
    }

    pub async fn persist_tokens(
        pkce: PkceCodeVerifier,
        csrf: CsrfToken,
        nonce: Nonce,
        redis_conn: &mut MultiplexedConnection,
    ) -> Result<(), anyhow::Error> {
        let mut opts = SetOptions::default();
        opts = opts
            .conditional_set(ExistenceCheck::NX)
            .with_expiration(SetExpiry::EX(*Self::KEY_EXPIRY_SECONDS));

        let csrf_key = format!("{}::{}", Self::CSRF_NAMESPACE, csrf.secret());

        let value = RedisValue {
            pkce_verifier: pkce.into_secret(),
            nonce: nonce.secret().to_owned(),
        };
        let value = serde_json::to_string(&value).context("Failed to encode tokens to json")?;

        let _ = redis_conn
            .set_options(csrf_key, value, opts.clone())
            .await
            .context("Failed to set pkce key")?
            .context("PKCE key already exists")?;

        Ok(())
    }

    pub async fn handle_login(
        &self,
        redis_conn: &mut MultiplexedConnection,
    ) -> Result<Url, LoginError> {
        let (pkce_code_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();
        let (authorize_url, csrf_state, nonce) = self
            .client
            .authorize_url(
                openidconnect::AuthenticationFlow::<CoreResponseType>::AuthorizationCode,
                CsrfToken::new_random,
                Nonce::new_random,
            )
            .add_scope(Scope::new(Self::SCOPE_EMAIL.to_string()))
            .add_scope(Scope::new(Self::SCOPE_PROFILE.to_string()))
            .add_scope(Scope::new(Self::SCOPE_OPENID.to_string()))
            .set_pkce_challenge(pkce_code_challenge)
            .url();

        Self::persist_tokens(pkce_verifier, csrf_state, nonce, redis_conn)
            .await
            .context("Failed to persist tokens")?;

        Ok(authorize_url)
    }

    pub async fn handle_callback(
        &self,
        security_token: CsrfToken,
        access_code: AuthorizationCode,
        http_client: &OidcHttpClient,
        pool: &PgPool,
        session: TypedSession,
        redis_conn: &mut MultiplexedConnection,
    ) -> Result<(), LoginError> {
        let value: RedisValue = Self::validate_csrf_token(security_token, redis_conn).await?;
        let pkce_verifier = PkceCodeVerifier::new(value.pkce_verifier);
        let nonce = Nonce::new(value.nonce);

        let token_response = self
            .client
            .exchange_code(access_code)
            .context("Error exchanging authorization code")?
            .set_pkce_verifier(pkce_verifier)
            .request_async(&http_client.client)
            .await
            .context("Error verifying authorization code exchange")?;

        let id_token_verifier: CoreIdTokenVerifier = self.client.id_token_verifier();
        let email: &EndUserEmail = token_response
            .extra_fields()
            .id_token()
            .context("No id token provided")?
            .claims(&id_token_verifier, &nonce)
            .context("Invalid or no claims provided")?
            .email()
            .context("Server did not return email")?;

        let user_id = authorize_email(email.to_string(), pool)
            .await
            .context("Failed to authorize the user email")
            .map_err(LoginError::AuthError)?;

        session.renew();
        session
            .insert_user_id(user_id)
            .context("Failed to insert session into valkey")?;

        Ok(())
    }

    async fn validate_csrf_token(
        received: CsrfToken,
        redis_conn: &mut MultiplexedConnection,
    ) -> Result<RedisValue, anyhow::Error> {
        let key = format!("{}::{}", Self::CSRF_NAMESPACE, received.secret());
        match redis_conn
            .get(key)
            .await
            .context("Failed to get csrf token from redis")?
        {
            Some(value) => {
                let value: RedisValue =
                    serde_json::from_str(&value).context("Invalid stored tokens")?;
                Ok(value)
            }
            None => Err(anyhow!("Invalid CSRF token")),
        }
    }
}
