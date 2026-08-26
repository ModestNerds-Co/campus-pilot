use actix_web::http::StatusCode;
use actix_web::{HttpResponse, get, post, put, web};
use cp_common::{ApiResponse, RequirePermission, TenantId};
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::generator::{generate, validate_configuration};
use crate::models::{TimetableConfiguration, TimetableRun, TimetableRunRow};

#[get("/configuration")]
async fn get_configuration(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
) -> Result<HttpResponse, actix_web::Error> {
    let tenant_id = tenant.into_inner().into_inner();
    let row = sqlx::query(
        r#"SELECT cycle_name, days, periods, classes, subjects, teachers, rooms, lesson_requirements
           FROM timetable_configurations WHERE tenant_id = $1"#,
    )
    .bind(tenant_id)
    .fetch_optional(pool.get_ref())
    .await
    .map_err(actix_web::error::ErrorInternalServerError)?;

    let configuration = match row {
        Some(row) => TimetableConfiguration {
            cycle_name: row
                .try_get("cycle_name")
                .map_err(actix_web::error::ErrorInternalServerError)?,
            days: serde_json::from_value(
                row.try_get("days")
                    .map_err(actix_web::error::ErrorInternalServerError)?,
            )
            .map_err(actix_web::error::ErrorInternalServerError)?,
            periods: serde_json::from_value(
                row.try_get("periods")
                    .map_err(actix_web::error::ErrorInternalServerError)?,
            )
            .map_err(actix_web::error::ErrorInternalServerError)?,
            classes: serde_json::from_value(
                row.try_get("classes")
                    .map_err(actix_web::error::ErrorInternalServerError)?,
            )
            .map_err(actix_web::error::ErrorInternalServerError)?,
            subjects: serde_json::from_value(
                row.try_get("subjects")
                    .map_err(actix_web::error::ErrorInternalServerError)?,
            )
            .map_err(actix_web::error::ErrorInternalServerError)?,
            teachers: serde_json::from_value(
                row.try_get("teachers")
                    .map_err(actix_web::error::ErrorInternalServerError)?,
            )
            .map_err(actix_web::error::ErrorInternalServerError)?,
            rooms: serde_json::from_value(
                row.try_get("rooms")
                    .map_err(actix_web::error::ErrorInternalServerError)?,
            )
            .map_err(actix_web::error::ErrorInternalServerError)?,
            lesson_requirements: serde_json::from_value(
                row.try_get("lesson_requirements")
                    .map_err(actix_web::error::ErrorInternalServerError)?,
            )
            .map_err(actix_web::error::ErrorInternalServerError)?,
        },
        None => TimetableConfiguration::default(),
    };
    Ok(HttpResponse::Ok().json(ApiResponse::from_status(
        StatusCode::OK,
        Some(configuration),
        None,
    )))
}

#[put("/configuration")]
async fn save_configuration(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    body: web::Json<TimetableConfiguration>,
) -> Result<HttpResponse, actix_web::Error> {
    if let Err(issues) = validate_configuration(&body) {
        return Ok(HttpResponse::BadRequest().json(ApiResponse::from_status(
            StatusCode::BAD_REQUEST,
            None::<()>,
            Some(issues),
        )));
    }
    let tenant_id = tenant.into_inner().into_inner();
    let value = body.into_inner();
    sqlx::query(
        r#"INSERT INTO timetable_configurations
              (tenant_id, cycle_name, days, periods, classes, subjects, teachers, rooms, lesson_requirements)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
           ON CONFLICT (tenant_id) DO UPDATE SET
              cycle_name = EXCLUDED.cycle_name, days = EXCLUDED.days, periods = EXCLUDED.periods,
              classes = EXCLUDED.classes, subjects = EXCLUDED.subjects, teachers = EXCLUDED.teachers,
              rooms = EXCLUDED.rooms, lesson_requirements = EXCLUDED.lesson_requirements, updated_at = NOW()"#,
    )
    .bind(tenant_id)
    .bind(&value.cycle_name)
    .bind(serde_json::to_value(&value.days).map_err(actix_web::error::ErrorInternalServerError)?)
    .bind(serde_json::to_value(&value.periods).map_err(actix_web::error::ErrorInternalServerError)?)
    .bind(serde_json::to_value(&value.classes).map_err(actix_web::error::ErrorInternalServerError)?)
    .bind(serde_json::to_value(&value.subjects).map_err(actix_web::error::ErrorInternalServerError)?)
    .bind(serde_json::to_value(&value.teachers).map_err(actix_web::error::ErrorInternalServerError)?)
    .bind(serde_json::to_value(&value.rooms).map_err(actix_web::error::ErrorInternalServerError)?)
    .bind(serde_json::to_value(&value.lesson_requirements).map_err(actix_web::error::ErrorInternalServerError)?)
    .execute(pool.get_ref())
    .await
    .map_err(actix_web::error::ErrorInternalServerError)?;
    Ok(HttpResponse::Ok().json(ApiResponse::from_status(StatusCode::OK, Some(value), None)))
}

