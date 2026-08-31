//! Defines provider data-eligibility classes shared by Agent and provider adapters.
//!
//! The evaluator is the single fail-closed matrix for campus approval and
//! execution-environment eligibility. It grants no product or record authority.

use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Minimum provider handling class required by an Agent turn or capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderDataClass {
    CampusApproved,
    SensitiveDataApproved,
    LocalOnly,
}

impl ProviderDataClass {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CampusApproved => "campus_approved",
            Self::SensitiveDataApproved => "sensitive_data_approved",
            Self::LocalOnly => "local_only",
        }
    }

    /// Returns the stricter of two requirements.
    #[must_use]
    pub const fn max(self, other: Self) -> Self {
        if self.rank() >= other.rank() {
            self
        } else {
            other
        }
    }

    const fn rank(self) -> u8 {
        match self {
            Self::CampusApproved => 1,
            Self::SensitiveDataApproved => 2,
            Self::LocalOnly => 3,
        }
    }
}

impl fmt::Display for ProviderDataClass {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for ProviderDataClass {
    type Err = ProviderDataClassParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim() {
            "campus_approved" => Ok(Self::CampusApproved),
            "sensitive_data_approved" => Ok(Self::SensitiveDataApproved),
            "local_only" => Ok(Self::LocalOnly),
            _ => Err(ProviderDataClassParseError),
        }
    }
}

/// Versioned campus approval assigned to one provider connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderApprovalClass {
    Unapproved,
    CampusApproved,
    SensitiveDataApproved,
}

impl ProviderApprovalClass {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unapproved => "unapproved",
            Self::CampusApproved => "campus_approved",
            Self::SensitiveDataApproved => "sensitive_data_approved",
        }
    }
}

impl fmt::Display for ProviderApprovalClass {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for ProviderApprovalClass {
    type Err = ProviderApprovalClassParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim() {
            "unapproved" => Ok(Self::Unapproved),
            "campus_approved" => Ok(Self::CampusApproved),
            "sensitive_data_approved" => Ok(Self::SensitiveDataApproved),
            _ => Err(ProviderApprovalClassParseError),
        }
    }
}

/// Trust boundary of the adapter that will receive provider input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderExecutionEnvironmentClass {
    ExternalManaged,
    InstallationLocal,
}

impl ProviderExecutionEnvironmentClass {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ExternalManaged => "external_managed",
            Self::InstallationLocal => "installation_local",
        }
    }
}

impl fmt::Display for ProviderExecutionEnvironmentClass {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for ProviderExecutionEnvironmentClass {
    type Err = ProviderExecutionEnvironmentClassParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim() {
            "external_managed" => Ok(Self::ExternalManaged),
            "installation_local" => Ok(Self::InstallationLocal),
            _ => Err(ProviderExecutionEnvironmentClassParseError),
        }
    }
}

/// Stable, safe denial returned by the canonical eligibility matrix.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum ProviderDataEligibilityError {
    #[error("provider connection is not approved for the required data class")]
    ProviderDataNotApproved,
    #[error("the required data class needs an installation-local provider")]
    LocalExecutionRequired,
}

