#
#  development.py
#  campus-pilot
#
#  Created by Ngonidzashe Mangudya on 12/04/2024.
#  Copyright (c) 2024 Codecraft Solutions. All rights reserved.


from pathlib import Path
import os
from loguru import logger

BASE_DIR = Path(__file__).resolve().parent.parent
DEBUG = True

ALLOWED_HOSTS = ["*"]

CSRF_TRUSTED_ORIGINS = ["http://127.0.0.1:8000"]

STATIC_ROOT = os.path.join("static")
STATIC_URL = "/static/"

MEDIA_ROOT = os.path.join(BASE_DIR, "media")
MEDIA_URL = "/media/"

CORS_ALLOWED_ORIGINS = ["http://localhost:3000", "http://localhost:3003"]

logger.add("application.log", rotation="1 week")
