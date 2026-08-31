//! Refined inputs and reduced projections for Agent usage enforcement.

use std::{collections::BTreeMap, str::FromStr, time::Duration};

use chrono::{DateTime, Utc};
use serde::Serialize;
use thiserror::Error;
use uuid::Uuid;

const MAX_SAFE_INTEGER: i64 = 9_007_199_254_740_991;
const MAX_IDEMPOTENCY_KEY_LENGTH: usize = 200;
const MAX_KEY_LENGTH: usize = 240;
const MAX_PRICING_VERSION_LENGTH: usize = 100;
const MAX_REPORT_LIMIT: u16 = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentUsageMeter {
    Runs,
    ProviderAttempts,
    CapabilityCalls,
    InputTokens,
    OutputTokens,
    CachedInputTokens,
    ReasoningTokens,
    ProviderReportedCost,
    EstimatedCost,
}

impl AgentUsageMeter {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Runs => "agent.runs",
            Self::ProviderAttempts => "agent.provider_attempts",
            Self::CapabilityCalls => "agent.capability_calls",
            Self::InputTokens => "agent.input_tokens",
            Self::OutputTokens => "agent.output_tokens",
            Self::CachedInputTokens => "agent.cached_input_tokens",
            Self::ReasoningTokens => "agent.reasoning_tokens",
            Self::ProviderReportedCost => "agent.provider_reported_cost",
            Self::EstimatedCost => "agent.estimated_cost",
        }
    }

    pub(crate) const fn is_money(self) -> bool {
        matches!(self, Self::ProviderReportedCost | Self::EstimatedCost)
    }
}

impl FromStr for AgentUsageMeter {
    type Err = AgentUsageError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "agent.runs" => Ok(Self::Runs),
            "agent.provider_attempts" => Ok(Self::ProviderAttempts),
            "agent.capability_calls" => Ok(Self::CapabilityCalls),
            "agent.input_tokens" => Ok(Self::InputTokens),
            "agent.output_tokens" => Ok(Self::OutputTokens),
            "agent.cached_input_tokens" => Ok(Self::CachedInputTokens),
            "agent.reasoning_tokens" => Ok(Self::ReasoningTokens),
            "agent.provider_reported_cost" => Ok(Self::ProviderReportedCost),
            "agent.estimated_cost" => Ok(Self::EstimatedCost),
            _ => Err(AgentUsageError::storage_contract()),
        }
    }
}

/// One indivisible currency tuple. No runtime API converts between tuples.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub struct AgentMoney {
    pub amount: i64,
    pub currency: String,
    pub exponent: i16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pricing_version: Option<String>,
}