#[post("/generate")]
async fn generate_timetable(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
) -> Result<HttpResponse, actix_web::Error> {
    let tenant_id = tenant.into_inner().into_inner();
    let configuration = load_configuration(pool.get_ref(), tenant_id).await?;
    if configuration.lesson_requirements.is_empty() {
        return Ok(HttpResponse::BadRequest().json(ApiResponse::from_status(
            StatusCode::BAD_REQUEST,
            None::<()>,
            Some(vec![
                "Add at least one teaching requirement before generating".to_string(),
            ]),
        )));
    }
    if let Err(issues) = validate_configuration(&configuration) {
        return Ok(HttpResponse::BadRequest().json(ApiResponse::from_status(
            StatusCode::BAD_REQUEST,
            None::<()>,
            Some(issues),
        )));
    }
    let generated = generate(&configuration);
    let row = sqlx::query_as::<_, TimetableRunRow>(
        r#"INSERT INTO timetable_runs
              (tenant_id, configuration_snapshot, entries, unresolved, quality_score)
           VALUES ($1, $2, $3, $4, $5)
           RETURNING id, status, configuration_snapshot, entries, unresolved, quality_score, created_at, published_at"#,
    )
    .bind(tenant_id)
    .bind(serde_json::to_value(&configuration).map_err(actix_web::error::ErrorInternalServerError)?)
    .bind(serde_json::to_value(&generated.entries).map_err(actix_web::error::ErrorInternalServerError)?)
    .bind(serde_json::to_value(&generated.unresolved).map_err(actix_web::error::ErrorInternalServerError)?)
    .bind(generated.quality_score)
    .fetch_one(pool.get_ref())
    .await
    .map_err(actix_web::error::ErrorInternalServerError)?;
    let run = TimetableRun::try_from(row).map_err(actix_web::error::ErrorInternalServerError)?;
    Ok(HttpResponse::Created().json(ApiResponse::from_status(
        StatusCode::CREATED,
        Some(run),
        None,
    )))
}

#[get("/runs/latest")]
async fn latest_run(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
) -> Result<HttpResponse, actix_web::Error> {
    let row = sqlx::query_as::<_, TimetableRunRow>(
        r#"SELECT id, status, configuration_snapshot, entries, unresolved, quality_score, created_at, published_at
           FROM timetable_runs WHERE tenant_id = $1 ORDER BY created_at DESC LIMIT 1"#,
    )
    .bind(tenant.into_inner().into_inner())
    .fetch_optional(pool.get_ref())
    .await
    .map_err(actix_web::error::ErrorInternalServerError)?;
    let run = row
        .map(TimetableRun::try_from)
        .transpose()
        .map_err(actix_web::error::ErrorInternalServerError)?;
    Ok(HttpResponse::Ok().json(ApiResponse::from_status(StatusCode::OK, run, None)))
}

#[put("/runs/{id}/publish")]
async fn publish_run(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, actix_web::Error> {
    let tenant_id = tenant.into_inner().into_inner();
    let run_id = path.into_inner();
    let unresolved: Option<serde_json::Value> = sqlx::query_scalar(
        "SELECT unresolved FROM timetable_runs WHERE id = $1 AND tenant_id = $2",
    )
    .bind(run_id)
    .bind(tenant_id)
    .fetch_optional(pool.get_ref())
    .await
    .map_err(actix_web::error::ErrorInternalServerError)?;
    let Some(unresolved) = unresolved else {
        return Ok(HttpResponse::NotFound().json(ApiResponse::from_status(
            StatusCode::NOT_FOUND,
            None::<()>,
            Some(vec!["Timetable run not found".to_string()]),
        )));
    };
    if unresolved.as_array().is_some_and(|items| !items.is_empty()) {
        return Ok(HttpResponse::Conflict().json(ApiResponse::from_status(
            StatusCode::CONFLICT,
            None::<()>,
            Some(vec![
                "Resolve every unplaced lesson before publishing".to_string(),
            ]),
        )));
    }
    let mut transaction = pool
        .begin()
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?;
    sqlx::query("UPDATE timetable_runs SET status = 'superseded' WHERE tenant_id = $1 AND status = 'published'")
        .bind(tenant_id)
        .execute(&mut *transaction)
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?;
    let row = sqlx::query_as::<_, TimetableRunRow>(
        r#"UPDATE timetable_runs SET status = 'published', published_at = NOW()
           WHERE id = $1 AND tenant_id = $2
           RETURNING id, status, configuration_snapshot, entries, unresolved, quality_score, created_at, published_at"#,
    )
    .bind(run_id)
    .bind(tenant_id)
    .fetch_one(&mut *transaction)
    .await
    .map_err(actix_web::error::ErrorInternalServerError)?;
    transaction
        .commit()
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?;
    let run = TimetableRun::try_from(row).map_err(actix_web::error::ErrorInternalServerError)?;
    Ok(HttpResponse::Ok().json(ApiResponse::from_status(StatusCode::OK, Some(run), None)))
}

async fn load_configuration(
    pool: &PgPool,
    tenant_id: Uuid,
) -> Result<TimetableConfiguration, actix_web::Error> {
    let row: Option<serde_json::Value> = sqlx::query_scalar(
        r#"SELECT jsonb_build_object(
               'cycle_name', cycle_name, 'days', days, 'periods', periods, 'classes', classes,
               'subjects', subjects, 'teachers', teachers, 'rooms', rooms,
               'lesson_requirements', lesson_requirements)
           FROM timetable_configurations WHERE tenant_id = $1"#,
    )
    .bind(tenant_id)
    .fetch_optional(pool)
    .await
    .map_err(actix_web::error::ErrorInternalServerError)?;
    match row {
        Some(value) => {
            serde_json::from_value(value).map_err(actix_web::error::ErrorInternalServerError)
        }
        None => Ok(TimetableConfiguration::default()),
    }
}

pub fn routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("")
            .wrap(RequirePermission::new("timetabling"))
            .service(get_configuration)
            .service(save_configuration)
            .service(generate_timetable)
            .service(latest_run)
            .service(publish_run),
    );
}
