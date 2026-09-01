//! Tenant-scoped Health operations with canonical person hydration and audit.

use std::collections::{HashMap, HashSet};

use anyhow::{Context, Result, anyhow, bail};
use chrono::{Duration, Utc};
use cp_audit::{
    AuditActor, AuditOutcome, AuditTarget, NewAuditEvent, RequestContext, append as append_audit,
};
use cp_hr_payroll::ops::EmployeeOps;
use cp_sis::ops::{EnrolmentOps, LearnerOps};
use serde_json::{Map, Value, json};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::{
    CareItemResponse, CloseVisitRequest, CreateCareItemRequest, CreateFollowUpRequest,
    CreateMedicationPlanRequest, CreatePatientRequest, CreateVisitRequest, EmployeeCandidate,
    FollowUpResponse, GuardianContact, HealthAccessScope, HealthListQuery, HealthReferenceData,
    MedicationAdministrationResponse, MedicationPlanResponse, PatientCandidate, PatientKind,
    PatientRecord, PatientSummary, RecordMedicationAdministrationRequest, UpdateCareItemRequest,
    UpdateFollowUpRequest, UpdateMedicationPlanRequest, UpdatePatientRequest, VisitResponse,
    models::{
        CareItemRow, FollowUpRow, MedicationAdministrationRow, MedicationPlanRow, PatientRow,
        VisitRow,
    },
};

pub struct HealthOps;

impl HealthOps {
    pub async fn reference_data(
        pool: &PgPool,
        tenant_id: Uuid,
        search: Option<&str>,
    ) -> Result<HealthReferenceData> {
        let learners = LearnerOps::library_references(pool, tenant_id, search, 100).await?;
        let employees =
            EmployeeOps::list_references(pool, tenant_id, search, Some("active"), 100).await?;
        let existing = sqlx::query_as::<_, (Option<Uuid>, Option<Uuid>)>(
            "SELECT learner_id, employee_id FROM health_patients WHERE tenant_id = $1",
        )
        .bind(tenant_id)
        .fetch_all(pool)
        .await
        .context("Failed to resolve current Health patients")?;
        let learner_patients = existing
            .iter()
            .filter_map(|value| value.0)
            .collect::<HashSet<_>>();
        let employee_patients = existing
            .iter()
            .filter_map(|value| value.1)
            .collect::<HashSet<_>>();
        let mut patients = learners
            .into_iter()
            .map(|value| PatientCandidate {
                kind: PatientKind::Learner,
                id: value.id,
                number: value.learner_number,
                display_name: value.display_name,
                source_status: value.status,
                already_patient: learner_patients.contains(&value.id),
            })
            .collect::<Vec<_>>();
        patients.extend(employees.iter().map(|value| PatientCandidate {
            kind: PatientKind::Employee,
            id: value.id,
            number: value.employee_number.clone(),
            display_name: value.display_name.clone(),
            source_status: value.employment_status.clone(),
            already_patient: employee_patients.contains(&value.id),
        }));
        patients.sort_by(|left, right| left.display_name.cmp(&right.display_name));
        Ok(HealthReferenceData {
            patients,
            employees: employees
                .into_iter()
                .map(|value| EmployeeCandidate {
                    id: value.id,
                    number: value.employee_number,
                    display_name: value.display_name,
                })
                .collect(),
        })
    }

    pub async fn list_patients(
        pool: &PgPool,
        tenant_id: Uuid,
        scope: HealthAccessScope,
        query: &HealthListQuery,
    ) -> Result<(Vec<PatientSummary>, i64)> {
        validate_patient_status(query.status.as_deref())?;
        let visible = visible_people(pool, tenant_id, scope).await?;
        let (page, per_page) = bounded_page(query);
        let offset = (page - 1) * per_page;
        let search = trimmed(query.search.as_deref());
        let (search_learners, search_employees) =
            search_person_ids(pool, tenant_id, search).await?;
        let learner_ids = visible.learner_filter();
        let employee_ids = visible.employee_filter();
        let rows = sqlx::query_as::<_, PatientRow>(PATIENT_SELECT)
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
            .context("Failed to list Health patients")?;
        let total = sqlx::query_scalar::<_, i64>(PATIENT_COUNT)
            .bind(tenant_id)
            .bind(query.status.as_deref())
            .bind(learner_ids.as_deref())
            .bind(employee_ids.as_deref())
            .bind(search_learners.as_deref())
            .bind(search_employees.as_deref())
            .fetch_one(pool)
            .await
            .context("Failed to count Health patients")?;
        Ok((hydrate_patients(pool, tenant_id, rows).await?, total))
    }

