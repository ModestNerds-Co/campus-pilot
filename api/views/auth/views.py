#
#  views.py
#  campus-pilot
#
#  Created by Ngonidzashe Mangudya on 13/04/2024.
#  Copyright (c) 2024 Codecraft Solutions. All rights reserved.


import jwt
from decouple import config
from django.contrib.auth import authenticate
from django.utils import timezone
from loguru import logger
from rest_framework.views import APIView

from api.views.auth.serializers.model import UserModelSerializer
from api.views.auth.serializers.payload import (
    SignInSerializer,
)
from api.views.auth.tasks import (
    send_login_activity_notification,
    save_login_log,
    send_email_verification_otp,
)
from services.helpers.api_response import ApiResponse
from services.helpers.generate_jwt_payload import generate_jwt_payload
from services.helpers.get_client_details import get_client_details


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
