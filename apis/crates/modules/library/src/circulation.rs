//! Library loan and hold workflows with optimistic versions and borrower scope.

use std::collections::HashMap;

use anyhow::{Context, Result, anyhow, bail};
use chrono::{Duration, NaiveDate, Utc};
use cp_audit::{AuditActor, RequestContext};
use cp_hr_payroll::ops::EmployeeOps;
use cp_sis::ops::LearnerOps;
use serde_json::json;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::{
    BorrowerKind, BorrowingListQuery, CheckoutRequest, HoldResponse, LibraryAccessScope,
    LoanResponse, PlaceHoldRequest, ReadyHoldRequest, ReasonedVersionRequest, RenewLoanRequest,
    ReturnLoanRequest,
    members::visible_membership_ids,
    models::{BorrowerIdentity, HoldRow, LoanRow},
    settings::{LibraryActivityEvent, append_domain_audit, append_event, person_actor_id},
};

pub struct LibraryCirculationOps;

impl LibraryCirculationOps {
    pub async fn list_loans(
        pool: &PgPool,
        tenant_id: Uuid,
        scope: LibraryAccessScope,
        query: &BorrowingListQuery,
    ) -> Result<(Vec<LoanResponse>, i64)> {
        validate_loan_status(query.status.as_deref())?;
        let visible = visible_membership_ids(pool, tenant_id, scope).await?;
        let membership_filter = constrain_membership(query.membership_id, visible.as_deref());
        let search_ids = search_membership_ids(pool, tenant_id, query.search.as_deref()).await?;
        let (page, per_page) = bounded_page(query.page, query.per_page);
        let offset = (page - 1) * per_page;
        let rows = sqlx::query_as::<_, LoanRow>(LOAN_LIST_SELECT)
            .bind(tenant_id)
            .bind(query.status.as_deref())
            .bind(query.overdue_only.unwrap_or(false))
            .bind(membership_filter.as_deref())
            .bind(search_ids.as_deref())
            .bind(per_page)
            .bind(offset)
            .fetch_all(pool)
            .await
            .context("Failed to list Library loans")?;
        let total = sqlx::query_scalar::<_, i64>(LOAN_COUNT_SELECT)
            .bind(tenant_id)
            .bind(query.status.as_deref())
            .bind(query.overdue_only.unwrap_or(false))
            .bind(membership_filter.as_deref())
            .bind(search_ids.as_deref())
            .fetch_one(pool)
            .await
            .context("Failed to count Library loans")?;
        Ok((hydrate_loans(pool, tenant_id, rows).await?, total))
    }

    pub async fn get_loan(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        scope: LibraryAccessScope,
    ) -> Result<Option<LoanResponse>> {
        let visible = visible_membership_ids(pool, tenant_id, scope).await?;
        let row = sqlx::query_as::<_, LoanRow>(LOAN_BY_ID_SELECT)
            .bind(tenant_id)
            .bind(id)
            .bind(visible.as_deref())
            .fetch_optional(pool)
            .await
            .context("Failed to load the Library loan")?;
        let Some(row) = row else {
            return Ok(None);
        };
        Ok(hydrate_loans(pool, tenant_id, vec![row]).await?.pop())
    }

    pub async fn checkout(
        pool: &PgPool,
        tenant_id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        request: &CheckoutRequest,
    ) -> Result<LoanResponse> {
        let actor_id = person_actor_id(actor)?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start Library checkout")?;
        let copy = sqlx::query_as::<_, (Uuid, String, String)>(
            "SELECT title_id, status, accession_number FROM library_copies WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL FOR UPDATE",
        ).bind(tenant_id).bind(request.copy_id).fetch_optional(&mut *transaction).await.context("Failed to lock the Library copy")?.ok_or_else(|| anyhow!("The Library copy was not found"))?;
        let member = sqlx::query_as::<_, (Option<Uuid>, Option<Uuid>, String, i16)>(
            "SELECT learner_id, employee_id, status, loan_limit FROM library_memberships WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL FOR UPDATE",
        ).bind(tenant_id).bind(request.membership_id).fetch_optional(&mut *transaction).await.context("Failed to lock the Library membership")?.ok_or_else(|| anyhow!("The Library membership was not found"))?;
        if member.2 != "active" {
            bail!("Checkout requires an active Library membership");
        }
        let active_loans = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM library_loans WHERE tenant_id = $1 AND membership_id = $2 AND status = 'active'")
            .bind(tenant_id).bind(request.membership_id).fetch_one(&mut *transaction).await.context("Failed to count active Library loans")?;
        if active_loans >= i64::from(member.3) {
            bail!("This member has reached their active loan limit");
        }
        let unresolved_fines = sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM library_fines WHERE tenant_id = $1 AND membership_id = $2 AND status = 'assessed')")
            .bind(tenant_id).bind(request.membership_id).fetch_one(&mut *transaction).await.context("Failed to inspect assessed Library fines")?;
        if unresolved_fines {
            bail!("Resolve assessed Library fines before checking out another copy");
        }

