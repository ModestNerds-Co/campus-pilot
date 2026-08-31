//! Exposes person-scoped Agent usage without campus-wide administration data.

use std::str::FromStr;

use actix_web::{
    HttpResponse, get,
    http::{StatusCode, header},
    web::{self, ServiceConfig},
};
use cp_agent_runtime::{
    AgentUsageError, AgentUsageMeter, AgentUsageReportCursor, AgentUsageReportDimension,
    AgentUsageReportQuery,
};
use cp_common::{ApiResponse, TenantId};
use serde_json::{Value, json};

use crate::{services::auth::models::User, state::AppState};

use super::usage_dtos::PersonalUsageRequest;

#[get("/usage/personal")]
async fn personal_usage(
    state: web::Data<AppState>,
    tenant: web::ReqData<TenantId>,
    user: web::ReqData<User>,
    query: web::Query<PersonalUsageRequest>,
) -> HttpResponse {
    let query = match personal_usage_query(user.id, query.into_inner()) {
        Ok(query) => query,
        Err(error) => return usage_error(error),
    };
    match state
        .agent_usage_runtime
        .report(tenant.into_inner().0, query)
        .await
    {
        Ok(page) => HttpResponse::Ok()
            .insert_header((header::CACHE_CONTROL, "no-store"))
            .json(ApiResponse::from_status(StatusCode::OK, Some(page), None)),
        Err(error) => usage_error(error),
    }
}

fn personal_usage_query(
    user_id: uuid::Uuid,
    request: PersonalUsageRequest,
) -> Result<AgentUsageReportQuery, AgentUsageError> {
    let meter = request
        .meter
        .as_deref()
        .map(AgentUsageMeter::from_str)
        .transpose()?;
    let currency = match (
        request.currency.as_deref(),
        request.currency_exponent,
        request.pricing_version.as_deref(),
    ) {
        (None, None, None) => None,
        (Some(currency), Some(exponent), pricing_version) => {
            Some((currency, exponent, pricing_version))
        }
        _ => {
            return Err(AgentUsageError::Invalid {
                code: "invalid_currency_filter",
            });
        }
    };
    let cursor = match (
        request.cursor_occurred_at,
        request.cursor_event_id,
        request.cursor_meter.as_deref(),
    ) {
        (None, None, None) => None,
        (Some(occurred_at), Some(event_id), Some(meter)) => Some(AgentUsageReportCursor {
            occurred_at,
            event_id,
            meter: AgentUsageMeter::from_str(meter)?,
        }),
        _ => {
            return Err(AgentUsageError::Invalid {
                code: "invalid_usage_cursor",
            });
        }
    };
    AgentUsageReportQuery::parse(
        AgentUsageReportDimension::Person(user_id),
        meter,
        currency,
        cursor,
        request.limit,
    )
}

fn usage_error(error: AgentUsageError) -> HttpResponse {
    let status = match &error {
        AgentUsageError::Invalid { .. } => StatusCode::BAD_REQUEST,
        AgentUsageError::NotFound => StatusCode::NOT_FOUND,
        AgentUsageError::Denied { .. } => StatusCode::TOO_MANY_REQUESTS,
        AgentUsageError::IdempotencyConflict
        | AgentUsageError::MissingDemand
        | AgentUsageError::CurrencyMismatch
        | AgentUsageError::InvalidTransition => StatusCode::CONFLICT,
        AgentUsageError::Storage => StatusCode::SERVICE_UNAVAILABLE,
    };
    if matches!(&error, AgentUsageError::Storage) {
        log::error!("Personal Agent usage could not be loaded");
    }
    HttpResponse::build(status)
        .insert_header((header::CACHE_CONTROL, "no-store"))
        .json(ApiResponse::<Value>::from_status(
            status,
            Some(json!({ "code": error.code() })),
            Some(vec!["Agent usage could not be loaded".to_owned()]),
        ))
}

pub fn routes(cfg: &mut ServiceConfig) {
    cfg.service(personal_usage);
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use uuid::Uuid;

    use super::{PersonalUsageRequest, personal_usage_query};

    #[test]
    fn personal_query_never_accepts_another_person_dimension() {
        let user_id = Uuid::new_v4();
        assert!(
            personal_usage_query(
                user_id,
                PersonalUsageRequest {
                    meter: Some("agent.input_tokens".to_owned()),
                    currency: None,
                    currency_exponent: None,
                    pricing_version: None,
                    cursor_occurred_at: Some(Utc::now()),
                    cursor_event_id: Some(Uuid::new_v4()),
                    cursor_meter: Some("agent.input_tokens".to_owned()),
                    limit: Some(100),
                },
            )
            .is_ok()
        );
    }

    #[test]
    fn partial_currency_and_cursor_tuples_fail_closed() {
        for request in [
            PersonalUsageRequest {
                meter: Some("agent.estimated_cost".to_owned()),
                currency: Some("USD".to_owned()),
                currency_exponent: None,
                pricing_version: None,
                cursor_occurred_at: None,
                cursor_event_id: None,
                cursor_meter: None,
                limit: None,
            },
            PersonalUsageRequest {
                meter: None,
                currency: None,
                currency_exponent: None,
                pricing_version: None,
                cursor_occurred_at: Some(Utc::now()),
                cursor_event_id: None,
                cursor_meter: None,
                limit: None,
            },
        ] {
            assert!(personal_usage_query(Uuid::new_v4(), request).is_err());
        }
    }
}
