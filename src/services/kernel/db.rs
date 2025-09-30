use sqlx::PgPool;

use crate::{models::typedefs::ApiResult, services::kernel::models::KernelStatus};

pub struct KernelDbOps {
    pool: PgPool,
}

impl KernelDbOps {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn get_kernel_status(&self) -> ApiResult<KernelStatus> {
        let state: String =
            sqlx::query_scalar("SELECT state::TEXT FROM system_state WHERE id='singleton'")
                .fetch_one(&self.pool)
                .await
                .unwrap_or_else(|_| "Uninitialized".to_string());

        Ok(KernelStatus { state })
    }
}
