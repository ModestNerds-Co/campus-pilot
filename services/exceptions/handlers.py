#
#  handlers.py
#  campus-pilot
#
#  Created by Ngonidzashe Mangudya on 12/04/2024.
#  Copyright (c) 2024 Codecraft Solutions. All rights reserved.


from rest_framework.views import exception_handler

from services.helpers.api_response import ApiResponse


def custom_exception_handler(exc, context):
    # Call REST framework's default exception handler first,
    # to get the standard error response.
    response = exception_handler(exc, context)

    # Now add the HTTP status code to the response.
    if response is not None:
        response.data["status_code"] = response.status_code

    return ApiResponse(
        message=response.data.get("detail") if response else "Something went wrong",
        num_status=response.data.get("status_code") if response else 500,
        bool_status=False,
        issues=str(exc),
    )
