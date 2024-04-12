#
#  payload.py
#  campus-pilot
#
#  Created by Ngonidzashe Mangudya on 13/04/2024.
#  Copyright (c) 2024 Codecraft Solutions. All rights reserved.

from rest_framework import serializers
from django.contrib.auth.password_validation import validate_password


class SignInSerializer(serializers.Serializer):
    email = serializers.EmailField(required=True)
    password = serializers.CharField(required=True)
    remember_me = serializers.BooleanField(default=False)


class ForgotPasswordSerializer(serializers.Serializer):
    email = serializers.EmailField(required=True)


class ResetPasswordSerializer(serializers.Serializer):
    otp = serializers.CharField(required=True, min_length=6, max_length=6)
    password = serializers.CharField(required=True, validators=[validate_password])
    password_confirmation = serializers.CharField(
        required=True, validators=[validate_password]
    )

    def validate(self, attrs):
        validate_password(attrs["password"])
        if attrs["password"] != attrs["password_confirmation"]:
            raise serializers.ValidationError(
                {"password": "Password fields didn't match."}
            )

        return attrs


class RefreshTokenSerializer(serializers.Serializer):
    refresh_token = serializers.CharField(required=True)
