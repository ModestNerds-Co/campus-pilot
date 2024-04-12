#
#  base.py
#  campus-pilot
#
#  Created by Ngonidzashe Mangudya on 12/04/2024.
#  Copyright (c) 2024 Codecraft Solutions. All rights reserved.


from pathlib import Path
from decouple import config
from corsheaders.defaults import default_headers

BASE_DIR = Path(__file__).resolve().parent.parent

SECRET_KEY = "django-insecure-jksb#82$iezm%vv^gib&+%!*9e8d2&zea(#e=wbc28v$94gcc4"

DEBUG = True

ALLOWED_HOSTS = []

USER_APPS = [
    "api",
    "system",
    "users",
]

INSTALLED_APPS = [
    "django.contrib.admin",
    "django.contrib.auth",
    "django.contrib.contenttypes",
    "django.contrib.sessions",
    "django.contrib.messages",
    "django.contrib.staticfiles",
    "rest_framework",
    "corsheaders",
    "algoliasearch_django",
    "django_user_agents",
    "health_check",
    "health_check.db",
    "health_check.cache",
    "health_check.storage",
    "health_check.contrib.migrations",
    "storages",
    "django_rq",
    "django_filters",
] + USER_APPS

MIDDLEWARE = [
    "corsheaders.middleware.CorsMiddleware",
    "django.middleware.security.SecurityMiddleware",
    "django.contrib.sessions.middleware.SessionMiddleware",
    "django.middleware.common.CommonMiddleware",
    "django.middleware.csrf.CsrfViewMiddleware",
    "django.contrib.auth.middleware.AuthenticationMiddleware",
    "django.contrib.messages.middleware.MessageMiddleware",
    "django.middleware.clickjacking.XFrameOptionsMiddleware",
    "django_user_agents.middleware.UserAgentMiddleware",
    "services.middleware.request.RequestLoggerMiddleware",
]

ROOT_URLCONF = "campuspilot.urls"

TEMPLATES = [
    {
        "BACKEND": "django.template.backends.django.DjangoTemplates",
        "DIRS": [],
        "APP_DIRS": True,
        "OPTIONS": {
            "context_processors": [
                "django.template.context_processors.debug",
                "django.template.context_processors.request",
                "django.contrib.auth.context_processors.auth",
                "django.contrib.messages.context_processors.messages",
            ],
        },
    },
]

WSGI_APPLICATION = "campuspilot.wsgi.application"

DATABASES = {
    "default": {
        "ENGINE": "django.db.backends.postgresql",
        "NAME": config("DATABASE_NAME"),
        "USER": config("DATABASE_USER"),
        "PASSWORD": config("DATABASE_PASSWORD"),
        "HOST": config("DATABASE_HOST"),
        "PORT": config("DATABASE_PORT"),
    },
}

ALGOLIA = {
    "APPLICATION_ID": config("ALGOLIA_APPLICATION_ID"),
    "API_KEY": config("ALGOLIA_API_KEY"),
}

AUTH_PASSWORD_VALIDATORS = [
    {
        "NAME": "django.contrib.auth.password_validation.UserAttributeSimilarityValidator",
    },
    {
        "NAME": "django.contrib.auth.password_validation.MinimumLengthValidator",
    },
    {
        "NAME": "django.contrib.auth.password_validation.CommonPasswordValidator",
    },
    {
        "NAME": "django.contrib.auth.password_validation.NumericPasswordValidator",
    },
]

LANGUAGE_CODE = "en-us"

TIME_ZONE = "Africa/Harare"

USE_I18N = True

USE_TZ = True

STATIC_URL = "static/"

DEFAULT_AUTO_FIELD = "django.db.models.BigAutoField"

REST_FRAMEWORK = {
    "DEFAULT_PAGINATION_CLASS": "rest_framework.pagination.PageNumberPagination",
    "PAGE_SIZE": 4,
    "DEFAULT_AUTHENTICATION_CLASSES": [
        "services.auth.is_authenticated.IsAPIAuthenticated"
    ],
    "DEFAULT_SCHEMA_CLASS": "rest_framework.schemas.coreapi.AutoSchema",
    "DEFAULT_PERMISSION_CLASSES": [
        "rest_framework.permissions.AllowAny",
    ],
    "DEFAULT_PARSER_CLASSES": [
        "rest_framework.parsers.JSONParser",
        "rest_framework.parsers.FormParser",
        "rest_framework.parsers.MultiPartParser",
    ],
    "DEFAULT_RENDERER_CLASSES": [
        "rest_framework.renderers.JSONRenderer",
        "rest_framework.renderers.BrowsableAPIRenderer",
    ],
    "TEST_REQUEST_DEFAULT_FORMAT": "json",
    "DEFAULT_FILTER_BACKENDS": [
        "django_filters.rest_framework.DjangoFilterBackend",
    ],
    "EXCEPTION_HANDLER": "services.exceptions.handlers.custom_exception_handler",
    "DEFAULT_THROTTLE_CLASSES": [
        "rest_framework.throttling.AnonRateThrottle",
        "rest_framework.throttling.UserRateThrottle",
    ],
    "DEFAULT_THROTTLE_RATES": {
        "anon": "300/min",
        "user": "1000/min",
    },
}

CORS_ALLOW_CREDENTIALS = True

CORS_ALLOW_METHODS = [
    "DELETE",
    "GET",
    "PATCH",
    "POST",
    "PUT",
]

CORS_ALLOW_HEADERS = (
    *default_headers,
    "x-api-key",
)

AUTH_USER_MODEL = "users.User"

APPEND_SLASH = False

# RQ Settings
CACHES = {
    "default": {
        "BACKEND": "django_redis.cache.RedisCache",
        "LOCATION": config("REDIS_HOST"),
        "OPTIONS": {
            "CLIENT_CLASS": "django_redis.client.DefaultClient",
            "MAX_ENTRIES": 500,
        },
    },
}

RQ_QUEUES = {
    "notifications": {"USE_REDIS_CACHE": "default"},
    "users": {"USE_REDIS_CACHE": "default"},
    "admin": {"USE_REDIS_CACHE": "default"},
    "auth": {"USE_REDIS_CACHE": "default"},
    "rankings": {"USE_REDIS_CACHE": "default"},
    "system": {"USE_REDIS_CACHE": "default"},
}

USER_AGENTS_CACHE = "default"

ORIGIN = config("ORIGIN")

CONN_MAX_AGE = 300

HEALTH_CHECK = {"DISK_USAGE_MAX": 90, "MEMORY_MIN": 100}

SESSION_ENGINE = "django.contrib.sessions.backends.cache"
SESSION_CACHE_ALIAS = "default"

DATA_UPLOAD_MAX_NUMBER_FILES = 100000
DATA_UPLOAD_MAX_MEMORY_SIZE = 100000 * 1024 * 1024