        let hold = if let Some(hold_id) = request.fulfilled_hold_id {
            let value = sqlx::query_as::<_, (Uuid, Uuid, Option<Uuid>, String, i32)>(
                "SELECT title_id, membership_id, copy_id, status, version FROM library_holds WHERE tenant_id = $1 AND id = $2 FOR UPDATE",
            ).bind(tenant_id).bind(hold_id).fetch_optional(&mut *transaction).await.context("Failed to lock the Library hold")?.ok_or_else(|| anyhow!("The Library hold was not found"))?;
            if value.0 != copy.0
                || value.1 != request.membership_id
                || value.2 != Some(request.copy_id)
                || value.3 != "ready"
            {
                bail!("The ready hold does not match this copy and member");
            }
            Some((hold_id, value.4))
        } else {
            None
        };
        if copy.1 == "reserved" && hold.is_none() {
            bail!("A reserved copy must be checked out against its ready hold");
        }
        if !matches!(copy.1.as_str(), "available" | "reserved") {
            bail!("Only an available or correctly reserved copy can be checked out");
        }
        let competing_ready_hold = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM library_holds WHERE tenant_id = $1 AND copy_id = $2 AND status = 'ready' AND membership_id <> $3)",
        ).bind(tenant_id).bind(request.copy_id).bind(request.membership_id).fetch_one(&mut *transaction).await.context("Failed to inspect ready Library holds")?;
        if competing_ready_hold {
            bail!("This copy is reserved for another member");
        }
        let settings = sqlx::query_as::<_, (i16, i16)>("SELECT learner_loan_days, employee_loan_days FROM library_settings WHERE tenant_id = $1")
            .bind(tenant_id).fetch_one(&mut *transaction).await.context("Failed to load Library loan policy")?;
        let loan_days = if member.0.is_some() {
            settings.0
        } else {
            settings.1
        };
        let due_on = request
            .checked_out_on
            .checked_add_signed(Duration::days(i64::from(loan_days)))
            .ok_or_else(|| anyhow!("The Library due date is outside the supported range"))?;
        let loan_id = Uuid::new_v4();
        sqlx::query(
            r#"INSERT INTO library_loans (id, tenant_id, copy_id, membership_id, fulfilled_hold_id, checked_out_on, due_on, checked_out_by, notes)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)"#,
        ).bind(loan_id).bind(tenant_id).bind(request.copy_id).bind(request.membership_id).bind(request.fulfilled_hold_id)
          .bind(request.checked_out_on).bind(due_on).bind(actor_id).bind(trimmed(request.notes.as_deref()))
          .execute(&mut *transaction).await.context("Failed to create the Library loan")?;
        sqlx::query("UPDATE library_copies SET status = 'on_loan', version = version + 1, updated_at = NOW() WHERE tenant_id = $1 AND id = $2")
            .bind(tenant_id).bind(request.copy_id).execute(&mut *transaction).await.context("Failed to mark the Library copy on loan")?;
        if let Some((hold_id, hold_version)) = hold {
            sqlx::query(
                "UPDATE library_holds SET status = 'fulfilled', resolved_by = $3, resolved_at = NOW(), resolution_reason = 'Checked out', version = version + 1, updated_at = NOW() WHERE tenant_id = $1 AND id = $2 AND version = $4",
            ).bind(tenant_id).bind(hold_id).bind(actor_id).bind(hold_version).execute(&mut *transaction).await.context("Failed to fulfil the Library hold")?;
            append_event(
                &mut transaction,
                LibraryActivityEvent {
                    tenant_id,
                    aggregate_id: hold_id,
                    aggregate_type: "hold",
                    event_type: "fulfilled",
                    actor_id,
                    reason: Some("Checked out"),
                    metadata: json!({ "loan_id": loan_id }),
                },
            )
            .await?;
        }
        record_change(&mut transaction, tenant_id, actor, request_context, "loan", loan_id, "checked_out", "library.loans.checkout", json!({ "copy_id": request.copy_id, "membership_id": request.membership_id, "due_on": due_on })).await?;
        transaction
            .commit()
            .await
            .context("Failed to commit Library checkout")?;
        Self::get_loan(pool, tenant_id, loan_id, LibraryAccessScope::Campus)
            .await?
            .ok_or_else(|| anyhow!("The created Library loan could not be loaded"))
    }

    pub async fn renew(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        scope: LibraryAccessScope,
        actor: AuditActor,
        request_context: RequestContext,
        request: &RenewLoanRequest,
    ) -> Result<Option<LoanResponse>> {
        ensure_loan_visible(pool, tenant_id, id, scope).await?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start Library renewal")?;
        let loan = sqlx::query_as::<_, (Uuid, Uuid, NaiveDate, i16, i32, String)>(
            r#"SELECT loan.membership_id, copy.title_id, loan.due_on, loan.renewal_count, loan.version, loan.status
                 FROM library_loans AS loan JOIN library_copies AS copy ON copy.tenant_id = loan.tenant_id AND copy.id = loan.copy_id
                WHERE loan.tenant_id = $1 AND loan.id = $2 FOR UPDATE"#,
        ).bind(tenant_id).bind(id).fetch_optional(&mut *transaction).await.context("Failed to lock the Library loan")?;
        let Some(loan) = loan else {
            transaction.rollback().await.ok();
            return Ok(None);
        };
        if loan.5 != "active" {
            bail!("Only an active Library loan can be renewed");
        }
        if loan.4 != request.expected_version {
            bail!("The Library loan changed since it was loaded");
        }
        let policy = sqlx::query_as::<_, (i16, i16, i16, bool)>(
            r#"
            SELECT settings.maximum_renewals,
                   settings.learner_loan_days,
                   settings.employee_loan_days,
                   membership.learner_id IS NOT NULL
              FROM library_settings AS settings
              JOIN library_memberships AS membership
                ON membership.tenant_id = settings.tenant_id
               AND membership.id = $2
             WHERE settings.tenant_id = $1
            "#,
        )
        .bind(tenant_id)
        .bind(loan.0)
        .fetch_one(&mut *transaction)
        .await
        .context("Failed to load Library renewal policy")?;
        if loan.3 >= policy.0 {
            bail!("This loan has reached the renewal limit");
        }
        let queued = sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM library_holds WHERE tenant_id = $1 AND title_id = $2 AND membership_id <> $3 AND status IN ('waiting', 'ready'))")
            .bind(tenant_id).bind(loan.1).bind(loan.0).fetch_one(&mut *transaction).await.context("Failed to inspect Library holds")?;
        if queued {
            bail!("This title is needed for another member's hold and cannot be renewed");
        }
        let renewal_days = if policy.3 { policy.1 } else { policy.2 };
        let due_on = request
            .due_on
            .unwrap_or_else(|| loan.2 + Duration::days(i64::from(renewal_days)));
        if due_on <= loan.2 || due_on > loan.2 + Duration::days(365) {
            bail!("The renewed due date must be later and within one year");
        }
        sqlx::query("UPDATE library_loans SET due_on = $3, renewal_count = renewal_count + 1, version = version + 1, updated_at = NOW() WHERE tenant_id = $1 AND id = $2")
            .bind(tenant_id).bind(id).bind(due_on).execute(&mut *transaction).await.context("Failed to renew the Library loan")?;
        record_change(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "loan",
            id,
            "renewed",
            "library.loans.renew",
            json!({ "due_on": due_on, "renewal_count": loan.3 + 1, "version": loan.4 + 1 }),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit the Library renewal")?;
        Self::get_loan(pool, tenant_id, id, scope).await
    }

    pub async fn return_loan(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        request: &ReturnLoanRequest,
    ) -> Result<Option<LoanResponse>> {
        let actor_id = person_actor_id(actor)?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start Library return")?;
        let loan = sqlx::query_as::<_, (Uuid, NaiveDate, String, i32)>("SELECT copy_id, checked_out_on, status, version FROM library_loans WHERE tenant_id = $1 AND id = $2 FOR UPDATE")
            .bind(tenant_id).bind(id).fetch_optional(&mut *transaction).await.context("Failed to lock the Library loan")?;
        let Some(loan) = loan else {
            transaction.rollback().await.ok();
            return Ok(None);
        };
        if loan.2 != "active" {
            bail!("Only an active Library loan can be returned");
        }
        if loan.3 != request.expected_version {
            bail!("The Library loan changed since it was loaded");
        }
        if request.returned_on < loan.1 {
            bail!("The return date cannot precede checkout");
        }
        sqlx::query("UPDATE library_loans SET status = 'returned', returned_on = $3, returned_by = $4, notes = COALESCE($5, notes), version = version + 1, updated_at = NOW() WHERE tenant_id = $1 AND id = $2")
            .bind(tenant_id).bind(id).bind(request.returned_on).bind(actor_id).bind(trimmed(request.notes.as_deref())).execute(&mut *transaction).await.context("Failed to return the Library loan")?;
        let copy_status = if request.copy_condition == crate::CopyCondition::Damaged {
            "repair"
        } else {
            "available"
        };
        sqlx::query("UPDATE library_copies SET status = $3, condition = $4, version = version + 1, updated_at = NOW() WHERE tenant_id = $1 AND id = $2")
            .bind(tenant_id).bind(loan.0).bind(copy_status).bind(request.copy_condition.as_str()).execute(&mut *transaction).await.context("Failed to release the Library copy")?;
        record_change(&mut transaction, tenant_id, actor, request_context, "loan", id, "returned", "library.loans.return", json!({ "returned_on": request.returned_on, "copy_status": copy_status, "copy_condition": request.copy_condition.as_str(), "version": loan.3 + 1 })).await?;
        transaction
            .commit()
            .await
            .context("Failed to commit the Library return")?;
        Self::get_loan(pool, tenant_id, id, LibraryAccessScope::Campus).await
    }

    pub async fn mark_lost(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        request: &ReasonedVersionRequest,
    ) -> Result<Option<LoanResponse>> {
        let actor_id = person_actor_id(actor)?;
        let reason = required_reason(&request.reason)?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start lost-copy update")?;
        let loan = sqlx::query_as::<_, (Uuid, String, i32)>("SELECT copy_id, status, version FROM library_loans WHERE tenant_id = $1 AND id = $2 FOR UPDATE")
            .bind(tenant_id).bind(id).fetch_optional(&mut *transaction).await.context("Failed to lock the Library loan")?;
        let Some(loan) = loan else {
            transaction.rollback().await.ok();
            return Ok(None);
        };
        if loan.1 != "active" {
            bail!("Only an active Library loan can be marked lost");
        }
        if loan.2 != request.expected_version {
            bail!("The Library loan changed since it was loaded");
        }
        sqlx::query("UPDATE library_loans SET status = 'lost', lost_by = $3, notes = CASE WHEN notes IS NULL THEN $4 ELSE notes || E'\\n' || $4 END, version = version + 1, updated_at = NOW() WHERE tenant_id = $1 AND id = $2")
            .bind(tenant_id).bind(id).bind(actor_id).bind(&reason).execute(&mut *transaction).await.context("Failed to mark the Library loan lost")?;
        sqlx::query("UPDATE library_copies SET status = 'lost', version = version + 1, updated_at = NOW() WHERE tenant_id = $1 AND id = $2")
            .bind(tenant_id).bind(loan.0).execute(&mut *transaction).await.context("Failed to mark the Library copy lost")?;
        append_event(
            &mut transaction,
            LibraryActivityEvent {
                tenant_id,
                aggregate_id: id,
                aggregate_type: "loan",
                event_type: "lost",
                actor_id,
                reason: Some(&reason),
                metadata: json!({ "copy_id": loan.0, "version": loan.2 + 1 }),
            },
        )
        .await?;
        append_domain_audit(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "library.loans.mark_lost",
            "library_loan",
            id,
            json!({ "copy_id": loan.0, "version": loan.2 + 1 }),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit the lost Library loan")?;
        Self::get_loan(pool, tenant_id, id, LibraryAccessScope::Campus).await
    }

    pub async fn list_holds(
        pool: &PgPool,
        tenant_id: Uuid,
        scope: LibraryAccessScope,
        query: &BorrowingListQuery,
    ) -> Result<(Vec<HoldResponse>, i64)> {
        validate_hold_status(query.status.as_deref())?;
        let visible = visible_membership_ids(pool, tenant_id, scope).await?;
        let membership_filter = constrain_membership(query.membership_id, visible.as_deref());
        let search_ids = search_membership_ids(pool, tenant_id, query.search.as_deref()).await?;
        let (page, per_page) = bounded_page(query.page, query.per_page);
        let offset = (page - 1) * per_page;
        let rows = sqlx::query_as::<_, HoldRow>(HOLD_LIST_SELECT)
            .bind(tenant_id)
            .bind(query.status.as_deref())
            .bind(membership_filter.as_deref())
            .bind(search_ids.as_deref())
            .bind(per_page)
            .bind(offset)
            .fetch_all(pool)
            .await
            .context("Failed to list Library holds")?;
        let total = sqlx::query_scalar::<_, i64>(HOLD_COUNT_SELECT)
            .bind(tenant_id)
            .bind(query.status.as_deref())
            .bind(membership_filter.as_deref())
            .bind(search_ids.as_deref())
            .fetch_one(pool)
            .await
            .context("Failed to count Library holds")?;
        Ok((hydrate_holds(pool, tenant_id, rows).await?, total))
    }

    pub async fn get_hold(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        scope: LibraryAccessScope,
    ) -> Result<Option<HoldResponse>> {
        let visible = visible_membership_ids(pool, tenant_id, scope).await?;
        let row = sqlx::query_as::<_, HoldRow>(HOLD_BY_ID_SELECT)
            .bind(tenant_id)
            .bind(id)
            .bind(visible.as_deref())
            .fetch_optional(pool)
            .await
            .context("Failed to load the Library hold")?;
        let Some(row) = row else {
            return Ok(None);
        };
        Ok(hydrate_holds(pool, tenant_id, vec![row]).await?.pop())
    }

    pub async fn place_hold(
        pool: &PgPool,
        tenant_id: Uuid,
        scope: LibraryAccessScope,
        actor: AuditActor,
        request_context: RequestContext,
        request: &PlaceHoldRequest,
    ) -> Result<HoldResponse> {
        ensure_membership_visible(pool, tenant_id, request.membership_id, scope).await?;
        let actor_id = person_actor_id(actor)?;
        let mut transaction = pool.begin().await.context("Failed to start Library hold")?;
        let title_status = sqlx::query_scalar::<_, String>("SELECT status FROM library_titles WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL FOR UPDATE")
            .bind(tenant_id).bind(request.title_id).fetch_optional(&mut *transaction).await.context("Failed to lock the Library title")?.ok_or_else(|| anyhow!("The Library title was not found"))?;
        if title_status != "active" {
            bail!("Holds can be placed only on active titles");
        }
        let member_status = sqlx::query_scalar::<_, String>("SELECT status FROM library_memberships WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL")
            .bind(tenant_id).bind(request.membership_id).fetch_optional(&mut *transaction).await.context("Failed to load the Library membership")?.ok_or_else(|| anyhow!("The Library membership was not found"))?;
        if member_status != "active" {
            bail!("Holds require an active Library membership");
        }
        let queue_position = sqlx::query_scalar::<_, i64>("SELECT COALESCE(MAX(queue_position), 0) + 1 FROM library_holds WHERE tenant_id = $1 AND title_id = $2")
            .bind(tenant_id).bind(request.title_id).fetch_one(&mut *transaction).await.context("Failed to allocate the Library hold position")?;
        let id = Uuid::new_v4();
        sqlx::query("INSERT INTO library_holds (id, tenant_id, title_id, membership_id, queue_position, placed_by) VALUES ($1, $2, $3, $4, $5, $6)")
            .bind(id).bind(tenant_id).bind(request.title_id).bind(request.membership_id).bind(queue_position).bind(actor_id)
            .execute(&mut *transaction).await.map_err(|error| hold_database_error(error, "Failed to place the Library hold"))?;
        record_change(&mut transaction, tenant_id, actor, request_context, "hold", id, "placed", "library.holds.place", json!({ "title_id": request.title_id, "membership_id": request.membership_id, "queue_position": queue_position })).await?;
        transaction
            .commit()
            .await
            .context("Failed to commit the Library hold")?;
        Self::get_hold(pool, tenant_id, id, scope)
            .await?
            .ok_or_else(|| anyhow!("The created Library hold could not be loaded"))
    }

    pub async fn ready_hold(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        request: &ReadyHoldRequest,
    ) -> Result<Option<HoldResponse>> {
        let actor_id = person_actor_id(actor)?;
        if request.expires_at <= Utc::now() {
            bail!("The ready-hold expiry must be in the future");
        }
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start ready-hold update")?;
        let hold = sqlx::query_as::<_, (Uuid, String, i32)>("SELECT title_id, status, version FROM library_holds WHERE tenant_id = $1 AND id = $2 FOR UPDATE")
            .bind(tenant_id).bind(id).fetch_optional(&mut *transaction).await.context("Failed to lock the Library hold")?;
        let Some(hold) = hold else {
            transaction.rollback().await.ok();
            return Ok(None);
        };
        if hold.1 != "waiting" {
            bail!("Only a waiting Library hold can be marked ready");
        }
        if hold.2 != request.expected_version {
            bail!("The Library hold changed since it was loaded");
        }
        let copy = sqlx::query_as::<_, (Uuid, String)>("SELECT title_id, status FROM library_copies WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL FOR UPDATE")
            .bind(tenant_id).bind(request.copy_id).fetch_optional(&mut *transaction).await.context("Failed to lock the Library copy")?.ok_or_else(|| anyhow!("The Library copy was not found"))?;
        if copy.0 != hold.0 || copy.1 != "available" {
            bail!("Choose an available copy of the held title");
        }
        sqlx::query("UPDATE library_holds SET copy_id = $3, status = 'ready', ready_by = $4, ready_at = NOW(), expires_at = $5, version = version + 1, updated_at = NOW() WHERE tenant_id = $1 AND id = $2")
            .bind(tenant_id).bind(id).bind(request.copy_id).bind(actor_id).bind(request.expires_at).execute(&mut *transaction).await.context("Failed to mark the Library hold ready")?;
        sqlx::query("UPDATE library_copies SET status = 'reserved', version = version + 1, updated_at = NOW() WHERE tenant_id = $1 AND id = $2")
            .bind(tenant_id).bind(request.copy_id).execute(&mut *transaction).await.context("Failed to reserve the Library copy")?;
        record_change(&mut transaction, tenant_id, actor, request_context, "hold", id, "ready", "library.holds.ready", json!({ "copy_id": request.copy_id, "expires_at": request.expires_at, "version": hold.2 + 1 })).await?;
        transaction
            .commit()
            .await
            .context("Failed to commit the ready Library hold")?;
        Self::get_hold(pool, tenant_id, id, LibraryAccessScope::Campus).await
    }

    pub async fn cancel_hold(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        scope: LibraryAccessScope,
        actor: AuditActor,
        request_context: RequestContext,
        request: &ReasonedVersionRequest,
    ) -> Result<Option<HoldResponse>> {
        Self::resolve_hold(
            pool,
            tenant_id,
            id,
            scope,
            actor,
            request_context,
            request,
            "cancelled",
            "library.holds.cancel",
        )
        .await
    }

    pub async fn expire_hold(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        request: &ReasonedVersionRequest,
    ) -> Result<Option<HoldResponse>> {
        Self::resolve_hold(
            pool,
            tenant_id,
            id,
            LibraryAccessScope::Campus,
            actor,
            request_context,
            request,
            "expired",
            "library.holds.expire",
        )
        .await
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "hold transition authority is explicit"
    )]
    async fn resolve_hold(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        scope: LibraryAccessScope,
        actor: AuditActor,
        request_context: RequestContext,
        request: &ReasonedVersionRequest,
        target_status: &str,
        action: &str,
    ) -> Result<Option<HoldResponse>> {
        ensure_hold_visible(pool, tenant_id, id, scope).await?;
        let actor_id = person_actor_id(actor)?;
        let reason = required_reason(&request.reason)?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start Library hold resolution")?;
        let hold = sqlx::query_as::<_, (Option<Uuid>, String, i32)>("SELECT copy_id, status, version FROM library_holds WHERE tenant_id = $1 AND id = $2 FOR UPDATE")
            .bind(tenant_id).bind(id).fetch_optional(&mut *transaction).await.context("Failed to lock the Library hold")?;
        let Some(hold) = hold else {
            transaction.rollback().await.ok();
            return Ok(None);
        };
        if !matches!(hold.1.as_str(), "waiting" | "ready") {
            bail!("Only a waiting or ready Library hold can be resolved");
        }
        if hold.2 != request.expected_version {
            bail!("The Library hold changed since it was loaded");
        }
        sqlx::query("UPDATE library_holds SET status = $3, resolved_by = $4, resolved_at = NOW(), resolution_reason = $5, version = version + 1, updated_at = NOW() WHERE tenant_id = $1 AND id = $2")
            .bind(tenant_id).bind(id).bind(target_status).bind(actor_id).bind(&reason).execute(&mut *transaction).await.context("Failed to resolve the Library hold")?;
        if let Some(copy_id) = hold.0 {
            sqlx::query("UPDATE library_copies SET status = 'available', version = version + 1, updated_at = NOW() WHERE tenant_id = $1 AND id = $2 AND status = 'reserved'")
            .bind(tenant_id).bind(copy_id).execute(&mut *transaction).await.context("Failed to release the reserved Library copy")?;
        }
        append_event(
            &mut transaction,
            LibraryActivityEvent {
                tenant_id,
                aggregate_id: id,
                aggregate_type: "hold",
                event_type: target_status,
                actor_id,
                reason: Some(&reason),
                metadata: json!({ "version": hold.2 + 1 }),
            },
        )
        .await?;
        append_domain_audit(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            action,
            "library_hold",
            id,
            json!({ "status": target_status, "version": hold.2 + 1 }),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit the Library hold resolution")?;
        Self::get_hold(pool, tenant_id, id, scope).await
    }
}

