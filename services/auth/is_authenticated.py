#
#  is_authenticated.py
#  campus-pilot
#
#  Created by Ngonidzashe Mangudya on 12/04/2024.
#  Copyright (c) 2024 Codecraft Solutions. All rights reserved.

import jwt
from decouple import config
from django.utils.translation import gettext_lazy as _
from jwt import ExpiredSignatureError, DecodeError
from rest_framework import exceptions
from rest_framework.authentication import BaseAuthentication

from services.helpers.redis_client import redis_client
from users.models import User


class IsAPIAuthenticated(BaseAuthentication):
    def _get_token(self, request):
        header = request.headers.get("Authorization")

        # Check if required authentication header is present
        if header is None:
            return None

        header_values = header.split(" ")

        if len(header_values) == 2:
            # Get the actual token from the header value
            token = header_values[1]
            return token
        else:
            msg = _("WHO ARE YOU? LETS TALK LIKE ADULTS.")
            raise exceptions.AuthenticationFailed(msg)

    def authenticate(self, request):
        # Get the actual token from the header value
        token = self._get_token(request)

        if token is None:
            if request.method in ("GET", "HEAD", "OPTIONS"):
                return None

            msg = _("WHO ARE YOU? LETS TALK LIKE ADULTS.")
            raise exceptions.AuthenticationFailed(msg)

        blacklisted_tokens = redis_client.get("destroyed_tokens")
        if blacklisted_tokens is not None:
            blacklisted_tokens = blacklisted_tokens.decode("utf-8")
            blacklisted_tokens = blacklisted_tokens.split(",")

            if token in blacklisted_tokens:
                msg = _("Invalid Token [User logged out already]")
                raise exceptions.AuthenticationFailed(msg)

        try:
            # Decode token
            decoded = jwt.decode(
                token,
                options={"require": ["exp", "iat", "iss"]},
                key=config("JWT_SECRET"),
                verify=True,
                algorithms=["HS256"],
            )

            if decoded.get("type") != "access":
                msg = _("Invalid Token Type")
                raise exceptions.AuthenticationFailed(msg)

            pk = decoded.get("id")
            user = User.objects.get(id=pk)
            return user, token
        except User.DoesNotExist:
            msg = _("Token doesn't represent any user.")
            raise exceptions.AuthenticationFailed(msg)
        except ExpiredSignatureError as e:
            msg = _("Session expired. Please login again.")
            raise exceptions.AuthenticationFailed(msg)
        except DecodeError as e:
            msg = _("Token Decoding Failure")
            raise exceptions.AuthenticationFailed(msg)