impl AgentMoney {
    pub fn parse(
        amount: u64,
        currency: &str,
        exponent: u8,
        pricing_version: Option<&str>,
    ) -> Result<Self, AgentUsageError> {
        let amount = safe_u64(amount, "invalid_money_amount")?;
        if currency.len() != 3 || !currency.bytes().all(|value| value.is_ascii_uppercase()) {
            return Err(AgentUsageError::invalid("invalid_currency"));
        }
        if exponent > 9 {
            return Err(AgentUsageError::invalid("invalid_currency_exponent"));
        }
        let pricing_version = pricing_version
            .map(|value| bounded_text(value, MAX_PRICING_VERSION_LENGTH, "invalid_pricing_version"))
            .transpose()?;
        Ok(Self {
            amount,
            currency: currency.to_owned(),
            exponent: i16::from(exponent),
            pricing_version,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentUsageDemand {
    pub(crate) meter: AgentUsageMeter,
    pub(crate) amount: i64,
    pub(crate) money: Option<AgentMoney>,
}

impl AgentUsageDemand {
    pub fn count(meter: AgentUsageMeter, amount: u64) -> Result<Self, AgentUsageError> {
        if meter.is_money() {
            return Err(AgentUsageError::invalid("money_tuple_required"));
        }
        let amount = positive_safe_u64(amount, "invalid_usage_amount")?;
        Ok(Self {
            meter,
            amount,
            money: None,
        })
    }

    pub fn money(meter: AgentUsageMeter, money: AgentMoney) -> Result<Self, AgentUsageError> {
        if !meter.is_money() || money.amount <= 0 {
            return Err(AgentUsageError::invalid("invalid_money_demand"));
        }
        if meter == AgentUsageMeter::EstimatedCost && money.pricing_version.is_none() {
            return Err(AgentUsageError::invalid(
                "estimated_cost_requires_pricing_version",
            ));
        }
        Ok(Self {
            meter,
            amount: money.amount,
            money: Some(money),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentUsageStage {
    Run,
    ProviderAttempt { attempt_id: Uuid },
    CapabilityCall { call_id: Uuid },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrepareAgentUsage {
    pub(crate) run_id: Uuid,
    pub(crate) stage: AgentUsageStage,
    pub(crate) idempotency_key: String,
    pub(crate) request_fingerprint: [u8; 32],
    pub(crate) demands: BTreeMap<AgentUsageMeter, AgentUsageDemand>,
    pub(crate) ttl: Duration,
}

impl PrepareAgentUsage {
    pub fn parse(
        run_id: Uuid,
        stage: AgentUsageStage,
        idempotency_key: &str,
        request_fingerprint: [u8; 32],
        demands: impl IntoIterator<Item = AgentUsageDemand>,
        ttl: Duration,
    ) -> Result<Self, AgentUsageError> {
        if run_id.is_nil() || request_fingerprint == [0; 32] {
            return Err(AgentUsageError::invalid("invalid_usage_identity"));
        }
        let idempotency_key = parse_idempotency_key(idempotency_key)?;
        if !(Duration::from_secs(1)..=Duration::from_secs(15 * 60)).contains(&ttl) {
            return Err(AgentUsageError::invalid("invalid_reservation_ttl"));
        }
        let mut indexed = BTreeMap::new();
        for demand in demands {
            if indexed.insert(demand.meter, demand).is_some() {
                return Err(AgentUsageError::invalid("duplicate_usage_meter"));
            }
        }
        let required = match stage {
            AgentUsageStage::Run => AgentUsageMeter::Runs,
            AgentUsageStage::ProviderAttempt { attempt_id } => {
                if attempt_id.is_nil() {
                    return Err(AgentUsageError::invalid("invalid_provider_attempt_id"));
                }
                AgentUsageMeter::ProviderAttempts
            }
            AgentUsageStage::CapabilityCall { call_id } => {
                if call_id.is_nil() {
                    return Err(AgentUsageError::invalid("invalid_capability_call_id"));
                }
                AgentUsageMeter::CapabilityCalls
            }
        };
        if indexed.get(&required).map(|value| value.amount) != Some(1) {
            return Err(AgentUsageError::invalid("stage_count_must_equal_one"));
        }
        Ok(Self {
            run_id,
            stage,
            idempotency_key,
            request_fingerprint,
            demands: indexed,
            ttl,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentUsageReservationStatus {
    NotLimited,
    Reserved,
    Committed,
    Released,
    Expired,
    Denied,
}

impl FromStr for AgentUsageReservationStatus {
    type Err = AgentUsageError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "not_limited" => Ok(Self::NotLimited),
            "reserved" => Ok(Self::Reserved),
            "committed" => Ok(Self::Committed),
            "released" => Ok(Self::Released),
            "expired" => Ok(Self::Expired),
            "denied" => Ok(Self::Denied),
            _ => Err(AgentUsageError::storage_contract()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PreparedAgentUsage {
    pub reservation_id: Uuid,
    pub status: AgentUsageReservationStatus,
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentUsageTerminalAction {
    Release,
    Expire,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentUsageReportDimension {
    Person(Uuid),
    OriginModule(String),
    CapabilityModule(String),
    Capability(String),
    Provider(String),
    Model { provider: String, model: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct AgentUsageReportCursor {
    pub occurred_at: DateTime<Utc>,
    pub event_id: Uuid,
    pub meter: AgentUsageMeter,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentUsageReportQuery {
    pub(crate) dimension: AgentUsageReportDimension,
    pub(crate) meter: Option<AgentUsageMeter>,
    pub(crate) currency: Option<(String, i16, Option<String>)>,
    pub(crate) cursor: Option<AgentUsageReportCursor>,
    pub(crate) limit: i64,
}

impl AgentUsageReportQuery {
    pub fn parse(
        dimension: AgentUsageReportDimension,
        meter: Option<AgentUsageMeter>,
        currency: Option<(&str, u8, Option<&str>)>,
        cursor: Option<AgentUsageReportCursor>,
        limit: Option<u16>,
    ) -> Result<Self, AgentUsageError> {
        let dimension = parse_dimension(dimension)?;
        let currency = currency
            .map(|(code, exponent, pricing)| {
                AgentMoney::parse(0, code, exponent, pricing)
                    .map(|money| (money.currency, money.exponent, money.pricing_version))
            })
            .transpose()?;
        if currency.is_some() && !meter.is_some_and(AgentUsageMeter::is_money) {
            return Err(AgentUsageError::invalid(
                "currency_filter_requires_money_meter",
            ));
        }
        let limit = limit.unwrap_or(50);
        if limit == 0 || limit > MAX_REPORT_LIMIT {
            return Err(AgentUsageError::invalid("invalid_report_limit"));
        }
        Ok(Self {
            dimension,
            meter,
            currency,
            cursor,
            limit: i64::from(limit),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AgentUsageReportRow {
    pub event_id: Uuid,
    pub event_kind: String,
    pub outcome: String,
    pub run_id: Uuid,
    pub actor_user_id: Uuid,
    pub origin_module_key: String,
    pub capability_module_key: Option<String>,
    pub capability_key: Option<String>,
    pub provider_key: Option<String>,
    pub provider_model_id: Option<String>,
    pub meter: AgentUsageMeter,
    pub amount: Option<i64>,
    pub enforcement_amount: Option<i64>,
    pub enforcement_basis: Option<String>,
    pub currency_code: Option<String>,
    pub currency_exponent: Option<i16>,
    pub pricing_version: Option<String>,
    pub occurred_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AgentUsageReportPage {
    pub items: Vec<AgentUsageReportRow>,
    pub next_cursor: Option<AgentUsageReportCursor>,
}

#[derive(Debug, Error)]
pub enum AgentUsageError {
    #[error("invalid Agent usage input: {code}")]
    Invalid { code: &'static str },
    #[error("Agent usage idempotency identity conflicts with stored state")]
    IdempotencyConflict,
    #[error("current Agent identity or durable stage was not found")]
    NotFound,
    #[error("a required hard-limit meter was not supplied")]
    MissingDemand,
    #[error("hard limits contain incompatible currency tuples")]
    CurrencyMismatch,
    #[error("Agent usage has been denied by a hard limit")]
    Denied { reservation_id: Uuid },
    #[error("Agent usage state transition is not allowed")]
    InvalidTransition,
    #[error("Agent usage persistence contract failed")]
    Storage,
}

impl AgentUsageError {
    pub(crate) const fn invalid(code: &'static str) -> Self {
        Self::Invalid { code }
    }

    pub(crate) const fn storage_contract() -> Self {
        Self::Storage
    }

    pub fn code(&self) -> &'static str {
        match self {
            Self::Invalid { code } => code,
            Self::IdempotencyConflict => "agent_usage_idempotency_conflict",
            Self::NotFound => "agent_usage_not_found",
            Self::MissingDemand => "agent_usage_missing_demand",
            Self::CurrencyMismatch => "agent_usage_currency_mismatch",
            Self::Denied { .. } => "agent_usage_limit_denied",
            Self::InvalidTransition => "agent_usage_invalid_transition",
            Self::Storage => "agent_usage_storage_error",
        }
    }
}

fn parse_dimension(
    dimension: AgentUsageReportDimension,
) -> Result<AgentUsageReportDimension, AgentUsageError> {
    match dimension {
        AgentUsageReportDimension::Person(id) if id.is_nil() => {
            Err(AgentUsageError::invalid("invalid_person_id"))
        }
        AgentUsageReportDimension::OriginModule(value) => Ok(
            AgentUsageReportDimension::OriginModule(stable_key(&value, 160)?),
        ),
        AgentUsageReportDimension::CapabilityModule(value) => Ok(
            AgentUsageReportDimension::CapabilityModule(stable_key(&value, 160)?),
        ),
        AgentUsageReportDimension::Capability(value) => Ok(AgentUsageReportDimension::Capability(
            stable_key(&value, 200)?,
        )),
        AgentUsageReportDimension::Provider(value) => Ok(AgentUsageReportDimension::Provider(
            provider_key(&value)?.to_owned(),
        )),
        AgentUsageReportDimension::Model { provider, model } => {
            let provider = provider_key(&provider)?.to_owned();
            let model = bounded_text(&model, MAX_KEY_LENGTH, "invalid_provider_model")?;
            Ok(AgentUsageReportDimension::Model { provider, model })
        }
        value => Ok(value),
    }
}

fn provider_key(value: &str) -> Result<&str, AgentUsageError> {
    match value {
        "openai" | "anthropic" | "openrouter" => Ok(value),
        _ => Err(AgentUsageError::invalid("invalid_provider_key")),
    }
}

fn stable_key(value: &str, maximum: usize) -> Result<String, AgentUsageError> {
    if value.is_empty()
        || value.len() > maximum
        || value != value.trim()
        || value.bytes().any(|byte| byte.is_ascii_uppercase())
        || !value.bytes().enumerate().all(|(index, byte)| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || (index > 0 && matches!(byte, b'_' | b'.' | b'-'))
        })
    {
        return Err(AgentUsageError::invalid("invalid_stable_key"));
    }
    Ok(value.to_owned())
}

fn parse_idempotency_key(value: &str) -> Result<String, AgentUsageError> {
    if !(8..=MAX_IDEMPOTENCY_KEY_LENGTH).contains(&value.len())
        || value != value.trim()
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b':' | b'-'))
    {
        return Err(AgentUsageError::invalid("invalid_idempotency_key"));
    }
    Ok(value.to_owned())
}

fn bounded_text(
    value: &str,
    maximum: usize,
    code: &'static str,
) -> Result<String, AgentUsageError> {
    if value.trim().is_empty()
        || value != value.trim()
        || value.len() > maximum
        || value.chars().any(char::is_control)
    {
        return Err(AgentUsageError::invalid(code));
    }
    Ok(value.to_owned())
}

fn safe_u64(value: u64, code: &'static str) -> Result<i64, AgentUsageError> {
    i64::try_from(value)
        .ok()
        .filter(|value| *value <= MAX_SAFE_INTEGER)
        .ok_or_else(|| AgentUsageError::invalid(code))
}

fn positive_safe_u64(value: u64, code: &'static str) -> Result<i64, AgentUsageError> {
    safe_u64(value, code).and_then(|value| {
        if value == 0 {
            Err(AgentUsageError::invalid(code))
        } else {
            Ok(value)
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demands_parse_exact_shapes() {
        assert!(AgentUsageDemand::count(AgentUsageMeter::Runs, 1).is_ok());
        assert!(AgentUsageDemand::count(AgentUsageMeter::EstimatedCost, 1).is_err());
        let money = AgentMoney::parse(150, "USD", 2, Some("prices-v1")).unwrap();
        assert!(AgentUsageDemand::money(AgentUsageMeter::EstimatedCost, money).is_ok());
        assert!(AgentMoney::parse(1, "usd", 2, None).is_err());
    }

    #[test]
    fn preparation_requires_one_stage_counter_and_unique_meters() {
        let demand = AgentUsageDemand::count(AgentUsageMeter::Runs, 1).unwrap();
        let parsed = PrepareAgentUsage::parse(
            Uuid::new_v4(),
            AgentUsageStage::Run,
            "usage:run:one",
            [1; 32],
            [demand.clone()],
            Duration::from_secs(30),
        );
        assert!(parsed.is_ok());
        assert!(
            PrepareAgentUsage::parse(
                Uuid::new_v4(),
                AgentUsageStage::Run,
                "usage:run:two",
                [1; 32],
                [demand.clone(), demand],
                Duration::from_secs(30),
            )
            .is_err()
        );
    }

    #[test]
    fn report_filters_keep_currency_tuples_indivisible() {
        assert!(
            AgentUsageReportQuery::parse(
                AgentUsageReportDimension::Provider("openai".to_owned()),
                Some(AgentUsageMeter::EstimatedCost),
                Some(("ZWL", 2, Some("zim-v1"))),
                None,
                Some(100),
            )
            .is_ok()
        );
        assert!(
            AgentUsageReportQuery::parse(
                AgentUsageReportDimension::Provider("openai".to_owned()),
                Some(AgentUsageMeter::InputTokens),
                Some(("USD", 2, None)),
                None,
                None,
            )
            .is_err()
        );
    }

    #[test]
    fn every_meter_and_reservation_status_has_a_stable_storage_value() {
        let meters = [
            AgentUsageMeter::Runs,
            AgentUsageMeter::ProviderAttempts,
            AgentUsageMeter::CapabilityCalls,
            AgentUsageMeter::InputTokens,
            AgentUsageMeter::OutputTokens,
            AgentUsageMeter::CachedInputTokens,
            AgentUsageMeter::ReasoningTokens,
            AgentUsageMeter::ProviderReportedCost,
            AgentUsageMeter::EstimatedCost,
        ];
        for meter in meters {
            assert_eq!(AgentUsageMeter::from_str(meter.as_str()).unwrap(), meter);
        }
        assert!(matches!(
            AgentUsageMeter::from_str("agent.unknown"),
            Err(AgentUsageError::Storage)
        ));

        for (stored, expected) in [
            ("not_limited", AgentUsageReservationStatus::NotLimited),
            ("reserved", AgentUsageReservationStatus::Reserved),
            ("committed", AgentUsageReservationStatus::Committed),
            ("released", AgentUsageReservationStatus::Released),
            ("expired", AgentUsageReservationStatus::Expired),
            ("denied", AgentUsageReservationStatus::Denied),
        ] {
            assert_eq!(
                AgentUsageReservationStatus::from_str(stored).unwrap(),
                expected
            );
        }
        assert!(matches!(
            AgentUsageReservationStatus::from_str("preparing"),
            Err(AgentUsageError::Storage)
        ));
    }

    #[test]
    fn money_and_demand_boundaries_fail_closed() {
        assert!(AgentMoney::parse(0, "ZWG", 0, None).is_ok());
        assert!(AgentMoney::parse(MAX_SAFE_INTEGER as u64, "USD", 9, Some("v1")).is_ok());
        assert!(AgentMoney::parse((MAX_SAFE_INTEGER as u64) + 1, "USD", 2, None).is_err());
        assert!(AgentMoney::parse(1, "US", 2, None).is_err());
        assert!(AgentMoney::parse(1, "USD", 10, None).is_err());
        assert!(AgentMoney::parse(1, "USD", 2, Some("")).is_err());
        assert!(AgentUsageDemand::count(AgentUsageMeter::Runs, 0).is_err());

        let reported = AgentMoney::parse(1, "USD", 2, None).unwrap();
        assert!(AgentUsageDemand::money(AgentUsageMeter::ProviderReportedCost, reported).is_ok());
        let estimate_without_version = AgentMoney::parse(1, "USD", 2, None).unwrap();
        assert!(
            AgentUsageDemand::money(AgentUsageMeter::EstimatedCost, estimate_without_version)
                .is_err()
        );
        let zero = AgentMoney::parse(0, "USD", 2, Some("v1")).unwrap();
        assert!(AgentUsageDemand::money(AgentUsageMeter::EstimatedCost, zero).is_err());
    }

    #[test]
    fn stage_identity_ttl_and_idempotency_boundaries_are_exact() {
        let run = Uuid::new_v4();
        let run_count = AgentUsageDemand::count(AgentUsageMeter::Runs, 1).unwrap();
        let provider_count = AgentUsageDemand::count(AgentUsageMeter::ProviderAttempts, 1).unwrap();
        let capability_count =
            AgentUsageDemand::count(AgentUsageMeter::CapabilityCalls, 1).unwrap();
        for (stage, demand) in [
            (
                AgentUsageStage::ProviderAttempt {
                    attempt_id: Uuid::new_v4(),
                },
                provider_count,
            ),
            (
                AgentUsageStage::CapabilityCall {
                    call_id: Uuid::new_v4(),
                },
                capability_count,
            ),
        ] {
            assert!(
                PrepareAgentUsage::parse(
                    run,
                    stage,
                    "usage:stage:valid",
                    [1; 32],
                    [demand],
                    Duration::from_secs(1),
                )
                .is_ok()
            );
        }
        for invalid in [
            PrepareAgentUsage::parse(
                Uuid::nil(),
                AgentUsageStage::Run,
                "usage:nil:run",
                [1; 32],
                [run_count.clone()],
                Duration::from_secs(1),
            ),
            PrepareAgentUsage::parse(
                run,
                AgentUsageStage::Run,
                "usage:nil:fingerprint",
                [0; 32],
                [run_count.clone()],
                Duration::from_secs(1),
            ),
            PrepareAgentUsage::parse(
                run,
                AgentUsageStage::Run,
                "short",
                [1; 32],
                [run_count.clone()],
                Duration::from_secs(1),
            ),
            PrepareAgentUsage::parse(
                run,
                AgentUsageStage::Run,
                "usage:ttl:short",
                [1; 32],
                [run_count.clone()],
                Duration::ZERO,
            ),
            PrepareAgentUsage::parse(
                run,
                AgentUsageStage::Run,
                "usage:ttl:long",
                [1; 32],
                [run_count.clone()],
                Duration::from_secs(901),
            ),
            PrepareAgentUsage::parse(
                run,
                AgentUsageStage::ProviderAttempt {
                    attempt_id: Uuid::nil(),
                },
                "usage:nil:attempt",
                [1; 32],
                [run_count.clone()],
                Duration::from_secs(1),
            ),
            PrepareAgentUsage::parse(
                run,
                AgentUsageStage::CapabilityCall {
                    call_id: Uuid::nil(),
                },
                "usage:nil:capability",
                [1; 32],
                [run_count],
                Duration::from_secs(1),
            ),
        ] {
            assert!(invalid.is_err());
        }
    }

    #[test]
    fn all_report_dimensions_and_error_codes_are_bounded() {
        for dimension in [
            AgentUsageReportDimension::Person(Uuid::new_v4()),
            AgentUsageReportDimension::OriginModule("sis".to_owned()),
            AgentUsageReportDimension::CapabilityModule("fleet".to_owned()),
            AgentUsageReportDimension::Capability("fleet.vehicles.list".to_owned()),
            AgentUsageReportDimension::Provider("anthropic".to_owned()),
            AgentUsageReportDimension::Model {
                provider: "openrouter".to_owned(),
                model: "vendor/model".to_owned(),
            },
        ] {
            assert!(AgentUsageReportQuery::parse(dimension, None, None, None, None).is_ok());
        }
        for dimension in [
            AgentUsageReportDimension::Person(Uuid::nil()),
            AgentUsageReportDimension::OriginModule(" SIS".to_owned()),
            AgentUsageReportDimension::Provider("unknown".to_owned()),
            AgentUsageReportDimension::Model {
                provider: "openai".to_owned(),
                model: "".to_owned(),
            },
        ] {
            assert!(AgentUsageReportQuery::parse(dimension, None, None, None, None).is_err());
        }
        assert!(
            AgentUsageReportQuery::parse(
                AgentUsageReportDimension::Provider("openai".to_owned()),
                None,
                None,
                None,
                Some(0),
            )
            .is_err()
        );
        assert!(
            AgentUsageReportQuery::parse(
                AgentUsageReportDimension::Provider("openai".to_owned()),
                None,
                None,
                None,
                Some(101),
            )
            .is_err()
        );

        for (error, code) in [
            (
                AgentUsageError::IdempotencyConflict,
                "agent_usage_idempotency_conflict",
            ),
            (AgentUsageError::NotFound, "agent_usage_not_found"),
            (AgentUsageError::MissingDemand, "agent_usage_missing_demand"),
            (
                AgentUsageError::CurrencyMismatch,
                "agent_usage_currency_mismatch",
            ),
            (
                AgentUsageError::Denied {
                    reservation_id: Uuid::new_v4(),
                },
                "agent_usage_limit_denied",
            ),
            (
                AgentUsageError::InvalidTransition,
                "agent_usage_invalid_transition",
            ),
            (AgentUsageError::Storage, "agent_usage_storage_error"),
        ] {
            assert_eq!(error.code(), code);
        }
        let invalid = AgentUsageError::invalid("invalid_contract");
        assert_eq!(invalid.code(), "invalid_contract");
    }
}
