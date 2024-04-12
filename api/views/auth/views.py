#
#  views.py
#  campus-pilot
#
#  Created by Ngonidzashe Mangudya on 13/04/2024.
#  Copyright (c) 2024 Codecraft Solutions. All rights reserved.


import json

from rest_framework.views import APIView
from loguru import logger
from django.contrib.auth import authenticate
import jwt
from jwt.exceptions import ExpiredSignatureError
from decouple import config
from django.utils import timezone
from rest_framework.permissions import IsAuthenticated
from django.db import transaction
import requests
from oauthlib.oauth2 import WebApplicationClient
from django.shortcuts import redirect

from api.views.auth.serializers.model import UserModelSerializer
from api.views.auth.serializers.payload import (
    SignInSerializer,
    SignUpSerializer,
    EmailVerificationByCodeSerializer,
    ForgotPasswordSerializer,
    ResetPasswordSerializer,
    RefreshTokenSerializer,
)
from auth0.models import AccessToken
from services.exceptions.passwords import PasswordUsedException
from services.helpers.api_response import ApiResponse
from services.helpers.create_username import create_username
from services.helpers.generate_jwt_payload import generate_jwt_payload
from services.helpers.get_client_details import get_client_details
from api.views.auth.tasks import (
    send_login_activity_notification,
    save_login_log,
    send_email_verification_otp,
    send_password_reset_otp,
    send_existing_email_verification_otp,
)
from services.helpers.html_response import HtmlResponse
from services.helpers.redis_client import redis_client
from system.models import Language
from users.models import (
    User,
    Roles,
    Member,
    MemberPrivacySettings,
    MemberRankHistory,
    Points,
    AuthProvider,
)


class SignInView(APIView):
    serializer_class = SignInSerializer
    authentication_classes = ()

    def post(self, request):
        try:
            payload = self.serializer_class(data=request.data)

            if payload.is_valid():
                email = payload.validated_data.get("email").lower()
                password = payload.validated_data.get("password")
                user = authenticate(request, email=email, password=password)

                if user is not None:
                    # if user hasn't been verified send another otp code
                    if not user.email_verified:
                        send_email_verification_otp.delay(user)

                    remember_me = payload.validated_data.get("remember_me")

                    jwt_payload = generate_jwt_payload(user, remember_me=remember_me)

                    access_token = jwt.encode(
                        payload=jwt_payload["access"],
                        key=config("JWT_SECRET"),
                        algorithm="HS256",
                    )
                    refresh_token = jwt.encode(
                        payload=jwt_payload["refresh"],
                        key=config("JWT_SECRET"),
                        algorithm="HS256",
                    )

                    # notify user of login activity
                    details = get_client_details(request)
                    send_login_activity_notification.delay(user, details)
                    save_login_log.delay(user, details)

                    user.last_login = timezone.now()

                    return ApiResponse(
                        data={
                            "access_token": access_token,
                            "refresh_token": refresh_token,
                            "user": UserModelSerializer(user).data,
                        }
                    )

                else:
                    logger.warning("Incorrect email or password")
                    return ApiResponse(
                        num_status=401,
                        bool_status=False,
                        message="Incorrect email or password",
                    )
            else:
                logger.warning(payload.errors)
                return ApiResponse(
                    num_status=400, bool_status=False, issues=payload.errors
                )
        except Exception as exc:
            logger.error(exc)
            return ApiResponse(num_status=500, bool_status=False)