async fn ensure_loan_visible(
    pool: &PgPool,
    tenant_id: Uuid,
    id: Uuid,
    scope: LibraryAccessScope,
) -> Result<()> {
    if LibraryCirculationOps::get_loan(pool, tenant_id, id, scope)
        .await?
        .is_none()
    {
        bail!("The Library loan was not found");
    }
    Ok(())
}
async fn ensure_hold_visible(
    pool: &PgPool,
    tenant_id: Uuid,
    id: Uuid,
    scope: LibraryAccessScope,
) -> Result<()> {
    if LibraryCirculationOps::get_hold(pool, tenant_id, id, scope)
        .await?
        .is_none()
    {
        bail!("The Library hold was not found");
    }
    Ok(())
}
async fn ensure_membership_visible(
    pool: &PgPool,
    tenant_id: Uuid,
    id: Uuid,
    scope: LibraryAccessScope,
) -> Result<()> {
    let visible = visible_membership_ids(pool, tenant_id, scope).await?;
    if visible.as_ref().is_some_and(|ids| !ids.contains(&id)) {
        bail!("The Library membership was not found");
    }
    Ok(())
}

fn constrain_membership(requested: Option<Uuid>, visible: Option<&[Uuid]>) -> Option<Vec<Uuid>> {
    match (requested, visible) {
        (Some(id), Some(values)) => Some(values.contains(&id).then_some(id).into_iter().collect()),
        (Some(id), None) => Some(vec![id]),
        (None, Some(values)) => Some(values.to_vec()),
        (None, None) => None,
    }
}
async fn search_membership_ids(
    pool: &PgPool,
    tenant_id: Uuid,
    search: Option<&str>,
) -> Result<Option<Vec<Uuid>>> {
    let Some(search) = search.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    let learners = LearnerOps::library_references(pool, tenant_id, Some(search), 100)
        .await?
        .into_iter()
        .map(|value| value.id)
        .collect::<Vec<_>>();
    let employees = EmployeeOps::list_references(pool, tenant_id, Some(search), None, 100)
        .await?
        .into_iter()
        .map(|value| value.id)
        .collect::<Vec<_>>();
    sqlx::query_scalar::<_, Uuid>("SELECT id FROM library_memberships WHERE tenant_id = $1 AND deleted_at IS NULL AND (learner_id = ANY($2) OR employee_id = ANY($3))").bind(tenant_id).bind(&learners).bind(&employees).fetch_all(pool).await.context("Failed to search Library memberships").map(Some)
}

