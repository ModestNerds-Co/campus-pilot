//! Canonicalizes Agent capability routing selectors from code-owned metadata.

use cp_agent::CapabilityRegistry;
use cp_common::{OperationEffect, operation_catalog};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct RoutingCapabilityOption {
    pub capability_key: String,
    pub label: String,
    pub module_key: String,
    pub operation_class: String,
    pub capability_version: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SelectorError {
    UnknownCapability,
    CapabilityModuleMismatch,
    CapabilityOperationMismatch,
}

impl SelectorError {
    pub(crate) const fn code(self) -> &'static str {
        match self {
            Self::UnknownCapability => "unknown_capability",
            Self::CapabilityModuleMismatch => "capability_module_mismatch",
            Self::CapabilityOperationMismatch => "capability_operation_mismatch",
        }
    }

    pub(crate) const fn safe_message(self) -> &'static str {
        match self {
            Self::UnknownCapability => "Choose a currently executable Agent capability and version",
            Self::CapabilityModuleMismatch => "The module must match the selected Agent capability",
            Self::CapabilityOperationMismatch => {
                "The operation class must match the selected Agent capability"
            }
        }
    }
}

pub(crate) fn routing_capability_options(
    registry: &CapabilityRegistry,
) -> Vec<RoutingCapabilityOption> {
    let mut options = registry
        .descriptors()
        .into_iter()
        .filter_map(|descriptor| {
            routing_capability_option(
                descriptor.key().as_str(),
                i32::from(descriptor.version().get()),
                descriptor.title(),
            )
        })
        .collect::<Vec<_>>();
    sort_capability_options(&mut options);
    options
}

pub(crate) fn routing_capability_option(
    capability_key: &str,
    capability_version: i32,
    label: &str,
) -> Option<RoutingCapabilityOption> {
    let operation = operation_catalog().iter().find_map(|entry| {
        (entry.operation().key() == capability_key).then_some(entry.operation())
    })?;
    let operation_class = routing_operation_class(operation.effect())?;
    Some(RoutingCapabilityOption {
        capability_key: capability_key.to_owned(),
        label: label.to_owned(),
        module_key: operation.module_key().to_owned(),
        operation_class: operation_class.to_owned(),
        capability_version,
    })
}

pub(crate) fn sort_capability_options(options: &mut [RoutingCapabilityOption]) {
    options.sort_by(|left, right| {
        left.module_key
            .cmp(&right.module_key)
            .then_with(|| left.label.cmp(&right.label))
            .then_with(|| left.capability_key.cmp(&right.capability_key))
            .then_with(|| left.capability_version.cmp(&right.capability_version))
    });
}

pub(crate) fn capability_option(
    options: &[RoutingCapabilityOption],
    capability_key: &str,
    capability_version: i32,
) -> Result<RoutingCapabilityOption, SelectorError> {
    options
        .iter()
        .find(|option| {
            option.capability_key == capability_key
                && option.capability_version == capability_version
        })
        .cloned()
        .ok_or(SelectorError::UnknownCapability)
}

pub(crate) fn canonicalize_capability_selectors(
    options: &[RoutingCapabilityOption],
    module_key: Option<&str>,
    operation_class: Option<&str>,
    capability_key: &str,
    capability_version: i32,
) -> Result<(String, String), SelectorError> {
    let capability = capability_option(options, capability_key, capability_version)?;
    if module_key.is_some_and(|module_key| module_key != capability.module_key) {
        return Err(SelectorError::CapabilityModuleMismatch);
    }
    if operation_class.is_some_and(|operation_class| operation_class != capability.operation_class)
    {
        return Err(SelectorError::CapabilityOperationMismatch);
    }
    Ok((capability.module_key, capability.operation_class))
}

const fn routing_operation_class(effect: OperationEffect) -> Option<&'static str> {
    match effect {
        OperationEffect::Read | OperationEffect::Export => Some("read"),
        OperationEffect::LicenseRepair
        | OperationEffect::Write
        | OperationEffect::Destructive
        | OperationEffect::External => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn option() -> RoutingCapabilityOption {
        RoutingCapabilityOption {
            capability_key: "finance.journals.list".to_owned(),
            label: "List finance journals".to_owned(),
            module_key: "finance".to_owned(),
            operation_class: "read".to_owned(),
            capability_version: 1,
        }
    }

    #[test]
    fn capability_selectors_inject_canonical_module_and_operation() {
        assert_eq!(
            canonicalize_capability_selectors(&[option()], None, None, "finance.journals.list", 1,),
            Ok(("finance".to_owned(), "read".to_owned()))
        );
    }

    #[test]
    fn capability_selectors_reject_module_and_operation_mismatches() {
        assert_eq!(
            canonicalize_capability_selectors(
                &[option()],
                Some("sis"),
                Some("read"),
                "finance.journals.list",
                1,
            ),
            Err(SelectorError::CapabilityModuleMismatch)
        );
        assert_eq!(
            canonicalize_capability_selectors(
                &[option()],
                Some("finance"),
                Some("mutate"),
                "finance.journals.list",
                1,
            ),
            Err(SelectorError::CapabilityOperationMismatch)
        );
    }
}
