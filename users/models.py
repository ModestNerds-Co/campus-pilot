import json
from datetime import datetime

from django.contrib.auth.hashers import make_password, check_password
from django.contrib.auth.models import AbstractUser
from django.db import models
from django.utils import timezone
from django.utils.translation import gettext_lazy as _
from loguru import logger
import avinit

from services.exceptions.passwords import PasswordUsedException

from services.helpers.generate_otp import random_otp
from services.helpers.generate_random_text import generate_random_text
from campuspilot.model import EnumModel, SoftDeleteModel
from system.models import Gender
from users.manager import CustomUserManager


class UserRoles(EnumModel):
    admin = "admin", _("Platform Admin")
    student = "student", _("Student")
    employee = "employee", _("Employee")


class AuthProvider(EnumModel):
    campus_pilot = "campus_pilot", _("Campus Pilot")


class LowercaseEmailField(models.EmailField):
    def to_python(self, value):
        value = super(LowercaseEmailField, self).to_python(value)
        if isinstance(value, str):
            return value.lower()
        return value


class User(SoftDeleteModel, AbstractUser):
    email = LowercaseEmailField(unique=True)
    is_active = models.BooleanField(default=False)
    user_role = models.CharField(
        choices=UserRoles.choices,
        max_length=50,
        blank=False,
        null=False,
        default=UserRoles.student,
    )
    email_pin = models.CharField(max_length=6, blank=True, null=True)
    email_code = models.CharField(max_length=15, blank=True, null=True)
    email_pin_sent_at = models.DateTimeField(null=True, blank=True)
    email_verified = models.BooleanField(default=False, null=False, blank=False)
    email_verified_at = models.DateTimeField(null=True, blank=True)
    sms_pin = models.CharField(max_length=6, blank=True, null=True)
    sms_pin_sent_at = models.DateTimeField(null=True, blank=True)
    password_history = models.TextField(blank=True, null=True)
    password_updated_at = models.DateTimeField(blank=True, null=True)
    picture = models.ImageField(upload_to="profile_pictures", blank=True, null=True)
    banner_picture = models.ImageField(
        upload_to="banner_profile_pictures", blank=True, null=True
    )
    is_blocked = models.BooleanField(default=False)
    receive_email_notifications = models.BooleanField(
        default=True, null=False, blank=False
    )
    pending_new_email = models.EmailField(null=True, blank=True)
    auth_provider = models.CharField(
        choices=AuthProvider.choices,
        max_length=50,
        blank=False,
        null=False,
        default=AuthProvider.campus_pilot,
    )
    auth_provider_id = models.CharField(max_length=255, blank=True, null=True)

    USERNAME_FIELD = "email"
    REQUIRED_FIELDS = ["username", "first_name", "last_name"]

    objects = CustomUserManager()

    class Meta:
        verbose_name = "User"
        verbose_name_plural = "Users"
        table_prefix = "user"

    def __str__(self):
        return f"{self.first_name} {self.last_name}"

    def set_password(self, raw_password):
        try:
            if self.password_history is not None:
                password_history = json.loads(self.password_history)
            else:
                password_history = []
            for password_json in password_history:
                password_object = json.loads(password_json)

                if check_password(
                    password=raw_password,
                    encoded=password_object.get("password"),
                    setter=None,
                ):
                    date_string = password_object.get("changed_on")
                    datetime_obj = datetime.fromisoformat(date_string)
                    formatted_date = datetime_obj.strftime("%d %B %Y, %I:%M%p")
                    raise PasswordUsedException(
                        f"This password was used before on {formatted_date}"
                    )

            password_history.append(
                json.dumps(
                    {"password": self.password, "changed_on": str(timezone.now())}
                )
            )

            self.password_history = json.dumps(password_history)
            self.password = make_password(raw_password)
            self._password = raw_password
            self.password_updated_at = timezone.now()
        except Exception as exc:
            logger.error(exc)
            raise

    def save(self, *args, **kwargs):
        # set admin user permissions
        if self.is_superuser:
            self.role = UserRoles.admin
            self.is_verified = True
        super().save(*args, **kwargs)

    def generate_email_otp(self):
        otp = random_otp()
        code = generate_random_text(n=15)
        self.email_pin = otp
        self.email_pin_sent_at = timezone.now()
        self.email_code = code
        self.save()

    @property
    def alternate_picture(self):
        return avinit.get_avatar_data_url(self.get_full_name(), colors=["#890620"])


class LoginLog(SoftDeleteModel):
    user = models.ForeignKey(
        "users.User",
        related_name="login_logs",
        on_delete=models.CASCADE,
        blank=False,
        null=False,
    )
    ip_address = models.CharField(max_length=50, blank=False, null=False)
    user_agent = models.CharField(max_length=500, blank=False, null=False)

    class Meta:
        verbose_name = "Login Log"
        verbose_name_plural = "Login Logs"
        table_prefix = "llog"


class Employee(SoftDeleteModel):
    user = models.OneToOneField(
        "users.User",
        on_delete=models.CASCADE,
        related_name="employee",
        blank=False,
        null=False,
    )
    employee_code = models.CharField(max_length=50, blank=False, null=False)
    department_section = models.ForeignKey(
        "system.DepartmentSection",
        on_delete=models.CASCADE,
        related_name="employees",
        blank=False,
        null=False,
    )
    position = models.CharField(max_length=100, blank=False, null=False)
    date_of_employment = models.DateField(blank=False, null=False)
    date_of_birth = models.DateField(blank=False, null=False)
    phone_number = models.CharField(max_length=50, blank=True, null=True)
    email_address = LowercaseEmailField(blank=True, null=True)
    gender = models.CharField(
        max_length=50,
        choices=Gender.choices,
        default=Gender.unknown,
        blank=False,
        null=False,
    )

    class Meta:
        verbose_name = "Employee"
        verbose_name_plural = "Employees"
        table_prefix = "emp"
