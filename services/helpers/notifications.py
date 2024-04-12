#
#  notifications.py
#  campus-pilot
#
#  Created by Ngonidzashe Mangudya on 12/04/2024.
#  Copyright (c) 2024 Codecraft Solutions. All rights reserved.


import smtplib
from email import encoders
from email.header import Header
from email.mime.base import MIMEBase
from email.mime.multipart import MIMEMultipart
from email.mime.text import MIMEText

from decouple import config
from loguru import logger
from rq import Retry
from rq.decorators import job

from users.models import User

system_host = config("EMAIL_HOST")
system_email = config("EMAIL_ADDRESS")
system_port = config("EMAIL_PORT")
system_password = config("EMAIL_PASSWORD")


@job("notifications", retry=Retry(max=3))
def send_email(
    user: User,
    html_content: str,
    email_subject: str = "Campus Pilot",
    override: bool = False,
):
    if user.receive_email_notifications is False and not override:
        logger.error("ser has disabled receiving email notifications")
        logger.error("Skipping process")
        return

    email_to_use = user.email
    if email_subject == "Email Verification":
        if user.pending_new_email is not None:
            logger.info("Using the specified new email for verification")
            email_to_use = user.pending_new_email

    logger.info(f"Sending email to {email_to_use}")

    server = smtplib.SMTP_SSL(host=system_host, port=system_port)

    email_message = MIMEMultipart()
    email_message["From"] = str(Header(f"Campus Pilot <{system_email}>"))
    email_message["To"] = email_to_use
    email_message["Subject"] = email_subject

    email_message.attach(MIMEText(html_content, "html"))
    email_content = email_message.as_string()

    try:
        logger.info("Authenticating with smtp server.")
        server.login(system_email, system_password)
        logger.info("Authenticated ✅. Sending email.")
        server.sendmail(config("EMAIL_ADDRESS"), email_to_use, email_content)
        logger.info("Sending email completed ✅.")
    except Exception as exc:
        logger.error(exc)
        raise


@job("notifications", retry=Retry(max=3))
def send_email_with_attachment(
    user: User,
    html_content: str,
    email_subject: str = "Campus Pilot",
    attachment: str = None,
    override: bool = False,
):
    if attachment is None:
        logger.error("[Send Email]: Attachment is not available. Sending as is")
        send_email(user, html_content, email_subject, override)
        return

    if user.receive_email_notifications is False and not override:
        logger.error("[Send Email]: User has disabled receiving email notifications")
        logger.error("[Send Email]: Skipping process")
        return

    email_to_use = user.email
    if email_subject == "Email Verification":
        if user.pending_new_email is not None:
            email_to_use = user.pending_new_email

    logger.info(f"Sending email to {email_to_use}")

    server = smtplib.SMTP_SSL(host=system_host, port=system_port)

    email_message = MIMEMultipart()
    email_message["From"] = str(Header(f"Campus Pilot <{system_email}>"))
    email_message["To"] = email_to_use
    email_message["Subject"] = email_subject

    with open(attachment, "rb") as attachment_file:
        attachment_part = MIMEBase("application", "octet-stream")
        attachment_part.set_payload(attachment_file.read())
        encoders.encode_base64(attachment_part)
        attachment_part.add_header(
            "Content-Disposition",
            f"attachment; filename={attachment.split('/')[-1]}",
        )
        email_message.attach(attachment_part)

    email_message.attach(MIMEText(html_content, "html"))
    email_content = email_message.as_string()

    try:
        logger.info("Authenticating with smtp server")
        server.login(system_email, system_password)
        logger.info("Authenticated ✅. Sending email")
        server.sendmail(config("EMAIL_ADDRESS"), email_to_use, email_content)
        logger.info("Sending email completed ✅.")
    except Exception as exc:
        logger.error(exc)
        raise
