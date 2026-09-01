//! Library member lookup and tenant-scoped membership lifecycle operations.

use std::collections::{HashMap, HashSet};

use anyhow::{Context, Result, anyhow, bail};
use cp_audit::{AuditActor, RequestContext};
use cp_fees::foundation::BillingAccountOps;
use cp_finance::ledger::CurrencyOps;
use cp_hr_payroll::ops::EmployeeOps;
use cp_sis::ops::LearnerOps;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    BillingAccountReference, BorrowerCandidate, BorrowerKind, CreateMembershipRequest,
    CurrencyReference, DirectoryQuery, LibraryAccessScope, LibraryReferenceData,
    MembershipResponse, MembershipStatus, UpdateMembershipRequest,
    models::{BorrowerIdentity, MembershipRow},
    settings::{LibraryActivityEvent, append_domain_audit, append_event, person_actor_id},
};

pub struct LibraryMemberOps;

impl LibraryMemberOps {
    pub async fn reference_data(
        pool: &PgPool,
        tenant_id: Uuid,
        search: Option<&str>,
    ) -> Result<LibraryReferenceData> {
        let learners = LearnerOps::library_references(pool, tenant_id, search, 100).await?;
        let employees =
            EmployeeOps::list_references(pool, tenant_id, search, Some("active"), 100).await?;
        let membership_rows = sqlx::query_as::<_, (Option<Uuid>, Option<Uuid>)>(
            "SELECT learner_id, employee_id FROM library_memberships WHERE tenant_id = $1 AND deleted_at IS NULL",
        )
        .bind(tenant_id)
        .fetch_all(pool)
        .await
        .context("Failed to resolve existing Library memberships")?;
        let member_learners = membership_rows
            .iter()
            .filter_map(|row| row.0)
            .collect::<HashSet<_>>();
        let member_employees = membership_rows
            .iter()
            .filter_map(|row| row.1)
            .collect::<HashSet<_>>();
        let (currencies, _) =
            CurrencyOps::list(pool, tenant_id, 1, 100, None, Some("active")).await?;
        let (billing_accounts, _) =
            BillingAccountOps::list(pool, tenant_id, 1, 100, search, Some("active"), None).await?;
        Ok(LibraryReferenceData {
            learners: learners
                .into_iter()
                .map(|value| BorrowerCandidate {
                    kind: BorrowerKind::Learner,
                    id: value.id,
                    number: value.learner_number,
                    display_name: value.display_name,
                    source_status: value.status,
                    account_linked: value.account_id.is_some(),
                    already_member: member_learners.contains(&value.id),
                })
                .collect(),
            employees: employees
                .into_iter()
                .map(|value| BorrowerCandidate {
                    kind: BorrowerKind::Employee,
                    id: value.id,
                    number: value.employee_number,
                    display_name: value.display_name,
                    source_status: value.employment_status,
                    account_linked: value.account_id.is_some(),
                    already_member: member_employees.contains(&value.id),
                })
                .collect(),
            currencies: currencies
                .into_iter()
                .map(|value| CurrencyReference {
                    id: value.id,
                    code: value.code,
                    minor_units: value.minor_units,
                    is_reporting: value.is_reporting,
                })
                .collect(),
            billing_accounts: billing_accounts
                .into_iter()
                .map(|value| BillingAccountReference {
                    id: value.id,
                    learner_id: value.learner_id,
                    learner_number: value.learner_number,
                    learner_name: value.learner_name,
                    account_number: value.account_number,
                })
                .collect(),
        })
    }

