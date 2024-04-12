#
#  model.py
#  campus-pilot
#
#  Created by Ngonidzashe Mangudya on 13/04/2024.
#  Copyright (c) 2024 Codecraft Solutions. All rights reserved.

from rest_framework.serializers import ModelSerializer

from users.models import User


class UserModelSerializer(ModelSerializer):
    class Meta:
        model = User
        fields = [
            "id",
            "email",
            "user_role",
            "first_name",
            "last_name",
            "last_login",
            "date_joined",
            "username",
            "email_verified",
            "picture",
            "banner_picture",
            "is_active",
        ]
