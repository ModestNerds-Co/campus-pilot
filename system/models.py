import secrets

from django.db import models
from django.utils.translation import gettext_lazy as _

from campuspilot.model import SoftDeleteModel, EnumModel


class Country(SoftDeleteModel):
    name = models.CharField(max_length=255, blank=False, null=False, unique=True)
    iso_code = models.CharField(max_length=6, blank=False, null=True)
    enabled = models.BooleanField(default=True)

    class Meta:
        ordering = ("-enabled", "name")
        verbose_name = "Country"
        verbose_name_plural = "Countries"
        table_prefix = "country"
        unique_together = ("name", "iso_code")

    def __str__(self):
        return f"{self.name} ({self.iso_code}) - {'Enabled' if self.enabled else 'Disabled'}"


class State(SoftDeleteModel):
    name = models.CharField(max_length=255, blank=False, null=False)
    country = models.ForeignKey(
        "system.Country",
        on_delete=models.CASCADE,
        related_name="states",
        blank=False,
        null=False,
    )
    iso_code = models.CharField(max_length=6, blank=True, null=True)
    enabled = models.BooleanField(default=True)

    class Meta:
        ordering = ("-enabled", "name")
        verbose_name = "State"
        verbose_name_plural = "States"
        table_prefix = "state"
        unique_together = ("name", "country")

    def __str__(self):
        return f"{self.name} ({self.iso_code}) - {'Enabled' if self.enabled else 'Disabled'}"


class Language(SoftDeleteModel):
    name = models.CharField(max_length=100, blank=False, null=False, unique=True)
    native_name = models.CharField(max_length=100, blank=False, null=False)
    language_code = models.CharField(
        max_length=10, blank=False, null=False, unique=True
    )
    enabled = models.BooleanField(default=False, null=False, blank=False)

    class Meta:
        verbose_name = "Language"
        verbose_name_plural = "Languages"
        table_prefix = "lang"

    def __str__(self):
        return f"{self.name} - {self.native_name} - {self.language_code}"

    @classmethod
    def get_language_from_code(cls, code):
        return cls.objects.filter(language_code=code).first()


class Gender(EnumModel):
    male = "male", _("male")
    female = "female", _("female")
    unknown = "unknown", _("unknown")
    diverse = "diverse", _("diverse")


class FrequentlyAskedQuestion(SoftDeleteModel):
    title = models.CharField(max_length=1000, blank=True, null=False, unique=True)
    answer = models.TextField(blank=True, null=False)
    created_by = models.ForeignKey(
        "users.User", on_delete=models.DO_NOTHING, blank=False, null=False
    )
    updated_by = models.ForeignKey(
        "users.User",
        related_name="enquiry_updated",
        on_delete=models.DO_NOTHING,
        blank=False,
        null=False,
    )

    class Meta:
        verbose_name = "Frequently Asked Question"
        verbose_name_plural = "Frequently Asked Questions"
        table_prefix = "faq"

    def __str__(self):
        return self.title


class ApiRequest(SoftDeleteModel):
    method = models.CharField(max_length=6, default="GET", blank=True, null=True)
    path = models.CharField(max_length=255, blank=True, null=True)
    headers = models.TextField(null=True, blank=True)

    class Meta:
        verbose_name = "API Request"
        verbose_name_plural = "API Requests"
        table_prefix = "apireq"


class APIClient(SoftDeleteModel):
    name = models.CharField(max_length=255, blank=False, null=False)
    api_key = models.CharField(max_length=255, blank=False, null=False)
    enabled = models.BooleanField(default=True, blank=False, null=False)

    class Meta:
        verbose_name = "API Client"
        verbose_name_plural = "API Clients"
        table_prefix = "apiclient"

    def __str__(self):
        return f"{self.name} - {'Enabled' if self.enabled else 'Disabled'}"

    @classmethod
    def create_client(cls, name):
        api_key = secrets.token_urlsafe(32)
        return cls.objects.create(name=name, api_key=api_key)

    @classmethod
    def get_client_by_key(cls, key):
        return cls.objects.filter(api_key=key).first()


class OrganizationDetails(SoftDeleteModel):
    name = models.CharField(max_length=255, blank=False, null=False)
    address = models.TextField(blank=True, null=True)
    city = models.CharField(max_length=255, blank=True, null=True)
    state = models.ForeignKey(
        "system.State", on_delete=models.CASCADE, blank=True, null=True
    )
    country = models.ForeignKey(
        "system.Country", on_delete=models.CASCADE, blank=True, null=True
    )
    zip_code = models.CharField(max_length=255, blank=True, null=True)
    phone = models.CharField(max_length=255, blank=True, null=True)
    email = models.EmailField(blank=True, null=True)
    website = models.URLField(blank=True, null=True)
    logo = models.ImageField(upload_to="organization_logo", blank=True, null=True)
    vat_number = models.CharField(max_length=255, blank=True, null=True)
    enabled = models.BooleanField(default=True, blank=False, null=False)
    license_token = models.CharField(max_length=255, blank=True, null=True)

    class Meta:
        verbose_name = "Organization Detail"
        verbose_name_plural = "Organization Details"
        table_prefix = "org"

    def __str__(self):
        return f"{self.name} - {'Enabled' if self.enabled else 'Disabled'}"


class Department(SoftDeleteModel):
    name = models.CharField(max_length=255, blank=False, null=False)
    notes = models.TextField(blank=True, null=True)
    department_code = models.CharField(max_length=255, blank=True, null=True)

    class Meta:
        verbose_name = "Department"
        verbose_name_plural = "Departments"
        table_prefix = "dept"

    def __str__(self):
        return f"{self.name} - {self.department_code}"


class DepartmentSection(SoftDeleteModel):
    name = models.CharField(max_length=255, blank=False, null=False)
    department = models.ForeignKey(
        "system.Department", on_delete=models.CASCADE, blank=False, null=False
    )

    class Meta:
        verbose_name = "Department Section"
        verbose_name_plural = "Department Sections"
        table_prefix = "deptsec"

    def __str__(self):
        return f"{self.name} - {self.department}"


class BankingDetails(SoftDeleteModel):
    bank_name = models.CharField(max_length=255, blank=False, null=False)
    account_name = models.CharField(max_length=255, blank=False, null=False)
    account_number = models.CharField(max_length=255, blank=False, null=False)
    branch_name = models.CharField(max_length=255, blank=False, null=False)
    branch_code = models.CharField(max_length=255, blank=False, null=False)
    swift_code = models.CharField(max_length=255, blank=True, null=True)
    iban = models.CharField(max_length=255, blank=True, null=True)
    enabled = models.BooleanField(default=True, blank=False, null=False)

    class Meta:
        verbose_name = "Banking Detail"
        verbose_name_plural = "Banking Details"
        table_prefix = "bank"

    def __str__(self):
        return f"{self.bank_name} - {'Enabled' if self.enabled else 'Disabled'}"
