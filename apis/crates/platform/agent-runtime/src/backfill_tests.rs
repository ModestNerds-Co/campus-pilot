//! Verifies the deliberately narrow existing-tenant AI administration backfill.

use chrono::{DateTime, Utc};
use sqlx::postgres::PgPoolOptions;
use uuid::Uuid;

const AI_ADMINISTRATION_BACKFILL: &str =
    include_str!("../../../../migrations/084_backfill_ai_administration_permissions.sql");

const EXPECTED_AI_ADMINISTRATION_PERMISSIONS: [&str; 4] = [
    "ai_providers:edit",
    "ai_providers:view",
    "ai_routing:edit",
    "ai_routing:view",
];

#[test]
fn migration_is_limited_to_current_seeded_school_administrators() {
    assert!(AI_ADMINISTRATION_BACKFILL.contains("role.key = 'school_administrator'"));
    assert!(AI_ADMINISTRATION_BACKFILL.contains("role.is_system = TRUE"));
    assert!(AI_ADMINISTRATION_BACKFILL.contains("role.deleted_at IS NULL"));
    assert!(AI_ADMINISTRATION_BACKFILL.contains("SELECT DISTINCT permission"));
    assert!(AI_ADMINISTRATION_BACKFILL.contains("ORDER BY permission"));
    assert!(AI_ADMINISTRATION_BACKFILL.contains("IS DISTINCT FROM"));

    for permission in EXPECTED_AI_ADMINISTRATION_PERMISSIONS {
        assert!(AI_ADMINISTRATION_BACKFILL.contains(permission));
    }
    for forbidden_permission in [
        "agent:view",
        "agent:run",
        "agent:history",
        "agent:share",
        "agent:approve",
        "agent_policy",
        "agent_usage",
        "agent_limits",
        "agent_audit",
    ] {
        assert!(!AI_ADMINISTRATION_BACKFILL.contains(forbidden_permission));
    }
    for wildcard_grant in ["'*'", ":*"] {
        assert!(!AI_ADMINISTRATION_BACKFILL.contains(wildcard_grant));
    }
    for forbidden_relation in [
        "tenant_modules",
        "module_license_activations",
        "tenant_entitlements",
        "tenant_entitlement_features",
        "entitlement_limits",
        "entitlement_meter_buckets",
        "entitlement_usage_reservations",
        "entitlement_usage_events",
        "license_leases",
        "user_role_assignments",
        "role_assignments",
        "ai_provider_connections",
        "ai_provider_models",
        "ai_route_sets",
        "ai_task_routes",
    ] {
        assert!(!AI_ADMINISTRATION_BACKFILL.contains(forbidden_relation));
    }
    for forbidden_user_mutation in ["INSERT INTO users", "UPDATE users", "DELETE FROM users"] {
        assert!(!AI_ADMINISTRATION_BACKFILL.contains(forbidden_user_mutation));
    }
    assert!(!AI_ADMINISTRATION_BACKFILL.contains("CREATE OR REPLACE FUNCTION"));
    assert!(!AI_ADMINISTRATION_BACKFILL.contains("CREATE TRIGGER"));
    assert!(!AI_ADMINISTRATION_BACKFILL.contains("DROP TRIGGER"));
}