async fn hydrate_loans(
    pool: &PgPool,
    tenant_id: Uuid,
    rows: Vec<LoanRow>,
) -> Result<Vec<LoanResponse>> {
    let identities = borrower_identities(
        pool,
        tenant_id,
        rows.iter().map(|row| (row.learner_id, row.employee_id)),
    )
    .await?;
    let today = Utc::now().date_naive();
    rows.into_iter()
        .map(|row| {
            let (kind, id) = borrower_key(row.learner_id, row.employee_id)?;
            let identity = identities
                .get(&(kind.as_str(), id))
                .ok_or_else(|| anyhow!("The Library loan borrower is unavailable"))?;
            let overdue_days = if row.status == "active" && row.due_on < today {
                (today - row.due_on).num_days()
            } else {
                0
            };
            Ok(LoanResponse {
                id: row.id,
                copy_id: row.copy_id,
                accession_number: row.accession_number,
                title_id: row.title_id,
                title: row.title,
                membership_id: row.membership_id,
                borrower_kind: kind,
                borrower_number: identity.number.clone(),
                borrower_name: identity.display_name.clone(),
                status: row.status,
                checked_out_on: row.checked_out_on,
                due_on: row.due_on,
                returned_on: row.returned_on,
                overdue: overdue_days > 0,
                overdue_days,
                renewal_count: row.renewal_count,
                version: row.version,
                notes: row.notes,
                created_at: row.created_at,
                updated_at: row.updated_at,
            })
        })
        .collect()
}
async fn hydrate_holds(
    pool: &PgPool,
    tenant_id: Uuid,
    rows: Vec<HoldRow>,
) -> Result<Vec<HoldResponse>> {
    let identities = borrower_identities(
        pool,
        tenant_id,
        rows.iter().map(|row| (row.learner_id, row.employee_id)),
    )
    .await?;
    rows.into_iter()
        .map(|row| {
            let (kind, id) = borrower_key(row.learner_id, row.employee_id)?;
            let identity = identities
                .get(&(kind.as_str(), id))
                .ok_or_else(|| anyhow!("The Library hold borrower is unavailable"))?;
            Ok(HoldResponse {
                id: row.id,
                title_id: row.title_id,
                title: row.title,
                membership_id: row.membership_id,
                borrower_kind: kind,
                borrower_number: identity.number.clone(),
                borrower_name: identity.display_name.clone(),
                copy_id: row.copy_id,
                accession_number: row.accession_number,
                queue_position: row.queue_position,
                status: row.status,
                version: row.version,
                expires_at: row.expires_at,
                resolution_reason: row.resolution_reason,
                created_at: row.created_at,
                updated_at: row.updated_at,
            })
        })
        .collect()
}