    pub async fn list(
        pool: &PgPool,
        tenant_id: Uuid,
        scope: LibraryAccessScope,
        query: &DirectoryQuery,
    ) -> Result<(Vec<MembershipResponse>, i64)> {
        validate_membership_status(query.status.as_deref())?;
        let visible = visible_borrowers(pool, tenant_id, scope).await?;
        let search = query
            .search
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let (search_learners, search_employees) = if let Some(search) = search {
            let learners =
                LearnerOps::library_references(pool, tenant_id, Some(search), 100).await?;
            let employees =
                EmployeeOps::list_references(pool, tenant_id, Some(search), None, 100).await?;
            (
                Some(
                    learners
                        .into_iter()
                        .map(|value| value.id)
                        .collect::<Vec<_>>(),
                ),
                Some(
                    employees
                        .into_iter()
                        .map(|value| value.id)
                        .collect::<Vec<_>>(),
                ),
            )
        } else {
            (None, None)
        };
        let (page, per_page) = bounded_page(query.page, query.per_page);
        let offset = (page - 1) * per_page;
        let learner_ids = visible.learner_ids();
        let employee_ids = visible.employee_ids();
        let rows = sqlx::query_as::<_, MembershipRow>(MEMBERSHIP_LIST_SELECT)
            .bind(tenant_id)
            .bind(query.status.as_deref())
            .bind(learner_ids.as_deref())
            .bind(employee_ids.as_deref())
            .bind(search_learners.as_deref())
            .bind(search_employees.as_deref())
            .bind(per_page)
            .bind(offset)
            .fetch_all(pool)
            .await
            .context("Failed to list Library memberships")?;
        let total = sqlx::query_scalar::<_, i64>(MEMBERSHIP_COUNT_SELECT)
            .bind(tenant_id)
            .bind(query.status.as_deref())
            .bind(learner_ids.as_deref())
            .bind(employee_ids.as_deref())
            .bind(search_learners.as_deref())
            .bind(search_employees.as_deref())
            .fetch_one(pool)
            .await
            .context("Failed to count Library memberships")?;
        Ok((hydrate_memberships(pool, tenant_id, rows).await?, total))
    }

    pub async fn get(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        scope: LibraryAccessScope,
    ) -> Result<Option<MembershipResponse>> {
        let visible = visible_borrowers(pool, tenant_id, scope).await?;
        let learner_ids = visible.learner_ids();
        let employee_ids = visible.employee_ids();
        let row = sqlx::query_as::<_, MembershipRow>(MEMBERSHIP_BY_ID_SELECT)
            .bind(tenant_id)
            .bind(id)
            .bind(learner_ids.as_deref())
            .bind(employee_ids.as_deref())
            .fetch_optional(pool)
            .await
            .context("Failed to load the Library membership")?;
        let Some(row) = row else {
            return Ok(None);
        };
        Ok(hydrate_memberships(pool, tenant_id, vec![row]).await?.pop())
    }

    pub async fn create(
        pool: &PgPool,
        tenant_id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        request: &CreateMembershipRequest,
    ) -> Result<MembershipResponse> {
        let actor_id = person_actor_id(actor)?;
        match request.borrower_kind {
            BorrowerKind::Learner => {
                let reference =
                    LearnerOps::library_references_by_ids(pool, tenant_id, &[request.borrower_id])
                        .await?
                        .pop()
                        .ok_or_else(|| anyhow!("The learner was not found"))?;
                if !matches!(reference.status.as_str(), "active" | "prospective") {
                    bail!("The learner is not eligible for an active Library membership");
                }
            }
            BorrowerKind::Employee => {
                let reference = EmployeeOps::get_reference(pool, tenant_id, request.borrower_id)
                    .await?
                    .ok_or_else(|| anyhow!("The employee was not found"))?;
                if reference.employment_status != "active" {
                    bail!("The employee is not eligible for an active Library membership");
                }
            }
        }
        let default_limit = sqlx::query_scalar::<_, i16>(
            "SELECT default_loan_limit FROM library_settings WHERE tenant_id = $1",
        )
        .bind(tenant_id)
        .fetch_one(pool)
        .await
        .context("Failed to load the Library loan limit")?;
        let loan_limit = request.loan_limit.unwrap_or(default_limit);
        let card_number = member_card_number();
        let id = Uuid::new_v4();
        let (learner_id, employee_id) = match request.borrower_kind {
            BorrowerKind::Learner => (Some(request.borrower_id), None),
            BorrowerKind::Employee => (None, Some(request.borrower_id)),
        };
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start membership creation")?;
        sqlx::query(
            r#"INSERT INTO library_memberships (id, tenant_id, learner_id, employee_id, card_number, loan_limit, created_by)
               VALUES ($1, $2, $3, $4, $5, $6, $7)"#,
        ).bind(id).bind(tenant_id).bind(learner_id).bind(employee_id).bind(&card_number).bind(loan_limit).bind(actor_id)
          .execute(&mut *transaction).await.map_err(|error| membership_database_error(error, "Failed to create the Library membership"))?;
        append_event(&mut transaction, LibraryActivityEvent { tenant_id, aggregate_id: id, aggregate_type: "membership", event_type: "created", actor_id, reason: None, metadata: json!({ "borrower_kind": request.borrower_kind.as_str(), "borrower_id": request.borrower_id, "card_number": card_number }) }).await?;
        append_domain_audit(&mut transaction, tenant_id, actor, request_context, "library.memberships.create", "library_membership", id, json!({ "borrower_kind": request.borrower_kind.as_str(), "borrower_id": request.borrower_id, "card_number": card_number })).await?;
        transaction
            .commit()
            .await
            .context("Failed to commit the Library membership")?;
        Self::get(pool, tenant_id, id, LibraryAccessScope::Campus)
            .await?
            .ok_or_else(|| anyhow!("The created Library membership could not be loaded"))
    }

