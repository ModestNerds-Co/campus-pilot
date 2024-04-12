#
#  generate_jwt_payload.py
#  campus-pilot
#
#  Created by Ngonidzashe Mangudya on 12/04/2024.
#  Copyright (c) 2024 Codecraft Solutions. All rights reserved.


import datetime

from django.utils import timezone

from users.models import User


def generate_jwt_payload(user: User, remember_me: bool = False) -> dict:
    payload = {
        "access": {
            "type": "access",
            "uid": str(user.id),
            "name": user.get_full_name(),
            "role": user.role,
            "is_active": user.is_active,
            "iat": timezone.now(),
            "exp": timezone.now() + datetime.timedelta(hours=24 if remember_me else 48),
            "iss": "Codecraft Solutions (Campus Pilot)",
        },
        "refresh": {
            "type": "refresh",
            "uid": str(user.id),
            "iat": timezone.now(),
            "exp": timezone.now() + datetime.timedelta(hours=28 if remember_me else 52),
            "iss": "Codecraft Solutions (Campus Pilot)",
        },
    }

    return payload
