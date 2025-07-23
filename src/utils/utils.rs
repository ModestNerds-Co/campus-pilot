//
//  campus-pilot-apis
//  utils.rs
//
//  Created by Ngonidzashe Mangudya on 2025/06/22.
//  Copyright (c) 2025 Codecraft Solutions. All rights reserved.
//

use std::env;

use crate::models::status_info::StatusInfo;
use crate::models::typedefs::ApiResult;
use crate::models::{Order, TurnstileResponse};

use actix_web::http::StatusCode;
use anyhow;
use base64::{engine::general_purpose, Engine as _};
use lettre::message::{header::ContentType, Attachment, Mailbox, SinglePart};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{Message, SmtpTransport, Transport};
use reqwest::Client;
use std::collections::HashMap;
use std::fs;

pub fn status_meaning(code: StatusCode) -> StatusInfo {
    match code {
        StatusCode::OK => StatusInfo {
            meaning: "OK",
            description: "Request was executed successfully",
        },
        StatusCode::CREATED => StatusInfo {
            meaning: "Created",
            description: "Resource was created successfully",
        },
        StatusCode::ACCEPTED => StatusInfo {
            meaning: "Accepted",
            description: "Request was accepted but not yet executed",
        },
        StatusCode::NO_CONTENT => StatusInfo {
            meaning: "No Content",
            description: "No content was returned",
        },
        StatusCode::BAD_REQUEST => StatusInfo {
            meaning: "Bad Request",
            description: "Request was malformed",
        },
        StatusCode::UNAUTHORIZED => StatusInfo {
            meaning: "Unauthorized",
            description: "Request was not authorized",
        },
        StatusCode::FORBIDDEN => StatusInfo {
            meaning: "Forbidden",
            description: "Request was forbidden",
        },
        StatusCode::NOT_FOUND => StatusInfo {
            meaning: "Not Found",
            description: "Resource was not found",
        },
        StatusCode::METHOD_NOT_ALLOWED => StatusInfo {
            meaning: "Method Not Allowed",
            description: "Method was not allowed",
        },
        StatusCode::CONFLICT => StatusInfo {
            meaning: "Conflict",
            description: "Request was not executed due to conflict",
        },
        StatusCode::INTERNAL_SERVER_ERROR => StatusInfo {
            meaning: "Server Error",
            description: "Server was unable to execute request",
        },
        _ => StatusInfo {
            meaning: "Unknown Status",
            description: "No mapped description available",
        },
    }
}

pub async fn verify_turnstile(capture_token: &str, turnstile_secret: &str) -> ApiResult<bool> {
    let client = Client::new();
    let token = &String::from(capture_token);
    let params = vec![("secret", turnstile_secret), ("response", token)];

    let res = client
        .post("https://challenges.cloudflare.com/turnstile/v0/siteverify")
        .form(&params)
        .send()
        .await?
        .json::<TurnstileResponse>()
        .await?;

    tracing::info!("{}", res.to_string());

    Ok(res.success)
}

pub async fn send_email(
    to_email: String,
    subject: String,
    message_body: String,
    to_full_name: Option<String>,
    email_config: &crate::config::EmailConfig,
) -> ApiResult<bool> {
    send_email_with_attachments(
        to_email,
        subject,
        message_body,
        to_full_name,
        email_config,
        None,
    )
    .await
}

pub async fn send_email_with_attachments(
    to_email: String,
    subject: String,
    message_body: String,
    to_full_name: Option<String>,
    email_config: &crate::config::EmailConfig,
    attachments: Option<Vec<AttachmentFile>>,
) -> ApiResult<bool> {
    let from_email = env::var("EMAIL_USER").unwrap_or_else(|_| "".to_string());

    let email_builder = Message::builder()
        .from(Mailbox::new(
            Some(String::from("Notifications")),
            from_email.parse()?,
        ))
        .reply_to(Mailbox::new(
            Some(String::from("Notifications")),
            from_email.parse()?,
        ))
        .to(Mailbox::new(to_full_name, to_email.parse()?))
        .subject(subject);

    let email = if let Some(attachments) = attachments {
        let mut multipart = lettre::message::MultiPart::mixed().singlepart(
            SinglePart::builder()
                .header(ContentType::parse("text/html; charset=utf8").unwrap())
                .body(message_body),
        );

        for attachment in attachments {
            match general_purpose::STANDARD.decode(&attachment.data) {
                Ok(decoded_data) => {
                    let attachment_part = Attachment::new(attachment.filename).body(
                        decoded_data,
                        ContentType::parse(&attachment.mime_type)
                            .unwrap_or(ContentType::TEXT_PLAIN),
                    );
                    multipart = multipart.singlepart(attachment_part);
                }
                Err(e) => {
                    tracing::error!("Failed to decode base64 attachment: {}", e);
                    continue;
                }
            }
        }

        email_builder.multipart(multipart)?
    } else {
        email_builder
            .header(ContentType::parse("text/html; charset=utf8").unwrap())
            .body(message_body)?
    };

    let mailer = create_mailer(email_config)?;

    match mailer.send(&email) {
        std::result::Result::Ok(_) => {
            tracing::info!("Email sent successfully to {}", to_email);
            Ok(true)
        }
        std::result::Result::Err(e) => {
            tracing::error!("Failed to send email to {}: {:?}", to_email, e);
            Ok(false)
        }
    }
}

fn create_mailer(email_config: &crate::config::EmailConfig) -> ApiResult<SmtpTransport> {
    let creds = Credentials::new(email_config.user.clone(), email_config.password.clone());

    let mailer = SmtpTransport::relay(&email_config.host)?
        .port(email_config.port)
        .credentials(creds)
        .build();

    Ok(mailer)
}

pub fn load_email_template(template_name: &str) -> ApiResult<String> {
    let template_path = format!("emails/{}.html", template_name);
    match fs::read_to_string(&template_path) {
        Ok(content) => Ok(content),
        Err(e) => {
            tracing::error!("Failed to load email template {}: {}", template_path, e);
            Err(anyhow::anyhow!("Failed to load email template: {}", e))
        }
    }
}

pub fn render_email_template(template: &str, variables: HashMap<String, String>) -> String {
    let mut rendered = template.to_string();

    for (key, value) in variables {
        let placeholder = format!("{{{{{}}}}}", key);
        rendered = rendered.replace(&placeholder, &value);
    }

    rendered
}
