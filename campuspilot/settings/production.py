#
#  production.py
#  campus-pilot
#
#  Created by Ngonidzashe Mangudya on 12/04/2024.
#  Copyright (c) 2024 Codecraft Solutions. All rights reserved.

from decouple import config
import sentry_sdk
from sentry_sdk.integrations.django import DjangoIntegration
from sentry_sdk.integrations.loguru import LoguruIntegration, LoggingLevels

DEBUG = False

sentry_loguru = LoguruIntegration(
    level=LoggingLevels.INFO.value,  # Capture info and above as breadcrumbs
    event_level=LoggingLevels.ERROR.value,  # Send errors as events
)

sentry_sdk.init(
    dsn="",
    integrations=[
        DjangoIntegration(),
        sentry_loguru,
    ],
    traces_sample_rate=1.0,
    send_default_pii=True,
    profiles_sample_rate=1.0,
)

ALLOWED_HOSTS = ["www.tos-api.modestnerds.co"]

CSRF_TRUSTED_ORIGINS = ["https://tos-api.modestnerds.co"]

AWS_ACCESS_KEY_ID = config("AWS_ACCESS_KEY_ID")
AWS_SECRET_ACCESS_KEY = config("AWS_SECRET_ACCESS_KEY")
AWS_S3_REGION_NAME = config("AWS_S3_REGION_NAME")
AWS_STORAGE_BUCKET_NAME = config("AWS_STORAGE_BUCKET_NAME")
AWS_DEFAULT_ACL = "public-read"
AWS_S3_ENDPOINT_URL = "https://tasteofspirits.nyc3.digitaloceanspaces.com"
AWS_S3_OBJECT_PARAMETERS = {"CacheControl": "max-age=86400"}
# static settings
AWS_LOCATION = "static"
STATIC_URL = f"https://{AWS_S3_ENDPOINT_URL}/{AWS_LOCATION}/"
STATICFILES_STORAGE = "storages.backends.s3boto3.S3Boto3Storage"
# public media settings
MEDIA_ROOT = "media"
PUBLIC_MEDIA_LOCATION = "media"
MEDIA_URL = f"https://{AWS_S3_ENDPOINT_URL}/{PUBLIC_MEDIA_LOCATION}/"
DEFAULT_FILE_STORAGE = "tos.storage.PublicMediaStorage"

# CORS_ALLOWED_ORIGINS = ["*"]
CORS_ALLOW_ALL_ORIGINS = True

USE_X_FORWARDED_HOST = True
SECURE_PROXY_SSL_HEADER = ("HTTP_X_FORWARDED_PROTO", "https")
# Security Header Settings
SECURE_SSL_REDIRECT = True
SECURE_BROWSER_XSS_FILTER = True
