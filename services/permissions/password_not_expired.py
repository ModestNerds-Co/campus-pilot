#
#  password_not_expired.py
#  campus-pilot
#
#  Created by Ngonidzashe Mangudya on 12/04/2024.
#  Copyright (c) 2024 Codecraft Solutions. All rights reserved.


from django.utils import timezone
from rest_framework.permissions import BasePermission


class PasswordNotExpired(BasePermission):
    message = "Password has expired. Update your password!"

    def has_permission(self, request, view):
        current_time = timezone.now()
        last_password_updated = request.user.password_updated_at
        delta = current_time.date() - last_password_updated.date()
        if delta.days >= 180:
            return False

        return True
