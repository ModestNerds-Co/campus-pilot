#
#  tasks.py
#  campus-pilot
#
#  Created by Ngonidzashe Mangudya on 13/04/2024.
#  Copyright (c) 2024 Codecraft Solutions. All rights reserved.


import ipinfo
from decouple import config
from django.template.loader import render_to_string
from django.utils import timezone
from django_rq import job
from loguru import logger
from rq import Retry

from services.helpers.notifications import send_email
from users.models import User
from users.models import LoginLog


@job("auth", retry=Retry(max=3))
def send_email_verification_otp(user: User):
    user.generate_email_otp()
    email_link = f"{config('ORIGIN')}/api/1.0/auth/verifications/email-by-link?c={user.email_code}"
    html_content = render_to_string(
        "auth.email-verification.html",
        {
            "code": user.email_pin,
            "email": user.email
            if not user.pending_new_email
            else user.pending_new_email,
            "email_link": email_link,
            "unsubscribe_link": "",
            "manage_preferences": "",
        },
    )
    send_email.delay(
        user=user,
        html_content=html_content,
        email_subject="Email Verification",
        override=True,
    )
    logger.success("Verification code sent")


@job("auth", retry=Retry(max=3))
def send_existing_email_verification_otp(user: User):
    email_link = f"{config('ORIGIN')}/api/1.0/auth/verifications/email-by-link?c={user.email_code}"
    html_content = render_to_string(
        "auth.email-verification.html",
        {
            "code": user.email_pin,
            "email": user.email
            if not user.pending_new_email
            else user.pending_new_email,
            "email_link": email_link,
            "unsubscribe_link": "",
            "manage_preferences": "",
        },
    )
    send_email.delay(
        user=user,
        html_content=html_content,
        email_subject="Email Verification",
        override=True,
    )
    logger.success("Verification code sent")


@job("auth", retry=Retry(max=3))
def save_login_log(user: User, details):
    ip = details.get("ip")
    user_agent = details.get("user_agent")

    logger.info(f"IP: {ip}")
    logger.info(f"User Agent: {user_agent}")

    log = LoginLog(user=user, ip_address=ip, user_agent=user_agent)
    log.save()
    logger.success("Login log saved")


@job("auth", retry=Retry(max=3))
def send_login_activity_notification(user: User, details):
    logger.debug(f"client details: {details}")
    signed_in_at = timezone.now()
    logger.info(f"signed in at: {signed_in_at}")

    user.last_login = timezone.now()
    user.save()

    # Get approximate location from ip
    try:
        token = config("IPINFO_TOKEN")
        logger.info(f"IPINFO TOKEN: {token}")
        handler = ipinfo.getHandler(token)
        data = handler.getDetails(ip_address=details.get("ip"))
        location = f"{data.city}, {data.region}, {data.country_name}"
    except Exception as exc:
        logger.error(exc)
        location = "Unknown"

    logger.info(f"Location: {location}")

    html_content = render_to_string(
        "auth.signin-activity.html",
        {
            "email": user.email,
            "location": location,
            "client": details.get("user_agent"),
            "ip": details.get("ip"),
            "time": signed_in_at,
            "unsubscribe_link": "",
            "manage_preferences": "",
        },
    )

    send_email.delay(
        user=user,
        html_content=html_content,
        email_subject="[login] New sign-in to your account",
        override=True,
    )
    logger.info("User notified about login activity")


@job("auth", retry=Retry(max=3))
def send_password_reset_otp(user: User):
    user.generate_email_otp()
    html_content = render_to_string(
        "auth.password-reset.html",
        {
            "code": user.email_pin,
            "email": user.email,
            "unsubscribe_link": "",
            "manage_preferences": "",
        },
    )
    send_email.delay(
        user=user,
        html_content=html_content,
        email_subject="Forgot Password",
        override=True,
    )
    logger.success("Job complete")
