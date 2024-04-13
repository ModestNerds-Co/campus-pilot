#
#  migrate_countries.py
#  campus-pilot
#
#  Created by Ngonidzashe Mangudya on 13/04/2024.
#  Copyright (c) 2024 Codecraft Solutions. All rights reserved.


import json

from django.core.management import BaseCommand
from loguru import logger

from system.models import Country


class Command(BaseCommand):
    help = "Migrate Countries Into Database"

    def handle(self, *args, **options):
        try:
            logger.debug("Migrating Countries")
            with open("system/files/countries.json", "r") as file:
                countries = json.load(file)
                for key, value in countries.items():
                    # use get_or_create to avoid duplicates
                    logger.info(f"creating or retrieving {key}")
                    country, created = Country.objects.get_or_create(
                        name=key,
                        iso_code=value.get("iso_code"),
                    )
                    logger.info(f"country was created: {created}")

                    for state in value.get("states"):
                        try:
                            logger.info(f"creating or retrieving {state.get('name')}")
                            country.states.get_or_create(name=state.get("name"))
                        except Exception as exc:
                            logger.error(exc)

            self.stdout.write("Task executed")
        except Exception as e:
            logger.error(e)
            self.stdout.write("Failed to execute task")
