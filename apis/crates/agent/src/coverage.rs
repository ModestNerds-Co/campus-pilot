//! Joins module delivery, licensing, product operations, and executable Agent capabilities.
//!
//! This registry is diagnostic product truth. It does not grant access or make
//! a capability executable; the broker remains the only execution boundary.

use std::collections::{BTreeMap, BTreeSet};

use cp_common::{AgentExposure, ProductOperation};
use serde::Serialize;
use thiserror::Error;

use crate::CapabilityRegistry;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ModuleDeliveryStage {
    Available,
    Foundation,
    Planned,
}

impl TryFrom<&str> for ModuleDeliveryStage {
    type Error = ModuleCoverageError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "available" => Ok(Self::Available),
            "foundation" => Ok(Self::Foundation),
            "planned" => Ok(Self::Planned),
            _ => Err(ModuleCoverageError::UnknownStage(value.to_string())),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleCoverageSource {
    key: String,
    stage: ModuleDeliveryStage,
    core: bool,
    workspace_route: String,
}

impl ModuleCoverageSource {
    pub fn parse(
        key: impl Into<String>,
        stage: &str,
        core: bool,
        workspace_route: impl Into<String>,
    ) -> Result<Self, ModuleCoverageError> {
        let key = key.into();
        let workspace_route = workspace_route.into();
        if key.trim().is_empty() {
            return Err(ModuleCoverageError::EmptyModuleKey);
        }
        if workspace_route.trim().is_empty() {
            return Err(ModuleCoverageError::EmptyWorkspaceRoute(key));
        }
        Ok(Self {
            key,
            stage: ModuleDeliveryStage::try_from(stage)?,
            core,
            workspace_route,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModuleCoverage {
    module_key: String,
    stage: ModuleDeliveryStage,
    core: bool,
    workspace_route: String,
    routed_operations: usize,
    exposed_operations: usize,
    approval_required_operations: usize,
    human_only_operations: usize,
    prohibited_operations: usize,
    executable_capabilities: usize,
    missing_executable_capabilities: Vec<String>,
    stage_aligned: bool,
    licensing_aligned: bool,
    release_ready: bool,
}

impl ModuleCoverage {
    #[must_use]
    pub fn module_key(&self) -> &str {
        &self.module_key
    }

    #[must_use]
    pub const fn stage(&self) -> ModuleDeliveryStage {
        self.stage
    }

    #[must_use]
    pub const fn core(&self) -> bool {
        self.core
    }

    #[must_use]
    pub fn workspace_route(&self) -> &str {
        &self.workspace_route
    }

    #[must_use]
    pub const fn routed_operations(&self) -> usize {
        self.routed_operations
    }

    #[must_use]
    pub const fn exposed_operations(&self) -> usize {
        self.exposed_operations
    }

    #[must_use]
    pub const fn approval_required_operations(&self) -> usize {
        self.approval_required_operations
    }

    #[must_use]
    pub const fn human_only_operations(&self) -> usize {
        self.human_only_operations
    }

    #[must_use]
    pub const fn prohibited_operations(&self) -> usize {
        self.prohibited_operations
    }

    #[must_use]
    pub const fn executable_capabilities(&self) -> usize {
        self.executable_capabilities
    }

    #[must_use]
    pub fn missing_executable_capabilities(&self) -> &[String] {
        &self.missing_executable_capabilities
    }

    #[must_use]
    pub const fn stage_aligned(&self) -> bool {
        self.stage_aligned
    }

    #[must_use]
    pub const fn licensing_aligned(&self) -> bool {
        self.licensing_aligned
    }

    #[must_use]
    pub const fn release_ready(&self) -> bool {
        self.release_ready
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleCoverageRegistry {
    entries: Vec<ModuleCoverage>,
}

impl ModuleCoverageRegistry {
    pub fn build(
        modules: impl IntoIterator<Item = ModuleCoverageSource>,
        operations: impl IntoIterator<Item = ProductOperation>,
        capabilities: &CapabilityRegistry,
    ) -> Result<Self, ModuleCoverageError> {
        let capability_operations = capabilities
            .descriptors()
            .into_iter()
            .map(|descriptor| descriptor.operation_key().as_str().to_string())
            .collect::<BTreeSet<_>>();
        Self::build_with_executable_operations(modules, operations, capability_operations)
    }

    fn build_with_executable_operations(
        modules: impl IntoIterator<Item = ModuleCoverageSource>,
        operations: impl IntoIterator<Item = ProductOperation>,
        capability_operations: BTreeSet<String>,
    ) -> Result<Self, ModuleCoverageError> {
        let mut accumulators = BTreeMap::new();
        for module in modules {
            let key = module.key.clone();
            if accumulators
                .insert(key.clone(), CoverageAccumulator::new(module))
                .is_some()
            {
                return Err(ModuleCoverageError::DuplicateModule(key));
            }
        }

        for operation in operations {
            let module_key = operation.module_key().to_string();
            let Some(accumulator) = accumulators.get_mut(&module_key) else {
                return Err(ModuleCoverageError::UnknownOperationModule {
                    operation_key: operation.key().to_string(),
                    module_key,
                });
            };
            accumulator.add_operation(&operation, &capability_operations);
        }

        let entries = accumulators
            .into_values()
            .map(CoverageAccumulator::finish)
            .collect();
        Ok(Self { entries })
    }

    #[must_use]
    pub fn entries(&self) -> &[ModuleCoverage] {
        &self.entries
    }

    #[must_use]
    pub fn entry(&self, module_key: &str) -> Option<&ModuleCoverage> {
        self.entries
            .iter()
            .find(|entry| entry.module_key() == module_key)
    }

    #[must_use]
    pub fn missing_executable_capability_count(&self) -> usize {
        self.entries
            .iter()
            .map(|entry| entry.missing_executable_capabilities.len())
            .sum()
    }
}

struct CoverageAccumulator {
    source: ModuleCoverageSource,
    routed_operations: usize,
    exposed_operations: usize,
    approval_required_operations: usize,
    human_only_operations: usize,
    prohibited_operations: usize,
    executable_capabilities: usize,
    missing_executable_capabilities: Vec<String>,
    licensing_aligned: bool,
}

impl CoverageAccumulator {
    const fn new(source: ModuleCoverageSource) -> Self {
        Self {
            source,
            routed_operations: 0,
            exposed_operations: 0,
            approval_required_operations: 0,
            human_only_operations: 0,
            prohibited_operations: 0,
            executable_capabilities: 0,
            missing_executable_capabilities: Vec::new(),
            licensing_aligned: true,
        }
    }

    fn add_operation(
        &mut self,
        operation: &ProductOperation,
        capability_operations: &BTreeSet<String>,
    ) {
        self.routed_operations += 1;
        if operation.license_required() == self.source.core {
            self.licensing_aligned = false;
        }
        match operation.agent_exposure() {
            AgentExposure::Exposed => {
                self.exposed_operations += 1;
                if capability_operations.contains(operation.key()) {
                    self.executable_capabilities += 1;
                } else {
                    self.missing_executable_capabilities
                        .push(operation.key().to_string());
                }
            }
            AgentExposure::ApprovalRequired => self.approval_required_operations += 1,
            AgentExposure::HumanOnly { .. } => self.human_only_operations += 1,
            AgentExposure::Prohibited { .. } => self.prohibited_operations += 1,
        }
    }

    fn finish(mut self) -> ModuleCoverage {
        self.missing_executable_capabilities.sort();
        let stage_aligned =
            self.source.stage != ModuleDeliveryStage::Available || self.routed_operations > 0;
        let release_ready = self.source.stage == ModuleDeliveryStage::Available
            && stage_aligned
            && self.licensing_aligned
            && self.missing_executable_capabilities.is_empty();
        ModuleCoverage {
            module_key: self.source.key,
            stage: self.source.stage,
            core: self.source.core,
            workspace_route: self.source.workspace_route,
            routed_operations: self.routed_operations,
            exposed_operations: self.exposed_operations,
            approval_required_operations: self.approval_required_operations,
            human_only_operations: self.human_only_operations,
            prohibited_operations: self.prohibited_operations,
            executable_capabilities: self.executable_capabilities,
            missing_executable_capabilities: self.missing_executable_capabilities,
            stage_aligned,
            licensing_aligned: self.licensing_aligned,
            release_ready,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ModuleCoverageError {
    #[error("module key must not be empty")]
    EmptyModuleKey,
    #[error("module {0} must declare a workspace route")]
    EmptyWorkspaceRoute(String),
    #[error("unknown module delivery stage: {0}")]
    UnknownStage(String),
    #[error("duplicate module coverage source: {0}")]
    DuplicateModule(String),
    #[error("operation {operation_key} references unknown module {module_key}")]
    UnknownOperationModule {
        operation_key: String,
        module_key: String,
    },
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use cp_common::{AgentExposure, OperationEffect, ProductOperation};

    use crate::CapabilityRegistry;

    use super::{
        ModuleCoverageError, ModuleCoverageRegistry, ModuleCoverageSource, ModuleDeliveryStage,
    };

    fn operation(
        key: &str,
        module_key: &str,
        exposure: AgentExposure,
        license_required: bool,
    ) -> ProductOperation {
        ProductOperation::route(
            key,
            module_key,
            format!("{module_key}:view"),
            OperationEffect::Read,
            exposure,
            license_required,
        )
    }

    #[test]
    fn coverage_reports_stage_licensing_and_missing_capability_gaps() {
        let registry = CapabilityRegistry::from_operations([
            operation(
                "administration.catalog.read",
                "administration",
                AgentExposure::Exposed,
                false,
            ),
            operation("fleet.vehicles.list", "fleet", AgentExposure::Exposed, true),
        ])
        .unwrap_or_else(|_| unreachable!());
        let coverage = ModuleCoverageRegistry::build(
            [
                ModuleCoverageSource::parse("administration", "available", true, "/admin")
                    .unwrap_or_else(|_| unreachable!()),
                ModuleCoverageSource::parse("fleet", "available", false, "/modules/fleet")
                    .unwrap_or_else(|_| unreachable!()),
                ModuleCoverageSource::parse("sis", "foundation", false, "/modules/sis")
                    .unwrap_or_else(|_| unreachable!()),
            ],
            [
                operation(
                    "administration.catalog.read",
                    "administration",
                    AgentExposure::Exposed,
                    false,
                ),
                operation("fleet.vehicles.list", "fleet", AgentExposure::Exposed, true),
            ],
            &registry,
        )
        .unwrap_or_else(|_| unreachable!());

        let administration = coverage
            .entry("administration")
            .unwrap_or_else(|| unreachable!());
        assert_eq!(administration.stage(), ModuleDeliveryStage::Available);
        assert!(administration.core());
        assert_eq!(administration.workspace_route(), "/admin");
        assert_eq!(administration.routed_operations(), 1);
        assert_eq!(administration.exposed_operations(), 1);
        assert_eq!(administration.executable_capabilities(), 0);
        assert_eq!(
            administration.missing_executable_capabilities(),
            &["administration.catalog.read"]
        );
        assert!(administration.stage_aligned());
        assert!(administration.licensing_aligned());
        assert!(!administration.release_ready());

        let sis = coverage.entry("sis").unwrap_or_else(|| unreachable!());
        assert_eq!(sis.routed_operations(), 0);
        assert!(sis.stage_aligned());
        assert!(!sis.release_ready());
        assert_eq!(coverage.entries().len(), 3);
        assert_eq!(coverage.missing_executable_capability_count(), 2);
    }

    #[test]
    fn coverage_counts_registered_executable_operations() {
        let coverage = ModuleCoverageRegistry::build_with_executable_operations(
            [
                ModuleCoverageSource::parse("fleet", "available", false, "/modules/fleet")
                    .unwrap_or_else(|_| unreachable!()),
            ],
            [operation(
                "fleet.vehicles.list",
                "fleet",
                AgentExposure::Exposed,
                true,
            )],
            BTreeSet::from(["fleet.vehicles.list".to_string()]),
        )
        .unwrap_or_else(|_| unreachable!());
        let fleet = coverage.entry("fleet").unwrap_or_else(|| unreachable!());

        assert_eq!(fleet.executable_capabilities(), 1);
        assert!(fleet.missing_executable_capabilities().is_empty());
        assert!(fleet.release_ready());
    }

    #[test]
    fn coverage_counts_every_exposure_and_rejects_invalid_sources() {
        let registry = CapabilityRegistry::from_operations(Vec::<ProductOperation>::new())
            .unwrap_or_else(|_| unreachable!());
        let coverage = ModuleCoverageRegistry::build(
            [
                ModuleCoverageSource::parse("fleet", "available", false, "/modules/fleet")
                    .unwrap_or_else(|_| unreachable!()),
            ],
            [
                operation(
                    "fleet.read.one",
                    "fleet",
                    AgentExposure::ApprovalRequired,
                    false,
                ),
                operation(
                    "fleet.read.two",
                    "fleet",
                    AgentExposure::HumanOnly { reason: "human" },
                    true,
                ),
                operation(
                    "fleet.read.three",
                    "fleet",
                    AgentExposure::Prohibited {
                        reason: "prohibited",
                    },
                    true,
                ),
            ],
            &registry,
        )
        .unwrap_or_else(|_| unreachable!());
        let fleet = coverage.entry("fleet").unwrap_or_else(|| unreachable!());
        assert_eq!(fleet.approval_required_operations(), 1);
        assert_eq!(fleet.human_only_operations(), 1);
        assert_eq!(fleet.prohibited_operations(), 1);
        assert!(!fleet.licensing_aligned());

        assert_eq!(
            ModuleCoverageSource::parse("", "available", false, "/modules/fleet"),
            Err(ModuleCoverageError::EmptyModuleKey)
        );
        assert_eq!(
            ModuleCoverageSource::parse("fleet", "unknown", false, "/modules/fleet"),
            Err(ModuleCoverageError::UnknownStage("unknown".to_string()))
        );
        assert_eq!(
            ModuleCoverageSource::parse("fleet", "available", false, ""),
            Err(ModuleCoverageError::EmptyWorkspaceRoute(
                "fleet".to_string()
            ))
        );
    }

    #[test]
    fn coverage_rejects_duplicate_and_unknown_module_links() {
        let registry = CapabilityRegistry::from_operations(Vec::<ProductOperation>::new())
            .unwrap_or_else(|_| unreachable!());
        let module = ModuleCoverageSource::parse("fleet", "available", false, "/modules/fleet")
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(
            ModuleCoverageRegistry::build(
                [module.clone(), module],
                Vec::<ProductOperation>::new(),
                &registry,
            ),
            Err(ModuleCoverageError::DuplicateModule("fleet".to_string()))
        );
        assert_eq!(
            ModuleCoverageRegistry::build(
                [
                    ModuleCoverageSource::parse("fleet", "available", false, "/modules/fleet",)
                        .unwrap_or_else(|_| unreachable!())
                ],
                [operation(
                    "unknown.records.read",
                    "unknown",
                    AgentExposure::Exposed,
                    true,
                )],
                &registry,
            ),
            Err(ModuleCoverageError::UnknownOperationModule {
                operation_key: "unknown.records.read".to_string(),
                module_key: "unknown".to_string(),
            })
        );
    }
}
