#
#  request.py
#  campus-pilot
#
#  Created by Ngonidzashe Mangudya on 12/04/2024.
#  Copyright (c) 2024 Codecraft Solutions. All rights reserved.


import json
import time

import sentry_sdk
from django.core.serializers.json import DjangoJSONEncoder
from django.http import HttpRequest, HttpResponse
from django.utils.deprecation import MiddlewareMixin
from loguru import logger

from system.models import ApiRequest


class RequestLoggerMiddleware(MiddlewareMixin):
    def __init__(self, get_response):
        super().__init__(get_response)

    def process_request(self, request):
        try:
            # save request
            req = ApiRequest(
                method=request.method,
                headers=str(request.headers),
                path=request.path,
            )
            req.save()

            request.id = req.id

            logger.info("------------------------------------------------")
            logger.info(request.method)
            logger.info(request.path)

            with logger.contextualize(request_id=req.id):
                request.start_time = time.time()
        except Exception as exc:
            logger.warning("Failed to log request, probably websocket request")
            sentry_sdk.capture_exception(exc)
            pass

    def process_response(
        self, request: HttpRequest, response: HttpResponse
    ) -> HttpResponse:
        try:
            elapsed = time.time() - request.start_time

            # After the response is received
            logger.bind(
                path=request.path,
                method=request.method,
                status_code=response.status_code,
                response_size=len(response.content),
                elapsed=elapsed,
            ).info(
                "incoming '{method}' request to '{path}' took {elapsed}s",
                method=request.method,
                path=request.path,
                elapsed=elapsed,
            )

            response["X-Request-ID"] = request.id
            response["X-Response-Time"] = elapsed
            response_content = json.loads(response.content)
            response_content["request_id"] = request.id
            response.content = json.dumps(response_content, cls=DjangoJSONEncoder)
        except Exception as exc:
            logger.warning("Response is not JSON parseable, probably HTML.")
            sentry_sdk.capture_exception(exc)
        return response
