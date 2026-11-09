use crate::manifest::{ApiDeclaration, ComponentPermissions, SkillManifest};
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
            category: ValidationIssueCategory::Spec,
            path: path.into(),
            message: message.into(),
            suggestion: None,
        });
    }

    pub fn push_warning(&mut self, path: impl Into<String>, message: impl Into<String>) {
        self.push_compatibility_warning(path, message, None::<String>);
    }

    pub fn push_compatibility_warning(
        &mut self,
        path: impl Into<String>,
        message: impl Into<String>,
        suggestion: Option<impl Into<String>>,
    ) {
        self.warnings.push(ValidationIssue {
            level: ValidationIssueLevel::Warning,
            category: ValidationIssueCategory::Compatibility,
            path: path.into(),
            message: message.into(),
            suggestion: suggestion.map(Into::into),
        });
    }

    pub fn push_production_warning(
        &mut self,
        path: impl Into<String>,
        message: impl Into<String>,
        suggestion: Option<impl Into<String>>,
    ) {
        self.warnings.push(ValidationIssue {
            level: ValidationIssueLevel::Warning,
            category: ValidationIssueCategory::Production,
            path: path.into(),
            message: message.into(),
            suggestion: suggestion.map(Into::into),
        });
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidationIssue {
    pub level: ValidationIssueLevel,
    pub category: ValidationIssueCategory,
    pub path: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ValidationIssueLevel {
    Error,
    Warning,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ValidationIssueCategory {
    Spec,
    Compatibility,
    Production,
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

    validate_supply_chain_metadata(&mut report, manifest.supply_chain_meta());

    for (index, component) in manifest.components.iter().enumerate() {
        let component_path = format!("components[{index}]");
        if component.path.trim().is_empty() {
            report.push_error(
                format!("{component_path}.path"),
                "component path is required",
            );
        }

        if component.path.contains('\0') {
            report.push_error(
                format!("{component_path}.path"),
                "component path must not contain NUL bytes",
            );
        }

        if let Some(related_page) = &component.related_page {
            validate_related_page(
                &mut report,
                &format!("{component_path}.relatedPage"),
                related_page,
            );
        }

        if let Some(expired_text) = &component.expired_text {
            if expired_text.trim().is_empty() {
                report.push_compatibility_warning(
                    format!("{component_path}.expiredText"),
                    "expiredText is empty and will not help Host fallback UX",
                    Some("Provide a short user-facing expired card message."),
                );
            }
            if expired_text.chars().count() > 120 {
                report.push_compatibility_warning(
                    format!("{component_path}.expiredText"),
                    "expiredText is longer than the recommended 120 characters",
                    Some("Keep expiredText concise so Hosts can render it inside compact cards."),
                );
            }
        }

        validate_component_permissions(
            &mut report,
            &format!("{component_path}.permissions"),
            component.permissions.as_ref(),
        );
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

        for field in api.input_formats() {
            let field_path = if field.path.is_empty() {
                format!("{api_path}.inputSchema")
            } else {
                format!("{api_path}.inputSchema.{}", field.path)
            };
            report.push_compatibility_warning(
                field_path,
                format!(
                    "input field uses format `{}` which requires a Host file/media provider",
                    field.format
                ),
                Some("Treat this field as an opaque Host handle until Phase 1/3 media providers are implemented."),
            );
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
            if component_path.contains('\0') {
                report.push_error(
                    format!("{api_path}._meta.ui.componentPath"),
                    "componentPath must not contain NUL bytes",
                );
            }
        }

        if is_demo_only_api(api) {
            report.push_production_warning(
                format!("{api_path}._meta"),
                "API metadata uses demo-only localhost DID/request compatibility fields",
                Some("Keep this Skill in demo/dev profiles until Step 01-04 replaces localhost wx.login/wx.request paths."),
            );
        }
    }

    report
}

fn validate_supply_chain_metadata(report: &mut ValidationReport, supply_chain: Option<&Value>) {
    let Some(supply_chain) = supply_chain else {
        report.push_production_warning(
            "_meta.anp.supplyChain",
            "Skill package is unsigned and lacks publisher DID supply-chain metadata",
            Some(
                "Declare publisherDid, sha256 digest, and package signature before production validation.",
            ),
        );
        return;
    };

    let Some(object) = supply_chain.as_object() else {
        report.push_production_warning(
            "_meta.anp.supplyChain",
            "supplyChain metadata should be an object",
            Some("Use {\"publisherDid\":\"did:...\",\"digest\":{...},\"signature\":{...}}."),
        );
        return;
    };

    match object.get("publisherDid").and_then(Value::as_str) {
        Some(publisher_did)
            if !publisher_did.trim().is_empty() && publisher_did.starts_with("did:") => {}
        _ => report.push_production_warning(
            "_meta.anp.supplyChain.publisherDid",
            "publisherDid is required and must be a DID",
            Some(
                "Use the publisher ANP DID that the Host can check against its trusted allowlist.",
            ),
        ),
    }

    validate_digest_metadata(report, object.get("digest"));
    validate_signature_metadata(report, object.get("signature"));
}

fn validate_digest_metadata(report: &mut ValidationReport, digest: Option<&Value>) {
    let Some(object) = digest.and_then(Value::as_object) else {
        report.push_production_warning(
            "_meta.anp.supplyChain.digest",
            "package digest metadata is required",
            Some("Use {\"algorithm\":\"sha256\",\"value\":\"<64 lowercase hex chars>\"}."),
        );
        return;
    };

    if object.get("algorithm").and_then(Value::as_str) != Some("sha256") {
        report.push_production_warning(
            "_meta.anp.supplyChain.digest.algorithm",
            "package digest algorithm must be sha256",
            Some("Recompute the package digest with the Step 03-06 loader contract."),
        );
    }

    let valid_value = object
        .get("value")
        .and_then(Value::as_str)
        .is_some_and(is_sha256_hex);
    if !valid_value {
        report.push_production_warning(
            "_meta.anp.supplyChain.digest.value",
            "package digest value must be a 64-character hex sha256",
            Some("Use the digest reported by dock-cli validate before signing the package."),
        );
    }
}

fn validate_signature_metadata(report: &mut ValidationReport, signature: Option<&Value>) {
    let Some(object) = signature.and_then(Value::as_object) else {
        report.push_production_warning(
            "_meta.anp.supplyChain.signature",
            "package signature metadata is required",
            Some("Attach a package signature over the normalized package digest."),
        );
        return;
    };

    for (field, message) in [
        ("algorithm", "package signature algorithm is required"),
        ("value", "package signature value is required"),
        ("keyId", "package signature keyId is required"),
    ] {
        let valid = object
            .get(field)
            .and_then(Value::as_str)
            .is_some_and(|value| !value.trim().is_empty());
        if !valid {
            report.push_production_warning(
                format!("_meta.anp.supplyChain.signature.{field}"),
                message,
                Some("Use a verifier-supported signature contract before production validation."),
            );
        }
    }
}

fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
}

fn validate_related_page(report: &mut ValidationReport, path: &str, related_page: &Value) {
    let Some(object) = related_page.as_object() else {
        report.push_compatibility_warning(
            path,
            "relatedPage should be an object with a safe relative path and optional query",
            Some("Use {\"path\":\"pages/detail/index\",\"query\":{...}} or remove relatedPage."),
        );
        return;
    };

    let Some(page_path) = object.get("path").and_then(Value::as_str) else {
        report.push_compatibility_warning(
            format!("{path}.path"),
            "relatedPage.path is missing",
            Some("Declare a relative Host detail page path or omit relatedPage."),
        );
        return;
    };

    if page_path.trim().is_empty() || page_path.starts_with('/') || page_path.contains("..") {
        report.push_compatibility_warning(
            format!("{path}.path"),
            "relatedPage.path should be a non-empty relative path inside the Skill/Host boundary",
            Some("Use a relative page path without leading slash or parent segments."),
        );
    }

    if object.get("query").is_some_and(|query| !query.is_object()) {
        report.push_compatibility_warning(
            format!("{path}.query"),
            "relatedPage.query should be an object when present",
            Some("Use key/value query data so Hosts can redact and serialize it consistently."),
        );
    }
}

fn validate_component_permissions(
    report: &mut ValidationReport,
    path: &str,
    permissions: Option<&ComponentPermissions>,
) {
    let Some(permissions) = permissions else {
        return;
    };

    if let Some(scope_dynamic) = &permissions.scope_dynamic {
        report.push_production_warning(
            format!("{path}.scope.dynamic"),
            "dynamic component capability is declared and requires explicit Host production policy",
            Some("Step 02-05 provides the runtime gate; keep production network transport, background lifecycle, and audit policy behind Host review."),
        );

        if !scope_dynamic.is_object() && !scope_dynamic.is_boolean() {
            report.push_compatibility_warning(
                format!("{path}.scope.dynamic"),
                "scope.dynamic should be an object or boolean",
                Some("Use an object with desc/reason fields so review tools can explain the dynamic capability."),
            );
        }
    }
}

fn is_demo_only_api(api: &ApiDeclaration) -> bool {
    api.meta.as_ref().is_some_and(|meta| {
        meta.extra.contains_key("remoteLogin")
            || meta.extra.contains_key("compatLoginEndpoint")
            || meta
                .extra
                .get("requestAuthMode")
                .and_then(Value::as_str)
                .is_some_and(|mode| mode == "host-managed-bearer")
    })
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