/// Evaluates approval and environment without decrypting credentials or sending data.
pub const fn evaluate_provider_data_eligibility(
    required: ProviderDataClass,
    approval: ProviderApprovalClass,
    environment: ProviderExecutionEnvironmentClass,
) -> Result<(), ProviderDataEligibilityError> {
    match required {
        ProviderDataClass::CampusApproved => match approval {
            ProviderApprovalClass::CampusApproved
            | ProviderApprovalClass::SensitiveDataApproved => Ok(()),
            ProviderApprovalClass::Unapproved => {
                Err(ProviderDataEligibilityError::ProviderDataNotApproved)
            }
        },
        ProviderDataClass::SensitiveDataApproved => match approval {
            ProviderApprovalClass::SensitiveDataApproved => Ok(()),
            ProviderApprovalClass::Unapproved | ProviderApprovalClass::CampusApproved => {
                Err(ProviderDataEligibilityError::ProviderDataNotApproved)
            }
        },
        ProviderDataClass::LocalOnly => {
            if !matches!(approval, ProviderApprovalClass::SensitiveDataApproved) {
                return Err(ProviderDataEligibilityError::ProviderDataNotApproved);
            }
            if !matches!(
                environment,
                ProviderExecutionEnvironmentClass::InstallationLocal
            ) {
                return Err(ProviderDataEligibilityError::LocalExecutionRequired);
            }
            Ok(())
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("invalid provider data class")]
pub struct ProviderDataClassParseError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("invalid provider approval class")]
pub struct ProviderApprovalClassParseError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("invalid provider execution environment class")]
pub struct ProviderExecutionEnvironmentClassParseError;

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::{
        ProviderApprovalClass, ProviderDataClass, ProviderDataEligibilityError,
        ProviderExecutionEnvironmentClass, evaluate_provider_data_eligibility,
    };

    #[test]
    fn canonical_matrix_covers_every_combination() {
        use ProviderApprovalClass::{
            CampusApproved as ApprovalCampus, SensitiveDataApproved as ApprovalSensitive,
            Unapproved,
        };
        use ProviderDataClass::{CampusApproved, LocalOnly, SensitiveDataApproved};
        use ProviderExecutionEnvironmentClass::{ExternalManaged, InstallationLocal};

        let cases = [
            (CampusApproved, Unapproved, ExternalManaged, false),
            (CampusApproved, Unapproved, InstallationLocal, false),
            (CampusApproved, ApprovalCampus, ExternalManaged, true),
            (CampusApproved, ApprovalCampus, InstallationLocal, true),
            (CampusApproved, ApprovalSensitive, ExternalManaged, true),
            (CampusApproved, ApprovalSensitive, InstallationLocal, true),
            (SensitiveDataApproved, Unapproved, ExternalManaged, false),
            (SensitiveDataApproved, Unapproved, InstallationLocal, false),
            (
                SensitiveDataApproved,
                ApprovalCampus,
                ExternalManaged,
                false,
            ),
            (
                SensitiveDataApproved,
                ApprovalCampus,
                InstallationLocal,
                false,
            ),
            (
                SensitiveDataApproved,
                ApprovalSensitive,
                ExternalManaged,
                true,
            ),
            (
                SensitiveDataApproved,
                ApprovalSensitive,
                InstallationLocal,
                true,
            ),
            (LocalOnly, Unapproved, ExternalManaged, false),
            (LocalOnly, Unapproved, InstallationLocal, false),
            (LocalOnly, ApprovalCampus, ExternalManaged, false),
            (LocalOnly, ApprovalCampus, InstallationLocal, false),
            (LocalOnly, ApprovalSensitive, ExternalManaged, false),
            (LocalOnly, ApprovalSensitive, InstallationLocal, true),
        ];
        for (required, approval, environment, allowed) in cases {
            assert_eq!(
                evaluate_provider_data_eligibility(required, approval, environment).is_ok(),
                allowed,
                "{required:?} {approval:?} {environment:?}"
            );
        }
        assert_eq!(
            evaluate_provider_data_eligibility(LocalOnly, ApprovalSensitive, ExternalManaged),
            Err(ProviderDataEligibilityError::LocalExecutionRequired)
        );
    }

    #[test]
    fn wire_values_parse_and_stricter_requirement_wins() {
        assert_eq!(
            ProviderDataClass::from_str("sensitive_data_approved"),
            Ok(ProviderDataClass::SensitiveDataApproved)
        );
        assert_eq!(
            ProviderApprovalClass::from_str("unapproved"),
            Ok(ProviderApprovalClass::Unapproved)
        );
        assert_eq!(
            ProviderExecutionEnvironmentClass::from_str("installation_local"),
            Ok(ProviderExecutionEnvironmentClass::InstallationLocal)
        );
        assert!(ProviderDataClass::from_str("unknown").is_err());
        assert!(ProviderApprovalClass::from_str("unknown").is_err());
        assert!(ProviderExecutionEnvironmentClass::from_str("unknown").is_err());
        assert_eq!(
            ProviderDataClass::CampusApproved.max(ProviderDataClass::LocalOnly),
            ProviderDataClass::LocalOnly
        );
        assert_eq!(
            ProviderDataClass::SensitiveDataApproved.max(ProviderDataClass::CampusApproved),
            ProviderDataClass::SensitiveDataApproved
        );
    }
}
