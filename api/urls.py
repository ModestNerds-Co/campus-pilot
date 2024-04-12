#
#  urls.py
#  campus-pilot
#
#  Created by Ngonidzashe Mangudya on 12/04/2024.
#  Copyright (c) 2024 Codecraft Solutions. All rights reserved.

from django.urls import path, include

urlpatterns = [
    path("auth/", include("api.views.auth.urls")),
]
