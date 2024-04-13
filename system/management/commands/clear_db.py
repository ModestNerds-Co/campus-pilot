#
#  clear_db.py
#  campus-pilot
#
#  Created by Ngonidzashe Mangudya on 13/04/2024.
#  Copyright (c) 2024 Codecraft Solutions. All rights reserved.


from django.core.management import BaseCommand
from loguru import logger


class Command(BaseCommand):
    help = "Clear database"

    def handle(self, *args, **options):
        try:
            logger.info("clearing database")
            # remove all tables
            logger.info("removing all tables")
            from django.db import connection

            cursor = connection.cursor()
            cursor.execute("DROP SCHEMA public CASCADE")
            cursor.execute("CREATE SCHEMA public")
            cursor.execute("GRANT ALL ON SCHEMA public TO postgres")
            cursor.execute("GRANT ALL ON SCHEMA public TO public")
            connection.close()

            # run migrations
            logger.info("running migrations")
            from django.core.management import call_command

            call_command("migrate")

            logger.success("database cleared")
        except Exception as exc:
            logger.error(exc)
