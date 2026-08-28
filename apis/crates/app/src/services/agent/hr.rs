//! Adapts the canonical HR directory reads to typed Agent capabilities.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use cp_agent::{
    AuthorizedCapabilityContext, Capability, CapabilityDescriptor, CapabilityExecutionError,
    CapabilityExecutionErrorCode, CapabilityResource, CapabilityScope, DataSensitivity,
};
use cp_common::PaginationMeta;
use cp_hr_payroll::{
    dtos::{
        AvailabilityKind, AvailabilityStatus, DepartmentResponse, DirectoryStatus,
        EmployeeAvailabilityResponse, EmployeeResponse, EmploymentEngagementResponse,
        EmploymentStatus, EmploymentType, EngagementStatus, PaginatedDepartmentsResponse,
        PaginatedEmployeeAvailabilityResponse, PaginatedEmployeesResponse,
        PaginatedEmploymentEngagementsResponse, PaginatedPositionsResponse, PositionResponse,
    },
    imports::HrImportOps,
    ops::{
        DepartmentOps, EmployeeAvailabilityOps, EmployeeOps, EmploymentEngagementOps, PositionOps,
    },
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

use super::administration::read_descriptor;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct DirectoryListInput {
    page: Option<i64>,
    per_page: Option<i64>,
    search: Option<String>,
    status: Option<DirectoryStatus>,
}

#[derive(Serialize)]
pub(super) struct ListDepartmentsOutput {
    departments: PaginatedDepartmentsResponse,
    pagination: PaginationMeta,
}

pub(super) struct HrDepartmentsListCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}

impl HrDepartmentsListCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: directory_list_descriptor(
                "hr_payroll.departments.list",
                "List departments",
                "Returns the tenant department directory using bounded pagination.",
                "departments",
                "hr_payroll.departments",
            ),
        }
    }
}

#[async_trait]
impl Capability for HrDepartmentsListCapability {
    type Input = DirectoryListInput;
    type Output = ListDepartmentsOutput;

    fn descriptor(&self) -> &CapabilityDescriptor {
        &self.descriptor
    }

    fn scope(&self, _input: &Self::Input) -> CapabilityScope {
        CapabilityScope::TenantWide
    }

    async fn execute(
        &self,
        context: AuthorizedCapabilityContext,
        input: Self::Input,
    ) -> Result<Self::Output, CapabilityExecutionError> {
        let (page, per_page) = bounded_page(input.page, input.per_page);
        let (departments, total) = DepartmentOps::list(
            &self.pool,
            context.principal().tenant_id(),
            page,
            per_page,
            trimmed(input.search.as_deref()),
            input.status.map(DirectoryStatus::as_str),
        )
        .await
        .map_err(|_| dependency_failure("Departments could not be loaded."))?;
        Ok(ListDepartmentsOutput {
            departments: PaginatedDepartmentsResponse {
                departments: departments
                    .into_iter()
                    .map(DepartmentResponse::from)
                    .collect(),
            },
            pagination: PaginationMeta::new(page as u32, per_page as u32, total),
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ReadDepartmentInput {
    department_id: Uuid,
}

#[derive(Serialize)]
pub(super) struct ReadDepartmentOutput {
    department: DepartmentResponse,
}

pub(super) struct HrDepartmentReadCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}

impl HrDepartmentReadCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "hr_payroll.departments.read",
                "Read department",
                "Returns one tenant department by its stable identifier.",
                json!({ "department_id": { "type": "string", "format": "uuid" } }),
                json!({ "department": { "type": "object" } }),
                DataSensitivity::General,
                "hr_payroll.departments",
            ),
        }
    }
}

#[async_trait]
impl Capability for HrDepartmentReadCapability {
    type Input = ReadDepartmentInput;
    type Output = ReadDepartmentOutput;

    fn descriptor(&self) -> &CapabilityDescriptor {
        &self.descriptor
    }

    fn scope(&self, input: &Self::Input) -> CapabilityScope {
        resource_scope("department", input.department_id)
    }