pub(crate) async fn borrower_identities<I>(
    pool: &PgPool,
    tenant_id: Uuid,
    values: I,
) -> Result<HashMap<(&'static str, Uuid), BorrowerIdentity>>
where
    I: IntoIterator<Item = (Option<Uuid>, Option<Uuid>)>,
{
    let pairs = values.into_iter().collect::<Vec<_>>();
    let learners = pairs.iter().filter_map(|value| value.0).collect::<Vec<_>>();
    let employees = pairs.iter().filter_map(|value| value.1).collect::<Vec<_>>();
    let mut map = HashMap::new();
    for value in LearnerOps::library_references_by_ids(pool, tenant_id, &learners).await? {
        map.insert(
            ("learner", value.id),
            BorrowerIdentity {
                number: value.learner_number,
                display_name: value.display_name,
                source_status: value.status,
                account_linked: value.account_id.is_some(),
            },
        );
    }
    for value in EmployeeOps::references_by_ids(pool, tenant_id, &employees).await? {
        map.insert(
            ("employee", value.id),
            BorrowerIdentity {
                number: value.employee_number,
                display_name: value.display_name,
                source_status: value.employment_status,
                account_linked: value.account_id.is_some(),
            },
        );
    }
    Ok(map)
}
pub(crate) fn borrower_key(
    learner_id: Option<Uuid>,
    employee_id: Option<Uuid>,
) -> Result<(BorrowerKind, Uuid)> {
    match (learner_id, employee_id) {
        (Some(id), None) => Ok((BorrowerKind::Learner, id)),
        (None, Some(id)) => Ok((BorrowerKind::Employee, id)),
        _ => bail!("The Library borrower reference is invalid"),
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "domain and audit records are committed together"
)]
async fn record_change(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    actor: AuditActor,
    request_context: RequestContext,
    aggregate_type: &str,
    aggregate_id: Uuid,
    event_type: &str,
    action: &str,
    metadata: serde_json::Value,
) -> Result<()> {
    let actor_id = person_actor_id(actor)?;
    append_event(
        transaction,
        LibraryActivityEvent {
            tenant_id,
            aggregate_id,
            aggregate_type,
            event_type,
            actor_id,
            reason: None,
            metadata: metadata.clone(),
        },
    )
    .await?;
    append_domain_audit(
        transaction,
        tenant_id,
        actor,
        request_context,
        action,
        &format!("library_{aggregate_type}"),
        aggregate_id,
        metadata,
    )
    .await
}

fn validate_loan_status(value: Option<&str>) -> Result<()> {
    if value.is_some_and(|value| !matches!(value, "active" | "returned" | "lost")) {
        bail!("The loan status filter is invalid");
    }
    Ok(())
}
fn validate_hold_status(value: Option<&str>) -> Result<()> {
    if value.is_some_and(|value| {
        !matches!(
            value,
            "waiting" | "ready" | "fulfilled" | "cancelled" | "expired"
        )
    }) {
        bail!("The hold status filter is invalid");
    }
    Ok(())
}
fn bounded_page(page: Option<i64>, per_page: Option<i64>) -> (i64, i64) {
    (
        page.unwrap_or(1).max(1),
        per_page.unwrap_or(25).clamp(1, 100),
    )
}
fn trimmed(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}
fn required_reason(value: &str) -> Result<String> {
    let value = value.trim();
    if value.is_empty() {
        bail!("A reason is required");
    }
    Ok(value.to_string())
}
fn hold_database_error(error: sqlx::Error, context: &str) -> anyhow::Error {
    if let sqlx::Error::Database(database) = &error
        && database.code().as_deref() == Some("23505")
    {
        return anyhow!("This member already has an active hold for the title");
    }
    anyhow!("{context}: {error}")
}

const LOAN_LIST_SELECT: &str = r#"SELECT loan.id, loan.copy_id, copy.accession_number, copy.title_id, title.title, loan.membership_id, member.learner_id, member.employee_id, loan.status, loan.checked_out_on, loan.due_on, loan.returned_on, loan.renewal_count, loan.version, loan.notes, loan.created_at, loan.updated_at FROM library_loans AS loan JOIN library_copies AS copy ON copy.tenant_id = loan.tenant_id AND copy.id = loan.copy_id JOIN library_titles AS title ON title.tenant_id = copy.tenant_id AND title.id = copy.title_id JOIN library_memberships AS member ON member.tenant_id = loan.tenant_id AND member.id = loan.membership_id WHERE loan.tenant_id = $1 AND ($2::TEXT IS NULL OR loan.status = $2) AND (NOT $3 OR (loan.status = 'active' AND loan.due_on < CURRENT_DATE)) AND ($4::UUID[] IS NULL OR loan.membership_id = ANY($4)) AND ($5::UUID[] IS NULL OR loan.membership_id = ANY($5)) ORDER BY CASE WHEN loan.status = 'active' THEN loan.due_on END, loan.updated_at DESC, loan.id LIMIT $6 OFFSET $7"#;
const LOAN_COUNT_SELECT: &str = "SELECT COUNT(*) FROM library_loans AS loan WHERE loan.tenant_id = $1 AND ($2::TEXT IS NULL OR loan.status = $2) AND (NOT $3 OR (loan.status = 'active' AND loan.due_on < CURRENT_DATE)) AND ($4::UUID[] IS NULL OR loan.membership_id = ANY($4)) AND ($5::UUID[] IS NULL OR loan.membership_id = ANY($5))";
const LOAN_BY_ID_SELECT: &str = r#"SELECT loan.id, loan.copy_id, copy.accession_number, copy.title_id, title.title, loan.membership_id, member.learner_id, member.employee_id, loan.status, loan.checked_out_on, loan.due_on, loan.returned_on, loan.renewal_count, loan.version, loan.notes, loan.created_at, loan.updated_at FROM library_loans AS loan JOIN library_copies AS copy ON copy.tenant_id = loan.tenant_id AND copy.id = loan.copy_id JOIN library_titles AS title ON title.tenant_id = copy.tenant_id AND title.id = copy.title_id JOIN library_memberships AS member ON member.tenant_id = loan.tenant_id AND member.id = loan.membership_id WHERE loan.tenant_id = $1 AND loan.id = $2 AND ($3::UUID[] IS NULL OR loan.membership_id = ANY($3))"#;
const HOLD_LIST_SELECT: &str = r#"SELECT hold.id, hold.title_id, title.title, hold.membership_id, member.learner_id, member.employee_id, hold.copy_id, copy.accession_number, hold.queue_position, hold.status, hold.version, hold.expires_at, hold.resolution_reason, hold.created_at, hold.updated_at FROM library_holds AS hold JOIN library_titles AS title ON title.tenant_id = hold.tenant_id AND title.id = hold.title_id JOIN library_memberships AS member ON member.tenant_id = hold.tenant_id AND member.id = hold.membership_id LEFT JOIN library_copies AS copy ON copy.tenant_id = hold.tenant_id AND copy.id = hold.copy_id WHERE hold.tenant_id = $1 AND ($2::TEXT IS NULL OR hold.status = $2) AND ($3::UUID[] IS NULL OR hold.membership_id = ANY($3)) AND ($4::UUID[] IS NULL OR hold.membership_id = ANY($4)) ORDER BY CASE WHEN hold.status IN ('waiting', 'ready') THEN 0 ELSE 1 END, hold.queue_position, hold.updated_at DESC LIMIT $5 OFFSET $6"#;
const HOLD_COUNT_SELECT: &str = "SELECT COUNT(*) FROM library_holds AS hold WHERE hold.tenant_id = $1 AND ($2::TEXT IS NULL OR hold.status = $2) AND ($3::UUID[] IS NULL OR hold.membership_id = ANY($3)) AND ($4::UUID[] IS NULL OR hold.membership_id = ANY($4))";
const HOLD_BY_ID_SELECT: &str = r#"SELECT hold.id, hold.title_id, title.title, hold.membership_id, member.learner_id, member.employee_id, hold.copy_id, copy.accession_number, hold.queue_position, hold.status, hold.version, hold.expires_at, hold.resolution_reason, hold.created_at, hold.updated_at FROM library_holds AS hold JOIN library_titles AS title ON title.tenant_id = hold.tenant_id AND title.id = hold.title_id JOIN library_memberships AS member ON member.tenant_id = hold.tenant_id AND member.id = hold.membership_id LEFT JOIN library_copies AS copy ON copy.tenant_id = hold.tenant_id AND copy.id = hold.copy_id WHERE hold.tenant_id = $1 AND hold.id = $2 AND ($3::UUID[] IS NULL OR hold.membership_id = ANY($3))"#;

#[cfg(test)]
mod tests {
    use super::constrain_membership;
    use uuid::Uuid;
    #[test]
    fn requested_member_never_widens_self_scope() {
        let visible = Uuid::new_v4();
        let hidden = Uuid::new_v4();
        assert!(
            constrain_membership(Some(hidden), Some(&[visible]))
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            constrain_membership(Some(visible), Some(&[visible])).unwrap(),
            vec![visible]
        );
    }
}
