#
#  urls.py
#  campus-pilot
#
#  Created by Ngonidzashe Mangudya on 13/04/2024.
#  Copyright (c) 2024 Codecraft Solutions. All rights reserved.

from django.urls import path

from api.views.auth.views import SignInView

urlpatterns = [
    path("signin", SignInView.as_view(), name="auth.sign-in"),
]