    async fn execute(
        &self,
        context: AuthorizedCapabilityContext,
        input: Self::Input,
    ) -> Result<Self::Output, CapabilityExecutionError> {
        let department = DepartmentOps::get_by_id(
            &self.pool,
            context.principal().tenant_id(),
            input.department_id,
        )
        .await
        .map_err(|_| dependency_failure("The department could not be loaded."))?
        .ok_or_else(|| not_found("The department was not found."))?;
        Ok(ReadDepartmentOutput {
            department: DepartmentResponse::from(department),
        })
    }
}

#[derive(Serialize)]
pub(super) struct ListPositionsOutput {
    positions: PaginatedPositionsResponse,
    pagination: PaginationMeta,
}

pub(super) struct HrPositionsListCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}

impl HrPositionsListCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: directory_list_descriptor(
                "hr_payroll.positions.list",
                "List positions",
                "Returns the tenant position directory using bounded pagination.",
                "positions",
                "hr_payroll.positions",
            ),
        }
    }
}

#[async_trait]
impl Capability for HrPositionsListCapability {
    type Input = DirectoryListInput;
    type Output = ListPositionsOutput;

    fn descriptor(&self) -> &CapabilityDescriptor {
        &self.descriptor
    }

    fn scope(&self, _input: &Self::Input) -> CapabilityScope {
        CapabilityScope::TenantWide
    }

    async fn execute(
        &self,
        context: AuthorizedCapabilityContext,
        input: Self::Input,
    ) -> Result<Self::Output, CapabilityExecutionError> {
        let (page, per_page) = bounded_page(input.page, input.per_page);
        let (positions, total) = PositionOps::list(
            &self.pool,
            context.principal().tenant_id(),
            page,
            per_page,
            trimmed(input.search.as_deref()),
            input.status.map(DirectoryStatus::as_str),
        )
        .await
        .map_err(|_| dependency_failure("Positions could not be loaded."))?;
        Ok(ListPositionsOutput {
            positions: PaginatedPositionsResponse {
                positions: positions.into_iter().map(PositionResponse::from).collect(),
            },
            pagination: PaginationMeta::new(page as u32, per_page as u32, total),
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ReadPositionInput {
    position_id: Uuid,
}

#[derive(Serialize)]
pub(super) struct ReadPositionOutput {
    position: PositionResponse,
}

pub(super) struct HrPositionReadCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}

impl HrPositionReadCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "hr_payroll.positions.read",
                "Read position",
                "Returns one tenant position by its stable identifier.",
                json!({ "position_id": { "type": "string", "format": "uuid" } }),
                json!({ "position": { "type": "object" } }),
                DataSensitivity::General,
                "hr_payroll.positions",
            ),
        }
    }
}

#[async_trait]
impl Capability for HrPositionReadCapability {
    type Input = ReadPositionInput;
    type Output = ReadPositionOutput;

    fn descriptor(&self) -> &CapabilityDescriptor {
        &self.descriptor
    }

    fn scope(&self, input: &Self::Input) -> CapabilityScope {
        resource_scope("position", input.position_id)
    }

    async fn execute(
        &self,
        context: AuthorizedCapabilityContext,
        input: Self::Input,
    ) -> Result<Self::Output, CapabilityExecutionError> {
        let position = PositionOps::get_by_id(
            &self.pool,
            context.principal().tenant_id(),
            input.position_id,
        )
        .await
        .map_err(|_| dependency_failure("The position could not be loaded."))?
        .ok_or_else(|| not_found("The position was not found."))?;
        Ok(ReadPositionOutput {
            position: PositionResponse::from(position),
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct EmployeeListInput {
    page: Option<i64>,
    per_page: Option<i64>,
    search: Option<String>,
    status: Option<EmploymentStatus>,
    department_id: Option<Uuid>,
    position_id: Option<Uuid>,
    account_linked: Option<bool>,
}

#[derive(Serialize)]
pub(super) struct ListEmployeesOutput {
    employees: PaginatedEmployeesResponse,
    pagination: PaginationMeta,
}

pub(super) struct HrEmployeesListCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}

impl HrEmployeesListCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "hr_payroll.employees.list",
                "List employees",
                "Returns canonical tenant employee records using bounded directory filters.",
                json!({
                    "page": { "type": ["integer", "null"], "minimum": 1 },
                    "per_page": { "type": ["integer", "null"], "minimum": 1, "maximum": 100 },
                    "search": { "type": ["string", "null"], "maxLength": 200 },
                    "status": { "type": ["string", "null"], "enum": ["active", "inactive", "suspended", "terminated", null] },
                    "department_id": { "type": ["string", "null"], "format": "uuid" },
                    "position_id": { "type": ["string", "null"], "format": "uuid" },
                    "account_linked": { "type": ["boolean", "null"] }
                }),
                json!({
                    "employees": { "type": "object" },
                    "pagination": { "type": "object" }
                }),
                DataSensitivity::Personal,
                "hr_payroll.employees",
            ),
        }
    }
}

