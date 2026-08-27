//! Adapts the canonical HR directory reads to typed Agent capabilities.

use async_trait::async_trait;
use cp_agent::{
    AuthorizedCapabilityContext, Capability, CapabilityDescriptor, CapabilityExecutionError,
    CapabilityExecutionErrorCode, CapabilityResource, CapabilityScope, DataSensitivity,
};
use cp_common::PaginationMeta;
use cp_hr_payroll::{
    dtos::{
        DepartmentResponse, DirectoryStatus, EmployeeResponse, EmploymentStatus,
        PaginatedDepartmentsResponse, PaginatedEmployeesResponse, PaginatedPositionsResponse,
        PositionResponse,
    },
    ops::{DepartmentOps, EmployeeOps, PositionOps},
};
use serde::{Deserialize, Serialize};
use serde_json::json;
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
