use actix_web::{HttpResponse, body::to_bytes, http::StatusCode};
use anyhow::{Context, anyhow};
use sqlx::{Postgres, Transaction};
use tracing::instrument;

use crate::{authentication::UserId, idempotency::key::IndempotencyKey};

#[derive(Debug, sqlx::Type)]
#[sqlx(type_name = "header_pair")]
struct HeaderPairRecord {
    name: String,
    value: Vec<u8>,
}

pub type PgTransaction = Transaction<'static, Postgres>;

#[derive(Debug)]
pub enum NextAction {
    ProcessEmails,
    RetrieveResponse,
}

#[instrument(name = "Initialise response", skip(tx))]
pub async fn initialise_response(
    user_id: &UserId,
    idempotency_key: &IndempotencyKey,
    tx: &mut PgTransaction,
) -> Result<NextAction, anyhow::Error> {
    let n = sqlx::query!(
        r#"
    INSERT INTO idempotency (
      user_id,
      idempotency_key
    ) VALUES (
      $1,
      $2
    ) 
    ON CONFLICT DO NOTHING;
    "#,
        **user_id,
        idempotency_key.as_ref()
    )
    .execute(&mut **tx)
    .await?;
    if n.rows_affected() > 0 {
        Ok(NextAction::ProcessEmails)
    } else {
        Ok(NextAction::RetrieveResponse)
    }
}

#[instrument(name = "Saving response", skip(tx, response))]
pub async fn save_response(
    user_id: &UserId,
    idempotency_key: &IndempotencyKey,
    response: HttpResponse,
    tx: &mut PgTransaction,
) -> Result<HttpResponse, anyhow::Error> {
    let (response_head, body) = response.into_parts();
    let status: i16 = response_head.status().as_u16().try_into()?;
    let headers: Vec<HeaderPairRecord> = response_head
        .headers()
        .iter()
        .map(|(header_name, header_value)| {
            let value: Vec<u8> = header_value.to_str()?.into();
            Ok(HeaderPairRecord {
                name: header_name.to_string(),
                value,
            })
        })
        .collect::<Result<Vec<HeaderPairRecord>, anyhow::Error>>()?;
    let body: Vec<u8> = to_bytes(body)
        .await
        .map_err(|e| anyhow!(e.to_string()))?
        .to_vec();

    sqlx::query_unchecked!(
        r#"
    UPDATE
        idempotency 
    SET 
        response_status_code=$3,
        response_headers=$4,
        response_body=$5
    WHERE 
        user_id=$1 AND idempotency_key=$2
    "#,
        **user_id,
        idempotency_key.as_ref(),
        status,
        headers,
        body
    )
    .execute(&mut **tx)
    .await?;

    let http_response = response_head.set_body(body).map_into_boxed_body();
    Ok(http_response)
}

#[instrument(name = "Getting saved response", skip(tx))]
pub async fn get_saved_response(
    user_id: &UserId,
    idempotency_key: &IndempotencyKey,
    tx: &mut PgTransaction,
) -> Result<Option<HttpResponse>, anyhow::Error> {
    let record = sqlx::query!(
        r#"
            SELECT 
                response_status_code as "response_status_code?",
                response_headers as "response_headers?: Vec<HeaderPairRecord>",
                response_body as "response_body?"
        FROM idempotency
        WHERE 
            user_id = $1 AND idempotency_key = $2;
    "#,
        **user_id,
        idempotency_key.as_ref()
    )
    .fetch_optional(&mut **tx)
    .await?;

    if let Some(r) = record {
        let response_status_code = r
            .response_status_code
            .context("Previous response not completed")?;
        let response_headers = r
            .response_headers
            .context("Previous response not completed")?;
        let response_body = r.response_body.context("Previous response not completed")?;

        let status_code = StatusCode::from_u16(response_status_code.try_into()?)?;
        let mut response = HttpResponse::build(status_code);
        for HeaderPairRecord { name, value } in response_headers {
            response.append_header((name, value));
        }
        Ok(Some(response.body(response_body)))
    } else {
        return Ok(None);
    }
}