#[async_trait]
impl Capability for HrEmployeesListCapability {
    type Input = EmployeeListInput;
    type Output = ListEmployeesOutput;

    fn descriptor(&self) -> &CapabilityDescriptor {
        &self.descriptor
    }

    fn scope(&self, input: &Self::Input) -> CapabilityScope {
        let resources = [
            input.department_id.map(|id| ("department", id)),
            input.position_id.map(|id| ("position", id)),
        ]
        .into_iter()
        .flatten()
        .map(|(kind, id)| resource(kind, id))
        .collect::<Vec<_>>();
        if resources.is_empty() {
            CapabilityScope::TenantWide
        } else {
            CapabilityScope::resources(resources)
                .unwrap_or_else(|error| panic!("invalid built-in capability scope: {error}"))
        }
    }

    async fn execute(
        &self,
        context: AuthorizedCapabilityContext,
        input: Self::Input,
    ) -> Result<Self::Output, CapabilityExecutionError> {
        let (page, per_page) = bounded_page(input.page, input.per_page);
        let (employees, total) = EmployeeOps::list(
            &self.pool,
            context.principal().tenant_id(),
            page,
            per_page,
            trimmed(input.search.as_deref()),
            input.status.map(EmploymentStatus::as_str),
            input.department_id,
            input.position_id,
            input.account_linked,
        )
        .await
        .map_err(|_| dependency_failure("Employees could not be loaded."))?;
        Ok(ListEmployeesOutput {
            employees: PaginatedEmployeesResponse {
                employees: employees.into_iter().map(EmployeeResponse::from).collect(),
            },
            pagination: PaginationMeta::new(page as u32, per_page as u32, total),
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ReadEmployeeInput {
    employee_id: Uuid,
}

#[derive(Serialize)]
pub(super) struct ReadEmployeeOutput {
    employee: EmployeeResponse,
}

pub(super) struct HrEmployeeReadCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}

impl HrEmployeeReadCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "hr_payroll.employees.read",
                "Read employee",
                "Returns one canonical tenant employee record by its stable identifier.",
                json!({ "employee_id": { "type": "string", "format": "uuid" } }),
                json!({ "employee": { "type": "object" } }),
                DataSensitivity::Personal,
                "hr_payroll.employees",
            ),
        }
    }
}

#[async_trait]
impl Capability for HrEmployeeReadCapability {
    type Input = ReadEmployeeInput;
    type Output = ReadEmployeeOutput;

    fn descriptor(&self) -> &CapabilityDescriptor {
        &self.descriptor
    }

    fn scope(&self, input: &Self::Input) -> CapabilityScope {
        resource_scope("employee", input.employee_id)
    }

