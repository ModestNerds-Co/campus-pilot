#
#  migrate_languages.py
#  campus-pilot
#
#  Created by Ngonidzashe Mangudya on 13/04/2024.
#  Copyright (c) 2024 Codecraft Solutions. All rights reserved.


import json

from django.core.management import BaseCommand
from loguru import logger

from system.models import Language


class Command(BaseCommand):
    help = "Migrate Languages Into Database"

    def handle(self, *args, **options):
        try:
            logger.debug("Migrating Languages")
            with open("system/files/languages.json", "r") as file:
                languages = json.load(file)
                for key, value in languages.items():
                    # use get_or_create to avoid duplicates
                    logger.info(f"creating or retrieving {value.get('name')}")
                    language, created = Language.objects.get_or_create(
                        language_code=key,
                        name=value.get("name"),
                        native_name=value.get("nativeName"),
                    )
                    logger.info(f"language created: {created}")
            self.stdout.write("Task executed")
        except Exception as e:
            logger.error(e)
            self.stdout.write("Failed to execute task")
