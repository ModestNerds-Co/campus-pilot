//! Fails closed unless another process has a fresh durable worker heartbeat.

use sqlx::PgPool;

#[derive(Debug, Clone)]
pub struct AgentSubmissionGate {
    pool: PgPool,
}

impl AgentSubmissionGate {
    #[must_use]
    pub const fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Uses database time and the immutable startup proof recorded by migration 091.
    pub async fn is_ready(&self) -> Result<bool, sqlx::Error> {
        sqlx::query_scalar::<_, bool>("SELECT agent_has_ready_worker()")
            .fetch_one(&self.pool)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::AgentSubmissionGate;

    #[tokio::test]
    async fn gate_construction_is_side_effect_free() {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .connect_lazy("postgresql://unused:unused@127.0.0.1:1/unused")
            .unwrap_or_else(|_| unreachable!());
        let _gate = AgentSubmissionGate::new(pool);
    }
}
