//! Implements tenant-scoped AI route persistence and fail-closed resolution.
//!
//! Full-chain replacements, archives, and actor-aware audit evidence share one
//! transaction. A matched but unusable scope never falls through to a broader one.

use std::{
    collections::{HashMap, HashSet},
    str::FromStr,
};

use chrono::{DateTime, Utc};
use cp_audit::{AuditActor, AuditOutcome, AuditTarget, NewAuditEvent, RequestContext};
use cp_common::{
    ProviderApprovalClass, ProviderDataClass, ProviderDataEligibilityError,
    ProviderExecutionEnvironmentClass, evaluate_provider_data_eligibility,
};
use serde_json::{Map, Value};
use sqlx::{Executor, FromRow, PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::types::{
    AiRouteScope, AiRouteSet, AiRouteTarget, AiRoutingError, ArchiveRouteCommand, ArchivedAiRoute,
    CreateRouteCommand, ReplaceRouteCommand, ResolveRouteCommand, ResolvedAiRoute,
    ResolvedAiRouteTarget, RoutePrecedence, RouteTargetDraft, RouteTargetReadiness,
    RouteUnusableReason,
};

const ROUTE_SET_PROJECTION: &str = r#"
    SELECT id, scope_kind, task_class, module_key, operation_class,
           capability_key, capability_version, version, created_at, updated_at
    FROM ai_route_sets
"#;

const ROUTE_TARGET_PROJECTION: &str = r#"
    SELECT
        r.id,
        r.route_set_id,
        r.priority,
        r.connection_id,
        r.model_id,
        r.provider_data_approval_id,
        r.requires_tools,
        c.provider,
        c.account_label,
        c.status AS connection_status,
        c.credential_version AS current_credential_version,
        c.model_catalog_version AS current_catalog_version,
        c.deleted_at AS connection_deleted_at,
        pinned_approval.approval_version AS provider_data_approval_version,
        pinned_approval.approval_class AS provider_data_approval_class,
        latest_approval.id AS current_provider_data_approval_id,
        'external_managed'::TEXT AS execution_environment_class,
        m.provider_model_id,
        m.display_name AS model_display_name,
        m.context_window_tokens,
        m.max_output_tokens,
        m.supports_tools,
        m.credential_version AS model_credential_version,
        m.catalog_version AS model_catalog_version,
        m.deleted_at AS model_deleted_at
    FROM ai_task_routes r
    JOIN ai_provider_connections c
      ON c.id = r.connection_id AND c.tenant_id = r.tenant_id
    JOIN ai_provider_models m
      ON m.id = r.model_id AND m.tenant_id = r.tenant_id
    JOIN ai_provider_data_approval_versions pinned_approval
      ON pinned_approval.id = r.provider_data_approval_id
     AND pinned_approval.tenant_id = r.tenant_id
     AND pinned_approval.connection_id = r.connection_id
    JOIN LATERAL (
        SELECT approval.id
        FROM ai_provider_data_approval_versions approval
        WHERE approval.tenant_id = r.tenant_id
          AND approval.connection_id = r.connection_id
        ORDER BY approval.approval_version DESC
        LIMIT 1
    ) latest_approval ON TRUE
"#;

/// Shared routing service used by Administration APIs and the Agent worker.
#[derive(Debug, Clone)]
pub struct AiRoutingOps {
    pool: PgPool,
}

impl AiRoutingOps {
    #[must_use]
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Builds the same service boundary for list, read, and resolve consumers.
    #[must_use]
    pub fn for_reads(pool: PgPool) -> Self {
        Self::new(pool)
    }

    /// Lists current route sets with secret-free target readiness.
    pub async fn list_routes(&self, tenant_id: Uuid) -> Result<Vec<AiRouteSet>, AiRoutingError> {
        let query = format!(
            "{ROUTE_SET_PROJECTION} WHERE tenant_id = $1 AND deleted_at IS NULL \
             ORDER BY CASE scope_kind \
                 WHEN 'tenant_default' THEN 1 \
                 WHEN 'task_class' THEN 2 \
                 WHEN 'module_operation' THEN 3 \
                 WHEN 'capability' THEN 4 END, \
             task_class NULLS FIRST, module_key NULLS FIRST, operation_class NULLS FIRST, \
             capability_key NULLS FIRST, capability_version NULLS FIRST, id"
        );
        let rows = sqlx::query_as::<_, RouteSetRow>(&query)
            .bind(tenant_id)
            .fetch_all(&self.pool)
            .await?;
        if rows.is_empty() {
            return Ok(Vec::new());
        }
        let route_set_ids = rows.iter().map(|row| row.id).collect::<Vec<_>>();
        let target_rows = load_target_rows(&self.pool, tenant_id, &route_set_ids).await?;
        assemble_routes(rows, target_rows, false, ProviderDataClass::CampusApproved)
    }

    /// Reads one current route set without returning provider credentials or fingerprints.
    pub async fn read_route(
        &self,
        tenant_id: Uuid,
        route_set_id: Uuid,
    ) -> Result<AiRouteSet, AiRoutingError> {
        let row = load_route_set(&self.pool, tenant_id, route_set_id).await?;
        let target_rows = load_target_rows(&self.pool, tenant_id, &[route_set_id]).await?;
        assemble_one_route(row, target_rows, false, ProviderDataClass::CampusApproved)
    }

    /// Creates one unique active scope and its complete immutable target chain.
    pub async fn create_route(
        &self,
        tenant_id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        command: CreateRouteCommand,
    ) -> Result<AiRouteSet, AiRoutingError> {
        let configured_by = actor_user_id(actor)?;
        let mut transaction = self.pool.begin().await?;
        let targets = validate_chain(
            &mut transaction,
            tenant_id,
            command.targets(),
            command.requires_tools,
        )
        .await?;
        let route_set_id = Uuid::new_v4();
        let insert = sqlx::query(
            r#"
            INSERT INTO ai_route_sets (
                id, tenant_id, scope_kind, task_class, module_key, operation_class,
                capability_key, capability_version, configured_by, change_reason
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            "#,
        )
        .bind(route_set_id)
        .bind(tenant_id)
        .bind(command.scope.kind())
        .bind(command.scope.task_class().map(|value| value.as_str()))
        .bind(command.scope.module_key())
        .bind(command.scope.operation_class().map(|value| value.as_str()))
        .bind(command.scope.capability_key())
        .bind(command.scope.capability_version())
        .bind(configured_by)
        .bind(command.reason())
        .execute(&mut *transaction)
        .await;
        if let Err(error) = insert {
            return Err(map_write_error(error));
        }
        insert_targets(
            &mut transaction,
            tenant_id,
            route_set_id,
            configured_by,
            command.requires_tools,
            &targets,
        )
        .await?;
        append_audit(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "administration.ai_routing.routes.create",
            route_set_id,
            command.reason(),
            &command.scope,
            1,
            targets.len(),
        )
        .await?;
        transaction.commit().await?;
        self.read_route(tenant_id, route_set_id).await
    }

    /// Replaces the whole ordered chain under one optimistic route-set version.
    pub async fn replace_route(
        &self,
        tenant_id: Uuid,
        route_set_id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        command: ReplaceRouteCommand,
    ) -> Result<AiRouteSet, AiRoutingError> {
        let configured_by = actor_user_id(actor)?;
        let mut transaction = self.pool.begin().await?;
        let current = lock_route_set(&mut transaction, tenant_id, route_set_id).await?;
        if current.version != command.expected_version {
            return Err(stale_route());
        }
        let scope = route_scope_from_row(&current)?;
        let targets = validate_chain(
            &mut transaction,
            tenant_id,
            command.targets(),
            command.requires_tools,
        )
        .await?;

        archive_targets(&mut transaction, tenant_id, route_set_id).await?;
        let update = sqlx::query(
            r#"
            UPDATE ai_route_sets
            SET change_reason = $1,
                version = version + 1,
                updated_at = CLOCK_TIMESTAMP()
            WHERE tenant_id = $2 AND id = $3 AND version = $4 AND deleted_at IS NULL
            "#,
        )
        .bind(command.reason())
        .bind(tenant_id)
        .bind(route_set_id)
        .bind(command.expected_version)
        .execute(&mut *transaction)
        .await?;
        if update.rows_affected() != 1 {
            return Err(stale_route());
        }
        insert_targets(
            &mut transaction,
            tenant_id,
            route_set_id,
            configured_by,
            command.requires_tools,
            &targets,
        )
        .await?;
        let next_version = command
            .expected_version
            .checked_add(1)
            .ok_or_else(stale_route)?;
        append_audit(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "administration.ai_routing.routes.update",
            route_set_id,
            command.reason(),
            &scope,
            next_version,
            targets.len(),
        )
        .await?;
        transaction.commit().await?;
        self.read_route(tenant_id, route_set_id).await
    }

    /// Archives a route and every target without deleting audit or target history.
    pub async fn archive_route(
        &self,
        tenant_id: Uuid,
        route_set_id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        command: ArchiveRouteCommand,
    ) -> Result<ArchivedAiRoute, AiRoutingError> {
        actor_user_id(actor)?;
        let mut transaction = self.pool.begin().await?;
        let current = lock_route_set(&mut transaction, tenant_id, route_set_id).await?;
        if current.version != command.expected_version {
            return Err(stale_route());
        }
        let scope = route_scope_from_row(&current)?;
        archive_targets(&mut transaction, tenant_id, route_set_id).await?;
        let update = sqlx::query(
            r#"
            UPDATE ai_route_sets
            SET archived_reason = $1,
                deleted_at = CLOCK_TIMESTAMP(),
                version = version + 1,
                updated_at = CLOCK_TIMESTAMP()
            WHERE tenant_id = $2 AND id = $3 AND version = $4 AND deleted_at IS NULL
            "#,
        )
        .bind(command.reason())
        .bind(tenant_id)
        .bind(route_set_id)
        .bind(command.expected_version)
        .execute(&mut *transaction)
        .await?;
        if update.rows_affected() != 1 {
            return Err(stale_route());
        }
        let next_version = command
            .expected_version
            .checked_add(1)
            .ok_or_else(stale_route)?;
        append_audit(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "administration.ai_routing.routes.archive",
            route_set_id,
            command.reason(),
            &scope,
            next_version,
            0,
        )
        .await?;
        transaction.commit().await?;
        Ok(ArchivedAiRoute {
            archived_id: route_set_id,
            version: next_version,
        })
    }

    /// Resolves exact scope precedence and rejects a matched stale chain in place.
    pub async fn resolve_route(
        &self,
        tenant_id: Uuid,
        command: ResolveRouteCommand,
    ) -> Result<ResolvedAiRoute, AiRoutingError> {
        let mut transaction = self.pool.begin().await?;
        for scope in command.candidate_scopes() {
            let Some(row) = find_route_set_by_scope(&mut transaction, tenant_id, &scope).await?
            else {
                continue;
            };
            let target_rows = load_target_rows(&mut *transaction, tenant_id, &[row.id]).await?;
            let resolved_target_rows = target_rows.clone();
            let route = assemble_one_route(
                row,
                target_rows,
                command.requires_tools,
                command.required_provider_data_class,
            )?;
            if route.targets.is_empty() {
                return Err(AiRoutingError::UnusableRoute {
                    route_set_id: route.id,
                    reason: RouteUnusableReason::EmptyChain,
                });
            }
            if let Some(target) = route
                .targets
                .iter()
                .find(|target| target.readiness != RouteTargetReadiness::Ready)
            {
                return Err(AiRoutingError::UnusableRoute {
                    route_set_id: route.id,
                    reason: unusable_reason(target.readiness),
                });
            }
            if route.targets.len() != resolved_target_rows.len() {
                return Err(stored_route_invariant());
            }
            let targets = route
                .targets
                .into_iter()
                .zip(resolved_target_rows)
                .map(|(projection, row)| {
                    // Readiness proved the immutable model snapshot belongs to
                    // the connection's current credential. Pin that snapshot
                    // version; never reload the mutable connection for routing.
                    ResolvedAiRouteTarget::from_ready_projection(
                        projection,
                        row.model_credential_version,
                    )
                    .ok_or_else(stored_route_invariant)
                })
                .collect::<Result<Vec<_>, _>>()?;
            transaction.commit().await?;
            return Ok(ResolvedAiRoute {
                route_set_id: route.id,
                matched_scope: route.scope.clone(),
                precedence: RoutePrecedence::for_scope(&route.scope),
                route_version: route.version,
                requires_tools: route.requires_tools,
                required_provider_data_class: command.required_provider_data_class,
                targets,
            });
        }
        Err(AiRoutingError::NoMatchingRoute)
    }
}

#[derive(Debug, Clone, FromRow)]
struct RouteSetRow {
    id: Uuid,
    scope_kind: String,
    task_class: Option<String>,
    module_key: Option<String>,
    operation_class: Option<String>,
    capability_key: Option<String>,
    capability_version: Option<i32>,
    version: i64,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
struct RouteTargetRow {
    id: Uuid,
    route_set_id: Uuid,
    priority: i16,
    connection_id: Uuid,
    model_id: Uuid,
    provider_data_approval_id: Uuid,
    requires_tools: bool,
    provider: String,
    account_label: String,
    connection_status: String,
    current_credential_version: i64,
    current_catalog_version: i64,
    connection_deleted_at: Option<DateTime<Utc>>,
    provider_data_approval_version: i64,
    provider_data_approval_class: String,
    current_provider_data_approval_id: Uuid,
    execution_environment_class: String,
    provider_model_id: String,
    model_display_name: String,
    context_window_tokens: Option<i64>,
    max_output_tokens: Option<i64>,
    supports_tools: Option<bool>,
    model_credential_version: i64,
    model_catalog_version: i64,
    model_deleted_at: Option<DateTime<Utc>>,
}

impl RouteTargetRow {
    #[cfg(test)]
    fn readiness(&self, requested_requires_tools: bool) -> RouteTargetReadiness {
        self.readiness_for(requested_requires_tools, ProviderDataClass::CampusApproved)
    }

    fn readiness_for(
        &self,
        requested_requires_tools: bool,
        required_provider_data_class: ProviderDataClass,
    ) -> RouteTargetReadiness {
        if self.connection_deleted_at.is_some() || self.connection_status != "ready" {
            return RouteTargetReadiness::ConnectionUnavailable;
        }
        if self.provider_data_approval_id != self.current_provider_data_approval_id {
            return RouteTargetReadiness::ProviderDataApprovalChanged;
        }
        let Ok(approval) = ProviderApprovalClass::from_str(&self.provider_data_approval_class)
        else {
            return RouteTargetReadiness::ProviderDataNotApproved;
        };
        let Ok(environment) =
            ProviderExecutionEnvironmentClass::from_str(&self.execution_environment_class)
        else {
            return RouteTargetReadiness::ProviderDataNotApproved;
        };
        if let Err(error) =
            evaluate_provider_data_eligibility(required_provider_data_class, approval, environment)
        {
            return match error {
                ProviderDataEligibilityError::ProviderDataNotApproved => {
                    RouteTargetReadiness::ProviderDataNotApproved
                }
                ProviderDataEligibilityError::LocalExecutionRequired => {
                    RouteTargetReadiness::LocalExecutionRequired
                }
            };
        }
        if self.model_deleted_at.is_some()
            || self.model_credential_version != self.current_credential_version
            || self.model_catalog_version != self.current_catalog_version
        {
            return RouteTargetReadiness::StaleModel;
        }
        if self.context_window_tokens.is_none_or(|value| value <= 0)
            || self.max_output_tokens.is_none_or(|value| value <= 0)
        {
            return RouteTargetReadiness::ModelLimitsUnavailable;
        }
        if (self.requires_tools || requested_requires_tools) && self.supports_tools != Some(true) {
            return RouteTargetReadiness::ToolsUnsupported;
        }
        RouteTargetReadiness::Ready
    }

    fn into_projection(
        self,
        requested_requires_tools: bool,
        required_provider_data_class: ProviderDataClass,
    ) -> Result<AiRouteTarget, AiRoutingError> {
        let readiness = self.readiness_for(requested_requires_tools, required_provider_data_class);
        let provider_data_approval_class =
            ProviderApprovalClass::from_str(&self.provider_data_approval_class)
                .map_err(|_| stored_route_invariant())?;
        let execution_environment_class =
            ProviderExecutionEnvironmentClass::from_str(&self.execution_environment_class)
                .map_err(|_| stored_route_invariant())?;
        Ok(AiRouteTarget {
            id: self.id,
            priority: self.priority,
            connection_id: self.connection_id,
            provider_data_approval_id: self.provider_data_approval_id,
            provider_data_approval_version: self.provider_data_approval_version,
            provider_data_approval_class,
            execution_environment_class,
            model_id: self.model_id,
            provider: self.provider,
            account_label: self.account_label,
            provider_model_id: self.provider_model_id,
            model_display_name: self.model_display_name,
            context_window_tokens: self.context_window_tokens,
            max_output_tokens: self.max_output_tokens,
            supports_tools: self.supports_tools,
            readiness,
        })
    }
}

#[derive(Debug, Clone, FromRow)]
struct ValidatedTarget {
    connection_id: Uuid,
    model_id: Uuid,
    provider_data_approval_id: Uuid,
}

async fn validate_chain(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    drafts: &[RouteTargetDraft],
    requires_tools: bool,
) -> Result<Vec<ValidatedTarget>, AiRoutingError> {
    let mut ordered_for_locking = drafts.to_vec();
    ordered_for_locking.sort_unstable_by(|left, right| {
        (left.connection_id, left.provider_model_id.as_str())
            .cmp(&(right.connection_id, right.provider_model_id.as_str()))
    });
    let mut validated = HashMap::with_capacity(drafts.len());
    for draft in ordered_for_locking {
        let row = sqlx::query_as::<_, TargetValidationRow>(
            r#"
            SELECT
                c.status,
                m.id AS model_id,
                m.supports_tools,
                approval.id AS provider_data_approval_id,
                approval.approval_class AS provider_data_approval_class
            FROM ai_provider_connections c
            JOIN ai_provider_models m
              ON m.connection_id = c.id
             AND m.tenant_id = c.tenant_id
             AND m.provider_model_id = $3
             AND m.credential_version = c.credential_version
             AND m.catalog_version = c.model_catalog_version
            JOIN LATERAL (
                SELECT current_approval.id, current_approval.approval_class
                FROM ai_provider_data_approval_versions current_approval
                WHERE current_approval.tenant_id = c.tenant_id
                  AND current_approval.connection_id = c.id
                ORDER BY current_approval.approval_version DESC
                LIMIT 1
            ) approval ON TRUE
            WHERE c.tenant_id = $1
              AND c.id = $2
              AND c.deleted_at IS NULL
              AND m.deleted_at IS NULL
            FOR SHARE OF c, m
            "#,
        )
        .bind(tenant_id)
        .bind(draft.connection_id)
        .bind(&draft.provider_model_id)
        .fetch_optional(&mut **transaction)
        .await?
        .ok_or_else(|| {
            AiRoutingError::invalid(
                "invalid_route_target",
                "Choose a current model from a connection in this campus",
            )
        })?;
        if row.status != "ready" {
            return Err(AiRoutingError::invalid(
                "connection_not_ready",
                "Every route target connection must be ready",
            ));
        }
        if row.provider_data_approval_class == "unapproved" {
            return Err(AiRoutingError::invalid(
                "provider_data_not_approved",
                "Every route target connection must have an explicit data approval",
            ));
        }
        if requires_tools && row.supports_tools != Some(true) {
            return Err(AiRoutingError::invalid(
                "tools_not_supported",
                "Every target in a tools route must have confirmed tool support",
            ));
        }
        validated.insert(
            draft.connection_id,
            ValidatedTarget {
                connection_id: draft.connection_id,
                model_id: row.model_id,
                provider_data_approval_id: row.provider_data_approval_id,
            },
        );
    }
    drafts
        .iter()
        .map(|draft| {
            validated
                .remove(&draft.connection_id)
                .ok_or_else(stored_route_invariant)
        })
        .collect()
}

#[derive(Debug, FromRow)]
struct TargetValidationRow {
    status: String,
    model_id: Uuid,
    supports_tools: Option<bool>,
    provider_data_approval_id: Uuid,
    provider_data_approval_class: String,
}

async fn insert_targets(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    route_set_id: Uuid,
    created_by: Uuid,
    requires_tools: bool,
    targets: &[ValidatedTarget],
) -> Result<(), AiRoutingError> {
    for (index, target) in targets.iter().enumerate() {
        let priority = route_priority(index)?;
        sqlx::query(
            r#"
            INSERT INTO ai_task_routes (
                id, tenant_id, route_set_id, priority, connection_id, model_id,
                provider_data_approval_id, requires_tools, created_by
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(tenant_id)
        .bind(route_set_id)
        .bind(priority)
        .bind(target.connection_id)
        .bind(target.model_id)
        .bind(target.provider_data_approval_id)
        .bind(requires_tools)
        .bind(created_by)
        .execute(&mut **transaction)
        .await
        .map_err(map_write_error)?;
    }
    Ok(())
}

fn route_priority(index: usize) -> Result<i16, AiRoutingError> {
    match index {
        0 => Ok(1),
        1 => Ok(2),
        2 => Ok(3),
        _ => Err(AiRoutingError::invalid(
            "invalid_route_chain",
            "Route chain is too long",
        )),
    }
}

async fn archive_targets(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    route_set_id: Uuid,
) -> Result<(), AiRoutingError> {
    sqlx::query(
        r#"
        UPDATE ai_task_routes
        SET deleted_at = CLOCK_TIMESTAMP(), updated_at = CLOCK_TIMESTAMP()
        WHERE tenant_id = $1 AND route_set_id = $2 AND deleted_at IS NULL
        "#,
    )
    .bind(tenant_id)
    .bind(route_set_id)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

async fn load_route_set<'e, E>(
    executor: E,
    tenant_id: Uuid,
    route_set_id: Uuid,
) -> Result<RouteSetRow, AiRoutingError>
where
    E: Executor<'e, Database = Postgres>,
{
    let query =
        format!("{ROUTE_SET_PROJECTION} WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL");
    sqlx::query_as::<_, RouteSetRow>(&query)
        .bind(tenant_id)
        .bind(route_set_id)
        .fetch_optional(executor)
        .await?
        .ok_or(AiRoutingError::NotFound)
}

async fn lock_route_set(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    route_set_id: Uuid,
) -> Result<RouteSetRow, AiRoutingError> {
    let query = format!(
        "{ROUTE_SET_PROJECTION} WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL FOR UPDATE"
    );
    sqlx::query_as::<_, RouteSetRow>(&query)
        .bind(tenant_id)
        .bind(route_set_id)
        .fetch_optional(&mut **transaction)
        .await?
        .ok_or(AiRoutingError::NotFound)
}

async fn find_route_set_by_scope(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    scope: &AiRouteScope,
) -> Result<Option<RouteSetRow>, AiRoutingError> {
    let query = format!(
        "{ROUTE_SET_PROJECTION} WHERE tenant_id = $1 AND deleted_at IS NULL \
         AND scope_kind = $2 \
         AND task_class IS NOT DISTINCT FROM $3 \
         AND module_key IS NOT DISTINCT FROM $4 \
         AND operation_class IS NOT DISTINCT FROM $5 \
         AND capability_key IS NOT DISTINCT FROM $6 \
         AND capability_version IS NOT DISTINCT FROM $7 \
         FOR SHARE"
    );
    sqlx::query_as::<_, RouteSetRow>(&query)
        .bind(tenant_id)
        .bind(scope.kind())
        .bind(scope.task_class().map(|value| value.as_str()))
        .bind(scope.module_key())
        .bind(scope.operation_class().map(|value| value.as_str()))
        .bind(scope.capability_key())
        .bind(scope.capability_version())
        .fetch_optional(&mut **transaction)
        .await
        .map_err(AiRoutingError::from)
}

async fn load_target_rows<'e, E>(
    executor: E,
    tenant_id: Uuid,
    route_set_ids: &[Uuid],
) -> Result<Vec<RouteTargetRow>, AiRoutingError>
where
    E: Executor<'e, Database = Postgres>,
{
    let query = format!(
        "{ROUTE_TARGET_PROJECTION} WHERE r.tenant_id = $1 \
         AND r.route_set_id = ANY($2) AND r.deleted_at IS NULL \
         ORDER BY r.route_set_id, r.priority, r.id"
    );
    sqlx::query_as::<_, RouteTargetRow>(&query)
        .bind(tenant_id)
        .bind(route_set_ids)
        .fetch_all(executor)
        .await
        .map_err(AiRoutingError::from)
}

fn assemble_routes(
    rows: Vec<RouteSetRow>,
    targets: Vec<RouteTargetRow>,
    requested_requires_tools: bool,
    required_provider_data_class: ProviderDataClass,
) -> Result<Vec<AiRouteSet>, AiRoutingError> {
    let mut by_route = HashMap::<Uuid, Vec<RouteTargetRow>>::new();
    for target in targets {
        by_route
            .entry(target.route_set_id)
            .or_default()
            .push(target);
    }
    rows.into_iter()
        .map(|row| {
            let targets = by_route.remove(&row.id).unwrap_or_default();
            assemble_one_route(
                row,
                targets,
                requested_requires_tools,
                required_provider_data_class,
            )
        })
        .collect()
}

fn assemble_one_route(
    row: RouteSetRow,
    targets: Vec<RouteTargetRow>,
    requested_requires_tools: bool,
    required_provider_data_class: ProviderDataClass,
) -> Result<AiRouteSet, AiRoutingError> {
    let requirements = targets
        .iter()
        .map(|target| target.requires_tools)
        .collect::<HashSet<_>>();
    if requirements.len() > 1 {
        return Err(stored_route_invariant());
    }
    let requires_tools = requirements.iter().next().copied().unwrap_or(false);
    let scope = route_scope_from_row(&row)?;
    Ok(AiRouteSet {
        id: row.id,
        scope,
        requires_tools,
        targets: targets
            .into_iter()
            .map(|target| {
                target.into_projection(requested_requires_tools, required_provider_data_class)
            })
            .collect::<Result<Vec<_>, _>>()?,
        version: row.version,
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
}

fn route_scope_from_row(row: &RouteSetRow) -> Result<AiRouteScope, AiRoutingError> {
    AiRouteScope::parse(
        &row.scope_kind,
        row.task_class.as_deref(),
        row.module_key.as_deref(),
        row.operation_class.as_deref(),
        row.capability_key.as_deref(),
        row.capability_version,
    )
    .map_err(|_| stored_route_invariant())
}

fn actor_user_id(actor: AuditActor) -> Result<Uuid, AiRoutingError> {
    actor.user_id().ok_or_else(|| {
        AiRoutingError::invalid(
            "route_actor_required",
            "AI routes must be changed by an authenticated person or approved Agent action",
        )
    })
}

fn map_write_error(error: sqlx::Error) -> AiRoutingError {
    if error
        .as_database_error()
        .is_some_and(|database_error| database_error.code().as_deref() == Some("23505"))
    {
        AiRoutingError::conflict(
            "route_scope_exists",
            "An active AI route already exists for this scope",
        )
    } else if error
        .as_database_error()
        .is_some_and(|database_error| database_error.code().as_deref() == Some("P0001"))
    {
        AiRoutingError::conflict(
            "route_target_changed",
            "A provider connection or model changed while the route was being saved",
        )
    } else {
        AiRoutingError::Storage(error)
    }
}

fn stale_route() -> AiRoutingError {
    AiRoutingError::conflict(
        "stale_route",
        "This AI route changed; reload it before trying again",
    )
}

fn stored_route_invariant() -> AiRoutingError {
    AiRoutingError::Storage(sqlx::Error::Protocol(
        "stored AI route violates its schema invariant".to_owned(),
    ))
}

fn unusable_reason(readiness: RouteTargetReadiness) -> RouteUnusableReason {
    match readiness {
        RouteTargetReadiness::Ready => RouteUnusableReason::EmptyChain,
        RouteTargetReadiness::ConnectionUnavailable => RouteUnusableReason::ConnectionUnavailable,
        RouteTargetReadiness::StaleModel => RouteUnusableReason::StaleModel,
        RouteTargetReadiness::ModelLimitsUnavailable => RouteUnusableReason::ModelLimitsUnavailable,
        RouteTargetReadiness::ToolsUnsupported => RouteUnusableReason::ToolsUnsupported,
        RouteTargetReadiness::ProviderDataNotApproved => {
            RouteUnusableReason::ProviderDataNotApproved
        }
        RouteTargetReadiness::ProviderDataApprovalChanged => {
            RouteUnusableReason::ProviderDataApprovalChanged
        }
        RouteTargetReadiness::LocalExecutionRequired => RouteUnusableReason::LocalExecutionRequired,
    }
}

#[allow(clippy::too_many_arguments)]
async fn append_audit(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    actor: AuditActor,
    request_context: RequestContext,
    action_key: &'static str,
    route_set_id: Uuid,
    reason: &str,
    scope: &AiRouteScope,
    version: i64,
    target_count: usize,
) -> Result<(), AiRoutingError> {
    let mut metadata = Map::new();
    metadata.insert(
        "scope_kind".to_owned(),
        Value::String(scope.kind().to_owned()),
    );
    metadata.insert("route_version".to_owned(), Value::from(version));
    metadata.insert("target_count".to_owned(), Value::from(target_count as u64));
    let event = NewAuditEvent::new(
        tenant_id,
        actor,
        action_key,
        AuditOutcome::Succeeded,
        request_context,
    )
    .with_target(AuditTarget::new("ai_route_set", route_set_id))
    .with_reason(reason)
    .with_redacted_metadata(metadata);
    cp_audit::append(&mut **transaction, &event).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        AiRoutingOps, ROUTE_SET_PROJECTION, ROUTE_TARGET_PROJECTION, RouteTargetRow,
        assemble_one_route, map_write_error, route_priority, route_scope_from_row, unusable_reason,
    };
    use crate::{AiRoutingError, RouteTargetReadiness, RouteUnusableReason};
    use chrono::Utc;
    use cp_audit::{AuditActor, RequestContext};
    use cp_common::{ProviderDataClass, ProviderDataEligibilityError};
    use sqlx::{PgPool, postgres::PgPoolOptions};
    use uuid::Uuid;

    const ROUTING_MIGRATION: &str =
        include_str!("../../../../migrations/083_create_ai_task_routing.sql");

    fn target_row(route_set_id: Uuid, priority: i16, requires_tools: bool) -> RouteTargetRow {
        let provider_data_approval_id = Uuid::new_v4();
        RouteTargetRow {
            id: Uuid::new_v4(),
            route_set_id,
            priority,
            connection_id: Uuid::new_v4(),
            model_id: Uuid::new_v4(),
            provider_data_approval_id,
            requires_tools,
            provider: "openai".to_owned(),
            account_label: "Primary".to_owned(),
            connection_status: "ready".to_owned(),
            current_credential_version: 2,
            current_catalog_version: 3,
            connection_deleted_at: None,
            provider_data_approval_version: 2,
            provider_data_approval_class: "sensitive_data_approved".to_owned(),
            current_provider_data_approval_id: provider_data_approval_id,
            execution_environment_class: "external_managed".to_owned(),
            provider_model_id: "gpt-test".to_owned(),
            model_display_name: "GPT Test".to_owned(),
            context_window_tokens: Some(1000),
            max_output_tokens: Some(250),
            supports_tools: Some(true),
            model_credential_version: 2,
            model_catalog_version: 3,
            model_deleted_at: None,
        }
    }

    #[test]
    fn target_readiness_rejects_approval_drift_and_every_external_local_only_route() {
        let route_set_id = Uuid::new_v4();
        let ready = target_row(route_set_id, 1, false);
        assert_eq!(
            ready.readiness_for(false, ProviderDataClass::SensitiveDataApproved),
            RouteTargetReadiness::Ready
        );

        let mut changed = ready.clone();
        changed.current_provider_data_approval_id = Uuid::new_v4();
        assert_eq!(
            changed.readiness_for(false, ProviderDataClass::CampusApproved),
            RouteTargetReadiness::ProviderDataApprovalChanged
        );

        let mut campus_only = ready.clone();
        campus_only.provider_data_approval_class = "campus_approved".to_owned();
        assert_eq!(
            campus_only.readiness_for(false, ProviderDataClass::SensitiveDataApproved),
            RouteTargetReadiness::ProviderDataNotApproved
        );
        assert_eq!(
            ready.readiness_for(false, ProviderDataClass::LocalOnly),
            RouteTargetReadiness::LocalExecutionRequired
        );
        assert_eq!(
            cp_common::evaluate_provider_data_eligibility(
                ProviderDataClass::LocalOnly,
                cp_common::ProviderApprovalClass::SensitiveDataApproved,
                cp_common::ProviderExecutionEnvironmentClass::ExternalManaged,
            ),
            Err(ProviderDataEligibilityError::LocalExecutionRequired)
        );
    }

    #[test]
    fn target_readiness_fails_closed_for_connection_model_and_tool_drift() {
        let route_set_id = Uuid::new_v4();
        let ready = target_row(route_set_id, 1, false);
        assert_eq!(ready.readiness(false), RouteTargetReadiness::Ready);
        assert_eq!(ready.readiness(true), RouteTargetReadiness::Ready);

        let mut unready = ready.clone();
        unready.connection_status = "error".to_owned();
        assert_eq!(
            unready.readiness(false),
            RouteTargetReadiness::ConnectionUnavailable
        );

        let mut stale = ready.clone();
        stale.current_catalog_version += 1;
        assert_eq!(stale.readiness(false), RouteTargetReadiness::StaleModel);

        let mut deleted_connection = ready.clone();
        deleted_connection.connection_deleted_at = Some(Utc::now());
        assert_eq!(
            deleted_connection.readiness(false),
            RouteTargetReadiness::ConnectionUnavailable
        );

        let mut deleted_model = ready.clone();
        deleted_model.model_deleted_at = Some(Utc::now());
        assert_eq!(
            deleted_model.readiness(false),
            RouteTargetReadiness::StaleModel
        );

        let mut missing_context_limit = ready.clone();
        missing_context_limit.context_window_tokens = None;
        assert_eq!(
            missing_context_limit.readiness(false),
            RouteTargetReadiness::ModelLimitsUnavailable
        );

        let mut nonpositive_output_limit = ready.clone();
        nonpositive_output_limit.max_output_tokens = Some(0);
        assert_eq!(
            nonpositive_output_limit.readiness(false),
            RouteTargetReadiness::ModelLimitsUnavailable
        );

        let mut no_tools = ready;
        no_tools.supports_tools = None;
        assert_eq!(
            no_tools.readiness(true),
            RouteTargetReadiness::ToolsUnsupported
        );
    }

    #[test]
    fn resolved_worker_target_retains_snapshot_pin_across_credential_rotation() {
        let route_set_id = Uuid::new_v4();
        let resolved_row = target_row(route_set_id, 1, false);
        let expected_credential_version = resolved_row.model_credential_version;
        let resolved = crate::types::ResolvedAiRouteTarget::from_ready_projection(
            resolved_row
                .clone()
                .into_projection(false, ProviderDataClass::CampusApproved)
                .unwrap(),
            expected_credential_version,
        )
        .unwrap();

        let mut rotated_connection = resolved_row;
        rotated_connection.current_credential_version += 1;
        assert_eq!(
            rotated_connection.readiness(false),
            RouteTargetReadiness::StaleModel
        );
        assert_eq!(
            resolved.expected_credential_version(),
            expected_credential_version
        );
        assert_ne!(
            resolved.expected_credential_version(),
            rotated_connection.current_credential_version
        );
        assert_eq!(resolved.model_snapshot_id(), rotated_connection.model_id);
        assert_eq!(resolved.max_output_tokens(), 250);
    }

    #[test]
    fn route_assembly_rejects_mixed_persisted_tool_flags() {
        let route_set_id = Uuid::new_v4();
        let row = super::RouteSetRow {
            id: route_set_id,
            scope_kind: "tenant_default".to_owned(),
            task_class: None,
            module_key: None,
            operation_class: None,
            capability_key: None,
            capability_version: None,
            version: 1,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let error = assemble_one_route(
            row,
            vec![
                target_row(route_set_id, 1, false),
                target_row(route_set_id, 2, true),
            ],
            false,
            ProviderDataClass::CampusApproved,
        )
        .unwrap_err();
        assert_eq!(error.code(), "routing_storage_error");
    }

    #[test]
    fn migration_and_projections_keep_scope_target_and_secret_boundaries_explicit() {
        for required in [
            "CREATE TABLE IF NOT EXISTS ai_route_sets",
            "CREATE TABLE IF NOT EXISTS ai_task_routes",
            "ai_route_sets_active_scope_unique",
            "ai_task_routes_active_target_unique",
            "configured_by",
            "created_by",
            "requires_tools",
            "validate_ai_task_route_target",
            "grant_new_tenant_ai_routing_permissions",
        ] {
            assert!(ROUTING_MIGRATION.contains(required));
        }
        assert!(ROUTE_SET_PROJECTION.contains("scope_kind"));
        assert!(ROUTE_TARGET_PROJECTION.contains("account_label"));
        assert!(ROUTE_TARGET_PROJECTION.contains("max_output_tokens"));
        for forbidden in [
            "credential_ciphertext",
            "credential_nonce",
            "credential_key_id",
            "credential_fingerprint",
        ] {
            assert!(!ROUTE_TARGET_PROJECTION.contains(forbidden));
        }
    }

    #[test]
    fn errors_are_safe_and_stable() {
        let no_route = AiRoutingError::NoMatchingRoute;
        assert_eq!(no_route.code(), "route_not_configured");
        assert!(!no_route.safe_message().is_empty());
        let unusable = AiRoutingError::UnusableRoute {
            route_set_id: Uuid::new_v4(),
            reason: RouteUnusableReason::StaleModel,
        };
        assert_eq!(unusable.code(), "route_unusable");
        assert!(unusable.safe_message().contains("no longer current"));
    }

    #[test]
    fn unique_write_errors_have_a_non_sensitive_conflict() {
        let error = sqlx::Error::RowNotFound;
        assert!(matches!(map_write_error(error), AiRoutingError::Storage(_)));
    }

    #[test]
    fn internal_priority_and_unusable_mappings_are_total() {
        assert_eq!(route_priority(0).unwrap(), 1);
        assert_eq!(route_priority(1).unwrap(), 2);
        assert_eq!(route_priority(2).unwrap(), 3);
        assert!(route_priority(3).is_err());
        assert_eq!(
            unusable_reason(RouteTargetReadiness::Ready),
            RouteUnusableReason::EmptyChain
        );
        assert_eq!(
            unusable_reason(RouteTargetReadiness::ConnectionUnavailable),
            RouteUnusableReason::ConnectionUnavailable
        );
        assert_eq!(
            unusable_reason(RouteTargetReadiness::ModelLimitsUnavailable),
            RouteUnusableReason::ModelLimitsUnavailable
        );
        assert_eq!(
            unusable_reason(RouteTargetReadiness::ToolsUnsupported),
            RouteUnusableReason::ToolsUnsupported
        );
    }

    #[test]
    fn malformed_stored_scope_is_an_internal_storage_failure() {
        let row = super::RouteSetRow {
            id: Uuid::new_v4(),
            scope_kind: "task_class".to_owned(),
            task_class: None,
            module_key: None,
            operation_class: None,
            capability_key: None,
            capability_version: None,
            version: 1,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        assert_eq!(
            route_scope_from_row(&row).unwrap_err().code(),
            "routing_storage_error"
        );
    }

    #[tokio::test]
    async fn system_actor_is_rejected_before_storage_is_contacted() {
        let pool = PgPoolOptions::new()
            .connect_lazy("postgresql://localhost/campus_pilot_not_contacted")
            .expect("static lazy PostgreSQL URL must parse");
        let ops = AiRoutingOps::new(pool);
        let command = crate::CreateRouteCommand::parse(
            crate::AiRouteScope::TenantDefault,
            false,
            vec![crate::RouteTargetDraft::parse(Uuid::new_v4(), "model").unwrap()],
            "System route",
        )
        .unwrap();
        let error = ops
            .create_route(
                Uuid::new_v4(),
                AuditActor::system(),
                RequestContext::generate(None),
                command,
            )
            .await
            .unwrap_err();
        assert_eq!(error.code(), "route_actor_required");
    }

    #[tokio::test]
    #[ignore = "requires a disposable migrated AI_ROUTING_TEST_DATABASE_URL"]
    async fn postgres_contract_covers_replay_tenants_precedence_and_drift() {
        let database_url = std::env::var("AI_ROUTING_TEST_DATABASE_URL")
            .expect("AI_ROUTING_TEST_DATABASE_URL must target a disposable database");
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(&database_url)
            .await
            .expect("AI routing contract database must connect");
        sqlx::raw_sql(ROUTING_MIGRATION)
            .execute(&pool)
            .await
            .expect("migration 083 must apply");
        sqlx::query(
            "ALTER TABLE ai_provider_models ADD COLUMN IF NOT EXISTS max_output_tokens BIGINT",
        )
        .execute(&pool)
        .await
        .expect("routing contract model-limit column must exist");

        let first = seed_tenant_with_provider(&pool, "routing-a", true).await;
        let second = seed_tenant_with_provider(&pool, "routing-b", true).await;
        let unsupported = seed_provider(&pool, first.tenant_id, first.actor_id, false).await;
        let legacy_limits =
            seed_provider_with_max_output(&pool, first.tenant_id, first.actor_id, true, None).await;
        let ops = AiRoutingOps::new(pool.clone());
        let read_ops = AiRoutingOps::for_reads(pool.clone());
        let actor = AuditActor::person(first.actor_id);

        let default = ops
            .create_route(
                first.tenant_id,
                actor,
                RequestContext::generate(None),
                crate::CreateRouteCommand::parse(
                    crate::AiRouteScope::TenantDefault,
                    false,
                    vec![
                        crate::RouteTargetDraft::parse(
                            first.connection_id,
                            &first.provider_model_id,
                        )
                        .unwrap(),
                    ],
                    "Default campus route",
                )
                .unwrap(),
            )
            .await
            .expect("default route must create");
        let missing_limits_route = ops
            .create_route(
                first.tenant_id,
                actor,
                RequestContext::generate(None),
                crate::CreateRouteCommand::parse(
                    crate::AiRouteScope::parse(
                        "task_class",
                        Some("document_extraction"),
                        None,
                        None,
                        None,
                        None,
                    )
                    .unwrap(),
                    false,
                    vec![
                        crate::RouteTargetDraft::parse(
                            legacy_limits.connection_id,
                            &legacy_limits.provider_model_id,
                        )
                        .unwrap(),
                    ],
                    "Legacy model-limit route",
                )
                .unwrap(),
            )
            .await
            .expect("legacy route must retain its immutable model snapshot");
        let missing_limits = ops
            .resolve_route(
                first.tenant_id,
                crate::ResolveRouteCommand::parse(
                    "document_extraction",
                    None,
                    None,
                    None,
                    None,
                    false,
                )
                .unwrap(),
            )
            .await;
        assert!(matches!(
            missing_limits,
            Err(AiRoutingError::UnusableRoute {
                route_set_id,
                reason: RouteUnusableReason::ModelLimitsUnavailable,
            }) if route_set_id == missing_limits_route.id
        ));
        let duplicate_default = ops
            .create_route(
                first.tenant_id,
                actor,
                RequestContext::generate(None),
                crate::CreateRouteCommand::parse(
                    crate::AiRouteScope::TenantDefault,
                    false,
                    vec![
                        crate::RouteTargetDraft::parse(
                            first.connection_id,
                            &first.provider_model_id,
                        )
                        .unwrap(),
                    ],
                    "Duplicate default route",
                )
                .unwrap(),
            )
            .await;
        assert!(matches!(
            duplicate_default,
            Err(AiRoutingError::Conflict {
                code: "route_scope_exists",
                ..
            })
        ));
        let task = ops
            .create_route(
                first.tenant_id,
                actor,
                RequestContext::generate(None),
                crate::CreateRouteCommand::parse(
                    crate::AiRouteScope::parse(
                        "task_class",
                        Some("module_read_reporting"),
                        None,
                        None,
                        None,
                        None,
                    )
                    .unwrap(),
                    true,
                    vec![
                        crate::RouteTargetDraft::parse(
                            first.connection_id,
                            &first.provider_model_id,
                        )
                        .unwrap(),
                    ],
                    "Reporting route",
                )
                .unwrap(),
            )
            .await
            .expect("task route must create");
        let listed = read_ops
            .list_routes(first.tenant_id)
            .await
            .expect("current routes must list");
        assert_eq!(listed.len(), 3);
        assert!(
            read_ops
                .list_routes(second.tenant_id)
                .await
                .expect("tenant without routes must list")
                .is_empty()
        );
        assert!(matches!(
            read_ops.read_route(second.tenant_id, default.id).await,
            Err(AiRoutingError::NotFound)
        ));
        assert!(matches!(
            read_ops
                .resolve_route(
                    second.tenant_id,
                    crate::ResolveRouteCommand::parse(
                        "campus_conversation_search",
                        None,
                        None,
                        None,
                        None,
                        false,
                    )
                    .unwrap(),
                )
                .await,
            Err(AiRoutingError::NoMatchingRoute)
        ));
        let capability = ops
            .create_route(
                first.tenant_id,
                actor,
                RequestContext::generate(None),
                crate::CreateRouteCommand::parse(
                    crate::AiRouteScope::parse(
                        "capability",
                        None,
                        None,
                        None,
                        Some("finance.journals.list"),
                        Some(1),
                    )
                    .unwrap(),
                    true,
                    vec![
                        crate::RouteTargetDraft::parse(
                            second.connection_id,
                            &second.provider_model_id,
                        )
                        .unwrap(),
                    ],
                    "Capability override",
                )
                .unwrap(),
            )
            .await;
        assert!(matches!(
            capability,
            Err(AiRoutingError::InvalidInput {
                code: "invalid_route_target",
                ..
            })
        ));

        let capability_provider = seed_provider(&pool, first.tenant_id, first.actor_id, true).await;
        let capability = ops
            .create_route(
                first.tenant_id,
                actor,
                RequestContext::generate(None),
                crate::CreateRouteCommand::parse(
                    crate::AiRouteScope::parse(
                        "capability",
                        None,
                        None,
                        None,
                        Some("finance.journals.list"),
                        Some(1),
                    )
                    .unwrap(),
                    true,
                    vec![
                        crate::RouteTargetDraft::parse(
                            capability_provider.connection_id,
                            &capability_provider.provider_model_id,
                        )
                        .unwrap(),
                    ],
                    "Capability override",
                )
                .unwrap(),
            )
            .await
            .expect("same-tenant capability route must create");

        let resolved = ops
            .resolve_route(
                first.tenant_id,
                crate::ResolveRouteCommand::parse(
                    "module_read_reporting",
                    Some("finance"),
                    Some("read"),
                    Some("finance.journals.list"),
                    Some(1),
                    true,
                )
                .unwrap(),
            )
            .await
            .expect("capability route must resolve first");
        assert_eq!(resolved.route_set_id, capability.id);
        assert_eq!(resolved.precedence, crate::RoutePrecedence::Capability);
        assert_eq!(resolved.targets.len(), 1);
        let resolved_target = &resolved.targets[0];
        let pinned_connection_id = resolved_target.connection_id();
        let pinned_credential_version = resolved_target.expected_credential_version();
        let pinned_model_snapshot_id = resolved_target.model_snapshot_id();
        assert_eq!(pinned_connection_id, capability_provider.connection_id);
        assert_eq!(pinned_credential_version, 1);
        let target_is_fresh = sqlx::query_scalar::<_, bool>(
            r#"
            SELECT EXISTS (
                SELECT 1
                  FROM ai_provider_connections AS connection
                  JOIN ai_provider_models AS model
                    ON model.connection_id = connection.id
                   AND model.tenant_id = connection.tenant_id
                 WHERE connection.tenant_id = $1
                   AND connection.id = $2
                   AND connection.status = 'ready'
                   AND connection.deleted_at IS NULL
                   AND connection.credential_version = $3
                   AND model.id = $4
                   AND model.credential_version = connection.credential_version
                   AND model.catalog_version = connection.model_catalog_version
                   AND model.deleted_at IS NULL
            )
            "#,
        )
        .bind(first.tenant_id)
        .bind(pinned_connection_id)
        .bind(pinned_credential_version)
        .bind(pinned_model_snapshot_id)
        .fetch_one(&pool)
        .await
        .expect("resolved worker target freshness must be queryable");
        assert!(target_is_fresh);

        sqlx::query(
            r#"
            UPDATE ai_provider_connections
               SET credential_version = credential_version + 1,
                   credential_fingerprint = $2,
                   version = version + 1,
                   updated_at = CLOCK_TIMESTAMP()
             WHERE id = $1
            "#,
        )
        .bind(pinned_connection_id)
        .bind(format!("sha256:rotated-{pinned_connection_id}"))
        .execute(&pool)
        .await
        .expect("credential rotation must persist");
        let target_is_fresh = sqlx::query_scalar::<_, bool>(
            r#"
            SELECT EXISTS (
                SELECT 1
                  FROM ai_provider_connections AS connection
                  JOIN ai_provider_models AS model
                    ON model.connection_id = connection.id
                   AND model.tenant_id = connection.tenant_id
                 WHERE connection.tenant_id = $1
                   AND connection.id = $2
                   AND connection.status = 'ready'
                   AND connection.deleted_at IS NULL
                   AND connection.credential_version = $3
                   AND model.id = $4
                   AND model.credential_version = connection.credential_version
                   AND model.catalog_version = connection.model_catalog_version
                   AND model.deleted_at IS NULL
            )
            "#,
        )
        .bind(first.tenant_id)
        .bind(pinned_connection_id)
        .bind(pinned_credential_version)
        .bind(pinned_model_snapshot_id)
        .fetch_one(&pool)
        .await
        .expect("rotated worker target freshness must be queryable");
        assert!(!target_is_fresh);
        let rotated_before_execution = ops
            .resolve_route(
                first.tenant_id,
                crate::ResolveRouteCommand::parse(
                    "module_read_reporting",
                    Some("finance"),
                    Some("read"),
                    Some("finance.journals.list"),
                    Some(1),
                    true,
                )
                .unwrap(),
            )
            .await;
        assert!(matches!(
            rotated_before_execution,
            Err(AiRoutingError::UnusableRoute {
                route_set_id,
                reason: RouteUnusableReason::StaleModel,
            }) if route_set_id == capability.id
        ));

        let stale = ops
            .replace_route(
                first.tenant_id,
                default.id,
                actor,
                RequestContext::generate(None),
                crate::ReplaceRouteCommand::parse(
                    default.version + 1,
                    false,
                    vec![
                        crate::RouteTargetDraft::parse(
                            first.connection_id,
                            &first.provider_model_id,
                        )
                        .unwrap(),
                    ],
                    "Stale replacement",
                )
                .unwrap(),
            )
            .await;
        assert!(matches!(
            stale,
            Err(AiRoutingError::Conflict {
                code: "stale_route",
                ..
            })
        ));
        let default = ops
            .replace_route(
                first.tenant_id,
                default.id,
                actor,
                RequestContext::generate(None),
                crate::ReplaceRouteCommand::parse(
                    default.version,
                    true,
                    vec![
                        crate::RouteTargetDraft::parse(
                            first.connection_id,
                            &first.provider_model_id,
                        )
                        .unwrap(),
                    ],
                    "Require tools on default route",
                )
                .unwrap(),
            )
            .await
            .expect("current route version must replace the full chain");
        assert_eq!(default.version, 2);
        assert!(default.requires_tools);

        sqlx::query(
            "UPDATE ai_provider_connections SET status = 'error', updated_at = CLOCK_TIMESTAMP() WHERE id = $1",
        )
        .bind(capability_provider.connection_id)
        .execute(&pool)
        .await
        .expect("provider health drift must persist");
        let unready = ops
            .resolve_route(
                first.tenant_id,
                crate::ResolveRouteCommand::parse(
                    "module_read_reporting",
                    None,
                    None,
                    Some("finance.journals.list"),
                    Some(1),
                    true,
                )
                .unwrap(),
            )
            .await;
        assert!(matches!(
            unready,
            Err(AiRoutingError::UnusableRoute {
                route_set_id,
                reason: RouteUnusableReason::ConnectionUnavailable,
            }) if route_set_id == capability.id
        ));

        sqlx::query(
            "UPDATE ai_provider_connections SET status = 'ready', model_catalog_version = model_catalog_version + 1, updated_at = CLOCK_TIMESTAMP() WHERE id = $1",
        )
        .bind(capability_provider.connection_id)
        .execute(&pool)
        .await
        .expect("model snapshot drift must persist");
        let stale_model = ops
            .resolve_route(
                first.tenant_id,
                crate::ResolveRouteCommand::parse(
                    "module_read_reporting",
                    None,
                    None,
                    Some("finance.journals.list"),
                    Some(1),
                    true,
                )
                .unwrap(),
            )
            .await;
        assert!(matches!(
            stale_model,
            Err(AiRoutingError::UnusableRoute {
                route_set_id,
                reason: RouteUnusableReason::StaleModel,
            }) if route_set_id == capability.id
        ));

        let module_route = ops
            .create_route(
                first.tenant_id,
                actor,
                RequestContext::generate(None),
                crate::CreateRouteCommand::parse(
                    crate::AiRouteScope::parse(
                        "module_operation",
                        None,
                        Some("finance"),
                        Some("read"),
                        None,
                        None,
                    )
                    .unwrap(),
                    false,
                    vec![
                        crate::RouteTargetDraft::parse(
                            unsupported.connection_id,
                            &unsupported.provider_model_id,
                        )
                        .unwrap(),
                    ],
                    "Module reporting route",
                )
                .unwrap(),
            )
            .await
            .expect("non-tools route may use a model without tools");
        let invalid_tools_route = ops
            .create_route(
                first.tenant_id,
                actor,
                RequestContext::generate(None),
                crate::CreateRouteCommand::parse(
                    crate::AiRouteScope::parse(
                        "module_operation",
                        None,
                        Some("sis"),
                        Some("read"),
                        None,
                        None,
                    )
                    .unwrap(),
                    true,
                    vec![
                        crate::RouteTargetDraft::parse(
                            unsupported.connection_id,
                            &unsupported.provider_model_id,
                        )
                        .unwrap(),
                    ],
                    "Invalid tools route",
                )
                .unwrap(),
            )
            .await;
        assert!(matches!(
            invalid_tools_route,
            Err(AiRoutingError::InvalidInput {
                code: "tools_not_supported",
                ..
            })
        ));
        let tools_required = ops
            .resolve_route(
                first.tenant_id,
                crate::ResolveRouteCommand::parse(
                    "module_read_reporting",
                    Some("finance"),
                    Some("read"),
                    None,
                    None,
                    true,
                )
                .unwrap(),
            )
            .await;
        assert!(matches!(
            tools_required,
            Err(AiRoutingError::UnusableRoute {
                route_set_id,
                reason: RouteUnusableReason::ToolsUnsupported,
            }) if route_set_id == module_route.id
        ));

        let malformed_scope = sqlx::query(
            r#"
            INSERT INTO ai_route_sets (
                tenant_id, scope_kind, task_class, module_key, operation_class,
                capability_key, capability_version, configured_by, change_reason
            )
            VALUES ($1, 'tenant_default', 'module_read_reporting', NULL, NULL, NULL, NULL, $2, 'invalid shape')
            "#,
        )
        .bind(first.tenant_id)
        .bind(first.actor_id)
        .execute(&pool)
        .await;
        assert!(malformed_scope.is_err());

        let stale_archive = ops
            .archive_route(
                first.tenant_id,
                capability.id,
                actor,
                RequestContext::generate(None),
                crate::ArchiveRouteCommand::parse(capability.version + 1, "Stale archive attempt")
                    .unwrap(),
            )
            .await;
        assert!(matches!(
            stale_archive,
            Err(AiRoutingError::Conflict {
                code: "stale_route",
                ..
            })
        ));
        let archived = ops
            .archive_route(
                first.tenant_id,
                capability.id,
                actor,
                RequestContext::generate(None),
                crate::ArchiveRouteCommand::parse(capability.version, "Retire stale override")
                    .unwrap(),
            )
            .await
            .expect("capability route must archive optimistically");
        assert_eq!(archived.version, capability.version + 1);
        assert!(matches!(
            ops.read_route(first.tenant_id, capability.id).await,
            Err(AiRoutingError::NotFound)
        ));
        let fallback = ops
            .resolve_route(
                first.tenant_id,
                crate::ResolveRouteCommand::parse(
                    "module_read_reporting",
                    None,
                    None,
                    Some("finance.journals.list"),
                    Some(1),
                    true,
                )
                .unwrap(),
            )
            .await
            .expect("missing capability override may fall through to task class");
        assert_eq!(fallback.route_set_id, task.id);
        assert_eq!(fallback.precedence, crate::RoutePrecedence::TaskClass);

        let empty_route_id = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO ai_route_sets (
                id, tenant_id, scope_kind, task_class, module_key, operation_class,
                capability_key, capability_version, configured_by, change_reason
            )
            VALUES ($1, $2, 'task_class', 'campus_conversation_search', NULL, NULL,
                    NULL, NULL, $3, 'Empty route contract')
            "#,
        )
        .bind(empty_route_id)
        .bind(first.tenant_id)
        .bind(first.actor_id)
        .execute(&pool)
        .await
        .expect("database permits a header before its transactional target inserts");
        let empty_chain = ops
            .resolve_route(
                first.tenant_id,
                crate::ResolveRouteCommand::parse(
                    "campus_conversation_search",
                    None,
                    None,
                    None,
                    None,
                    false,
                )
                .unwrap(),
            )
            .await;
        assert!(matches!(
            empty_chain,
            Err(AiRoutingError::UnusableRoute {
                route_set_id,
                reason: RouteUnusableReason::EmptyChain,
            }) if route_set_id == empty_route_id
        ));

        let audit_count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM actor_audit_events WHERE tenant_id = $1 AND target_type = 'ai_route_set' AND reason IS NOT NULL",
        )
        .bind(first.tenant_id)
        .fetch_one(&pool)
        .await
        .expect("route audit evidence must be queryable");
        assert!(audit_count >= 4);
        let school_admin_has_permissions = sqlx::query_scalar::<_, bool>(
            "SELECT permissions @> ARRAY['ai_routing:view', 'ai_routing:edit']::TEXT[] FROM roles WHERE tenant_id = $1 AND key = 'school_administrator'",
        )
        .bind(first.tenant_id)
        .fetch_one(&pool)
        .await
        .expect("new tenant School Administrator must exist");
        assert!(school_admin_has_permissions);

        sqlx::raw_sql(ROUTING_MIGRATION)
            .execute(&pool)
            .await
            .expect("migration 083 must replay without mutating route data");
        assert_eq!(
            ops.read_route(first.tenant_id, default.id)
                .await
                .unwrap()
                .id,
            default.id
        );
    }

    #[derive(Debug)]
    struct SeededProvider {
        tenant_id: Uuid,
        actor_id: Uuid,
        connection_id: Uuid,
        provider_model_id: String,
    }

    async fn seed_tenant_with_provider(
        pool: &PgPool,
        prefix: &str,
        supports_tools: bool,
    ) -> SeededProvider {
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        let suffix = tenant_id.simple();
        sqlx::query("INSERT INTO tenants (id, slug, name) VALUES ($1, $2, $3)")
            .bind(tenant_id)
            .bind(format!("{prefix}-{suffix}"))
            .bind(format!("{prefix} routing contract"))
            .execute(pool)
            .await
            .expect("contract tenant must insert");
        sqlx::query(
            "INSERT INTO users (id, tenant_id, email, password_hash, full_name) VALUES ($1, $2, $3, 'not-a-login', 'Routing Contract')",
        )
        .bind(actor_id)
        .bind(tenant_id)
        .bind(format!("{prefix}-{suffix}@example.invalid"))
        .execute(pool)
        .await
        .expect("contract actor must insert");
        seed_provider(pool, tenant_id, actor_id, supports_tools).await
    }

    async fn seed_provider(
        pool: &PgPool,
        tenant_id: Uuid,
        actor_id: Uuid,
        supports_tools: bool,
    ) -> SeededProvider {
        seed_provider_with_max_output(pool, tenant_id, actor_id, supports_tools, Some(16_384)).await
    }

    async fn seed_provider_with_max_output(
        pool: &PgPool,
        tenant_id: Uuid,
        actor_id: Uuid,
        supports_tools: bool,
        max_output_tokens: Option<i64>,
    ) -> SeededProvider {
        let connection_id = Uuid::new_v4();
        let model_id = Uuid::new_v4();
        let provider_model_id = format!("test-model-{connection_id}");
        sqlx::query(
            r#"
            INSERT INTO ai_provider_connections (
                id, tenant_id, provider, auth_method, account_label, status,
                credential_ciphertext, credential_nonce, credential_key_id,
                credential_version, credential_fingerprint, configured_by,
                model_catalog_version, model_catalog_refreshed_at
            )
            VALUES ($1, $2, 'openai', 'api_key', $3, 'ready', $4, $5,
                    'test-key', 1, $6, $7, 1, NOW())
            "#,
        )
        .bind(connection_id)
        .bind(tenant_id)
        .bind(format!("Connection {connection_id}"))
        .bind(vec![7_u8; 16])
        .bind(vec![9_u8; 12])
        .bind(format!("sha256:{connection_id}"))
        .bind(actor_id)
        .execute(pool)
        .await
        .expect("contract connection must insert");
        sqlx::query(
            r#"
            INSERT INTO ai_provider_models (
                id, tenant_id, connection_id, credential_version, catalog_version,
                provider_model_id, display_name, context_window_tokens,
                max_output_tokens, supports_tools, refreshed_at
            )
            VALUES ($1, $2, $3, 1, 1, $4, 'Routing Test Model', 100000, $5, $6, NOW())
            "#,
        )
        .bind(model_id)
        .bind(tenant_id)
        .bind(connection_id)
        .bind(&provider_model_id)
        .bind(max_output_tokens)
        .bind(supports_tools)
        .execute(pool)
        .await
        .expect("contract model must insert");
        SeededProvider {
            tenant_id,
            actor_id,
            connection_id,
            provider_model_id,
        }
    }
}