    pub async fn create_patient(
        pool: &PgPool,
        tenant_id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        request: &CreatePatientRequest,
    ) -> Result<PatientRecord> {
        validate_person(pool, tenant_id, request.person_kind, request.person_id).await?;
        let actor_id = person_actor_id(actor)?;
        let (learner_id, employee_id) = match request.person_kind {
            PatientKind::Learner => (Some(request.person_id), None),
            PatientKind::Employee => (None, Some(request.person_id)),
        };
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start patient creation")?;
        let id = sqlx::query_scalar::<_, Uuid>(
            r#"
            INSERT INTO health_patients (
                tenant_id, learner_id, employee_id, created_by, updated_by
            ) VALUES ($1, $2, $3, $4, $4)
            ON CONFLICT DO NOTHING
            RETURNING id
            "#,
        )
        .bind(tenant_id)
        .bind(learner_id)
        .bind(employee_id)
        .bind(actor_id)
        .fetch_optional(&mut *transaction)
        .await
        .context("Failed to create Health patient")?
        .ok_or_else(|| anyhow!("This person already has a Health patient record"))?;
        append_evidence(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "patient",
            id,
            id,
            "created",
            "health.patients.create",
            json!({ "person_kind": request.person_kind.as_str() }),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit patient creation")?;
        Self::get_patient(pool, tenant_id, id, HealthAccessScope::Campus)
            .await?
            .ok_or_else(|| anyhow!("The Health patient could not be reloaded"))
    }

    pub async fn get_patient(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        scope: HealthAccessScope,
    ) -> Result<Option<PatientRecord>> {
        let visible_ids = visible_patient_ids(pool, tenant_id, scope).await?;
        let row = sqlx::query_as::<_, PatientRow>(PATIENT_BY_ID)
            .bind(tenant_id)
            .bind(id)
            .bind(visible_ids.as_deref())
            .fetch_optional(pool)
            .await
            .context("Failed to load Health patient")?;
        let Some(row) = row else {
            return Ok(None);
        };
        let mut patients = hydrate_patients(pool, tenant_id, vec![row]).await?;
        let patient = patients
            .pop()
            .ok_or_else(|| anyhow!("The Health patient identity is unavailable"))?;
        let care_items = sqlx::query_as::<_, CareItemRow>(CARE_ITEM_SELECT)
            .bind(tenant_id)
            .bind(id)
            .fetch_all(pool)
            .await
            .context("Failed to load patient care items")?
            .into_iter()
            .map(care_item_response)
            .collect();
        let guardian_contacts = if patient.person_kind == PatientKind::Learner {
            LearnerOps::health_guardian_contacts_by_learner_ids(
                pool,
                tenant_id,
                &[patient.person_id],
            )
            .await?
            .into_iter()
            .map(|value| GuardianContact {
                guardian_id: value.guardian_id,
                display_name: value.display_name,
                relationship_type: value.relationship_type,
                is_primary: value.is_primary,
                can_collect: value.can_collect,
                phone: value.phone,
                email: value.email,
            })
            .collect()
        } else {
            Vec::new()
        };
        Ok(Some(PatientRecord {
            patient,
            guardian_contacts,
            care_items,
        }))
    }

    pub async fn update_patient(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        request: &UpdatePatientRequest,
    ) -> Result<Option<PatientRecord>> {
        let actor_id = person_actor_id(actor)?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start patient update")?;
        let changed = sqlx::query_scalar::<_, Uuid>(
            r#"
            UPDATE health_patients
               SET status = $3, version = version + 1, updated_by = $4, updated_at = NOW()
             WHERE tenant_id = $1 AND id = $2 AND version = $5
             RETURNING id
            "#,
        )
        .bind(tenant_id)
        .bind(id)
        .bind(request.status.as_str())
        .bind(actor_id)
        .bind(request.expected_version)
        .fetch_optional(&mut *transaction)
        .await
        .context("Failed to update Health patient")?;
        if changed.is_none() {
            return versioned_not_found(
                &mut transaction,
                tenant_id,
                "health_patients",
                id,
                request.expected_version,
            )
            .await;
        }
        append_evidence(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "patient",
            id,
            id,
            "updated",
            "health.patients.update",
            json!({ "status": request.status.as_str() }),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit patient update")?;
        Self::get_patient(pool, tenant_id, id, HealthAccessScope::Campus).await
    }

    pub async fn create_care_item(
        pool: &PgPool,
        tenant_id: Uuid,
        patient_id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        request: &CreateCareItemRequest,
    ) -> Result<CareItemResponse> {
        ensure_active_patient(pool, tenant_id, patient_id).await?;
        let actor_id = person_actor_id(actor)?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start care item creation")?;
        let id = sqlx::query_scalar::<_, Uuid>(
            r#"
            INSERT INTO health_care_items (
                tenant_id, patient_id, kind, title, details, severity, reviewed_on,
                created_by, updated_by
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $8)
            RETURNING id
            "#,
        )
        .bind(tenant_id)
        .bind(patient_id)
        .bind(request.kind.as_str())
        .bind(required("Care item title", &request.title)?)
        .bind(optional_text(request.details.as_deref()))
        .bind(request.severity.as_str())
        .bind(request.reviewed_on)
        .bind(actor_id)
        .fetch_one(&mut *transaction)
        .await
        .context("Failed to create care item")?;
        append_evidence(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "care_item",
            id,
            patient_id,
            "created",
            "health.care_items.create",
            json!({ "kind": request.kind.as_str(), "severity": request.severity.as_str() }),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit care item creation")?;
        load_care_item(pool, tenant_id, id)
            .await?
            .ok_or_else(|| anyhow!("The care item could not be reloaded"))
    }

    pub async fn update_care_item(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        request: &UpdateCareItemRequest,
    ) -> Result<Option<CareItemResponse>> {
        if !matches!(request.status.as_str(), "active" | "resolved") {
            bail!("Care item status must be active or resolved");
        }
        let actor_id = person_actor_id(actor)?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start care item update")?;
        let row = sqlx::query_as::<_, (Uuid, String)>(
            "SELECT patient_id, status FROM health_care_items WHERE tenant_id = $1 AND id = $2 FOR UPDATE",
        ).bind(tenant_id).bind(id).fetch_optional(&mut *transaction).await
        .context("Failed to lock care item")?;
        let Some((patient_id, current_status)) = row else {
            return Ok(None);
        };
        let (resolved_by, resolved_at) = if request.status == "resolved" {
            (Some(actor_id), Some(Utc::now()))
        } else {
            (None, None)
        };
        let changed = sqlx::query_scalar::<_, Uuid>(
            r#"
            UPDATE health_care_items
               SET kind = $3, title = $4, details = $5, severity = $6,
                   reviewed_on = $7, status = $8, resolved_by = $9, resolved_at = $10,
                   updated_by = $11, version = version + 1, updated_at = NOW()
             WHERE tenant_id = $1 AND id = $2 AND version = $12
             RETURNING id
            "#,
        )
        .bind(tenant_id)
        .bind(id)
        .bind(request.kind.as_str())
        .bind(required("Care item title", &request.title)?)
        .bind(optional_text(request.details.as_deref()))
        .bind(request.severity.as_str())
        .bind(request.reviewed_on)
        .bind(&request.status)
        .bind(resolved_by)
        .bind(resolved_at)
        .bind(actor_id)
        .bind(request.expected_version)
        .fetch_optional(&mut *transaction)
        .await
        .context("Failed to update care item")?;
        if changed.is_none() {
            bail!("The care item changed; reload it before saving");
        }
        append_evidence(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "care_item",
            id,
            patient_id,
            if current_status != request.status {
                "status_changed"
            } else {
                "updated"
            },
            "health.care_items.update",
            json!({ "status": request.status, "severity": request.severity.as_str() }),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit care item update")?;
        load_care_item(pool, tenant_id, id).await
    }

    pub async fn list_visits(
        pool: &PgPool,
        tenant_id: Uuid,
        scope: HealthAccessScope,
        query: &HealthListQuery,
    ) -> Result<(Vec<VisitResponse>, i64)> {
        validate_status(query.status.as_deref(), &["open", "closed"], "visit")?;
        let visible_ids = visible_patient_ids(pool, tenant_id, scope).await?;
        let (page, per_page) = bounded_page(query);
        let rows = sqlx::query_as::<_, VisitRow>(VISIT_LIST)
            .bind(tenant_id)
            .bind(query.status.as_deref())
            .bind(query.patient_id)
            .bind(visible_ids.as_deref())
            .bind(per_page)
            .bind((page - 1) * per_page)
            .fetch_all(pool)
            .await
            .context("Failed to list clinic visits")?;
        let total = sqlx::query_scalar::<_, i64>(VISIT_COUNT)
            .bind(tenant_id)
            .bind(query.status.as_deref())
            .bind(query.patient_id)
            .bind(visible_ids.as_deref())
            .fetch_one(pool)
            .await
            .context("Failed to count clinic visits")?;
        Ok((hydrate_visits(pool, tenant_id, rows).await?, total))
    }

    pub async fn get_visit(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        scope: HealthAccessScope,
    ) -> Result<Option<VisitResponse>> {
        let visible_ids = visible_patient_ids(pool, tenant_id, scope).await?;
        let row = sqlx::query_as::<_, VisitRow>(VISIT_BY_ID)
            .bind(tenant_id)
            .bind(id)
            .bind(visible_ids.as_deref())
            .fetch_optional(pool)
            .await
            .context("Failed to load clinic visit")?;
        let Some(row) = row else {
            return Ok(None);
        };
        Ok(hydrate_visits(pool, tenant_id, vec![row]).await?.pop())
    }

    pub async fn create_visit(
        pool: &PgPool,
        tenant_id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        request: &CreateVisitRequest,
    ) -> Result<VisitResponse> {
        ensure_active_patient(pool, tenant_id, request.patient_id).await?;
        if request.checked_in_at > Utc::now() + Duration::minutes(5) {
            bail!("Clinic visit check-in cannot be in the future");
        }
        let actor_id = person_actor_id(actor)?;
        let mut transaction = pool.begin().await.context("Failed to start clinic visit")?;
        let id = sqlx::query_scalar::<_, Uuid>(
            r#"
            INSERT INTO health_visits (
                tenant_id, patient_id, checked_in_at, category, presenting_concern,
                assessment, care_given, opened_by
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8) RETURNING id
            "#,
        )
        .bind(tenant_id)
        .bind(request.patient_id)
        .bind(request.checked_in_at)
        .bind(request.category.as_str())
        .bind(required("Presenting concern", &request.presenting_concern)?)
        .bind(optional_text(request.assessment.as_deref()))
        .bind(optional_text(request.care_given.as_deref()))
        .bind(actor_id)
        .fetch_one(&mut *transaction)
        .await
        .context("Failed to create clinic visit")?;
        append_evidence(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "visit",
            id,
            request.patient_id,
            "opened",
            "health.visits.create",
            json!({ "category": request.category.as_str() }),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit clinic visit")?;
        Self::get_visit(pool, tenant_id, id, HealthAccessScope::Campus)
            .await?
            .ok_or_else(|| anyhow!("The clinic visit could not be reloaded"))
    }

    pub async fn close_visit(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        request: &CloseVisitRequest,
    ) -> Result<Option<VisitResponse>> {
        let actor_id = person_actor_id(actor)?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start visit closure")?;
        let changed = sqlx::query_as::<_, (Uuid, Uuid)>(
            r#"
            UPDATE health_visits
               SET assessment = COALESCE($3, assessment), care_given = COALESCE($4, care_given),
                   disposition = $5, status = 'closed', closed_by = $6, closed_at = NOW(),
                   version = version + 1, updated_at = NOW()
             WHERE tenant_id = $1 AND id = $2 AND version = $7 AND status = 'open'
             RETURNING id, patient_id
            "#,
        )
        .bind(tenant_id)
        .bind(id)
        .bind(optional_text(request.assessment.as_deref()))
        .bind(optional_text(request.care_given.as_deref()))
        .bind(request.disposition.as_str())
        .bind(actor_id)
        .bind(request.expected_version)
        .fetch_optional(&mut *transaction)
        .await
        .context("Failed to close clinic visit")?;
        let Some((_, patient_id)) = changed else {
            if sqlx::query_scalar::<_, bool>(
                "SELECT EXISTS(SELECT 1 FROM health_visits WHERE tenant_id = $1 AND id = $2)",
            )
            .bind(tenant_id)
            .bind(id)
            .fetch_one(&mut *transaction)
            .await?
            {
                bail!("The clinic visit changed or is already closed; reload it before saving");
            }
            return Ok(None);
        };
        append_evidence(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "visit",
            id,
            patient_id,
            "closed",
            "health.visits.close",
            json!({ "disposition": request.disposition.as_str() }),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit visit closure")?;
        Self::get_visit(pool, tenant_id, id, HealthAccessScope::Campus).await
    }

    pub async fn list_medication_plans(
        pool: &PgPool,
        tenant_id: Uuid,
        scope: HealthAccessScope,
        query: &HealthListQuery,
    ) -> Result<(Vec<MedicationPlanResponse>, i64)> {
        validate_status(
            query.status.as_deref(),
            &["active", "suspended", "ended"],
            "medication plan",
        )?;
        let visible_ids = visible_patient_ids(pool, tenant_id, scope).await?;
        let (page, per_page) = bounded_page(query);
        let rows = sqlx::query_as::<_, MedicationPlanRow>(MEDICATION_PLAN_LIST)
            .bind(tenant_id)
            .bind(query.status.as_deref())
            .bind(query.patient_id)
            .bind(visible_ids.as_deref())
            .bind(per_page)
            .bind((page - 1) * per_page)
            .fetch_all(pool)
            .await
            .context("Failed to list medication plans")?;
        let total = sqlx::query_scalar::<_, i64>(MEDICATION_PLAN_COUNT)
            .bind(tenant_id)
            .bind(query.status.as_deref())
            .bind(query.patient_id)
            .bind(visible_ids.as_deref())
            .fetch_one(pool)
            .await
            .context("Failed to count medication plans")?;
        Ok((
            hydrate_medication_plans(pool, tenant_id, rows).await?,
            total,
        ))
    }

    pub async fn create_medication_plan(
        pool: &PgPool,
        tenant_id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        request: &CreateMedicationPlanRequest,
    ) -> Result<MedicationPlanResponse> {
        validate_plan_dates(request.starts_on, request.ends_on)?;
        ensure_active_patient(pool, tenant_id, request.patient_id).await?;
        let actor_id = person_actor_id(actor)?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start medication plan creation")?;
        let id = sqlx::query_scalar::<_, Uuid>(
            r#"
            INSERT INTO health_medication_plans (
                tenant_id, patient_id, medication_name, dosage, route, schedule,
                instructions, authorization_reference, starts_on, ends_on, created_by, updated_by
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$11) RETURNING id
            "#,
        )
        .bind(tenant_id)
        .bind(request.patient_id)
        .bind(required("Medication name", &request.medication_name)?)
        .bind(required("Dosage", &request.dosage)?)
        .bind(required("Route", &request.route)?)
        .bind(required("Schedule", &request.schedule)?)
        .bind(optional_text(request.instructions.as_deref()))
        .bind(required(
            "Authorization reference",
            &request.authorization_reference,
        )?)
        .bind(request.starts_on)
        .bind(request.ends_on)
        .bind(actor_id)
        .fetch_one(&mut *transaction)
        .await
        .context("Failed to create medication plan")?;
        append_evidence(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "medication_plan",
            id,
            request.patient_id,
            "created",
            "health.medication_plans.create",
            json!({}),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit medication plan")?;
        load_medication_plan(pool, tenant_id, id)
            .await?
            .ok_or_else(|| anyhow!("The medication plan could not be reloaded"))
    }

    pub async fn update_medication_plan(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        request: &UpdateMedicationPlanRequest,
    ) -> Result<Option<MedicationPlanResponse>> {
        validate_plan_dates(request.starts_on, request.ends_on)?;
        let actor_id = person_actor_id(actor)?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start medication plan update")?;
        let changed = sqlx::query_as::<_, (Uuid, Uuid)>(
            r#"
            UPDATE health_medication_plans SET
                medication_name=$3, dosage=$4, route=$5, schedule=$6, instructions=$7,
                authorization_reference=$8, starts_on=$9, ends_on=$10, status=$11,
                updated_by=$12, version=version+1, updated_at=NOW()
             WHERE tenant_id=$1 AND id=$2 AND version=$13 RETURNING id, patient_id
            "#,
        )
        .bind(tenant_id)
        .bind(id)
        .bind(required("Medication name", &request.medication_name)?)
        .bind(required("Dosage", &request.dosage)?)
        .bind(required("Route", &request.route)?)
        .bind(required("Schedule", &request.schedule)?)
        .bind(optional_text(request.instructions.as_deref()))
        .bind(required(
            "Authorization reference",
            &request.authorization_reference,
        )?)
        .bind(request.starts_on)
        .bind(request.ends_on)
        .bind(request.status.as_str())
        .bind(actor_id)
        .bind(request.expected_version)
        .fetch_optional(&mut *transaction)
        .await
        .context("Failed to update medication plan")?;
        let Some((_, patient_id)) = changed else {
            return versioned_not_found(
                &mut transaction,
                tenant_id,
                "health_medication_plans",
                id,
                request.expected_version,
            )
            .await;
        };
        append_evidence(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "medication_plan",
            id,
            patient_id,
            "updated",
            "health.medication_plans.update",
            json!({ "status": request.status.as_str() }),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit medication plan update")?;
        load_medication_plan(pool, tenant_id, id).await
    }

    pub async fn list_medication_administrations(
        pool: &PgPool,
        tenant_id: Uuid,
        scope: HealthAccessScope,
        query: &HealthListQuery,
    ) -> Result<(Vec<MedicationAdministrationResponse>, i64)> {
        let visible_ids = visible_patient_ids(pool, tenant_id, scope).await?;
        let (page, per_page) = bounded_page(query);
        let rows = sqlx::query_as::<_, MedicationAdministrationRow>(MEDICATION_ADMIN_LIST)
            .bind(tenant_id)
            .bind(query.patient_id)
            .bind(visible_ids.as_deref())
            .bind(per_page)
            .bind((page - 1) * per_page)
            .fetch_all(pool)
            .await
            .context("Failed to list medication administrations")?;
        let total = sqlx::query_scalar::<_, i64>(MEDICATION_ADMIN_COUNT)
            .bind(tenant_id)
            .bind(query.patient_id)
            .bind(visible_ids.as_deref())
            .fetch_one(pool)
            .await
            .context("Failed to count medication administrations")?;
        Ok((hydrate_administrations(pool, tenant_id, rows).await?, total))
    }

    pub async fn record_medication_administration(
        pool: &PgPool,
        tenant_id: Uuid,
        plan_id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        request: &RecordMedicationAdministrationRequest,
    ) -> Result<MedicationAdministrationResponse> {
        if request.administered_at > Utc::now() + Duration::minutes(5) {
            bail!("Medication administration time cannot be in the future");
        }
        let plan = sqlx::query_as::<_, (Uuid, String, chrono::NaiveDate, Option<chrono::NaiveDate>)>(
            "SELECT patient_id, status, starts_on, ends_on FROM health_medication_plans WHERE tenant_id=$1 AND id=$2",
        ).bind(tenant_id).bind(plan_id).fetch_optional(pool).await
        .context("Failed to load medication plan")?.ok_or_else(|| anyhow!("The medication plan was not found"))?;
        if plan.1 != "active" {
            bail!("The medication plan is not active");
        }
        let administered_on = request.administered_at.date_naive();
        if administered_on < plan.2 || plan.3.is_some_and(|ends_on| administered_on > ends_on) {
            bail!("The administration time is outside the medication plan dates");
        }
        let actor_id = person_actor_id(actor)?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start medication administration")?;
        let id = sqlx::query_scalar::<_, Uuid>(
            r#"
            INSERT INTO health_medication_administrations (
                tenant_id, medication_plan_id, patient_id, administered_at, dose,
                outcome, note, recorded_by
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8) RETURNING id
            "#,
        )
        .bind(tenant_id)
        .bind(plan_id)
        .bind(plan.0)
        .bind(request.administered_at)
        .bind(required("Administered dose", &request.dose)?)
        .bind(request.outcome.as_str())
        .bind(optional_text(request.note.as_deref()))
        .bind(actor_id)
        .fetch_one(&mut *transaction)
        .await
        .context("Failed to record medication administration")?;
        append_evidence(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "medication_administration",
            id,
            plan.0,
            "recorded",
            "health.medication_administrations.create",
            json!({ "outcome": request.outcome.as_str() }),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit medication administration")?;
        load_administration(pool, tenant_id, id)
            .await?
            .ok_or_else(|| anyhow!("The medication administration could not be reloaded"))
    }

    pub async fn list_follow_ups(
        pool: &PgPool,
        tenant_id: Uuid,
        scope: HealthAccessScope,
        query: &HealthListQuery,
    ) -> Result<(Vec<FollowUpResponse>, i64)> {
        validate_status(
            query.status.as_deref(),
            &["open", "completed", "cancelled"],
            "follow-up",
        )?;
        let visible_ids = visible_patient_ids(pool, tenant_id, scope).await?;
        let (page, per_page) = bounded_page(query);
        let rows = sqlx::query_as::<_, FollowUpRow>(FOLLOW_UP_LIST)
            .bind(tenant_id)
            .bind(query.status.as_deref())
            .bind(query.patient_id)
            .bind(visible_ids.as_deref())
            .bind(per_page)
            .bind((page - 1) * per_page)
            .fetch_all(pool)
            .await
            .context("Failed to list health follow-ups")?;
        let total = sqlx::query_scalar::<_, i64>(FOLLOW_UP_COUNT)
            .bind(tenant_id)
            .bind(query.status.as_deref())
            .bind(query.patient_id)
            .bind(visible_ids.as_deref())
            .fetch_one(pool)
            .await
            .context("Failed to count health follow-ups")?;
        Ok((hydrate_follow_ups(pool, tenant_id, rows).await?, total))
    }

    pub async fn create_follow_up(
        pool: &PgPool,
        tenant_id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        request: &CreateFollowUpRequest,
    ) -> Result<FollowUpResponse> {
        ensure_active_patient(pool, tenant_id, request.patient_id).await?;
        validate_follow_up_references(
            pool,
            tenant_id,
            request.patient_id,
            request.visit_id,
            request.assigned_employee_id,
        )
        .await?;
        let actor_id = person_actor_id(actor)?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start follow-up creation")?;
        let id = sqlx::query_scalar::<_, Uuid>(
            r#"
            INSERT INTO health_follow_ups (
                tenant_id, patient_id, visit_id, assigned_employee_id, due_on,
                purpose, created_by, updated_by
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$7) RETURNING id
            "#,
        )
        .bind(tenant_id)
        .bind(request.patient_id)
        .bind(request.visit_id)
        .bind(request.assigned_employee_id)
        .bind(request.due_on)
        .bind(required("Follow-up purpose", &request.purpose)?)
        .bind(actor_id)
        .fetch_one(&mut *transaction)
        .await
        .context("Failed to create health follow-up")?;
        append_evidence(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "follow_up",
            id,
            request.patient_id,
            "created",
            "health.follow_ups.create",
            json!({ "due_on": request.due_on }),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit health follow-up")?;
        load_follow_up(pool, tenant_id, id)
            .await?
            .ok_or_else(|| anyhow!("The health follow-up could not be reloaded"))
    }

    pub async fn update_follow_up(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        request: &UpdateFollowUpRequest,
    ) -> Result<Option<FollowUpResponse>> {
        let patient_id = sqlx::query_scalar::<_, Uuid>(
            "SELECT patient_id FROM health_follow_ups WHERE tenant_id=$1 AND id=$2",
        )
        .bind(tenant_id)
        .bind(id)
        .fetch_optional(pool)
        .await
        .context("Failed to load health follow-up")?;
        let Some(patient_id) = patient_id else {
            return Ok(None);
        };
        validate_follow_up_references(
            pool,
            tenant_id,
            patient_id,
            None,
            request.assigned_employee_id,
        )
        .await?;
        let outcome = optional_text(request.outcome.as_deref());
        match request.status {
            crate::FollowUpStatus::Open if outcome.is_some() => {
                bail!("An open follow-up cannot have an outcome")
            }
            crate::FollowUpStatus::Completed | crate::FollowUpStatus::Cancelled
                if outcome.is_none() =>
            {
                bail!("A completed or cancelled follow-up requires an outcome")
            }
            _ => {}
        }
        let completed_at = if request.status == crate::FollowUpStatus::Open {
            None
        } else {
            Some(Utc::now())
        };
        let actor_id = person_actor_id(actor)?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start follow-up update")?;
        let changed = sqlx::query_scalar::<_, Uuid>(
            r#"
            UPDATE health_follow_ups SET assigned_employee_id=$3, due_on=$4, purpose=$5,
                   status=$6, outcome=$7, completed_at=$8, updated_by=$9,
                   version=version+1, updated_at=NOW()
             WHERE tenant_id=$1 AND id=$2 AND version=$10 RETURNING id
            "#,
        )
        .bind(tenant_id)
        .bind(id)
        .bind(request.assigned_employee_id)
        .bind(request.due_on)
        .bind(required("Follow-up purpose", &request.purpose)?)
        .bind(request.status.as_str())
        .bind(outcome)
        .bind(completed_at)
        .bind(actor_id)
        .bind(request.expected_version)
        .fetch_optional(&mut *transaction)
        .await
        .context("Failed to update health follow-up")?;
        if changed.is_none() {
            bail!("The health follow-up changed; reload it before saving");
        }
        append_evidence(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "follow_up",
            id,
            patient_id,
            "updated",
            "health.follow_ups.update",
            json!({ "status": request.status.as_str(), "due_on": request.due_on }),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit health follow-up update")?;
        load_follow_up(pool, tenant_id, id).await
    }
}

#[derive(Debug, Clone)]
struct PersonIdentity {
    kind: PatientKind,
    id: Uuid,
    number: String,
    name: String,
    status: String,
}

#[derive(Debug)]
struct VisiblePeople {
    campus: bool,
    learners: Vec<Uuid>,
    employees: Vec<Uuid>,
}
impl VisiblePeople {
    fn learner_filter(&self) -> Option<Vec<Uuid>> {
        (!self.campus).then(|| self.learners.clone())
    }
    fn employee_filter(&self) -> Option<Vec<Uuid>> {
        (!self.campus).then(|| self.employees.clone())
    }
}

async fn visible_people(
    pool: &PgPool,
    tenant_id: Uuid,
    scope: HealthAccessScope,
) -> Result<VisiblePeople> {
    match scope {
        HealthAccessScope::Campus => Ok(VisiblePeople {
            campus: true,
            learners: vec![],
            employees: vec![],
        }),
        HealthAccessScope::SelfFor(account_id) => Ok(VisiblePeople {
            campus: false,
            learners: EnrolmentOps::learner_ids_for_account(pool, tenant_id, account_id).await?,
            employees: EmployeeOps::active_reference_by_account(pool, tenant_id, account_id)
                .await?
                .into_iter()
                .map(|value| value.id)
                .collect(),
        }),
    }
}

async fn visible_patient_ids(
    pool: &PgPool,
    tenant_id: Uuid,
    scope: HealthAccessScope,
) -> Result<Option<Vec<Uuid>>> {
    let people = visible_people(pool, tenant_id, scope).await?;
    if people.campus {
        return Ok(None);
    }
    sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM health_patients WHERE tenant_id=$1 AND (learner_id=ANY($2) OR employee_id=ANY($3))",
    ).bind(tenant_id).bind(&people.learners).bind(&people.employees).fetch_all(pool).await
    .context("Failed to resolve visible Health patients").map(Some)
}

async fn search_person_ids(
    pool: &PgPool,
    tenant_id: Uuid,
    search: Option<&str>,
) -> Result<(Option<Vec<Uuid>>, Option<Vec<Uuid>>)> {
    let Some(search) = search else {
        return Ok((None, None));
    };
    let learners = LearnerOps::library_references(pool, tenant_id, Some(search), 100)
        .await?
        .into_iter()
        .map(|value| value.id)
        .collect();
    let employees = EmployeeOps::list_references(pool, tenant_id, Some(search), None, 100)
        .await?
        .into_iter()
        .map(|value| value.id)
        .collect();
    Ok((Some(learners), Some(employees)))
}

async fn patient_identities(
    pool: &PgPool,
    tenant_id: Uuid,
    patient_ids: &[Uuid],
) -> Result<HashMap<Uuid, PersonIdentity>> {
    if patient_ids.is_empty() {
        return Ok(HashMap::new());
    }
    let links = sqlx::query_as::<_, (Uuid, Option<Uuid>, Option<Uuid>)>(
        "SELECT id, learner_id, employee_id FROM health_patients WHERE tenant_id=$1 AND id=ANY($2)",
    )
    .bind(tenant_id)
    .bind(patient_ids)
    .fetch_all(pool)
    .await
    .context("Failed to load Health patient identity links")?;
    let learner_ids = links.iter().filter_map(|value| value.1).collect::<Vec<_>>();
    let employee_ids = links.iter().filter_map(|value| value.2).collect::<Vec<_>>();
    let learners = LearnerOps::library_references_by_ids(pool, tenant_id, &learner_ids)
        .await?
        .into_iter()
        .map(|value| {
            (
                value.id,
                PersonIdentity {
                    kind: PatientKind::Learner,
                    id: value.id,
                    number: value.learner_number,
                    name: value.display_name,
                    status: value.status,
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
                PersonIdentity {
                    kind: PatientKind::Employee,
                    id: value.id,
                    number: value.employee_number,
                    name: value.display_name,
                    status: value.employment_status,
                },
            )
        })
        .collect::<HashMap<_, _>>();
    links
        .into_iter()
        .map(|(patient_id, learner_id, employee_id)| {
            let identity = learner_id
                .and_then(|id| learners.get(&id))
                .or_else(|| employee_id.and_then(|id| employees.get(&id)))
                .cloned()
                .ok_or_else(|| anyhow!("The Health patient source record is unavailable"))?;
            Ok((patient_id, identity))
        })
        .collect()
}

async fn hydrate_patients(
    pool: &PgPool,
    tenant_id: Uuid,
    rows: Vec<PatientRow>,
) -> Result<Vec<PatientSummary>> {
    let identities = patient_identities(
        pool,
        tenant_id,
        &rows.iter().map(|value| value.id).collect::<Vec<_>>(),
    )
    .await?;
    rows.into_iter()
        .map(|row| {
            let identity = identities
                .get(&row.id)
                .ok_or_else(|| anyhow!("The Health patient source record is unavailable"))?;
            Ok(PatientSummary {
                id: row.id,
                person_kind: identity.kind,
                person_id: identity.id,
                person_number: identity.number.clone(),
                person_name: identity.name.clone(),
                source_status: identity.status.clone(),
                status: row.status,
                version: row.version,
                active_care_item_count: row.active_care_item_count,
                open_visit_count: row.open_visit_count,
                active_medication_count: row.active_medication_count,
                open_follow_up_count: row.open_follow_up_count,
                created_at: row.created_at,
                updated_at: row.updated_at,
            })
        })
        .collect()
}

async fn hydrate_visits(
    pool: &PgPool,
    tenant_id: Uuid,
    rows: Vec<VisitRow>,
) -> Result<Vec<VisitResponse>> {
    let identities = patient_identities(
        pool,
        tenant_id,
        &rows
            .iter()
            .map(|value| value.patient_id)
            .collect::<Vec<_>>(),
    )
    .await?;
    rows.into_iter()
        .map(|row| {
            let identity = identities
                .get(&row.patient_id)
                .ok_or_else(|| anyhow!("The visit patient is unavailable"))?;
            Ok(VisitResponse {
                id: row.id,
                patient_id: row.patient_id,
                patient_kind: identity.kind,
                patient_number: identity.number.clone(),
                patient_name: identity.name.clone(),
                checked_in_at: row.checked_in_at,
                category: row.category,
                presenting_concern: row.presenting_concern,
                assessment: row.assessment,
                care_given: row.care_given,
                disposition: row.disposition,
                status: row.status,
                version: row.version,
                closed_at: row.closed_at,
                created_at: row.created_at,
                updated_at: row.updated_at,
            })
        })
        .collect()
}

async fn hydrate_medication_plans(
    pool: &PgPool,
    tenant_id: Uuid,
    rows: Vec<MedicationPlanRow>,
) -> Result<Vec<MedicationPlanResponse>> {
    let identities = patient_identities(
        pool,
        tenant_id,
        &rows
            .iter()
            .map(|value| value.patient_id)
            .collect::<Vec<_>>(),
    )
    .await?;
    rows.into_iter()
        .map(|row| {
            let identity = identities
                .get(&row.patient_id)
                .ok_or_else(|| anyhow!("The medication patient is unavailable"))?;
            Ok(MedicationPlanResponse {
                id: row.id,
                patient_id: row.patient_id,
                patient_kind: identity.kind,
                patient_number: identity.number.clone(),
                patient_name: identity.name.clone(),
                medication_name: row.medication_name,
                dosage: row.dosage,
                route: row.route,
                schedule: row.schedule,
                instructions: row.instructions,
                authorization_reference: row.authorization_reference,
                starts_on: row.starts_on,
                ends_on: row.ends_on,
                status: row.status,
                version: row.version,
                created_at: row.created_at,
                updated_at: row.updated_at,
            })
        })
        .collect()
}

async fn hydrate_administrations(
    pool: &PgPool,
    tenant_id: Uuid,
    rows: Vec<MedicationAdministrationRow>,
) -> Result<Vec<MedicationAdministrationResponse>> {
    let identities = patient_identities(
        pool,
        tenant_id,
        &rows
            .iter()
            .map(|value| value.patient_id)
            .collect::<Vec<_>>(),
    )
    .await?;
    rows.into_iter()
        .map(|row| {
            let identity = identities
                .get(&row.patient_id)
                .ok_or_else(|| anyhow!("The medication patient is unavailable"))?;
            Ok(MedicationAdministrationResponse {
                id: row.id,
                medication_plan_id: row.medication_plan_id,
                patient_id: row.patient_id,
                patient_number: identity.number.clone(),
                patient_name: identity.name.clone(),
                medication_name: row.medication_name,
                administered_at: row.administered_at,
                dose: row.dose,
                outcome: row.outcome,
                note: row.note,
                created_at: row.created_at,
            })
        })
        .collect()
}

async fn hydrate_follow_ups(
    pool: &PgPool,
    tenant_id: Uuid,
    rows: Vec<FollowUpRow>,
) -> Result<Vec<FollowUpResponse>> {
    let identities = patient_identities(
        pool,
        tenant_id,
        &rows
            .iter()
            .map(|value| value.patient_id)
            .collect::<Vec<_>>(),
    )
    .await?;
    let employee_ids = rows
        .iter()
        .filter_map(|value| value.assigned_employee_id)
        .collect::<Vec<_>>();
    let employees = EmployeeOps::references_by_ids(pool, tenant_id, &employee_ids)
        .await?
        .into_iter()
        .map(|value| (value.id, value.display_name))
        .collect::<HashMap<_, _>>();
    rows.into_iter()
        .map(|row| {
            let identity = identities
                .get(&row.patient_id)
                .ok_or_else(|| anyhow!("The follow-up patient is unavailable"))?;
            Ok(FollowUpResponse {
                id: row.id,
                patient_id: row.patient_id,
                patient_kind: identity.kind,
                patient_number: identity.number.clone(),
                patient_name: identity.name.clone(),
                visit_id: row.visit_id,
                assigned_employee_id: row.assigned_employee_id,
                assigned_employee_name: row
                    .assigned_employee_id
                    .and_then(|id| employees.get(&id).cloned()),
                due_on: row.due_on,
                purpose: row.purpose,
                status: row.status,
                outcome: row.outcome,
                version: row.version,
                completed_at: row.completed_at,
                created_at: row.created_at,
                updated_at: row.updated_at,
            })
        })
        .collect()
}

async fn load_care_item(
    pool: &PgPool,
    tenant_id: Uuid,
    id: Uuid,
) -> Result<Option<CareItemResponse>> {
    sqlx::query_as::<_, CareItemRow>(&format!("{CARE_ITEM_COLUMNS} WHERE tenant_id=$1 AND id=$2"))
        .bind(tenant_id)
        .bind(id)
        .fetch_optional(pool)
        .await
        .context("Failed to load care item")
        .map(|value| value.map(care_item_response))
}
fn care_item_response(row: CareItemRow) -> CareItemResponse {
    CareItemResponse {
        id: row.id,
        patient_id: row.patient_id,
        kind: row.kind,
        title: row.title,
        details: row.details,
        severity: row.severity,
        status: row.status,
        reviewed_on: row.reviewed_on,
        version: row.version,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }
}
async fn load_medication_plan(
    pool: &PgPool,
    tenant_id: Uuid,
    id: Uuid,
) -> Result<Option<MedicationPlanResponse>> {
    let row = sqlx::query_as::<_, MedicationPlanRow>(&format!(
        "{MEDICATION_PLAN_COLUMNS} WHERE tenant_id=$1 AND id=$2"
    ))
    .bind(tenant_id)
    .bind(id)
    .fetch_optional(pool)
    .await
    .context("Failed to load medication plan")?;
    let Some(row) = row else {
        return Ok(None);
    };
    Ok(hydrate_medication_plans(pool, tenant_id, vec![row])
        .await?
        .pop())
}
async fn load_administration(
    pool: &PgPool,
    tenant_id: Uuid,
    id: Uuid,
) -> Result<Option<MedicationAdministrationResponse>> {
    let row = sqlx::query_as::<_, MedicationAdministrationRow>(
        r#"SELECT administration.id, administration.medication_plan_id, administration.patient_id,
                  plan.medication_name, administration.administered_at, administration.dose,
                  administration.outcome, administration.note, administration.created_at
             FROM health_medication_administrations AS administration
             JOIN health_medication_plans AS plan ON plan.id=administration.medication_plan_id AND plan.tenant_id=administration.tenant_id
            WHERE administration.tenant_id=$1 AND administration.id=$2"#,
    ).bind(tenant_id).bind(id).fetch_optional(pool).await.context("Failed to load medication administration")?;
    let Some(row) = row else {
        return Ok(None);
    };
    Ok(hydrate_administrations(pool, tenant_id, vec![row])
        .await?
        .pop())
}
async fn load_follow_up(
    pool: &PgPool,
    tenant_id: Uuid,
    id: Uuid,
) -> Result<Option<FollowUpResponse>> {
    let row = sqlx::query_as::<_, FollowUpRow>(&format!(
        "{FOLLOW_UP_COLUMNS} WHERE tenant_id=$1 AND id=$2"
    ))
    .bind(tenant_id)
    .bind(id)
    .fetch_optional(pool)
    .await
    .context("Failed to load health follow-up")?;
    let Some(row) = row else {
        return Ok(None);
    };
    Ok(hydrate_follow_ups(pool, tenant_id, vec![row]).await?.pop())
}

async fn validate_person(
    pool: &PgPool,
    tenant_id: Uuid,
    kind: PatientKind,
    id: Uuid,
) -> Result<()> {
    match kind {
        PatientKind::Learner => {
            let learner = LearnerOps::library_references_by_ids(pool, tenant_id, &[id])
                .await?
                .pop()
                .ok_or_else(|| anyhow!("The learner was not found"))?;
            if !matches!(learner.status.as_str(), "active" | "prospective") {
                bail!("The learner is not active");
            }
        }
        PatientKind::Employee => {
            let employee = EmployeeOps::get_reference(pool, tenant_id, id)
                .await?
                .ok_or_else(|| anyhow!("The employee was not found"))?;
            if employee.employment_status != "active" {
                bail!("The employee is not active");
            }
        }
    }
    Ok(())
}
async fn ensure_active_patient(pool: &PgPool, tenant_id: Uuid, id: Uuid) -> Result<()> {
    let status = sqlx::query_scalar::<_, String>(
        "SELECT status FROM health_patients WHERE tenant_id=$1 AND id=$2",
    )
    .bind(tenant_id)
    .bind(id)
    .fetch_optional(pool)
    .await
    .context("Failed to load Health patient")?
    .ok_or_else(|| anyhow!("The Health patient was not found"))?;
    if status != "active" {
        bail!("The Health patient is inactive");
    }
    Ok(())
}
async fn validate_follow_up_references(
    pool: &PgPool,
    tenant_id: Uuid,
    patient_id: Uuid,
    visit_id: Option<Uuid>,
    employee_id: Option<Uuid>,
) -> Result<()> {
    if let Some(visit_id) = visit_id {
        let found = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM health_visits WHERE tenant_id=$1 AND id=$2 AND patient_id=$3)",
        ).bind(tenant_id).bind(visit_id).bind(patient_id).fetch_one(pool).await?;
        if !found {
            bail!("The clinic visit does not belong to this patient");
        }
    }
    if let Some(employee_id) = employee_id {
        let employee = EmployeeOps::get_reference(pool, tenant_id, employee_id)
            .await?
            .ok_or_else(|| anyhow!("The assigned employee was not found"))?;
        if employee.employment_status != "active" {
            bail!("The assigned employee is not active");
        }
    }
    Ok(())
}
fn validate_plan_dates(
    starts_on: chrono::NaiveDate,
    ends_on: Option<chrono::NaiveDate>,
) -> Result<()> {
    if ends_on.is_some_and(|ends_on| ends_on < starts_on) {
        bail!("Medication plan end date cannot be before its start date");
    }
    Ok(())
}
fn validate_patient_status(status: Option<&str>) -> Result<()> {
    validate_status(status, &["active", "inactive"], "patient")
}
fn validate_status(status: Option<&str>, allowed: &[&str], label: &str) -> Result<()> {
    if status.is_some_and(|value| !allowed.contains(&value)) {
        bail!("The {label} status filter is invalid");
    }
    Ok(())
}
fn bounded_page(query: &HealthListQuery) -> (i64, i64) {
    (
        query.page.unwrap_or(1).max(1),
        query.per_page.unwrap_or(25).clamp(1, 100),
    )
}
fn trimmed(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}
fn required<'a>(label: &str, value: &'a str) -> Result<&'a str> {
    let value = value.trim();
    if value.is_empty() {
        bail!("{label} is required");
    }
    Ok(value)
}
fn optional_text(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}
fn person_actor_id(actor: AuditActor) -> Result<Uuid> {
    actor
        .user_id()
        .ok_or_else(|| anyhow!("Authenticated person actor is required"))
}

async fn versioned_not_found<T>(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    table: &str,
    id: Uuid,
    _version: i32,
) -> Result<Option<T>> {
    let query = format!("SELECT EXISTS(SELECT 1 FROM {table} WHERE tenant_id=$1 AND id=$2)");
    if sqlx::query_scalar::<_, bool>(&query)
        .bind(tenant_id)
        .bind(id)
        .fetch_one(&mut **transaction)
        .await?
    {
        bail!("The record changed; reload it before saving");
    }
    Ok(None)
}

#[allow(
    clippy::too_many_arguments,
    reason = "health evidence keeps target and patient scope explicit"
)]
async fn append_evidence(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    actor: AuditActor,
    request_context: RequestContext,
    aggregate_type: &str,
    aggregate_id: Uuid,
    patient_id: Uuid,
    event_type: &str,
    action: &str,
    metadata: Value,
) -> Result<()> {
    let actor_id = person_actor_id(actor)?;
    sqlx::query(
        r#"INSERT INTO health_activity_events (
               tenant_id, aggregate_type, aggregate_id, patient_id, event_type, actor_id, metadata
           ) VALUES ($1,$2,$3,$4,$5,$6,$7)"#,
    )
    .bind(tenant_id)
    .bind(aggregate_type)
    .bind(aggregate_id)
    .bind(patient_id)
    .bind(event_type)
    .bind(actor_id)
    .bind(metadata.clone())
    .execute(&mut **transaction)
    .await
    .context("Failed to append Health activity evidence")?;
    append_audit(
        &mut **transaction,
        &NewAuditEvent::new(
            tenant_id,
            actor,
            action,
            AuditOutcome::Succeeded,
            request_context,
        )
        .with_target(AuditTarget::new(aggregate_type, aggregate_id.to_string()))
        .with_redacted_metadata(metadata.as_object().cloned().unwrap_or_else(Map::new)),
    )
    .await
    .context("Failed to append Health audit evidence")?;
    Ok(())
}

const PATIENT_SELECT: &str = r#"
    SELECT patient.id, patient.status, patient.version,
           (SELECT COUNT(*) FROM health_care_items item WHERE item.tenant_id=patient.tenant_id AND item.patient_id=patient.id AND item.status='active') AS active_care_item_count,
           (SELECT COUNT(*) FROM health_visits visit WHERE visit.tenant_id=patient.tenant_id AND visit.patient_id=patient.id AND visit.status='open') AS open_visit_count,
           (SELECT COUNT(*) FROM health_medication_plans plan WHERE plan.tenant_id=patient.tenant_id AND plan.patient_id=patient.id AND plan.status='active') AS active_medication_count,
           (SELECT COUNT(*) FROM health_follow_ups follow_up WHERE follow_up.tenant_id=patient.tenant_id AND follow_up.patient_id=patient.id AND follow_up.status='open') AS open_follow_up_count,
           patient.created_at, patient.updated_at
      FROM health_patients AS patient
     WHERE patient.tenant_id=$1 AND ($2::TEXT IS NULL OR patient.status=$2)
       AND ($3::UUID[] IS NULL OR patient.learner_id=ANY($3) OR patient.employee_id=ANY($4))
       AND ($5::UUID[] IS NULL OR patient.learner_id=ANY($5) OR patient.employee_id=ANY($6))
     ORDER BY patient.updated_at DESC, patient.id LIMIT $7 OFFSET $8
"#;
const PATIENT_COUNT: &str = r#"
    SELECT COUNT(*) FROM health_patients AS patient
     WHERE patient.tenant_id=$1 AND ($2::TEXT IS NULL OR patient.status=$2)
       AND ($3::UUID[] IS NULL OR patient.learner_id=ANY($3) OR patient.employee_id=ANY($4))
       AND ($5::UUID[] IS NULL OR patient.learner_id=ANY($5) OR patient.employee_id=ANY($6))
"#;
const PATIENT_BY_ID: &str = r#"
    SELECT patient.id, patient.status, patient.version,
           (SELECT COUNT(*) FROM health_care_items item WHERE item.tenant_id=patient.tenant_id AND item.patient_id=patient.id AND item.status='active') AS active_care_item_count,
           (SELECT COUNT(*) FROM health_visits visit WHERE visit.tenant_id=patient.tenant_id AND visit.patient_id=patient.id AND visit.status='open') AS open_visit_count,
           (SELECT COUNT(*) FROM health_medication_plans plan WHERE plan.tenant_id=patient.tenant_id AND plan.patient_id=patient.id AND plan.status='active') AS active_medication_count,
           (SELECT COUNT(*) FROM health_follow_ups follow_up WHERE follow_up.tenant_id=patient.tenant_id AND follow_up.patient_id=patient.id AND follow_up.status='open') AS open_follow_up_count,
           patient.created_at, patient.updated_at
      FROM health_patients AS patient
     WHERE patient.tenant_id=$1 AND patient.id=$2 AND ($3::UUID[] IS NULL OR patient.id=ANY($3))
"#;
const CARE_ITEM_COLUMNS: &str = "SELECT id,patient_id,kind,title,details,severity,status,reviewed_on,version,created_at,updated_at FROM health_care_items";
const CARE_ITEM_SELECT: &str = "SELECT id,patient_id,kind,title,details,severity,status,reviewed_on,version,created_at,updated_at FROM health_care_items WHERE tenant_id=$1 AND patient_id=$2 ORDER BY status, severity DESC, updated_at DESC";
const VISIT_LIST: &str = "SELECT id,patient_id,checked_in_at,category,presenting_concern,assessment,care_given,disposition,status,version,closed_at,created_at,updated_at FROM health_visits WHERE tenant_id=$1 AND ($2::TEXT IS NULL OR status=$2) AND ($3::UUID IS NULL OR patient_id=$3) AND ($4::UUID[] IS NULL OR patient_id=ANY($4)) ORDER BY checked_in_at DESC,id LIMIT $5 OFFSET $6";
const VISIT_COUNT: &str = "SELECT COUNT(*) FROM health_visits WHERE tenant_id=$1 AND ($2::TEXT IS NULL OR status=$2) AND ($3::UUID IS NULL OR patient_id=$3) AND ($4::UUID[] IS NULL OR patient_id=ANY($4))";
const VISIT_BY_ID: &str = "SELECT id,patient_id,checked_in_at,category,presenting_concern,assessment,care_given,disposition,status,version,closed_at,created_at,updated_at FROM health_visits WHERE tenant_id=$1 AND id=$2 AND ($3::UUID[] IS NULL OR patient_id=ANY($3))";
const MEDICATION_PLAN_COLUMNS: &str = "SELECT id,patient_id,medication_name,dosage,route,schedule,instructions,authorization_reference,starts_on,ends_on,status,version,created_at,updated_at FROM health_medication_plans";
const MEDICATION_PLAN_LIST: &str = "SELECT id,patient_id,medication_name,dosage,route,schedule,instructions,authorization_reference,starts_on,ends_on,status,version,created_at,updated_at FROM health_medication_plans WHERE tenant_id=$1 AND ($2::TEXT IS NULL OR status=$2) AND ($3::UUID IS NULL OR patient_id=$3) AND ($4::UUID[] IS NULL OR patient_id=ANY($4)) ORDER BY status,starts_on DESC,id LIMIT $5 OFFSET $6";
const MEDICATION_PLAN_COUNT: &str = "SELECT COUNT(*) FROM health_medication_plans WHERE tenant_id=$1 AND ($2::TEXT IS NULL OR status=$2) AND ($3::UUID IS NULL OR patient_id=$3) AND ($4::UUID[] IS NULL OR patient_id=ANY($4))";
const MEDICATION_ADMIN_LIST: &str = r#"SELECT administration.id,administration.medication_plan_id,administration.patient_id,plan.medication_name,administration.administered_at,administration.dose,administration.outcome,administration.note,administration.created_at FROM health_medication_administrations administration JOIN health_medication_plans plan ON plan.id=administration.medication_plan_id AND plan.tenant_id=administration.tenant_id WHERE administration.tenant_id=$1 AND ($2::UUID IS NULL OR administration.patient_id=$2) AND ($3::UUID[] IS NULL OR administration.patient_id=ANY($3)) ORDER BY administration.administered_at DESC,administration.id LIMIT $4 OFFSET $5"#;
const MEDICATION_ADMIN_COUNT: &str = "SELECT COUNT(*) FROM health_medication_administrations WHERE tenant_id=$1 AND ($2::UUID IS NULL OR patient_id=$2) AND ($3::UUID[] IS NULL OR patient_id=ANY($3))";
const FOLLOW_UP_COLUMNS: &str = "SELECT id,patient_id,visit_id,assigned_employee_id,due_on,purpose,status,outcome,version,completed_at,created_at,updated_at FROM health_follow_ups";
const FOLLOW_UP_LIST: &str = "SELECT id,patient_id,visit_id,assigned_employee_id,due_on,purpose,status,outcome,version,completed_at,created_at,updated_at FROM health_follow_ups WHERE tenant_id=$1 AND ($2::TEXT IS NULL OR status=$2) AND ($3::UUID IS NULL OR patient_id=$3) AND ($4::UUID[] IS NULL OR patient_id=ANY($4)) ORDER BY CASE WHEN status='open' THEN 0 ELSE 1 END,due_on,id LIMIT $5 OFFSET $6";
const FOLLOW_UP_COUNT: &str = "SELECT COUNT(*) FROM health_follow_ups WHERE tenant_id=$1 AND ($2::TEXT IS NULL OR status=$2) AND ($3::UUID IS NULL OR patient_id=$3) AND ($4::UUID[] IS NULL OR patient_id=ANY($4))";

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn patient_query_keeps_identity_outside_health_storage() {
        assert!(PATIENT_SELECT.contains("learner_id"));
        assert!(!PATIENT_SELECT.contains("display_name"));
        assert!(!PATIENT_SELECT.contains("phone"));
    }
    #[test]
    fn status_filters_are_closed() {
        assert!(validate_patient_status(Some("active")).is_ok());
        assert!(validate_patient_status(Some("unknown")).is_err());
    }
}
