//
//  campus-pilot-apis
//  ops.rs
//
//  Created by Ngonidzashe Mangudya on 2025/10/01.
//  Copyright (c) 2025 Codecraft Solutions. All rights reserved.
//

use anyhow::{Context, Result};
use aws_sdk_s3::{Client as S3Client, presigning::PresigningConfig};
use std::time::Duration;

pub struct StorageOps {
    client: S3Client,
    bucket: String,
}

impl StorageOps {
    pub fn new(client: S3Client, bucket: String) -> Self {
        Self { client, bucket }
    }

    /// Generate a presigned URL for uploading a file
    /// Note: Files are uploaded with public-read ACL to allow subsequent access
    pub async fn generate_upload_url(&self, key: &str, expires_in_secs: u64) -> Result<String> {
        let presigning_config = PresigningConfig::builder()
            .expires_in(Duration::from_secs(expires_in_secs))
            .build()
            .context("Failed to build presigning config")?;

        let presigned_request = self
            .client
            .put_object()
            .bucket(&self.bucket)
            .key(key)
            .acl(aws_sdk_s3::types::ObjectCannedAcl::PublicRead) // Make uploaded objects publicly readable
            .presigned(presigning_config)
            .await
            .context("Failed to generate presigned URL")?;

        Ok(presigned_request.uri().to_string())
    }

    /// Generate a presigned URL for downloading/viewing a file
    pub async fn generate_download_url(&self, key: &str, expires_in_secs: u64) -> Result<String> {
        let presigning_config = PresigningConfig::builder()
            .expires_in(Duration::from_secs(expires_in_secs))
            .build()
            .context("Failed to build presigning config")?;

        let presigned_request = self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(key)
            .presigned(presigning_config)
            .await
            .context("Failed to generate presigned download URL")?;

        Ok(presigned_request.uri().to_string())
    }

    /// Delete a file from storage
    pub async fn delete_file(&self, key: &str) -> Result<()> {
        self.client
            .delete_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await
            .context("Failed to delete file from storage")?;

        Ok(())
    }

    /// Check if a file exists
    pub async fn file_exists(&self, key: &str) -> Result<bool> {
        match self
            .client
            .head_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await
        {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }
}
