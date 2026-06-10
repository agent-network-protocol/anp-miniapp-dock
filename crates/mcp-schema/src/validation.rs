use crate::manifest::SkillManifest;
use crate::result::AtomicApiResult;
use jsonschema::{Draft, JSONSchema};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeSet, HashSet};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidationReport {
    pub errors: Vec<ValidationIssue>,
    pub warnings: Vec<ValidationIssue>,
}

impl ValidationReport {
    pub fn ok() -> Self {
        Self {
            errors: Vec::new(),
            warnings: Vec::new(),
        }
    }

    pub fn is_valid(&self) -> bool {
        self.errors.is_empty()
    }

    pub fn push_error(&mut self, path: impl Into<String>, message: impl Into<String>) {
        self.errors.push(ValidationIssue {
            level: ValidationIssueLevel::Error,
            path: path.into(),
            message: message.into(),
        });
    }

    pub fn push_warning(&mut self, path: impl Into<String>, message: impl Into<String>) {
        self.warnings.push(ValidationIssue {
            level: ValidationIssueLevel::Warning,
            path: path.into(),
            message: message.into(),
        });
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidationIssue {
    pub level: ValidationIssueLevel,
    pub path: String,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ValidationIssueLevel {
    Error,
    Warning,
}

pub fn validate_manifest(manifest: &SkillManifest) -> ValidationReport {
    validate_manifest_with_component_paths(manifest, std::iter::empty::<&str>())
}

pub fn validate_manifest_with_component_paths<'a>(
    manifest: &SkillManifest,
    additional_component_paths: impl IntoIterator<Item = &'a str>,
) -> ValidationReport {
    let mut report = ValidationReport::ok();
    let mut seen_api_names = HashSet::new();
    let mut component_paths: BTreeSet<String> = manifest
        .components
        .iter()
        .map(|component| component.path.clone())
        .collect();

    component_paths.extend(additional_component_paths.into_iter().map(str::to_owned));

    for (index, component) in manifest.components.iter().enumerate() {
        if component.path.trim().is_empty() {
            report.push_error(
                format!("components[{index}].path"),
                "component path is required",
            );
        }
    }

    for (index, api) in manifest.apis.iter().enumerate() {
        let api_path = format!("apis[{index}]");

        if api.name.trim().is_empty() {
            report.push_error(format!("{api_path}.name"), "API name is required");
        } else if !seen_api_names.insert(api.name.as_str()) {
            report.push_error(
                format!("{api_path}.name"),
                format!("duplicate API name `{}`", api.name),
            );
        }

        if api.description.trim().is_empty() {
            report.push_error(
                format!("{api_path}.description"),
                "API description is required",
            );
        }

        if !is_json_schema_object(&api.input_schema) {
            report.push_error(
                format!("{api_path}.inputSchema"),
                "inputSchema must be a JSON object schema",
            );
        } else if has_non_object_schema_type(&api.input_schema) {
            report.push_error(
                format!("{api_path}.inputSchema"),
                "inputSchema type must be object when type is declared",
            );
        } else if let Err(message) = compile_schema(&api.input_schema) {
            report.push_error(format!("{api_path}.inputSchema"), message);
        }

        if let Some(output_schema) = &api.output_schema {
            if let Err(message) = compile_schema(output_schema) {
                report.push_warning(format!("{api_path}.outputSchema"), message);
            }
        }

        if let Some(component_path) = api.component_path() {
            if !component_paths.contains(component_path) {
                report.push_error(
                    format!("{api_path}._meta.ui.componentPath"),
                    format!("componentPath `{component_path}` does not match components[]"),
                );
            }
        }
    }

    report
}

pub fn validate_api_result(result: &AtomicApiResult) -> ValidationReport {
    let mut report = ValidationReport::ok();

    if result.content.is_empty() {
        report.push_error(
            "content",
            "content must contain at least one TextContent block",
        );
    }

    for (index, content) in result.content.iter().enumerate() {
        if content.text.trim().is_empty() {
            report.push_error(
                format!("content[{index}].text"),
                "TextContent.text must not be empty",
            );
        }
    }

    report
}

pub fn validate_input(schema: &Value, arguments: &Value) -> ValidationReport {
    let mut report = ValidationReport::ok();

    if !is_json_schema_object(schema) {
        report.push_error("inputSchema", "inputSchema must be a JSON object schema");
        return report;
    }

    if has_non_object_schema_type(schema) {
        report.push_error(
            "inputSchema",
            "inputSchema type must be object when type is declared",
        );
        return report;
    }

    let compiled = match compile_schema(schema) {
        Ok(schema) => schema,
        Err(message) => {
            report.push_error("inputSchema", message);
            return report;
        }
    };

    if !arguments.is_object() {
        report.push_error("arguments", "arguments must be a JSON object");
        return report;
    }

    if let Err(errors) = compiled.validate(arguments) {
        for error in errors {
            report.push_error("arguments", error.to_string());
        }
    }

    report
}

pub fn validate_output_warning(
    schema: &Value,
    structured_content: Option<&Value>,
) -> ValidationReport {
    let mut report = ValidationReport::ok();
    let compiled = match compile_schema(schema) {
        Ok(schema) => schema,
        Err(message) => {
            report.push_warning("outputSchema", message);
            return report;
        }
    };

    let Some(structured_content) = structured_content else {
        report.push_warning(
            "structuredContent",
            "structuredContent is absent; outputSchema validation skipped",
        );
        return report;
    };

    if let Err(errors) = compiled.validate(structured_content) {
        for error in errors {
            report.push_warning("structuredContent", error.to_string());
        }
    }

    report
}

fn is_json_schema_object(schema: &Value) -> bool {
    schema.is_object()
}

fn has_non_object_schema_type(schema: &Value) -> bool {
    schema
        .as_object()
        .and_then(|object| object.get("type"))
        .and_then(Value::as_str)
        .is_some_and(|schema_type| schema_type != "object")
}

fn compile_schema(schema: &Value) -> Result<JSONSchema, String> {
    JSONSchema::options()
        .with_draft(Draft::Draft7)
        .compile(schema)
        .map_err(|error| format!("invalid JSON Schema: {error}"))
}
