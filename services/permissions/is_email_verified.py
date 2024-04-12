#
#  is_email_verified.py
#  campus-pilot
#
#  Created by Ngonidzashe Mangudya on 12/04/2024.
#  Copyright (c) 2024 Codecraft Solutions. All rights reserved.


from rest_framework.permissions import BasePermission

from users.models import UserRoles


class IsEmailVerified(BasePermission):
    message = "Your email must be verified to access these resources."

    def has_permission(self, request, view):
        if request.user.role == UserRoles.admin:
            return True

        return request.user.email_verified
