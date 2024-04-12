#
#  html_response.py
#  campus-pilot
#
#  Created by Ngonidzashe Mangudya on 12/04/2024.
#  Copyright (c) 2024 Codecraft Solutions. All rights reserved.


from django.http import HttpResponse
from django.template.loader import render_to_string


class HtmlResponse(HttpResponse):
    def __init__(
        self,
        message: str,
        title: str = "Notification",
    ):
        html_content = render_to_string(
            "message.html",
            {
                "title": title,
                "message": message,
            },
        )
        super().__init__(content=html_content, content_type="text/html", status=200)
