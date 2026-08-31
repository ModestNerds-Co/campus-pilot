//! Provides tenant-scoped Agent governance reads for Administration.
//!
//! This boundary exposes code-owned capability coverage, reduced runtime health, usage evidence,
//! and redacted run trails. It never returns message content, provider credentials, execution
//! artifacts, input fingerprints, or raw resource references. Agent runtime mutations remain
//! owned by `cp-agent-runtime`; limits stay read-only here until that crate exposes reviewed CRUD.

use std::collections::{BTreeMap, BTreeSet};

use actix_web::{
    HttpResponse, get,
    http::{StatusCode, header},
    web::{self, ServiceConfig},
};
use chrono::{DateTime, Duration, Utc};
use cp_agent::{CapabilityDescriptor, CapabilityRegistry};
use cp_agent_runtime::{AiRoutingOps, RouteTargetReadiness};
use cp_ai_providers::{AiProviderOps, provider_catalog};
use cp_common::{
    AgentExposure, ApiResponse, OperationEffect, ProductOperation, TenantId, operation_catalog,
};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use thiserror::Error;
use uuid::Uuid;

use crate::{
    services::access::{catalog::module_catalog, models::TenantModuleResponse, ops::AccessOps},
    state::AppState,
};

const DEFAULT_PAGE_SIZE: u16 = 50;
const MAX_PAGE_SIZE: u16 = 100;
const MAX_PAGE: u32 = 1_000;
const MAX_REPORT_DAYS: i64 = 92;
const DEFAULT_REPORT_DAYS: i64 = 30;
const MAX_EXPORT_ROWS: i64 = 10_000;

/// Application-owned, read-only Agent governance service.
#[derive(Debug, Clone)]
pub struct AgentGovernanceOps {
    pool: PgPool,
}

impl AgentGovernanceOps {
    #[must_use]
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Loads operational readiness without interpreting queue activity as worker liveness.
    pub async fn readiness(
        &self,
        tenant_id: Uuid,
        registry: &CapabilityRegistry,
        providers: &AiProviderOps,
        routing: &AiRoutingOps,
    ) -> Result<AgentReadiness, GovernanceError> {
        let tenant_modules = AccessOps::list_tenant_modules(&self.pool, tenant_id)
            .await
            .map_err(|_| GovernanceError::DependencyUnavailable)?;
        let module_states = module_state_index(tenant_modules);
        let connections = providers
            .list_connections(tenant_id)
            .await
            .map_err(|_| GovernanceError::DependencyUnavailable)?;
        let routes = routing
            .list_routes(tenant_id)
            .await
            .map_err(|_| GovernanceError::DependencyUnavailable)?;
        let runtime = sqlx::query_as::<_, RuntimeReadinessRow>(
            r#"
            SELECT
                (SELECT COUNT(*) FROM agent_threads
                 WHERE tenant_id = $1 AND deleted_at IS NULL) AS session_count,
                (SELECT COUNT(*) FROM agent_runs
                 WHERE tenant_id = $1 AND status = 'queued' AND deleted_at IS NULL)
                    AS queued_run_count,
                (SELECT COUNT(*) FROM agent_runs
                 WHERE tenant_id = $1
                   AND status IN ('running', 'awaiting_approval')
                   AND deleted_at IS NULL) AS active_run_count,
                (SELECT COUNT(*) FROM agent_run_queue
                 WHERE tenant_id = $1 AND state = 'leased'
                   AND lease_expires_at <= STATEMENT_TIMESTAMP()
                   AND deleted_at IS NULL) AS expired_lease_count,
                (SELECT COUNT(*) FROM agent_limit_rules
                 WHERE tenant_id = $1 AND deleted_at IS NULL) AS configured_limit_count,
                agent_has_ready_worker() AS worker_available,
                (SELECT COUNT(*) FROM agent_worker_instances
                 WHERE deleted_at IS NULL) AS registered_worker_count,
                (SELECT COUNT(*) FROM agent_worker_instances
                 WHERE lifecycle_state = 'ready'
                   AND ready_at IS NOT NULL
                   AND heartbeat_at >= CLOCK_TIMESTAMP() - INTERVAL '45 seconds'
                   AND draining_at IS NULL
                   AND unavailable_at IS NULL
                   AND deleted_at IS NULL) AS ready_worker_count
            "#,
        )
        .bind(tenant_id)
        .fetch_one(&self.pool)
        .await?;

        let agent_module = module_states.get("agent");
        let executable_keys = registry
            .descriptors()
            .into_iter()
            .map(|descriptor| descriptor.operation_key().as_str())
            .collect::<BTreeSet<_>>();
        let operations = operation_catalog()
            .iter()
            .map(|route| route.operation())
            .collect::<Vec<_>>();
        let executable_capabilities = operations
            .iter()
            .filter(|operation| executable_keys.contains(operation.key()))
            .count();
        let approval_required = operations
            .iter()
            .filter(|operation| operation.agent_exposure() == AgentExposure::ApprovalRequired)
            .count();
        let human_only = operations
            .iter()
            .filter(|operation| {
                matches!(operation.agent_exposure(), AgentExposure::HumanOnly { .. })
            })
            .count();
        let prohibited = operations
            .iter()
            .filter(|operation| {
                matches!(operation.agent_exposure(), AgentExposure::Prohibited { .. })
            })
            .count();
        let ready_targets = routes
            .iter()
            .flat_map(|route| &route.targets)
            .filter(|target| target.readiness == RouteTargetReadiness::Ready)
            .count();
        let total_targets = routes
            .iter()
            .map(|route| route.targets.len())
            .sum::<usize>();

        Ok(AgentReadiness {
            module: AgentModuleReadiness {
                enabled: agent_module.is_some_and(|module| module.enabled),
                status: agent_module
                    .map(|module| module.status.clone())
                    .unwrap_or_else(|| "not_configured".to_string()),
            },
            providers: ProviderReadiness {
                total: connections.len(),
                ready: connections
                    .iter()
                    .filter(|connection| connection.status == "ready")
                    .count(),
                attention: connections
                    .iter()
                    .filter(|connection| connection.status != "ready")
                    .count(),
            },
            routing: RoutingReadiness {
                route_sets: routes.len(),
                ready_targets,
                blocked_targets: total_targets.saturating_sub(ready_targets),
            },
            capabilities: CapabilityReadiness {
                catalogued_operations: operations.len(),
                executable_capabilities,
                approval_required,
                human_only,
                prohibited,
            },
            runtime: RuntimeReadiness {
                sessions: runtime.session_count,
                queued_runs: runtime.queued_run_count,
                active_runs: runtime.active_run_count,
                expired_leases: runtime.expired_lease_count,
            },
            workers: WorkerReadiness {
                available: runtime.worker_available,
                registered_instances: runtime.registered_worker_count,
                ready_instances: runtime.ready_worker_count,
                reason: worker_readiness_reason(
                    runtime.worker_available,
                    runtime.registered_worker_count,
                ),
            },
            limits: LimitReadiness {
                configured_rules: runtime.configured_limit_count,
                enforcement_available: true,
                management_available: false,
            },
        })
    }