    async fn execute(
        &self,
        context: AuthorizedCapabilityContext,
        input: Self::Input,
    ) -> Result<Self::Output, CapabilityExecutionError> {
        let employee = EmployeeOps::get_by_id(
            &self.pool,
            context.principal().tenant_id(),
            input.employee_id,
        )
        .await
        .map_err(|_| dependency_failure("The employee could not be loaded."))?
        .ok_or_else(|| not_found("The employee was not found."))?;
        Ok(ReadEmployeeOutput {
            employee: EmployeeResponse::from(employee),
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct EmploymentEngagementListInput {
    page: Option<i64>,
    per_page: Option<i64>,
    search: Option<String>,
    employee_id: Option<Uuid>,
    status: Option<EngagementStatus>,
    employment_type: Option<EmploymentType>,
}

#[derive(Serialize)]
pub(super) struct ListEmploymentEngagementsOutput {
    employment_engagements: PaginatedEmploymentEngagementsResponse,
    pagination: PaginationMeta,
}

pub(super) struct HrEmploymentEngagementsListCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}

impl HrEmploymentEngagementsListCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "hr_payroll.employment_engagements.list",
                "List employment engagements",
                "Returns dated employment history for authorized tenant employees.",
                json!({
                    "page": { "type": ["integer", "null"], "minimum": 1 },
                    "per_page": { "type": ["integer", "null"], "minimum": 1, "maximum": 100 },
                    "search": { "type": ["string", "null"], "maxLength": 200 },
                    "employee_id": { "type": ["string", "null"], "format": "uuid" },
                    "status": { "type": ["string", "null"], "enum": ["draft", "active", "ended", "cancelled", null] },
                    "employment_type": { "type": ["string", "null"], "enum": ["permanent", "fixed_term", "temporary", "casual", "contractor", "intern", null] }
                }),
                json!({
                    "employment_engagements": { "type": "object" },
                    "pagination": { "type": "object" }
                }),
                DataSensitivity::Personal,
                "hr_payroll.employment_engagements",
            ),
        }
    }
}

#[async_trait]
impl Capability for HrEmploymentEngagementsListCapability {
    type Input = EmploymentEngagementListInput;
    type Output = ListEmploymentEngagementsOutput;

    fn descriptor(&self) -> &CapabilityDescriptor {
        &self.descriptor
    }

    fn scope(&self, input: &Self::Input) -> CapabilityScope {
        input.employee_id.map_or(CapabilityScope::TenantWide, |id| {
            resource_scope("employee", id)
        })
    }

    async fn execute(
        &self,
        context: AuthorizedCapabilityContext,
        input: Self::Input,
    ) -> Result<Self::Output, CapabilityExecutionError> {
        let (page, per_page) = bounded_page(input.page, input.per_page);
        let (records, total) = EmploymentEngagementOps::list(
            &self.pool,
            context.principal().tenant_id(),
            page,
            per_page,
            trimmed(input.search.as_deref()),
            input.employee_id,
            input.status.map(EngagementStatus::as_str),
            input.employment_type.map(EmploymentType::as_str),
        )
        .await
        .map_err(|_| dependency_failure("Employment engagements could not be loaded."))?;
        Ok(ListEmploymentEngagementsOutput {
            employment_engagements: PaginatedEmploymentEngagementsResponse {
                employment_engagements: records
                    .into_iter()
                    .map(EmploymentEngagementResponse::from)
                    .collect(),
            },
            pagination: PaginationMeta::new(page as u32, per_page as u32, total),
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ReadEmploymentEngagementInput {
    employment_engagement_id: Uuid,
}

#[derive(Serialize)]
pub(super) struct ReadEmploymentEngagementOutput {
    employment_engagement: EmploymentEngagementResponse,
}

pub(super) struct HrEmploymentEngagementReadCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}

impl HrEmploymentEngagementReadCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "hr_payroll.employment_engagements.read",
                "Read employment engagement",
                "Returns one dated employment engagement by its stable identifier.",
                json!({ "employment_engagement_id": { "type": "string", "format": "uuid" } }),
                json!({ "employment_engagement": { "type": "object" } }),
                DataSensitivity::Personal,
                "hr_payroll.employment_engagements",
            ),
        }
    }
}

#[async_trait]
impl Capability for HrEmploymentEngagementReadCapability {
    type Input = ReadEmploymentEngagementInput;
    type Output = ReadEmploymentEngagementOutput;

    fn descriptor(&self) -> &CapabilityDescriptor {
        &self.descriptor
    }

    fn scope(&self, input: &Self::Input) -> CapabilityScope {
        resource_scope("employment_engagement", input.employment_engagement_id)
    }