#[tokio::test]
#[ignore = "requires a disposable migrated AI_ADMIN_BACKFILL_TEST_DATABASE_URL"]
async fn postgres_backfill_is_narrow_sorted_and_replay_safe() {
    let database_url = std::env::var("AI_ADMIN_BACKFILL_TEST_DATABASE_URL")
        .expect("AI_ADMIN_BACKFILL_TEST_DATABASE_URL must target a disposable database");
    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(&database_url)
        .await
        .expect("AI administration backfill contract database must connect");

    let tenant_id = Uuid::new_v4();
    let tenant_suffix = tenant_id.simple();
    sqlx::query("INSERT INTO tenants (id, slug, name) VALUES ($1, $2, $3)")
        .bind(tenant_id)
        .bind(format!("ai-admin-backfill-{tenant_suffix}"))
        .bind("AI administration backfill contract")
        .execute(&pool)
        .await
        .expect("contract tenant must insert");

    // Model a campus created before migrations 078 and 083. The late triggers
    // add permissions only on tenant creation, so its administrator lacks them.
    sqlx::query(
        r#"
        UPDATE roles
        SET permissions = ARRAY['users:view', 'administration:view', 'users:view']::TEXT[],
            updated_at = NOW() - INTERVAL '1 day'
        WHERE tenant_id = $1
          AND key = 'school_administrator'
          AND is_system = TRUE
          AND deleted_at IS NULL
        "#,
    )
    .bind(tenant_id)
    .execute(&pool)
    .await
    .expect("existing School Administrator fixture must reset");

    let custom_role_id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO roles (
            id, tenant_id, key, name, description, permissions, is_system
        )
        VALUES ($1, $2, 'custom_operator', $3, 'Contract custom role',
                ARRAY['users:view', 'reports:view']::TEXT[], FALSE)
        "#,
    )
    .bind(custom_role_id)
    .bind(tenant_id)
    .bind(format!("Custom operator {tenant_suffix}"))
    .execute(&pool)
    .await
    .expect("custom role fixture must insert");

    let owner_before = role_permissions(&pool, tenant_id, "campus_owner").await;
    let custom_before = role_permissions_by_id(&pool, custom_role_id).await;

    sqlx::raw_sql(AI_ADMINISTRATION_BACKFILL)
        .execute(&pool)
        .await
        .expect("migration 084 must apply");

    let administrator_after = role_permissions(&pool, tenant_id, "school_administrator").await;
    assert_eq!(
        administrator_after,
        vec![
            "administration:view",
            "ai_providers:edit",
            "ai_providers:view",
            "ai_routing:edit",
            "ai_routing:view",
            "users:view",
        ]
    );
    for forbidden_permission in ["agent:view", "agent:run", "agent:history"] {
        assert!(
            !administrator_after
                .iter()
                .any(|value| value == forbidden_permission)
        );
    }

    assert_eq!(
        role_permissions(&pool, tenant_id, "campus_owner").await,
        owner_before
    );
    assert_eq!(
        role_permissions_by_id(&pool, custom_role_id).await,
        custom_before
    );
    let agent_module_count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM tenant_modules WHERE tenant_id = $1 AND module_key = 'agent'",
    )
    .bind(tenant_id)
    .fetch_one(&pool)
    .await
    .expect("Agent module absence must be queryable");
    assert_eq!(agent_module_count, 0);

    let provider_trigger_exists = trigger_exists(
        &pool,
        "tenants",
        "zz_grant_new_tenant_ai_provider_permissions",
    )
    .await;
    let routing_trigger_exists = trigger_exists(
        &pool,
        "tenants",
        "zz_grant_new_tenant_ai_routing_permissions",
    )
    .await;
    assert!(provider_trigger_exists);
    assert!(routing_trigger_exists);

    let updated_after_first = role_updated_at(&pool, tenant_id, "school_administrator").await;
    sqlx::raw_sql(AI_ADMINISTRATION_BACKFILL)
        .execute(&pool)
        .await
        .expect("migration 084 must replay");
    assert_eq!(
        role_permissions(&pool, tenant_id, "school_administrator").await,
        administrator_after
    );
    assert_eq!(
        role_updated_at(&pool, tenant_id, "school_administrator").await,
        updated_after_first
    );
    assert_eq!(
        role_permissions(&pool, tenant_id, "campus_owner").await,
        owner_before
    );
    assert_eq!(
        role_permissions_by_id(&pool, custom_role_id).await,
        custom_before
    );
}

async fn role_permissions(pool: &sqlx::PgPool, tenant_id: Uuid, key: &str) -> Vec<String> {
    sqlx::query_scalar::<_, Vec<String>>(
        "SELECT permissions FROM roles WHERE tenant_id = $1 AND key = $2 AND deleted_at IS NULL",
    )
    .bind(tenant_id)
    .bind(key)
    .fetch_one(pool)
    .await
    .expect("fixture role permissions must be queryable")
}

async fn role_permissions_by_id(pool: &sqlx::PgPool, role_id: Uuid) -> Vec<String> {
    sqlx::query_scalar::<_, Vec<String>>("SELECT permissions FROM roles WHERE id = $1")
        .bind(role_id)
        .fetch_one(pool)
        .await
        .expect("fixture custom role permissions must be queryable")
}

async fn role_updated_at(pool: &sqlx::PgPool, tenant_id: Uuid, key: &str) -> DateTime<Utc> {
    sqlx::query_scalar::<_, DateTime<Utc>>(
        "SELECT updated_at FROM roles WHERE tenant_id = $1 AND key = $2 AND deleted_at IS NULL",
    )
    .bind(tenant_id)
    .bind(key)
    .fetch_one(pool)
    .await
    .expect("fixture role timestamp must be queryable")
}

async fn trigger_exists(pool: &sqlx::PgPool, table_name: &str, trigger_name: &str) -> bool {
    sqlx::query_scalar::<_, bool>(
        r#"
        SELECT EXISTS (
            SELECT 1
            FROM pg_trigger AS trigger
            JOIN pg_class AS relation ON relation.oid = trigger.tgrelid
            JOIN pg_namespace AS schema ON schema.oid = relation.relnamespace
            WHERE schema.nspname = 'public'
              AND relation.relname = $1
              AND trigger.tgname = $2
              AND NOT trigger.tgisinternal
        )
        "#,
    )
    .bind(table_name)
    .bind(trigger_name)
    .fetch_one(pool)
    .await
    .expect("fixture trigger must be queryable")
}
