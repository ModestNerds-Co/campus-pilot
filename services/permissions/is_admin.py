#
#  is_admin.py
#  campus-pilot
#
#  Created by Ngonidzashe Mangudya on 12/04/2024.
#  Copyright (c) 2024 Codecraft Solutions. All rights reserved.


from rest_framework.permissions import BasePermission

from users.models import UserRoles


class IsAdmin(BasePermission):
    message = "You must be an admin to access this resource."

    def has_permission(self, request, view):
        return request.user.role == UserRoles.admin