    async fn execute(
        &self,
        context: AuthorizedCapabilityContext,
        input: Self::Input,
    ) -> Result<Self::Output, CapabilityExecutionError> {
        let record = EmploymentEngagementOps::get_by_id(
            &self.pool,
            context.principal().tenant_id(),
            input.employment_engagement_id,
        )
        .await
        .map_err(|_| dependency_failure("The employment engagement could not be loaded."))?
        .ok_or_else(|| not_found("The employment engagement was not found."))?;
        Ok(ReadEmploymentEngagementOutput {
            employment_engagement: EmploymentEngagementResponse::from(record),
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct EmployeeAvailabilityListInput {
    page: Option<i64>,
    per_page: Option<i64>,
    search: Option<String>,
    employee_id: Option<Uuid>,
    status: Option<AvailabilityStatus>,
    kind: Option<AvailabilityKind>,
    from: Option<DateTime<Utc>>,
    to: Option<DateTime<Utc>>,
}

#[derive(Serialize)]
pub(super) struct ListEmployeeAvailabilityOutput {
    availability: PaginatedEmployeeAvailabilityResponse,
    pagination: PaginationMeta,
}

pub(super) struct HrEmployeeAvailabilityListCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}

impl HrEmployeeAvailabilityListCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "hr_payroll.availability.list",
                "List employee availability",
                "Returns authorized employee availability periods for workforce scheduling.",
                json!({
                    "page": { "type": ["integer", "null"], "minimum": 1 },
                    "per_page": { "type": ["integer", "null"], "minimum": 1, "maximum": 100 },
                    "search": { "type": ["string", "null"], "maxLength": 200 },
                    "employee_id": { "type": ["string", "null"], "format": "uuid" },
                    "status": { "type": ["string", "null"], "enum": ["draft", "submitted", "approved", "rejected", "cancelled", null] },
                    "kind": { "type": ["string", "null"], "enum": ["leave", "training", "medical", "personal", "other", null] },
                    "from": { "type": ["string", "null"], "format": "date-time" },
                    "to": { "type": ["string", "null"], "format": "date-time" }
                }),
                json!({
                    "availability": { "type": "object" },
                    "pagination": { "type": "object" }
                }),
                DataSensitivity::Sensitive,
                "hr_payroll.availability",
            ),
        }
    }
}

#[async_trait]
impl Capability for HrEmployeeAvailabilityListCapability {
    type Input = EmployeeAvailabilityListInput;
    type Output = ListEmployeeAvailabilityOutput;

    fn descriptor(&self) -> &CapabilityDescriptor {
        &self.descriptor
    }

    fn scope(&self, input: &Self::Input) -> CapabilityScope {
        input.employee_id.map_or(CapabilityScope::TenantWide, |id| {
            resource_scope("employee", id)
        })
    }

    async fn execute(
        &self,
        context: AuthorizedCapabilityContext,
        input: Self::Input,
    ) -> Result<Self::Output, CapabilityExecutionError> {
        let (page, per_page) = bounded_page(input.page, input.per_page);
        let (records, total) = EmployeeAvailabilityOps::list(
            &self.pool,
            context.principal().tenant_id(),
            page,
            per_page,
            trimmed(input.search.as_deref()),
            input.employee_id,
            input.status.map(AvailabilityStatus::as_str),
            input.kind.map(AvailabilityKind::as_str),
            input.from,
            input.to,
        )
        .await
        .map_err(|_| dependency_failure("Employee availability could not be loaded."))?;
        Ok(ListEmployeeAvailabilityOutput {
            availability: PaginatedEmployeeAvailabilityResponse {
                availability_periods: records
                    .into_iter()
                    .map(EmployeeAvailabilityResponse::from)
                    .collect(),
            },
            pagination: PaginationMeta::new(page as u32, per_page as u32, total),
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ReadEmployeeAvailabilityInput {
    availability_id: Uuid,
}

#[derive(Serialize)]
pub(super) struct ReadEmployeeAvailabilityOutput {
    availability: EmployeeAvailabilityResponse,
}

pub(super) struct HrEmployeeAvailabilityReadCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}

impl HrEmployeeAvailabilityReadCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "hr_payroll.availability.read",
                "Read employee availability",
                "Returns one employee availability period by its stable identifier.",
                json!({ "availability_id": { "type": "string", "format": "uuid" } }),
                json!({ "availability": { "type": "object" } }),
                DataSensitivity::Sensitive,
                "hr_payroll.availability",
            ),
        }
    }
}