    pub async fn update(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        request: &UpdateMembershipRequest,
    ) -> Result<Option<MembershipResponse>> {
        let actor_id = person_actor_id(actor)?;
        if request.status == MembershipStatus::Closed {
            let blocked = sqlx::query_scalar::<_, bool>(
                r#"SELECT EXISTS(SELECT 1 FROM library_loans WHERE tenant_id = $1 AND membership_id = $2 AND status = 'active'
                   UNION ALL SELECT 1 FROM library_holds WHERE tenant_id = $1 AND membership_id = $2 AND status IN ('waiting', 'ready')
                   UNION ALL SELECT 1 FROM library_fines WHERE tenant_id = $1 AND membership_id = $2 AND status = 'assessed')"#,
            ).bind(tenant_id).bind(id).fetch_one(pool).await.context("Failed to inspect membership dependencies")?;
            if blocked {
                bail!(
                    "Return active loans and resolve holds and assessed fines before closing this membership"
                );
            }
        }
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start membership update")?;
        let affected = sqlx::query(
            r#"UPDATE library_memberships SET status = $3, loan_limit = $4,
                      closed_by = CASE WHEN $3 = 'closed' THEN $5 ELSE NULL END,
                      closed_at = CASE WHEN $3 = 'closed' THEN NOW() ELSE NULL END,
                      version = version + 1, updated_at = NOW()
                WHERE tenant_id = $1 AND id = $2 AND status <> 'closed' AND version = $6 AND deleted_at IS NULL"#,
        ).bind(tenant_id).bind(id).bind(request.status.as_str()).bind(request.loan_limit).bind(actor_id).bind(request.expected_version)
          .execute(&mut *transaction).await.context("Failed to update the Library membership")?.rows_affected();
        if affected == 0 {
            transaction.rollback().await.ok();
            return membership_current_or_conflict(pool, tenant_id, id, request.expected_version)
                .await;
        }
        append_event(&mut transaction, LibraryActivityEvent { tenant_id, aggregate_id: id, aggregate_type: "membership", event_type: "updated", actor_id, reason: None, metadata: json!({ "status": request.status.as_str(), "loan_limit": request.loan_limit, "version": request.expected_version + 1 }) }).await?;
        append_domain_audit(&mut transaction, tenant_id, actor, request_context, "library.memberships.update", "library_membership", id, json!({ "status": request.status.as_str(), "loan_limit": request.loan_limit, "version": request.expected_version + 1 })).await?;
        transaction
            .commit()
            .await
            .context("Failed to commit the Library membership update")?;
        Self::get(pool, tenant_id, id, LibraryAccessScope::Campus).await
    }
}

#[derive(Debug)]
struct VisibleBorrowers {
    campus: bool,
    learners: Vec<Uuid>,
    employees: Vec<Uuid>,
}
impl VisibleBorrowers {
    fn learner_ids(&self) -> Option<Vec<Uuid>> {
        (!self.campus).then(|| self.learners.clone())
    }
    fn employee_ids(&self) -> Option<Vec<Uuid>> {
        (!self.campus).then(|| self.employees.clone())
    }
}

