#
#  create_superuser.py
#  campus-pilot
#
#  Created by Ngonidzashe Mangudya on 13/04/2024.
#  Copyright (c) 2024 Codecraft Solutions. All rights reserved.

from django.core.management import BaseCommand

from services.helpers.create_username import create_username
from system.models import Department, DepartmentSection
from users.models import User, UserRoles, Employee


class Command(BaseCommand):
    help = "Create a superuser"

    def handle(self, *args, **options):
        """
        Create a superuser

        Details:
        email -> "superuser@campuspilot.edu",
        password -> "123456@Campus"
        """

        # Check if the superuser already exists
        if User.objects.filter(email="superuser@campuspilot.edu").exists():
            self.stdout.write(self.style.SUCCESS("Superuser already exists"))
            return

        # Create the superuser
        username = create_username(first_name="Super", last_name="User")
        user = User(
            email="superuser@campuspilot.edu",
            is_active=True,
            user_role=UserRoles.admin,
            first_name="Super",
            last_name="User",
            is_superuser=True,
        )
        user.save()

        # Set the password
        user.set_password("123456@Campus")
        user.save()

        self.stdout.write(self.style.SUCCESS("Superuser created successfully"))

        # create the superuser departments & employee information
        department, _ = Department.objects.get_or_create(name="Administration")
        department_section, _ = DepartmentSection.objects.get_or_create(
            name="Administration", department=department
        )

        employee = Employee(
            user=user,
            employee_code="SU001",
            department_section=department_section,
            position="Superuser",
            date_of_employment=user.date_joined,
            date_of_birth=user.date_joined,
        )
        employee.save()

        self.stdout.write(
            self.style.SUCCESS(
                "Superuser departments & employee information created successfully"
            )
        )

        return