#[async_trait]
impl Capability for HrEmployeeAvailabilityReadCapability {
    type Input = ReadEmployeeAvailabilityInput;
    type Output = ReadEmployeeAvailabilityOutput;

    fn descriptor(&self) -> &CapabilityDescriptor {
        &self.descriptor
    }

    fn scope(&self, input: &Self::Input) -> CapabilityScope {
        resource_scope("employee_availability", input.availability_id)
    }

    async fn execute(
        &self,
        context: AuthorizedCapabilityContext,
        input: Self::Input,
    ) -> Result<Self::Output, CapabilityExecutionError> {
        let record = EmployeeAvailabilityOps::get_by_id(
            &self.pool,
            context.principal().tenant_id(),
            input.availability_id,
        )
        .await
        .map_err(|_| dependency_failure("The employee availability period could not be loaded."))?
        .ok_or_else(|| not_found("The employee availability period was not found."))?;
        Ok(ReadEmployeeAvailabilityOutput {
            availability: EmployeeAvailabilityResponse::from(record),
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct HrImportsListInput {
    page: Option<i64>,
    per_page: Option<i64>,
}

pub(super) struct HrImportsListCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}

impl HrImportsListCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "hr_payroll.imports.list",
                "List employee imports",
                "Returns employee import metadata and validation or commit totals without source bytes.",
                json!({
                    "page": { "type": ["integer", "null"], "minimum": 1 },
                    "per_page": { "type": ["integer", "null"], "minimum": 1, "maximum": 100 }
                }),
                json!({ "imports": { "type": "array" }, "pagination": { "type": "object" } }),
                DataSensitivity::Personal,
                "hr_payroll.imports",
            ),
        }
    }
}

#[async_trait]
impl Capability for HrImportsListCapability {
    type Input = HrImportsListInput;
    type Output = Value;

    fn descriptor(&self) -> &CapabilityDescriptor {
        &self.descriptor
    }

    fn scope(&self, _input: &Self::Input) -> CapabilityScope {
        CapabilityScope::TenantWide
    }

    async fn execute(
        &self,
        context: AuthorizedCapabilityContext,
        input: Self::Input,
    ) -> Result<Self::Output, CapabilityExecutionError> {
        let (page, per_page) = bounded_page(input.page, input.per_page);
        let (imports, total) =
            HrImportOps::list(&self.pool, context.principal().tenant_id(), page, per_page)
                .await
                .map_err(|_| dependency_failure("Employee imports could not be loaded."))?;
        Ok(json!({
            "imports": imports,
            "pagination": PaginationMeta::new(page as u32, per_page as u32, total)
        }))
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct HrImportReadInput {
    import_id: Uuid,
}

pub(super) struct HrImportReadCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}

impl HrImportReadCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "hr_payroll.imports.read",
                "Read employee import",
                "Returns one employee import and its latest totals without source bytes.",
                json!({ "import_id": { "type": "string", "format": "uuid" } }),
                json!({ "import": { "type": "object" } }),
                DataSensitivity::Personal,
                "hr_payroll.imports",
            ),
        }
    }
}

#[async_trait]
impl Capability for HrImportReadCapability {
    type Input = HrImportReadInput;
    type Output = Value;

    fn descriptor(&self) -> &CapabilityDescriptor {
        &self.descriptor
    }

    fn scope(&self, input: &Self::Input) -> CapabilityScope {
        resource_scope("data_import", input.import_id)
    }

