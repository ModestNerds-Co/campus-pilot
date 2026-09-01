//
//  campus-pilot-apis
//  state.rs
//
//  Created by Ngonidzashe Mangudya on 2025/06/22.
//  Copyright (c) 2025 Codecraft Solutions. All rights reserved.
//

use aws_credential_types::Credentials;
use aws_sdk_s3::{Client as S3Client, config::Region};
use cp_agent::{
    AuthorityLoader, CapabilityBroker, CapabilityRegistry, PostgresBrokerAuditSink,
    RecordScopeAuthorizer,
};
use cp_agent_runtime::{AgentSessionOps, AgentUsageRuntime, AiRoutingOps};
use cp_ai_providers::AiProviderOps;
use cp_document_registry::DocumentStorage;
use sqlx::PgPool;

use crate::config::Config;
use crate::db::DatabaseOperations;
use crate::services::agent::{
    AgentSubmissionGate, AgentWorkerReadinessOps, AppAuthorityLoader, AppRecordScopeAuthorizer,
    build_capability_registry,
};
use crate::services::kernel::db::KernelDbOps;
use crate::services::storage::ops::StorageOps;

use std::sync::Arc;

pub struct AppState {
    pub db: PgPool,
    pub db_ops: Arc<DatabaseOperations>,
    pub kernel_db: Arc<KernelDbOps>,
    pub storage_ops: Arc<StorageOps>,
    pub document_storage: Arc<DocumentStorage>,
    pub agent_capabilities: Arc<CapabilityRegistry>,
    pub agent_authority_loader: Arc<dyn AuthorityLoader>,
    pub agent_record_scope_authorizer: Arc<dyn RecordScopeAuthorizer>,
    pub agent_session_ops: Arc<AgentSessionOps>,
    pub agent_usage_runtime: Arc<AgentUsageRuntime>,
    pub agent_worker_readiness: Arc<AgentWorkerReadinessOps>,
    pub agent_capability_broker: Arc<CapabilityBroker>,
    pub agent_submission_gate: AgentSubmissionGate,
    pub ai_provider_ops: Arc<AiProviderOps>,
    pub ai_routing_ops: Arc<AiRoutingOps>,
    pub config: Arc<Config>,
}

impl AppState {
    pub fn init(pool: PgPool, config: Config) -> Self {
        let config_arc = Arc::new(config.clone());
        let db_ops = Arc::new(DatabaseOperations::new(pool.clone()));
        let kernel_db = Arc::new(KernelDbOps::new(pool.clone()));
        let agent_capabilities = Arc::new(build_capability_registry(
            pool.clone(),
            config.license.clone(),
        ));
        let agent_authority_loader = Arc::new(AppAuthorityLoader::new(pool.clone()));
        let agent_record_scope_authorizer = Arc::new(AppRecordScopeAuthorizer::new(pool.clone()));
        let agent_session_ops = Arc::new(AgentSessionOps::new(pool.clone()));
        let agent_usage_runtime = Arc::new(AgentUsageRuntime::new(pool.clone()));
        let agent_worker_readiness = Arc::new(AgentWorkerReadinessOps::new(pool.clone()));
        let agent_capability_broker = Arc::new(CapabilityBroker::new(
            build_capability_registry(pool.clone(), config.license.clone()),
            agent_authority_loader.clone(),
            agent_record_scope_authorizer.clone(),
            agent_usage_runtime.prepared_capability_call_verifier(),
            Arc::new(PostgresBrokerAuditSink::new(pool.clone())),
        ));
        let agent_submission_gate = AgentSubmissionGate::new(pool.clone());
        let ai_provider_ops = Arc::new(AiProviderOps::new(
            pool.clone(),
            config.ai_providers.credential_keyring.clone(),
            config.ai_providers.http_client.clone(),
        ));
        let ai_routing_ops = Arc::new(AiRoutingOps::new(pool.clone()));

        // Initialize MinIO/S3 clients - one for internal ops, one for presigned URLs with public host
        let credentials = Credentials::new(
            &config.storage.access_key,
            &config.storage.secret_key,
            None,
            None,
            "static",
        );
        let s3_config = aws_sdk_s3::config::Builder::new()
            .endpoint_url(&config.storage.endpoint)
            .region(Region::new(config.storage.region.clone()))
            .credentials_provider(credentials.clone())
            .force_path_style(true)
            .behavior_version(aws_sdk_s3::config::BehaviorVersion::latest())
            .build();
        let s3_client = S3Client::from_conf(s3_config);

        // Separate client for presigned URLs that signs with the public host (so SigV4 matches browser request)
        let presign_client = if let Some(public_endpoint) = &config.storage.public_endpoint {
            let presign_config = aws_sdk_s3::config::Builder::new()
                .endpoint_url(public_endpoint)
                .region(Region::new(config.storage.region.clone()))
                .credentials_provider(credentials)
                .force_path_style(true)
                .behavior_version(aws_sdk_s3::config::BehaviorVersion::latest())
                .build();
            Some(S3Client::from_conf(presign_config))
        } else {
            None
        };

        let storage_ops = Arc::new(StorageOps::new(
            s3_client.clone(),
            presign_client.clone(),
            config.storage.bucket.clone(),
            config.storage.endpoint.clone(),
        ));
        let document_storage = Arc::new(DocumentStorage::new(
            s3_client,
            presign_client,
            config.storage.private_bucket.clone(),
            config.storage.document_scanner_address.clone(),
        ));

        Self {
            db: pool,
            db_ops,
            kernel_db,
            storage_ops,
            document_storage,
            agent_capabilities,
            agent_authority_loader,
            agent_record_scope_authorizer,
            agent_session_ops,
            agent_usage_runtime,
            agent_worker_readiness,
            agent_capability_broker,
            agent_submission_gate,
            ai_provider_ops,
            ai_routing_ops,
            config: config_arc,
        }
    }
}
