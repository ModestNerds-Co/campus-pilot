//! Private object storage and malware scanning boundary for official documents.

use std::time::Duration;

use anyhow::{Context, Result, bail};
use aws_sdk_s3::{Client as S3Client, presigning::PresigningConfig};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    time::timeout,
};
use uuid::Uuid;

pub const MAX_DOCUMENT_BYTES: usize = 15 * 1024 * 1024;

#[derive(Clone)]
pub struct DocumentStorage {
    client: S3Client,
    presign_client: Option<S3Client>,
    bucket: String,
    scanner_address: String,
}

impl DocumentStorage {
    #[must_use]
    pub fn new(
        client: S3Client,
        presign_client: Option<S3Client>,
        bucket: String,
        scanner_address: String,
    ) -> Self {
        Self {
            client,
            presign_client,
            bucket,
            scanner_address,
        }
    }

    pub async fn ensure_ready(&self) -> Result<()> {
        if self
            .client
            .head_bucket()
            .bucket(&self.bucket)
            .send()
            .await
            .is_err()
        {
            self.client
                .create_bucket()
                .bucket(&self.bucket)
                .send()
                .await
                .context("create the private Document Registry bucket")?;
        }
        // A private bucket must never inherit the public policy used by generic assets.
        let _ = self
            .client
            .delete_bucket_policy()
            .bucket(&self.bucket)
            .send()
            .await;
        let response = self
            .scanner_command(b"zPING\0", Duration::from_secs(5))
            .await
            .context("reach the Document Registry malware scanner")?;
        if response.trim_end_matches('\0').trim() != "PONG" {
            bail!("Document Registry malware scanner did not become ready");
        }
        Ok(())
    }

    pub async fn scan_and_store(
        &self,
        tenant_id: Uuid,
        file_id: Uuid,
        bytes: &[u8],
        media_type: &str,
    ) -> Result<String> {
        validate_document_bytes(bytes, media_type)?;
        self.scan(bytes).await?;
        let extension = match media_type {
            "application/pdf" => "pdf",
            "image/jpeg" => "jpg",
            "image/png" => "png",
            _ => unreachable!("media type was validated"),
        };
        let key = format!(
            "tenants/{tenant_id}/document-registry/{file_id}/{}.{}",
            Uuid::new_v4(),
            extension
        );
        self.client
            .put_object()
            .bucket(&self.bucket)
            .key(&key)
            .body(bytes.to_vec().into())
            .content_type(media_type)
            .send()
            .await
            .context("store the scanned private document")?;
        Ok(key)
    }

    pub async fn download_url(&self, key: &str, expires_in_seconds: u64) -> Result<String> {
        let config = PresigningConfig::builder()
            .expires_in(Duration::from_secs(expires_in_seconds))
            .build()
            .context("build the private download expiry")?;
        let client = self.presign_client.as_ref().unwrap_or(&self.client);
        let request = client
            .get_object()
            .bucket(&self.bucket)
            .key(key)
            .response_content_disposition("attachment")
            .presigned(config)
            .await
            .context("create the private document download")?;
        Ok(request.uri().to_string())
    }

    pub async fn delete(&self, key: &str) -> Result<()> {
        self.client
            .delete_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await
            .context("destroy the private document object")?;
        Ok(())
    }

    async fn scan(&self, bytes: &[u8]) -> Result<()> {
        let mut stream = timeout(
            Duration::from_secs(5),
            TcpStream::connect(&self.scanner_address),
        )
        .await
        .context("malware scanner connection timed out")?
        .context("connect to the malware scanner")?;
        timeout(Duration::from_secs(45), async {
            stream.write_all(b"zINSTREAM\0").await?;
            for chunk in bytes.chunks(64 * 1024) {
                stream
                    .write_all(&(chunk.len() as u32).to_be_bytes())
                    .await?;
                stream.write_all(chunk).await?;
            }
            stream.write_all(&0_u32.to_be_bytes()).await?;
            stream.flush().await?;
            let mut response = Vec::with_capacity(128);
            loop {
                let mut byte = [0_u8; 1];
                let count = stream.read(&mut byte).await?;
                if count == 0 || byte[0] == 0 || response.len() >= 4096 {
                    break;
                }
                response.push(byte[0]);
            }
            Ok::<_, std::io::Error>(String::from_utf8_lossy(&response).into_owned())
        })
        .await
        .context("malware scan timed out")?
        .context("complete the malware scan")
        .and_then(|response| {
            if response.ends_with(" OK") {
                Ok(())
            } else if response.contains(" FOUND") {
                bail!("The document failed the security scan")
            } else {
                bail!("The document security scan could not be completed")
            }
        })
    }

    async fn scanner_command(&self, command: &[u8], limit: Duration) -> Result<String> {
        let mut stream = timeout(
            Duration::from_secs(5),
            TcpStream::connect(&self.scanner_address),
        )
        .await
        .context("scanner connection timed out")??;
        timeout(limit, async {
            stream.write_all(command).await?;
            stream.flush().await?;
            let mut response = [0_u8; 64];
            let count = stream.read(&mut response).await?;
            Ok::<_, std::io::Error>(String::from_utf8_lossy(&response[..count]).into_owned())
        })
        .await
        .context("scanner command timed out")?
        .context("read scanner response")
    }
}

fn validate_document_bytes(bytes: &[u8], media_type: &str) -> Result<()> {
    if bytes.is_empty() {
        bail!("Choose a document to upload");
    }
    if bytes.len() > MAX_DOCUMENT_BYTES {
        bail!("The document exceeds the 15 MB limit");
    }
    let valid = match media_type {
        "application/pdf" => bytes.starts_with(b"%PDF-"),
        "image/jpeg" => bytes.starts_with(&[0xff, 0xd8, 0xff]),
        "image/png" => bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]),
        _ => bail!("Upload a PDF, JPEG, or PNG document"),
    };
    if !valid {
        bail!("The document contents do not match its file type");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::validate_document_bytes;

    #[test]
    fn validates_supported_magic_bytes() {
        assert!(validate_document_bytes(b"%PDF-1.7", "application/pdf").is_ok());
        assert!(validate_document_bytes(&[0xff, 0xd8, 0xff, 0xdb], "image/jpeg").is_ok());
        assert!(
            validate_document_bytes(&[0x89, b'P', b'N', b'G', 13, 10, 26, 10], "image/png").is_ok()
        );
        assert!(validate_document_bytes(b"not a pdf", "application/pdf").is_err());
        assert!(validate_document_bytes(b"text", "text/plain").is_err());
    }
}
