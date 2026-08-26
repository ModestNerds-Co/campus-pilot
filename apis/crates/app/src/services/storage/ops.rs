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
    presign_client: Option<S3Client>,
    bucket: String,
    endpoint: String,
    public_endpoint: Option<String>,
}

impl StorageOps {
    pub fn new(
        client: S3Client,
        presign_client: Option<S3Client>,
        bucket: String,
        endpoint: String,
        public_endpoint: Option<String>,
    ) -> Self {
        Self {
            client,
            presign_client,
            bucket,
            endpoint,
            public_endpoint,
        }
    }

    /// Ensure the bucket exists and has public read policy
    pub async fn ensure_bucket_setup(&self) -> Result<()> {
        // Check if bucket exists
        match self.client.head_bucket().bucket(&self.bucket).send().await {
            Ok(_) => {
                // Bucket exists, set policy
                self.set_public_read_policy().await?;
            }
            Err(_) => {
                // Create bucket
                self.client
                    .create_bucket()
                    .bucket(&self.bucket)
                    .send()
                    .await
                    .context("Failed to create bucket")?;

                // Set policy on new bucket
                self.set_public_read_policy().await?;
            }
        }

        Ok(())
    }

    /// Set bucket policy to allow public read access
    async fn set_public_read_policy(&self) -> Result<()> {
        let policy = serde_json::json!({
            "Version": "2012-10-17",
            "Statement": [
                {
                    "Effect": "Allow",
                    "Principal": {"AWS": ["*"]},
                    "Action": ["s3:GetObject"],
                    "Resource": [format!("arn:aws:s3:::{}/*", self.bucket)]
                }
            ]
        });

        self.client
            .put_bucket_policy()
            .bucket(&self.bucket)
            .policy(policy.to_string())
            .send()
            .await
            .context("Failed to set bucket policy")?;

        Ok(())
    }

    /// Upload a file directly to storage and return the public URL
    pub async fn upload_file(&self, key: &str, data: &[u8], content_type: &str) -> Result<String> {
        self.client
            .put_object()
            .bucket(&self.bucket)
            .key(key)
            .body(data.to_vec().into())
            .content_type(content_type)
            .send()
            .await
            .context("Failed to upload file to storage")?;

        // Return the public URL (bucket is publicly readable)
        let public_url = format!("{}/{}/{}", self.endpoint, self.bucket, key);

        Ok(public_url)
    }

    /// Generate a presigned URL for uploading a file
    /// Note: Files are publicly accessible via bucket policy
    pub async fn generate_upload_url(&self, key: &str, expires_in_secs: u64) -> Result<String> {
        let presigning_config = PresigningConfig::builder()
            .expires_in(Duration::from_secs(expires_in_secs))
            .build()
            .context("Failed to build presigning config")?;

        // Use presign_client with public endpoint if available, so SigV4 is calculated with the public host
        let client = self.presign_client.as_ref().unwrap_or(&self.client);
        let presigned_request = client
            .put_object()
            .bucket(&self.bucket)
            .key(key)
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