async fn visible_borrowers(
    pool: &PgPool,
    tenant_id: Uuid,
    scope: LibraryAccessScope,
) -> Result<VisibleBorrowers> {
    match scope {
        LibraryAccessScope::Campus => Ok(VisibleBorrowers {
            campus: true,
            learners: Vec::new(),
            employees: Vec::new(),
        }),
        LibraryAccessScope::SelfFor(account_id) => {
            let learners = LearnerOps::ids_for_linked_account(pool, tenant_id, account_id).await?;
            let employees = EmployeeOps::active_reference_by_account(pool, tenant_id, account_id)
                .await?
                .into_iter()
                .map(|value| value.id)
                .collect();
            Ok(VisibleBorrowers {
                campus: false,
                learners,
                employees,
            })
        }
    }
}

pub(crate) async fn visible_membership_ids(
    pool: &PgPool,
    tenant_id: Uuid,
    scope: LibraryAccessScope,
) -> Result<Option<Vec<Uuid>>> {
    let visible = visible_borrowers(pool, tenant_id, scope).await?;
    if visible.campus {
        return Ok(None);
    }
    sqlx::query_scalar::<_, Uuid>(
        r#"
        SELECT id FROM library_memberships
         WHERE tenant_id = $1 AND deleted_at IS NULL
           AND (learner_id = ANY($2) OR employee_id = ANY($3))
        "#,
    )
    .bind(tenant_id)
    .bind(&visible.learners)
    .bind(&visible.employees)
    .fetch_all(pool)
    .await
    .context("Failed to resolve visible Library memberships")
    .map(Some)
}

pub(crate) async fn hydrate_memberships(
    pool: &PgPool,
    tenant_id: Uuid,
    rows: Vec<MembershipRow>,
) -> Result<Vec<MembershipResponse>> {
    let learner_ids = rows
        .iter()
        .filter_map(|row| row.learner_id)
        .collect::<Vec<_>>();
    let employee_ids = rows
        .iter()
        .filter_map(|row| row.employee_id)
        .collect::<Vec<_>>();
    let learners = LearnerOps::library_references_by_ids(pool, tenant_id, &learner_ids)
        .await?
        .into_iter()
        .map(|value| {
            (
                value.id,
                BorrowerIdentity {
                    number: value.learner_number,
                    display_name: value.display_name,
                    source_status: value.status,
                    account_linked: value.account_id.is_some(),
                },
            )
        })
        .collect::<HashMap<_, _>>();
    let employees = EmployeeOps::references_by_ids(pool, tenant_id, &employee_ids)
        .await?
        .into_iter()
        .map(|value| {
            (
                value.id,
                BorrowerIdentity {
                    number: value.employee_number,
                    display_name: value.display_name,
                    source_status: value.employment_status,
                    account_linked: value.account_id.is_some(),
                },
            )
        })
        .collect::<HashMap<_, _>>();
    rows.into_iter()
        .map(|row| {
            let (kind, id, identity) = if let Some(id) = row.learner_id {
                (BorrowerKind::Learner, id, learners.get(&id))
            } else if let Some(id) = row.employee_id {
                (BorrowerKind::Employee, id, employees.get(&id))
            } else {
                bail!("The Library membership borrower reference is invalid");
            };
            let identity = identity
                .ok_or_else(|| anyhow!("The Library membership borrower is unavailable"))?;
            Ok(MembershipResponse {
                id: row.id,
                borrower_kind: kind,
                borrower_id: id,
                borrower_number: identity.number.clone(),
                borrower_name: identity.display_name.clone(),
                borrower_source_status: identity.source_status.clone(),
                account_linked: identity.account_linked,
                card_number: row.card_number,
                status: row.status,
                loan_limit: row.loan_limit,
                active_loan_count: row.active_loan_count,
                active_hold_count: row.active_hold_count,
                version: row.version,
                created_at: row.created_at,
                updated_at: row.updated_at,
            })
        })
        .collect()
}

async fn membership_current_or_conflict(
    pool: &PgPool,
    tenant_id: Uuid,
    id: Uuid,
    expected: i32,
) -> Result<Option<MembershipResponse>> {
    let current = LibraryMemberOps::get(pool, tenant_id, id, LibraryAccessScope::Campus).await?;
    if current
        .as_ref()
        .is_some_and(|value| value.version != expected)
    {
        bail!("The Library membership changed since it was loaded");
    }
    Ok(current)
}
fn member_card_number() -> String {
    let value = Uuid::new_v4().simple().to_string();
    format!("MBR-{}", &value[..12].to_ascii_uppercase())
}
fn validate_membership_status(value: Option<&str>) -> Result<()> {
    if value.is_some_and(|value| !matches!(value, "active" | "suspended" | "closed")) {
        bail!("The membership status filter is invalid");
    }
    Ok(())
}
fn bounded_page(page: Option<i64>, per_page: Option<i64>) -> (i64, i64) {
    (
        page.unwrap_or(1).max(1),
        per_page.unwrap_or(25).clamp(1, 100),
    )
}
fn membership_database_error(error: sqlx::Error, context: &str) -> anyhow::Error {
    if let sqlx::Error::Database(database) = &error
        && database.code().as_deref() == Some("23505")
    {
        return anyhow!("That learner or employee already has a Library membership");
    }
    anyhow!("{context}: {error}")
}