    /// Returns the code catalogue intersected with current tenant module availability.
    pub async fn capability_inventory(
        &self,
        tenant_id: Uuid,
        registry: &CapabilityRegistry,
        query: CapabilityInventoryQuery,
    ) -> Result<CapabilityInventoryPage, GovernanceError> {
        let tenant_modules = AccessOps::list_tenant_modules(&self.pool, tenant_id)
            .await
            .map_err(|_| GovernanceError::DependencyUnavailable)?;
        let module_states = module_state_index(tenant_modules);
        let modules = module_catalog();
        let module_labels = modules
            .iter()
            .map(|module| (module.key, module.label))
            .collect::<BTreeMap<_, _>>();
        let descriptors = registry
            .descriptors()
            .into_iter()
            .map(|descriptor| (descriptor.operation_key().as_str(), descriptor))
            .collect::<BTreeMap<_, _>>();
        let parsed = ParsedCapabilityInventoryQuery::parse(query)?;

        let mut all_items = operation_catalog()
            .iter()
            .map(|route| {
                capability_item(
                    route.operation(),
                    descriptors.get(route.operation().key()).copied(),
                    &module_states,
                    &module_labels,
                )
            })
            .collect::<Vec<_>>();
        all_items.sort_by(|left, right| {
            left.module_label
                .cmp(&right.module_label)
                .then_with(|| left.operation_key.cmp(&right.operation_key))
        });
        let summary = capability_summary(&all_items);
        let filtered = all_items
            .into_iter()
            .filter(|item| parsed.matches(item))
            .collect::<Vec<_>>();
        let filtered_count = filtered.len();
        let start = ((parsed.page - 1) * u32::from(parsed.per_page)) as usize;
        let items = filtered
            .into_iter()
            .skip(start)
            .take(usize::from(parsed.per_page))
            .collect();

        Ok(CapabilityInventoryPage {
            summary,
            modules: modules
                .into_iter()
                .map(|module| UsageModuleOption {
                    key: module.key.to_string(),
                    label: module.label.to_string(),
                })
                .collect(),
            filtered_count,
            page: parsed.page,
            per_page: parsed.per_page,
            total_pages: page_count(filtered_count, parsed.per_page),
            items,
        })
    }

    /// Returns safe filter options derived from the tenant's own usage evidence.
    pub async fn usage_options(
        &self,
        tenant_id: Uuid,
        registry: &CapabilityRegistry,
    ) -> Result<UsageFilterOptions, GovernanceError> {
        let people = sqlx::query_as::<_, PersonOptionRow>(
            r#"
            SELECT DISTINCT u.id, u.full_name
            FROM agent_usage_events event
            INNER JOIN users u
              ON u.id = event.actor_user_id AND u.tenant_id = event.tenant_id
            WHERE event.tenant_id = $1
              AND event.deleted_at IS NULL
              AND u.deleted_at IS NULL
            ORDER BY LOWER(u.full_name), u.id
            LIMIT 500
            "#,
        )
        .bind(tenant_id)
        .fetch_all(&self.pool)
        .await?;
        let models = sqlx::query_as::<_, ModelOptionRow>(
            r#"
            SELECT DISTINCT provider_key, provider_model_id
            FROM agent_usage_events
            WHERE tenant_id = $1
              AND provider_key IS NOT NULL
              AND provider_model_id IS NOT NULL
              AND deleted_at IS NULL
            ORDER BY provider_key, provider_model_id
            LIMIT 500
            "#,
        )
        .bind(tenant_id)
        .fetch_all(&self.pool)
        .await?;
        let capability_options = registry
            .descriptors()
            .into_iter()
            .map(|descriptor| UsageCapabilityOption {
                key: descriptor.key().to_string(),
                label: descriptor.title().to_string(),
            })
            .collect();

        Ok(UsageFilterOptions {
            people: people
                .into_iter()
                .map(|person| UsagePersonOption {
                    id: person.id,
                    name: person.full_name,
                })
                .collect(),
            modules: module_catalog()
                .into_iter()
                .map(|module| UsageModuleOption {
                    key: module.key.to_string(),
                    label: module.label.to_string(),
                })
                .collect(),
            capabilities: capability_options,
            providers: provider_catalog()
                .iter()
                .map(|provider| provider.key.to_string())
                .collect(),
            models: models
                .into_iter()
                .map(|model| UsageModelOption {
                    provider: model.provider_key,
                    model: model.provider_model_id,
                })
                .collect(),
            outcomes: vec![
                "succeeded".to_string(),
                "failed".to_string(),
                "denied".to_string(),
                "cancelled".to_string(),
                "interrupted".to_string(),
            ],
            meters: usage_meter_keys(),
        })
    }

    /// Aggregates exact immutable usage measures over one bounded UTC range.
    pub async fn usage_report(
        &self,
        tenant_id: Uuid,
        query: UsageQuery,
    ) -> Result<UsageReport, GovernanceError> {
        let parsed = ParsedUsageQuery::parse(query)?;
        let totals = self.usage_totals(tenant_id, &parsed).await?;
        let trend = self.usage_trend(tenant_id, &parsed).await?;
        Ok(UsageReport {
            from: parsed.range.from,
            to: parsed.range.to,
            totals: totals.into_iter().map(UsageTotal::from).collect(),
            trend: trend.into_iter().map(UsageTrendPoint::from).collect(),
        })
    }

    /// Produces a bounded CSV using the same parsed filters as the on-screen report.
    pub async fn usage_export(
        &self,
        tenant_id: Uuid,
        query: UsageQuery,
    ) -> Result<UsageCsvExport, GovernanceError> {
        let parsed = ParsedUsageQuery::parse(query)?;
        let rows = self
            .usage_export_rows(tenant_id, &parsed, MAX_EXPORT_ROWS + 1)
            .await?;
        let truncated = rows.len() as i64 > MAX_EXPORT_ROWS;
        let mut csv = String::from(
            "occurred_at,event_id,person_id,person_name,event_kind,outcome,origin_module,capability_module,capability,provider,model,meter,amount,currency,exponent,pricing_version\n",
        );
        for row in rows.into_iter().take(MAX_EXPORT_ROWS as usize) {
            let amount = row
                .amount
                .map(|value| value.to_string())
                .unwrap_or_default();
            let exponent = row
                .currency_exponent
                .map(|value| value.to_string())
                .unwrap_or_default();
            let fields = vec![
                csv_field(&row.occurred_at.to_rfc3339()),
                csv_field(&row.event_id.to_string()),
                csv_field(&row.actor_user_id.to_string()),
                csv_text_field(&row.full_name),
                csv_text_field(&row.event_kind),
                csv_text_field(&row.outcome),
                csv_text_field(&row.origin_module_key),
                csv_text_field(row.capability_module_key.as_deref().unwrap_or_default()),
                csv_text_field(row.capability_key.as_deref().unwrap_or_default()),
                csv_text_field(row.provider_key.as_deref().unwrap_or_default()),
                csv_text_field(row.provider_model_id.as_deref().unwrap_or_default()),
                csv_text_field(&row.meter_key),
                csv_field(&amount),
                csv_text_field(row.currency_code.as_deref().unwrap_or_default()),
                csv_field(&exponent),
                csv_text_field(row.pricing_version.as_deref().unwrap_or_default()),
            ];
            csv.push_str(&fields.join(","));
            csv.push('\n');
        }
        Ok(UsageCsvExport { csv, truncated })
    }

    /// Lists campus run metadata without transcript, artifacts, or raw execution inputs.
    pub async fn list_runs(
        &self,
        tenant_id: Uuid,
        query: RunAuditQuery,
    ) -> Result<RunAuditPage, GovernanceError> {
        let parsed = ParsedRunAuditQuery::parse(query)?;
        let offset = i64::from((parsed.page - 1) * u32::from(parsed.per_page));
        let rows = sqlx::query_as::<_, RunAuditRow>(RUN_AUDIT_LIST_SQL)
            .bind(tenant_id)
            .bind(parsed.range.from)
            .bind(parsed.range.to)
            .bind(parsed.status)
            .bind(parsed.person_id)
            .bind(parsed.origin_module)
            .bind(parsed.correlation_id)
            .bind(parsed.search)
            .bind(i64::from(parsed.per_page))
            .bind(offset)
            .fetch_all(&self.pool)
            .await?;
        let people = sqlx::query_as::<_, PersonOptionRow>(
            r#"
            SELECT DISTINCT actor.id, actor.full_name
            FROM agent_runs run
            INNER JOIN users actor
              ON actor.id = run.requested_by AND actor.tenant_id = run.tenant_id
            WHERE run.tenant_id = $1
              AND run.deleted_at IS NULL
              AND actor.deleted_at IS NULL
            ORDER BY LOWER(actor.full_name), actor.id
            LIMIT 500
            "#,
        )
        .bind(tenant_id)
        .fetch_all(&self.pool)
        .await?;
        let total = rows.first().map_or(0, |row| row.total_count);
        Ok(RunAuditPage {
            page: parsed.page,
            per_page: parsed.per_page,
            total,
            total_pages: page_count(total.max(0) as usize, parsed.per_page),
            people: people
                .into_iter()
                .map(|person| UsagePersonOption {
                    id: person.id,
                    name: person.full_name,
                })
                .collect(),
            modules: module_catalog()
                .into_iter()
                .map(|module| UsageModuleOption {
                    key: module.key.to_string(),
                    label: module.label.to_string(),
                })
                .collect(),
            items: rows.into_iter().map(RunAuditListItem::from).collect(),
        })
    }