    async fn execute(
        &self,
        context: AuthorizedCapabilityContext,
        input: Self::Input,
    ) -> Result<Self::Output, CapabilityExecutionError> {
        let record = HrImportOps::get(&self.pool, context.principal().tenant_id(), input.import_id)
            .await
            .map_err(|_| dependency_failure("The employee import could not be loaded."))?
            .ok_or_else(|| not_found("The employee import was not found."))?;
        Ok(json!({ "import": record }))
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct HrImportPreviewInput {
    import_id: Uuid,
    page: Option<i64>,
    per_page: Option<i64>,
}

pub(super) struct HrImportPreviewCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}

impl HrImportPreviewCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "hr_payroll.imports.preview.read",
                "Read employee import preview",
                "Returns bounded validated employee rows and issues. The retained source file and unmapped columns are never returned.",
                json!({
                    "import_id": { "type": "string", "format": "uuid" },
                    "page": { "type": ["integer", "null"], "minimum": 1 },
                    "per_page": { "type": ["integer", "null"], "minimum": 1, "maximum": 100 }
                }),
                json!({ "preview": { "type": "object" } }),
                DataSensitivity::Sensitive,
                "hr_payroll.import_previews",
            ),
        }
    }
}

#[async_trait]
impl Capability for HrImportPreviewCapability {
    type Input = HrImportPreviewInput;
    type Output = Value;

    fn descriptor(&self) -> &CapabilityDescriptor {
        &self.descriptor
    }

    fn scope(&self, input: &Self::Input) -> CapabilityScope {
        resource_scope("data_import", input.import_id)
    }

    async fn execute(
        &self,
        context: AuthorizedCapabilityContext,
        input: Self::Input,
    ) -> Result<Self::Output, CapabilityExecutionError> {
        let (page, per_page) = bounded_page(input.page, input.per_page);
        let preview = HrImportOps::preview(
            &self.pool,
            context.principal().tenant_id(),
            input.import_id,
            page,
            per_page,
        )
        .await
        .map_err(|_| dependency_failure("The employee import preview could not be loaded."))?
        .ok_or_else(|| not_found("The employee import preview was not found."))?;
        Ok(json!({ "preview": preview }))
    }
}

fn directory_list_descriptor(
    key: &str,
    title: &str,
    description: &str,
    result_key: &str,
    usage_tag: &str,
) -> CapabilityDescriptor {
    read_descriptor(
        key,
        title,
        description,
        json!({
            "page": { "type": ["integer", "null"], "minimum": 1 },
            "per_page": { "type": ["integer", "null"], "minimum": 1, "maximum": 100 },
            "search": { "type": ["string", "null"], "maxLength": 200 },
            "status": { "type": ["string", "null"], "enum": ["active", "inactive", null] }
        }),
        json!({
            result_key: { "type": "object" },
            "pagination": { "type": "object" }
        }),
        DataSensitivity::General,
        usage_tag,
    )
}

fn bounded_page(page: Option<i64>, per_page: Option<i64>) -> (i64, i64) {
    (
        page.unwrap_or(1).max(1),
        per_page.unwrap_or(20).clamp(1, 100),
    )
}

fn trimmed(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn resource(kind: &str, id: Uuid) -> CapabilityResource {
    CapabilityResource::parse(kind, id.to_string())
        .unwrap_or_else(|error| panic!("invalid built-in capability resource: {error}"))
}

fn resource_scope(kind: &str, id: Uuid) -> CapabilityScope {
    CapabilityScope::resources([resource(kind, id)])
        .unwrap_or_else(|error| panic!("invalid built-in capability scope: {error}"))
}

fn dependency_failure(message: &'static str) -> CapabilityExecutionError {
    CapabilityExecutionError::new(CapabilityExecutionErrorCode::DependencyUnavailable, message)
}

fn not_found(message: &'static str) -> CapabilityExecutionError {
    CapabilityExecutionError::new(CapabilityExecutionErrorCode::InvalidState, message)
}

#[cfg(test)]
mod tests {
    use super::{bounded_page, trimmed};

    #[test]
    fn hr_filters_are_bounded_and_blank_values_are_ignored() {
        assert_eq!(bounded_page(Some(0), Some(200)), (1, 100));
        assert_eq!(bounded_page(None, None), (1, 20));
        assert_eq!(trimmed(Some("  teaching  ")), Some("teaching"));
        assert_eq!(trimmed(Some("   ")), None);
    }
}
