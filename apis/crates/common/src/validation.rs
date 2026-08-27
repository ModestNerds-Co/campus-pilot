//
//  cp-common
//  validation.rs
//
//  Created by Ngonidzashe Mangudya on 2025/06/22.
//  Copyright (c) 2025 Codecraft Solutions. All rights reserved.
//

use validator::ValidationErrors;

pub fn flatten_validation_errors(errs: &ValidationErrors) -> Vec<String> {
    use validator::ValidationErrorsKind::*;
    let mut out = Vec::new();
    for (field, kind) in errs.errors() {
        match kind {
            Field(fe) => {
                for e in fe {
                    let msg = e
                        .message
                        .clone()
                        .unwrap_or_else(|| std::borrow::Cow::from(e.code.to_string()));
                    out.push(format!("{field}: {msg}"));
                }
            }
            Struct(se) => out.extend(
                flatten_validation_errors(se)
                    .into_iter()
                    .map(|m| format!("{field}.{m}")),
            ),
            List(map) => {
                for (idx, ve) in map {
                    out.extend(
                        flatten_validation_errors(ve)
                            .into_iter()
                            .map(|m| format!("{field}[{idx}].{m}")),
                    );
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use std::{borrow::Cow, collections::BTreeMap};

    use validator::{ValidationError, ValidationErrors, ValidationErrorsKind};

    use super::flatten_validation_errors;

    #[test]
    fn flattens_field_struct_and_list_errors() {
        let mut root = ValidationErrors::new();
        root.add(
            "email",
            ValidationError::new("email")
                .with_message(Cow::Borrowed("Enter a valid email address")),
        );
        root.add("name", ValidationError::new("length"));

        let mut profile = ValidationErrors::new();
        profile.add("phone", ValidationError::new("phone"));
        root.errors_mut().insert(
            Cow::Borrowed("profile"),
            ValidationErrorsKind::Struct(Box::new(profile)),
        );

        let mut first_member = ValidationErrors::new();
        first_member.add(
            "role",
            ValidationError::new("required").with_message(Cow::Borrowed("Select a role")),
        );
        let mut members = BTreeMap::new();
        members.insert(0, Box::new(first_member));
        root.errors_mut().insert(
            Cow::Borrowed("members"),
            ValidationErrorsKind::List(members),
        );

        let flattened = flatten_validation_errors(&root);
        assert_eq!(flattened.len(), 4);
        assert!(flattened.contains(&"email: Enter a valid email address".to_string()));
        assert!(flattened.contains(&"name: length".to_string()));
        assert!(flattened.contains(&"profile.phone: phone".to_string()));
        assert!(flattened.contains(&"members[0].role: Select a role".to_string()));
    }

    #[test]
    fn empty_errors_flatten_to_an_empty_list() {
        assert!(flatten_validation_errors(&ValidationErrors::new()).is_empty());
    }
}
