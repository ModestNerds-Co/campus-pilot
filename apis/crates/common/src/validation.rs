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