    /// Reads one reduced run trail scoped to the current tenant.
    pub async fn read_run(
        &self,
        tenant_id: Uuid,
        run_id: Uuid,
    ) -> Result<RunAuditDetail, GovernanceError> {
        if run_id.is_nil() {
            return Err(GovernanceError::NotFound);
        }
        let run = sqlx::query_as::<_, RunDetailRow>(RUN_DETAIL_SQL)
            .bind(tenant_id)
            .bind(run_id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or(GovernanceError::NotFound)?;
        let provider_attempts =
            sqlx::query_as::<_, ProviderAttemptAudit>(PROVIDER_ATTEMPT_AUDIT_SQL)
                .bind(tenant_id)
                .bind(run_id)
                .fetch_all(&self.pool)
                .await?;
        let capability_calls = sqlx::query_as::<_, CapabilityCallAudit>(CAPABILITY_CALL_AUDIT_SQL)
            .bind(tenant_id)
            .bind(run_id)
            .fetch_all(&self.pool)
            .await?;
        let events = sqlx::query_as::<_, RunEventAudit>(RUN_EVENT_AUDIT_SQL)
            .bind(tenant_id)
            .bind(run_id)
            .fetch_all(&self.pool)
            .await?;
        let audit_events = sqlx::query_as::<_, ActorAuditProjection>(ACTOR_AUDIT_SQL)
            .bind(tenant_id)
            .bind(run_id)
            .bind(run.correlation_id)
            .fetch_all(&self.pool)
            .await?;

        Ok(RunAuditDetail {
            run: RunAuditListItem::from(run),
            provider_attempts,
            capability_calls,
            events,
            audit_events,
        })
    }

    async fn usage_totals(
        &self,
        tenant_id: Uuid,
        query: &ParsedUsageQuery,
    ) -> Result<Vec<UsageTotalRow>, GovernanceError> {
        let sql = usage_totals_sql();
        Ok(sqlx::query_as::<_, UsageTotalRow>(&sql)
            .bind(tenant_id)
            .bind(query.range.from)
            .bind(query.range.to)
            .bind(query.person_id)
            .bind(query.origin_module.as_deref())
            .bind(query.capability_module.as_deref())
            .bind(query.capability.as_deref())
            .bind(query.provider.as_deref())
            .bind(query.model.as_deref())
            .bind(query.outcome.as_deref())
            .bind(query.meter.as_deref())
            .fetch_all(&self.pool)
            .await?)
    }

    async fn usage_trend(
        &self,
        tenant_id: Uuid,
        query: &ParsedUsageQuery,
    ) -> Result<Vec<UsageTrendRow>, GovernanceError> {
        let sql = usage_trend_sql();
        Ok(sqlx::query_as::<_, UsageTrendRow>(&sql)
            .bind(tenant_id)
            .bind(query.range.from)
            .bind(query.range.to)
            .bind(query.person_id)
            .bind(query.origin_module.as_deref())
            .bind(query.capability_module.as_deref())
            .bind(query.capability.as_deref())
            .bind(query.provider.as_deref())
            .bind(query.model.as_deref())
            .bind(query.outcome.as_deref())
            .bind(query.meter.as_deref())
            .fetch_all(&self.pool)
            .await?)
    }

    async fn usage_export_rows(
        &self,
        tenant_id: Uuid,
        query: &ParsedUsageQuery,
        limit: i64,
    ) -> Result<Vec<UsageExportRow>, GovernanceError> {
        let sql = usage_export_sql();
        Ok(sqlx::query_as::<_, UsageExportRow>(&sql)
            .bind(tenant_id)
            .bind(query.range.from)
            .bind(query.range.to)
            .bind(query.person_id)
            .bind(query.origin_module.as_deref())
            .bind(query.capability_module.as_deref())
            .bind(query.capability.as_deref())
            .bind(query.provider.as_deref())
            .bind(query.model.as_deref())
            .bind(query.outcome.as_deref())
            .bind(query.meter.as_deref())
            .bind(limit)
            .fetch_all(&self.pool)
            .await?)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct AgentReadiness {
    pub module: AgentModuleReadiness,
    pub providers: ProviderReadiness,
    pub routing: RoutingReadiness,
    pub capabilities: CapabilityReadiness,
    pub runtime: RuntimeReadiness,
    pub workers: WorkerReadiness,
    pub limits: LimitReadiness,
}

#[derive(Debug, Clone, Serialize)]
pub struct AgentModuleReadiness {
    pub enabled: bool,
    pub status: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProviderReadiness {
    pub total: usize,
    pub ready: usize,
    pub attention: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct RoutingReadiness {
    pub route_sets: usize,
    pub ready_targets: usize,
    pub blocked_targets: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct CapabilityReadiness {
    pub catalogued_operations: usize,
    pub executable_capabilities: usize,
    pub approval_required: usize,
    pub human_only: usize,
    pub prohibited: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeReadiness {
    pub sessions: i64,
    pub queued_runs: i64,
    pub active_runs: i64,
    pub expired_leases: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkerReadiness {
    pub available: bool,
    pub registered_instances: i64,
    pub ready_instances: i64,
    pub reason: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct LimitReadiness {
    pub configured_rules: i64,
    pub enforcement_available: bool,
    pub management_available: bool,
}

#[derive(Debug, FromRow)]
struct RuntimeReadinessRow {
    session_count: i64,
    queued_run_count: i64,
    active_run_count: i64,
    expired_lease_count: i64,
    configured_limit_count: i64,
    worker_available: bool,
    registered_worker_count: i64,
    ready_worker_count: i64,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct CapabilityInventoryQuery {
    pub search: Option<String>,
    pub module: Option<String>,
    pub exposure: Option<String>,
    pub availability: Option<String>,
    pub page: Option<u32>,
    pub per_page: Option<u16>,
}

struct ParsedCapabilityInventoryQuery {
    search: Option<String>,
    module: Option<String>,
    exposure: Option<String>,
    availability: Option<String>,
    page: u32,
    per_page: u16,
}

impl ParsedCapabilityInventoryQuery {
    fn parse(query: CapabilityInventoryQuery) -> Result<Self, GovernanceError> {
        Ok(Self {
            search: query
                .search
                .map(|value| bounded_search(&value))
                .transpose()?,
            module: query
                .module
                .map(|value| stable_filter(&value, "invalid_module_filter"))
                .transpose()?,
            exposure: query
                .exposure
                .map(|value| {
                    one_of(
                        &value,
                        &["exposed", "approval_required", "human_only", "prohibited"],
                        "invalid_exposure_filter",
                    )
                })
                .transpose()?,
            availability: query
                .availability
                .map(|value| {
                    one_of(
                        &value,
                        &[
                            "executable",
                            "module_unavailable",
                            "approval_not_released",
                            "handler_unavailable",
                            "human_only",
                            "prohibited",
                        ],
                        "invalid_availability_filter",
                    )
                })
                .transpose()?,
            page: parse_page(query.page)?,
            per_page: parse_page_size(query.per_page)?,
        })
    }

    fn matches(&self, item: &CapabilityInventoryItem) -> bool {
        self.search.as_ref().is_none_or(|search| {
            let search = search.to_lowercase();
            item.operation_key.to_lowercase().contains(&search)
                || item.label.to_lowercase().contains(&search)
                || item.module_label.to_lowercase().contains(&search)
                || item.permission.to_lowercase().contains(&search)
        }) && self
            .module
            .as_ref()
            .is_none_or(|module| module == &item.module_key)
            && self
                .exposure
                .as_ref()
                .is_none_or(|exposure| exposure == &item.exposure)
            && self
                .availability
                .as_ref()
                .is_none_or(|availability| availability == &item.availability)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct CapabilityInventoryPage {
    pub summary: CapabilityInventorySummary,
    pub modules: Vec<UsageModuleOption>,
    pub filtered_count: usize,
    pub page: u32,
    pub per_page: u16,
    pub total_pages: u32,
    pub items: Vec<CapabilityInventoryItem>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CapabilityInventorySummary {
    pub total: usize,
    pub exposed: usize,
    pub approval_required: usize,
    pub human_only: usize,
    pub prohibited: usize,
    pub executable: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct CapabilityInventoryItem {
    pub operation_key: String,
    pub label: String,
    pub module_key: String,
    pub module_label: String,
    pub permission: String,
    pub effect: String,
    pub exposure: String,
    pub exposure_reason: Option<String>,
    pub availability: String,
    pub availability_reason: Option<String>,
    pub capability_version: Option<u16>,
    pub required_modules: Vec<String>,
    pub required_features: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct UsageQuery {
    pub from: Option<String>,
    pub to: Option<String>,
    pub person_id: Option<String>,
    pub origin_module: Option<String>,
    pub capability_module: Option<String>,
    pub capability: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub outcome: Option<String>,
    pub meter: Option<String>,
}

struct ParsedUsageQuery {
    range: ReportRange,
    person_id: Option<Uuid>,
    origin_module: Option<String>,
    capability_module: Option<String>,
    capability: Option<String>,
    provider: Option<String>,
    model: Option<String>,
    outcome: Option<String>,
    meter: Option<String>,
}

impl ParsedUsageQuery {
    fn parse(query: UsageQuery) -> Result<Self, GovernanceError> {
        Ok(Self {
            range: ReportRange::parse(query.from.as_deref(), query.to.as_deref())?,
            person_id: query
                .person_id
                .map(|value| parse_uuid(&value, "invalid_person_filter"))
                .transpose()?,
            origin_module: query
                .origin_module
                .map(|value| stable_filter(&value, "invalid_origin_module_filter"))
                .transpose()?,
            capability_module: query
                .capability_module
                .map(|value| stable_filter(&value, "invalid_capability_module_filter"))
                .transpose()?,
            capability: query
                .capability
                .map(|value| stable_filter(&value, "invalid_capability_filter"))
                .transpose()?,
            provider: query
                .provider
                .map(|value| stable_filter(&value, "invalid_provider_filter"))
                .transpose()?,
            model: query
                .model
                .map(|value| bounded_text(&value, 240, "invalid_model_filter"))
                .transpose()?,
            outcome: query
                .outcome
                .map(|value| {
                    one_of(
                        &value,
                        &["succeeded", "failed", "denied", "cancelled", "interrupted"],
                        "invalid_outcome_filter",
                    )
                })
                .transpose()?,
            meter: query
                .meter
                .map(|value| one_of_owned(&value, &usage_meter_keys(), "invalid_meter_filter"))
                .transpose()?,
        })
    }
}

#[derive(Debug, Clone, Copy)]
struct ReportRange {
    from: DateTime<Utc>,
    to: DateTime<Utc>,
}

impl ReportRange {
    fn parse(from: Option<&str>, to: Option<&str>) -> Result<Self, GovernanceError> {
        let to = to.map(parse_datetime).transpose()?.unwrap_or_else(Utc::now);
        let from = from
            .map(parse_datetime)
            .transpose()?
            .unwrap_or_else(|| to - Duration::days(DEFAULT_REPORT_DAYS));
        if to <= from || to - from > Duration::days(MAX_REPORT_DAYS) {
            return Err(GovernanceError::invalid(
                "invalid_report_range",
                "Choose a UTC date range of no more than 92 days",
            ));
        }
        Ok(Self { from, to })
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct UsageReport {
    pub from: DateTime<Utc>,
    pub to: DateTime<Utc>,
    pub totals: Vec<UsageTotal>,
    pub trend: Vec<UsageTrendPoint>,
}

#[derive(Debug, Clone, Serialize)]
pub struct UsageTotal {
    pub meter: String,
    pub known_amount: i64,
    pub unknown_events: i64,
    pub currency: Option<String>,
    pub exponent: Option<i16>,
    pub pricing_version: Option<String>,
}

#[derive(Debug, FromRow)]
struct UsageTotalRow {
    meter_key: String,
    known_amount: i64,
    unknown_events: i64,
    currency_code: Option<String>,
    currency_exponent: Option<i16>,
    pricing_version: Option<String>,
}

impl From<UsageTotalRow> for UsageTotal {
    fn from(row: UsageTotalRow) -> Self {
        Self {
            meter: row.meter_key,
            known_amount: row.known_amount,
            unknown_events: row.unknown_events,
            currency: row.currency_code,
            exponent: row.currency_exponent,
            pricing_version: row.pricing_version,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct UsageTrendPoint {
    pub day: DateTime<Utc>,
    pub meter: String,
    pub known_amount: i64,
    pub unknown_events: i64,
    pub currency: Option<String>,
    pub exponent: Option<i16>,
    pub pricing_version: Option<String>,
}

#[derive(Debug, FromRow)]
struct UsageTrendRow {
    day: DateTime<Utc>,
    meter_key: String,
    known_amount: i64,
    unknown_events: i64,
    currency_code: Option<String>,
    currency_exponent: Option<i16>,
    pricing_version: Option<String>,
}

impl From<UsageTrendRow> for UsageTrendPoint {
    fn from(row: UsageTrendRow) -> Self {
        Self {
            day: row.day,
            meter: row.meter_key,
            known_amount: row.known_amount,
            unknown_events: row.unknown_events,
            currency: row.currency_code,
            exponent: row.currency_exponent,
            pricing_version: row.pricing_version,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct UsageFilterOptions {
    pub people: Vec<UsagePersonOption>,
    pub modules: Vec<UsageModuleOption>,
    pub capabilities: Vec<UsageCapabilityOption>,
    pub providers: Vec<String>,
    pub models: Vec<UsageModelOption>,
    pub outcomes: Vec<String>,
    pub meters: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct UsagePersonOption {
    pub id: Uuid,
    pub name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct UsageModuleOption {
    pub key: String,
    pub label: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct UsageCapabilityOption {
    pub key: String,
    pub label: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct UsageModelOption {
    pub provider: String,
    pub model: String,
}

#[derive(Debug, FromRow)]
struct PersonOptionRow {
    id: Uuid,
    full_name: String,
}

#[derive(Debug, FromRow)]
struct ModelOptionRow {
    provider_key: String,
    provider_model_id: String,
}

pub struct UsageCsvExport {
    csv: String,
    truncated: bool,
}

#[derive(Debug, FromRow)]
struct UsageExportRow {
    occurred_at: DateTime<Utc>,
    event_id: Uuid,
    actor_user_id: Uuid,
    full_name: String,
    event_kind: String,
    outcome: String,
    origin_module_key: String,
    capability_module_key: Option<String>,
    capability_key: Option<String>,
    provider_key: Option<String>,
    provider_model_id: Option<String>,
    meter_key: String,
    amount: Option<i64>,
    currency_code: Option<String>,
    currency_exponent: Option<i16>,
    pricing_version: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct RunAuditQuery {
    pub from: Option<String>,
    pub to: Option<String>,
    pub status: Option<String>,
    pub person_id: Option<String>,
    pub origin_module: Option<String>,
    pub correlation_id: Option<String>,
    pub search: Option<String>,
    pub page: Option<u32>,
    pub per_page: Option<u16>,
}

struct ParsedRunAuditQuery {
    range: ReportRange,
    status: Option<String>,
    person_id: Option<Uuid>,
    origin_module: Option<String>,
    correlation_id: Option<Uuid>,
    search: Option<String>,
    page: u32,
    per_page: u16,
}

impl ParsedRunAuditQuery {
    fn parse(query: RunAuditQuery) -> Result<Self, GovernanceError> {
        Ok(Self {
            range: ReportRange::parse(query.from.as_deref(), query.to.as_deref())?,
            status: query
                .status
                .map(|value| {
                    one_of(
                        &value,
                        &[
                            "queued",
                            "running",
                            "awaiting_approval",
                            "completed",
                            "failed",
                            "cancelled",
                            "interrupted",
                        ],
                        "invalid_run_status",
                    )
                })
                .transpose()?,
            person_id: query
                .person_id
                .map(|value| parse_uuid(&value, "invalid_person_filter"))
                .transpose()?,
            origin_module: query
                .origin_module
                .map(|value| stable_filter(&value, "invalid_origin_module_filter"))
                .transpose()?,
            correlation_id: query
                .correlation_id
                .map(|value| parse_uuid(&value, "invalid_correlation_filter"))
                .transpose()?,
            search: query
                .search
                .map(|value| bounded_search(&value))
                .transpose()?,
            page: parse_page(query.page)?,
            per_page: parse_page_size(query.per_page)?,
        })
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct RunAuditPage {
    pub page: u32,
    pub per_page: u16,
    pub total: i64,
    pub total_pages: u32,
    pub people: Vec<UsagePersonOption>,
    pub modules: Vec<UsageModuleOption>,
    pub items: Vec<RunAuditListItem>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RunAuditListItem {
    pub id: Uuid,
    pub correlation_id: Uuid,
    pub session_id: Uuid,
    pub session_title: String,
    pub requested_by_id: Uuid,
    pub requested_by_name: String,
    pub task_class: String,
    pub origin_module_key: String,
    pub status: String,
    pub safe_failure_code: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub provider_attempts: i64,
    pub capability_calls: i64,
}

#[derive(Debug, FromRow)]
struct RunAuditRow {
    id: Uuid,
    correlation_id: Uuid,
    session_id: Uuid,
    session_title: String,
    requested_by_id: Uuid,
    requested_by_name: String,
    task_class: String,
    origin_module_key: String,
    status: String,
    safe_failure_code: Option<String>,
    started_at: Option<DateTime<Utc>>,
    finished_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
    provider_attempts: i64,
    capability_calls: i64,
    total_count: i64,
}

impl From<RunAuditRow> for RunAuditListItem {
    fn from(row: RunAuditRow) -> Self {
        Self {
            id: row.id,
            correlation_id: row.correlation_id,
            session_id: row.session_id,
            session_title: row.session_title,
            requested_by_id: row.requested_by_id,
            requested_by_name: row.requested_by_name,
            task_class: row.task_class,
            origin_module_key: row.origin_module_key,
            status: row.status,
            safe_failure_code: row.safe_failure_code,
            started_at: row.started_at,
            finished_at: row.finished_at,
            created_at: row.created_at,
            provider_attempts: row.provider_attempts,
            capability_calls: row.capability_calls,
        }
    }
}

#[derive(Debug, FromRow)]
struct RunDetailRow {
    id: Uuid,
    correlation_id: Uuid,
    session_id: Uuid,
    session_title: String,
    requested_by_id: Uuid,
    requested_by_name: String,
    task_class: String,
    origin_module_key: String,
    status: String,
    safe_failure_code: Option<String>,
    started_at: Option<DateTime<Utc>>,
    finished_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
    provider_attempts: i64,
    capability_calls: i64,
}

impl From<RunDetailRow> for RunAuditListItem {
    fn from(row: RunDetailRow) -> Self {
        Self {
            id: row.id,
            correlation_id: row.correlation_id,
            session_id: row.session_id,
            session_title: row.session_title,
            requested_by_id: row.requested_by_id,
            requested_by_name: row.requested_by_name,
            task_class: row.task_class,
            origin_module_key: row.origin_module_key,
            status: row.status,
            safe_failure_code: row.safe_failure_code,
            started_at: row.started_at,
            finished_at: row.finished_at,
            created_at: row.created_at,
            provider_attempts: row.provider_attempts,
            capability_calls: row.capability_calls,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct RunAuditDetail {
    pub run: RunAuditListItem,
    pub provider_attempts: Vec<ProviderAttemptAudit>,
    pub capability_calls: Vec<CapabilityCallAudit>,
    pub events: Vec<RunEventAudit>,
    pub audit_events: Vec<ActorAuditProjection>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct ProviderAttemptAudit {
    pub id: Uuid,
    pub turn_index: i16,
    pub attempt_index: i16,
    pub provider_key: String,
    pub provider_model_id: String,
    pub status: String,
    pub failure_origin: Option<String>,
    pub failure_category: Option<String>,
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub cached_tokens: Option<i64>,
    pub reasoning_tokens: Option<i64>,
    pub provider_reported_cost_amount: Option<i64>,
    pub provider_reported_cost_currency: Option<String>,
    pub provider_reported_cost_exponent: Option<i16>,
    pub estimated_cost_amount: Option<i64>,
    pub estimated_cost_currency: Option<String>,
    pub estimated_cost_exponent: Option<i16>,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct CapabilityCallAudit {
    pub id: Uuid,
    pub call_sequence: i16,
    pub capability_key: String,
    pub capability_version: i32,
    pub owning_module_key: String,
    pub required_permission: String,
    pub scope_kind: String,
    pub resource_count: i64,
    pub status: String,
    pub safe_failure_code: Option<String>,
    pub duration_ms: Option<i64>,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct RunEventAudit {
    pub event_id: String,
    pub event_type: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct ActorAuditProjection {
    pub id: Uuid,
    pub actor_type: String,
    pub actor_name: Option<String>,
    pub action_key: String,
    pub target_type: Option<String>,
    pub outcome: String,
    pub occurred_at: DateTime<Utc>,
}

#[derive(Debug, Error)]
pub enum GovernanceError {
    #[error("invalid Agent governance input: {code}")]
    Invalid {
        code: &'static str,
        message: &'static str,
    },
    #[error("Agent governance record was not found")]
    NotFound,
    #[error("Agent governance dependency is unavailable")]
    DependencyUnavailable,
    #[error("Agent governance persistence failed")]
    Storage(#[from] sqlx::Error),
}

impl GovernanceError {
    const fn invalid(code: &'static str, message: &'static str) -> Self {
        Self::Invalid { code, message }
    }
}

#[get("/readiness")]
async fn readiness(state: web::Data<AppState>, tenant: web::ReqData<TenantId>) -> HttpResponse {
    let ops = AgentGovernanceOps::new(state.db.clone());
    respond(
        ops.readiness(
            tenant.into_inner().0,
            &state.agent_capabilities,
            &state.ai_provider_ops,
            &state.ai_routing_ops,
        )
        .await,
    )
}

#[get("/capabilities")]
async fn capabilities(
    state: web::Data<AppState>,
    tenant: web::ReqData<TenantId>,
    query: web::Query<CapabilityInventoryQuery>,
) -> HttpResponse {
    let ops = AgentGovernanceOps::new(state.db.clone());
    respond(
        ops.capability_inventory(
            tenant.into_inner().0,
            &state.agent_capabilities,
            query.into_inner(),
        )
        .await,
    )
}

#[get("/usage/options")]
async fn usage_options(state: web::Data<AppState>, tenant: web::ReqData<TenantId>) -> HttpResponse {
    let ops = AgentGovernanceOps::new(state.db.clone());
    respond(
        ops.usage_options(tenant.into_inner().0, &state.agent_capabilities)
            .await,
    )
}

#[get("/usage")]
async fn usage(
    state: web::Data<AppState>,
    tenant: web::ReqData<TenantId>,
    query: web::Query<UsageQuery>,
) -> HttpResponse {
    let ops = AgentGovernanceOps::new(state.db.clone());
    respond(
        ops.usage_report(tenant.into_inner().0, query.into_inner())
            .await,
    )
}

#[get("/usage/export")]
async fn usage_export(
    state: web::Data<AppState>,
    tenant: web::ReqData<TenantId>,
    query: web::Query<UsageQuery>,
) -> HttpResponse {
    let ops = AgentGovernanceOps::new(state.db.clone());
    match ops
        .usage_export(tenant.into_inner().0, query.into_inner())
        .await
    {
        Ok(export) => HttpResponse::build(if export.truncated {
            StatusCode::PARTIAL_CONTENT
        } else {
            StatusCode::OK
        })
        .insert_header((header::CONTENT_TYPE, "text/csv; charset=utf-8"))
        .insert_header((
            header::CONTENT_DISPOSITION,
            "attachment; filename=campus-pilot-agent-usage.csv",
        ))
        .insert_header((header::CACHE_CONTROL, "no-store"))
        .insert_header(("x-export-truncated", export.truncated.to_string()))
        .body(export.csv),
        Err(error) => governance_error(error),
    }
}

#[get("/runs")]
async fn runs(
    state: web::Data<AppState>,
    tenant: web::ReqData<TenantId>,
    query: web::Query<RunAuditQuery>,
) -> HttpResponse {
    let ops = AgentGovernanceOps::new(state.db.clone());
    respond(
        ops.list_runs(tenant.into_inner().0, query.into_inner())
            .await,
    )
}

#[get("/runs/{run_id}")]
async fn run_detail(
    state: web::Data<AppState>,
    tenant: web::ReqData<TenantId>,
    run_id: web::Path<Uuid>,
) -> HttpResponse {
    let ops = AgentGovernanceOps::new(state.db.clone());
    respond(
        ops.read_run(tenant.into_inner().0, run_id.into_inner())
            .await,
    )
}

/// Registers governance handlers. The parent scope must apply `AuthMiddleware` outermost and
/// `RequirePermission`; exact operation-catalog permissions remain authoritative per route.
pub fn routes(config: &mut ServiceConfig) {
    config
        .service(readiness)
        .service(capabilities)
        .service(usage_options)
        .service(usage)
        .service(usage_export)
        .service(runs)
        .service(run_detail);
}

fn respond<T: Serialize>(result: Result<T, GovernanceError>) -> HttpResponse {
    match result {
        Ok(data) => HttpResponse::Ok()
            .insert_header((header::CACHE_CONTROL, "no-store"))
            .json(ApiResponse::from_status(StatusCode::OK, Some(data), None)),
        Err(error) => governance_error(error),
    }
}

fn governance_error(error: GovernanceError) -> HttpResponse {
    let (status, issue) = match error {
        GovernanceError::Invalid { message, .. } => (StatusCode::BAD_REQUEST, message),
        GovernanceError::NotFound => (StatusCode::NOT_FOUND, "Agent run was not found"),
        GovernanceError::DependencyUnavailable | GovernanceError::Storage(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Agent administration data could not be loaded",
        ),
    };
    HttpResponse::build(status)
        .insert_header((header::CACHE_CONTROL, "no-store"))
        .json(ApiResponse::from_status(
            status,
            None::<()>,
            Some(vec![issue.to_string()]),
        ))
}

const fn worker_readiness_reason(available: bool, registered_instances: i64) -> &'static str {
    if available {
        "ready"
    } else if registered_instances == 0 {
        "not_registered"
    } else {
        "no_fresh_ready_worker"
    }
}

fn module_state_index(
    modules: Vec<crate::services::access::models::TenantModule>,
) -> BTreeMap<String, TenantModuleResponse> {
    modules
        .into_iter()
        .map(TenantModuleResponse::from)
        .map(|module| (module.key.clone(), module))
        .collect()
}

fn capability_item(
    operation: &ProductOperation,
    descriptor: Option<&CapabilityDescriptor>,
    module_states: &BTreeMap<String, TenantModuleResponse>,
    module_labels: &BTreeMap<&str, &str>,
) -> CapabilityInventoryItem {
    let exposure = operation.agent_exposure();
    let module_enabled = module_states
        .get(operation.module_key())
        .is_some_and(|module| module.enabled);
    let missing_dependencies = operation
        .required_modules()
        .filter(|module| {
            !module_states
                .get(*module)
                .is_some_and(|state| state.enabled)
        })
        .map(str::to_string)
        .collect::<Vec<_>>();
    let (availability, availability_reason) = match exposure {
        AgentExposure::HumanOnly { reason } => ("human_only", Some(reason.to_string())),
        AgentExposure::Prohibited { reason } => ("prohibited", Some(reason.to_string())),
        AgentExposure::ApprovalRequired => (
            "approval_not_released",
            Some("Proposal and approval execution is not released".to_string()),
        ),
        AgentExposure::Exposed if !module_enabled => (
            "module_unavailable",
            Some("The owning module is not currently enabled".to_string()),
        ),
        AgentExposure::Exposed if !missing_dependencies.is_empty() => (
            "module_unavailable",
            Some(format!(
                "Required module unavailable: {}",
                missing_dependencies.join(", ")
            )),
        ),
        AgentExposure::Exposed if descriptor.is_none() => (
            "handler_unavailable",
            Some("No executable capability handler is registered".to_string()),
        ),
        AgentExposure::Exposed => ("executable", None),
    };
    CapabilityInventoryItem {
        operation_key: operation.key().to_string(),
        label: descriptor
            .map(|value| value.title().to_string())
            .unwrap_or_else(|| humanize_operation_key(operation.key())),
        module_key: operation.module_key().to_string(),
        module_label: module_labels
            .get(operation.module_key())
            .copied()
            .unwrap_or(operation.module_key())
            .to_string(),
        permission: operation.permission().to_string(),
        effect: operation_effect(operation.effect()).to_string(),
        exposure: exposure.as_str().to_string(),
        exposure_reason: exposure.reason().map(str::to_string),
        availability: availability.to_string(),
        availability_reason,
        capability_version: descriptor.map(|value| value.version().get()),
        required_modules: operation.required_modules().map(str::to_string).collect(),
        required_features: operation.required_features().map(str::to_string).collect(),
    }
}

fn capability_summary(items: &[CapabilityInventoryItem]) -> CapabilityInventorySummary {
    CapabilityInventorySummary {
        total: items.len(),
        exposed: items
            .iter()
            .filter(|item| item.exposure == "exposed")
            .count(),
        approval_required: items
            .iter()
            .filter(|item| item.exposure == "approval_required")
            .count(),
        human_only: items
            .iter()
            .filter(|item| item.exposure == "human_only")
            .count(),
        prohibited: items
            .iter()
            .filter(|item| item.exposure == "prohibited")
            .count(),
        executable: items
            .iter()
            .filter(|item| item.availability == "executable")
            .count(),
    }
}

fn operation_effect(effect: OperationEffect) -> &'static str {
    match effect {
        OperationEffect::Read => "read",
        OperationEffect::Export => "export",
        OperationEffect::LicenseRepair => "license_repair",
        OperationEffect::Write => "write",
        OperationEffect::Destructive => "destructive",
        OperationEffect::External => "external_side_effect",
    }
}

fn humanize_operation_key(key: &str) -> String {
    let value = key.rsplit('.').next().unwrap_or(key).replace('_', " ");
    let mut characters = value.chars();
    characters.next().map_or(value.clone(), |first| {
        format!("{}{}", first.to_uppercase(), characters.as_str())
    })
}

fn page_count(total: usize, per_page: u16) -> u32 {
    if total == 0 {
        0
    } else {
        total.div_ceil(usize::from(per_page)) as u32
    }
}

fn parse_page(value: Option<u32>) -> Result<u32, GovernanceError> {
    let value = value.unwrap_or(1);
    if value == 0 || value > MAX_PAGE {
        return Err(GovernanceError::invalid(
            "invalid_page",
            "Page must be between 1 and 1000",
        ));
    }
    Ok(value)
}

fn parse_page_size(value: Option<u16>) -> Result<u16, GovernanceError> {
    let value = value.unwrap_or(DEFAULT_PAGE_SIZE);
    if value == 0 || value > MAX_PAGE_SIZE {
        return Err(GovernanceError::invalid(
            "invalid_page_size",
            "Page size must be between 1 and 100",
        ));
    }
    Ok(value)
}

fn parse_datetime(value: &str) -> Result<DateTime<Utc>, GovernanceError> {
    DateTime::parse_from_rfc3339(value)
        .map(|value| value.with_timezone(&Utc))
        .map_err(|_| {
            GovernanceError::invalid(
                "invalid_report_datetime",
                "Use a complete ISO 8601 UTC date and time",
            )
        })
}

fn parse_uuid(value: &str, code: &'static str) -> Result<Uuid, GovernanceError> {
    Uuid::parse_str(value)
        .ok()
        .filter(|value| !value.is_nil())
        .ok_or_else(|| GovernanceError::invalid(code, "Choose a valid identifier"))
}

fn bounded_search(value: &str) -> Result<String, GovernanceError> {
    let value = value.trim();
    if value.is_empty() || value.chars().count() > 120 {
        return Err(GovernanceError::invalid(
            "invalid_search",
            "Search must contain between 1 and 120 characters",
        ));
    }
    Ok(value.to_string())
}

fn bounded_text(
    value: &str,
    maximum: usize,
    code: &'static str,
) -> Result<String, GovernanceError> {
    let value = value.trim();
    if value.is_empty() || value.chars().count() > maximum || value.chars().any(char::is_control) {
        return Err(GovernanceError::invalid(
            code,
            "Choose a valid filter value",
        ));
    }
    Ok(value.to_string())
}

fn stable_filter(value: &str, code: &'static str) -> Result<String, GovernanceError> {
    let value = bounded_text(value, 240, code)?;
    if value != value.to_lowercase()
        || !value.split('.').all(|segment| {
            !segment.is_empty()
                && segment
                    .chars()
                    .next()
                    .is_some_and(|character| character.is_ascii_lowercase())
                && segment.chars().all(|character| {
                    character.is_ascii_lowercase()
                        || character.is_ascii_digit()
                        || matches!(character, '_' | '-')
                })
        })
    {
        return Err(GovernanceError::invalid(
            code,
            "Choose a valid filter value",
        ));
    }
    Ok(value)
}

fn one_of(value: &str, allowed: &[&str], code: &'static str) -> Result<String, GovernanceError> {
    allowed
        .contains(&value)
        .then(|| value.to_string())
        .ok_or_else(|| GovernanceError::invalid(code, "Choose a supported filter value"))
}

fn one_of_owned(
    value: &str,
    allowed: &[String],
    code: &'static str,
) -> Result<String, GovernanceError> {
    allowed
        .iter()
        .any(|allowed| allowed == value)
        .then(|| value.to_string())
        .ok_or_else(|| GovernanceError::invalid(code, "Choose a supported filter value"))
}

fn usage_meter_keys() -> Vec<String> {
    [
        "agent.runs",
        "agent.provider_attempts",
        "agent.capability_calls",
        "agent.input_tokens",
        "agent.output_tokens",
        "agent.cached_input_tokens",
        "agent.reasoning_tokens",
        "agent.provider_reported_cost",
        "agent.estimated_cost",
    ]
    .into_iter()
    .map(str::to_string)
    .collect()
}

fn csv_field(value: &str) -> String {
    if value
        .chars()
        .any(|character| matches!(character, ',' | '"' | '\n' | '\r'))
    {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

fn csv_text_field(value: &str) -> String {
    let begins_with_formula = value
        .chars()
        .find(|character| !character.is_whitespace() && !character.is_control())
        .is_some_and(|character| matches!(character, '=' | '+' | '-' | '@'));
    if !begins_with_formula {
        return csv_field(value);
    }

    let mut literal = String::with_capacity(value.len() + 1);
    literal.push('\'');
    literal.push_str(value);
    csv_field(&literal)
}

const USAGE_FILTERS_SQL: &str = r#"
    event.tenant_id = $1
    AND event.occurred_at >= $2
    AND event.occurred_at < $3
    AND event.deleted_at IS NULL
    AND measure.deleted_at IS NULL
    AND ($4::UUID IS NULL OR event.actor_user_id = $4)
    AND ($5::TEXT IS NULL OR event.origin_module_key = $5)
    AND ($6::TEXT IS NULL OR event.capability_module_key = $6)
    AND ($7::TEXT IS NULL OR event.capability_key = $7)
    AND ($8::TEXT IS NULL OR event.provider_key = $8)
    AND ($9::TEXT IS NULL OR event.provider_model_id = $9)
    AND ($10::TEXT IS NULL OR event.outcome = $10)
    AND ($11::TEXT IS NULL OR measure.meter_key = $11)
"#;

fn usage_totals_sql() -> String {
    format!(
        r#"
    SELECT measure.meter_key,
           COALESCE(SUM(measure.amount), 0)::BIGINT AS known_amount,
           COUNT(*) FILTER (WHERE measure.amount IS NULL)::BIGINT AS unknown_events,
           measure.currency_code, measure.currency_exponent, measure.pricing_version
    FROM agent_usage_events event
    INNER JOIN agent_usage_measures measure
      ON measure.usage_event_id = event.id AND measure.tenant_id = event.tenant_id
    WHERE
    {USAGE_FILTERS_SQL}
    GROUP BY measure.meter_key, measure.currency_code,
             measure.currency_exponent, measure.pricing_version
    ORDER BY measure.meter_key, measure.currency_code NULLS FIRST,
             measure.currency_exponent NULLS FIRST, measure.pricing_version NULLS FIRST
    "#
    )
}

fn usage_trend_sql() -> String {
    format!(
        r#"
    SELECT DATE_TRUNC('day', event.occurred_at) AS day,
           measure.meter_key,
           COALESCE(SUM(measure.amount), 0)::BIGINT AS known_amount,
           COUNT(*) FILTER (WHERE measure.amount IS NULL)::BIGINT AS unknown_events,
           measure.currency_code, measure.currency_exponent, measure.pricing_version
    FROM agent_usage_events event
    INNER JOIN agent_usage_measures measure
      ON measure.usage_event_id = event.id AND measure.tenant_id = event.tenant_id
    WHERE
    {USAGE_FILTERS_SQL}
    GROUP BY day, measure.meter_key, measure.currency_code,
             measure.currency_exponent, measure.pricing_version
    ORDER BY day, measure.meter_key, measure.currency_code NULLS FIRST,
             measure.currency_exponent NULLS FIRST, measure.pricing_version NULLS FIRST
    "#
    )
}

fn usage_export_sql() -> String {
    format!(
        r#"
    SELECT event.occurred_at, event.id AS event_id, event.actor_user_id,
           actor.full_name, event.event_kind, event.outcome, event.origin_module_key,
           event.capability_module_key, event.capability_key, event.provider_key,
           event.provider_model_id, measure.meter_key, measure.amount,
           measure.currency_code, measure.currency_exponent, measure.pricing_version
    FROM agent_usage_events event
    INNER JOIN agent_usage_measures measure
      ON measure.usage_event_id = event.id AND measure.tenant_id = event.tenant_id
    INNER JOIN users actor
      ON actor.id = event.actor_user_id AND actor.tenant_id = event.tenant_id
    WHERE
    {USAGE_FILTERS_SQL}
    ORDER BY event.occurred_at DESC, event.id DESC, measure.meter_key
    LIMIT $12
    "#
    )
}

const RUN_AUDIT_LIST_SQL: &str = r#"
    SELECT run.id, run.correlation_id, run.thread_id AS session_id,
           session.title AS session_title, run.requested_by AS requested_by_id,
           actor.full_name AS requested_by_name, run.task_class,
           run.origin_module_key, run.status, run.safe_failure_code,
           run.started_at, run.finished_at, run.created_at,
           (SELECT COUNT(*) FROM agent_provider_attempts attempt
            WHERE attempt.tenant_id = run.tenant_id AND attempt.run_id = run.id
              AND attempt.deleted_at IS NULL) AS provider_attempts,
           (SELECT COUNT(*) FROM agent_capability_calls call
            WHERE call.tenant_id = run.tenant_id AND call.run_id = run.id
              AND call.deleted_at IS NULL) AS capability_calls,
           COUNT(*) OVER() AS total_count
    FROM agent_runs run
    INNER JOIN agent_threads session
      ON session.id = run.thread_id AND session.tenant_id = run.tenant_id
    INNER JOIN users actor
      ON actor.id = run.requested_by AND actor.tenant_id = run.tenant_id
    WHERE run.tenant_id = $1
      AND run.created_at >= $2 AND run.created_at < $3
      AND run.deleted_at IS NULL
      AND session.deleted_at IS NULL
      AND actor.deleted_at IS NULL
      AND ($4::TEXT IS NULL OR run.status = $4)
      AND ($5::UUID IS NULL OR run.requested_by = $5)
      AND ($6::TEXT IS NULL OR run.origin_module_key = $6)
      AND ($7::UUID IS NULL OR run.correlation_id = $7)
      AND ($8::TEXT IS NULL OR POSITION(LOWER($8) IN LOWER(session.title)) > 0)
    ORDER BY run.created_at DESC, run.id DESC
    LIMIT $9 OFFSET $10
"#;

const RUN_DETAIL_SQL: &str = r#"
    SELECT run.id, run.correlation_id, run.thread_id AS session_id,
           session.title AS session_title, run.requested_by AS requested_by_id,
           actor.full_name AS requested_by_name, run.task_class,
           run.origin_module_key, run.status, run.safe_failure_code,
           run.started_at, run.finished_at, run.created_at,
           (SELECT COUNT(*) FROM agent_provider_attempts attempt
            WHERE attempt.tenant_id = run.tenant_id AND attempt.run_id = run.id
              AND attempt.deleted_at IS NULL) AS provider_attempts,
           (SELECT COUNT(*) FROM agent_capability_calls call
            WHERE call.tenant_id = run.tenant_id AND call.run_id = run.id
              AND call.deleted_at IS NULL) AS capability_calls
    FROM agent_runs run
    INNER JOIN agent_threads session
      ON session.id = run.thread_id AND session.tenant_id = run.tenant_id
    INNER JOIN users actor
      ON actor.id = run.requested_by AND actor.tenant_id = run.tenant_id
    WHERE run.tenant_id = $1 AND run.id = $2
      AND run.deleted_at IS NULL AND session.deleted_at IS NULL
"#;

const PROVIDER_ATTEMPT_AUDIT_SQL: &str = r#"
    SELECT id, turn_index, attempt_index, provider_key, provider_model_id,
           status, failure_origin, failure_category, input_tokens, output_tokens,
           cached_tokens, reasoning_tokens, provider_reported_cost_amount,
           provider_reported_cost_currency, provider_reported_cost_exponent,
           estimated_cost_amount, estimated_cost_currency, estimated_cost_exponent,
           started_at, finished_at
    FROM agent_provider_attempts
    WHERE tenant_id = $1 AND run_id = $2 AND deleted_at IS NULL
    ORDER BY turn_index, attempt_index, id
"#;

const CAPABILITY_CALL_AUDIT_SQL: &str = r#"
    SELECT id, call_sequence, capability_key, capability_version,
           owning_module_key, required_permission, scope_kind,
           CASE WHEN scope_kind = 'resources'
                THEN JSONB_ARRAY_LENGTH(resource_references)::BIGINT
                ELSE 0::BIGINT END AS resource_count,
           status, safe_failure_code, duration_ms, started_at, finished_at
    FROM agent_capability_calls
    WHERE tenant_id = $1 AND run_id = $2 AND deleted_at IS NULL
    ORDER BY call_sequence, id
"#;

const RUN_EVENT_AUDIT_SQL: &str = r#"
    SELECT id::TEXT AS event_id, event_type, created_at
    FROM agent_run_events
    WHERE tenant_id = $1 AND run_id = $2 AND deleted_at IS NULL
    ORDER BY id
"#;

const ACTOR_AUDIT_SQL: &str = r#"
    SELECT event.id, event.actor_type, actor.full_name AS actor_name,
           event.action_key, event.target_type, event.outcome, event.occurred_at
    FROM actor_audit_events event
    LEFT JOIN users actor
      ON actor.id = event.actor_user_id AND actor.tenant_id = event.tenant_id
    WHERE event.tenant_id = $1
      AND (event.agent_run_id = $2 OR event.correlation_id = $3)
      AND event.deleted_at IS NULL
    ORDER BY event.occurred_at, event.id
    LIMIT 500
"#;

#[cfg(test)]
mod tests {
    use actix_web::http::header;
    use chrono::{Duration, Utc};

    use super::{
        CapabilityInventoryQuery, GovernanceError, ParsedCapabilityInventoryQuery, ReportRange,
        UsageQuery, csv_field, csv_text_field, governance_error, page_count, respond,
        worker_readiness_reason,
    };

    #[test]
    fn capability_filters_are_bounded_and_closed() {
        assert!(
            ParsedCapabilityInventoryQuery::parse(CapabilityInventoryQuery {
                exposure: Some("exposed".to_string()),
                availability: Some("executable".to_string()),
                page: Some(1),
                per_page: Some(100),
                ..CapabilityInventoryQuery::default()
            })
            .is_ok()
        );
        assert!(
            ParsedCapabilityInventoryQuery::parse(CapabilityInventoryQuery {
                exposure: Some("anything".to_string()),
                ..CapabilityInventoryQuery::default()
            })
            .is_err()
        );
        assert_eq!(page_count(0, 50), 0);
        assert_eq!(page_count(101, 50), 3);
    }

    #[test]
    fn reporting_range_is_utc_ordered_and_capped() {
        let to = Utc::now();
        let from = to - Duration::days(31);
        assert!(ReportRange::parse(Some(&from.to_rfc3339()), Some(&to.to_rfc3339())).is_ok());
        let too_old = to - Duration::days(93);
        assert!(ReportRange::parse(Some(&too_old.to_rfc3339()), Some(&to.to_rfc3339())).is_err());
        assert!(ReportRange::parse(Some(&to.to_rfc3339()), Some(&from.to_rfc3339())).is_err());
        assert!(
            super::ParsedUsageQuery::parse(UsageQuery {
                provider: Some("provider-owned-key".to_string()),
                ..UsageQuery::default()
            })
            .is_ok()
        );
    }

    #[test]
    fn csv_fields_escape_delimiters_quotes_and_lines() {
        assert_eq!(csv_field("plain"), "plain");
        assert_eq!(csv_field("School, North"), "\"School, North\"");
        assert_eq!(
            csv_field("A \"quoted\" value"),
            "\"A \"\"quoted\"\" value\""
        );
        assert_eq!(csv_field("line\nbreak"), "\"line\nbreak\"");
    }

    #[test]
    fn csv_text_fields_neutralize_spreadsheet_formulas() {
        assert_eq!(csv_text_field("=2+3"), "'=2+3");
        assert_eq!(csv_text_field("  @SUM(A1:A2)"), "'  @SUM(A1:A2)");
        assert_eq!(csv_text_field("\t-cmd"), "'\t-cmd");
        assert_eq!(csv_text_field("+27 11 555 0100"), "'+27 11 555 0100");
        assert_eq!(csv_text_field("ordinary text"), "ordinary text");
        assert_eq!(csv_field("-42"), "-42");
        assert_eq!(csv_field("2026-08-31T12:00:00Z"), "2026-08-31T12:00:00Z");
    }

    #[test]
    fn readiness_reason_distinguishes_absent_stale_and_ready_workers() {
        assert_eq!(worker_readiness_reason(true, 1), "ready");
        assert_eq!(worker_readiness_reason(false, 0), "not_registered");
        assert_eq!(worker_readiness_reason(false, 2), "no_fresh_ready_worker");
    }

    #[test]
    fn governance_json_and_error_responses_are_never_cacheable() {
        let success = respond::<serde_json::Value>(Ok(serde_json::json!({ "ready": true })));
        assert_eq!(
            success.headers().get(header::CACHE_CONTROL).unwrap(),
            "no-store"
        );
        let error = governance_error(GovernanceError::NotFound);
        assert_eq!(
            error.headers().get(header::CACHE_CONTROL).unwrap(),
            "no-store"
        );
    }
}