const MEMBERSHIP_LIST_SELECT: &str = concat!(
    "SELECT member.id, member.learner_id, member.employee_id, member.card_number, member.status, member.loan_limit, COUNT(DISTINCT loan.id)::BIGINT AS active_loan_count, COUNT(DISTINCT hold.id)::BIGINT AS active_hold_count, member.version, member.created_at, member.updated_at FROM library_memberships AS member LEFT JOIN library_loans AS loan ON loan.tenant_id = member.tenant_id AND loan.membership_id = member.id AND loan.status = 'active' LEFT JOIN library_holds AS hold ON hold.tenant_id = member.tenant_id AND hold.membership_id = member.id AND hold.status IN ('waiting', 'ready') ",
    "WHERE member.tenant_id = $1 AND member.deleted_at IS NULL AND ($2::TEXT IS NULL OR member.status = $2) AND (($3::UUID[] IS NULL AND $4::UUID[] IS NULL) OR member.learner_id = ANY(COALESCE($3, ARRAY[]::UUID[])) OR member.employee_id = ANY(COALESCE($4, ARRAY[]::UUID[]))) AND (($5::UUID[] IS NULL AND $6::UUID[] IS NULL) OR member.learner_id = ANY(COALESCE($5, ARRAY[]::UUID[])) OR member.employee_id = ANY(COALESCE($6, ARRAY[]::UUID[]))) GROUP BY member.id ORDER BY member.updated_at DESC, member.id LIMIT $7 OFFSET $8"
);
const MEMBERSHIP_COUNT_SELECT: &str = "SELECT COUNT(*) FROM library_memberships AS member WHERE member.tenant_id = $1 AND member.deleted_at IS NULL AND ($2::TEXT IS NULL OR member.status = $2) AND (($3::UUID[] IS NULL AND $4::UUID[] IS NULL) OR member.learner_id = ANY(COALESCE($3, ARRAY[]::UUID[])) OR member.employee_id = ANY(COALESCE($4, ARRAY[]::UUID[]))) AND (($5::UUID[] IS NULL AND $6::UUID[] IS NULL) OR member.learner_id = ANY(COALESCE($5, ARRAY[]::UUID[])) OR member.employee_id = ANY(COALESCE($6, ARRAY[]::UUID[])))";
const MEMBERSHIP_BY_ID_SELECT: &str = concat!(
    "SELECT member.id, member.learner_id, member.employee_id, member.card_number, member.status, member.loan_limit, COUNT(DISTINCT loan.id)::BIGINT AS active_loan_count, COUNT(DISTINCT hold.id)::BIGINT AS active_hold_count, member.version, member.created_at, member.updated_at FROM library_memberships AS member LEFT JOIN library_loans AS loan ON loan.tenant_id = member.tenant_id AND loan.membership_id = member.id AND loan.status = 'active' LEFT JOIN library_holds AS hold ON hold.tenant_id = member.tenant_id AND hold.membership_id = member.id AND hold.status IN ('waiting', 'ready') ",
    "WHERE member.tenant_id = $1 AND member.id = $2 AND member.deleted_at IS NULL AND (($3::UUID[] IS NULL AND $4::UUID[] IS NULL) OR member.learner_id = ANY(COALESCE($3, ARRAY[]::UUID[])) OR member.employee_id = ANY(COALESCE($4, ARRAY[]::UUID[]))) GROUP BY member.id"
);

#[cfg(test)]
mod tests {
    use super::member_card_number;
    #[test]
    fn member_cards_are_compact_and_prefixed() {
        let card = member_card_number();
        assert!(card.starts_with("MBR-"));
        assert_eq!(card.len(), 16);
    }
}
