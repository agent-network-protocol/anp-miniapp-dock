use crate::api_registry::ApiRegistry;
use crate::error::{DockCoreError, ErrorCode};
use crate::host::{
    ApiExecutor, AuditEvent, AuditSink, ConsentDecision, ConsentGate, PermissionDecision,
    RenderOutcome, RenderRouter, RuntimeHost,
};
use consent_audit::{
    build_consent_request, consent_proof, parameter_digest, redact_value, AuditOutcome,
    ConsentProof, ConsentRequestInput, RiskLevel, RiskPolicy,
};
use mcp_schema::{validate_api_result, validate_input, AtomicApiResult, TextContent};
use serde_json::{Map, Value};
use skill_loader::LoadedSkill;

#[derive(Debug, Clone, PartialEq)]
pub struct ApiCallContext {
    pub user_did: Option<String>,
    pub agent_did: Option<String>,
    pub merchant_did: Option<String>,
    pub skill_id: String,
    pub session_id: String,
    pub api_name: String,
    pub arguments: Value,
    pub capability_token: Option<String>,
}

impl ApiCallContext {
    pub fn for_api(&self, api_name: impl Into<String>, arguments: Value) -> Self {
        Self {
            api_name: api_name.into(),
            arguments,
            ..self.clone()
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ComponentRenderInput {
    pub api_name: String,
    pub arguments: Value,
    pub content: Vec<TextContent>,
    pub structured_content: Option<Map<String, Value>>,
    pub meta: Option<Map<String, Value>>,
    pub component_path: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ComponentAction {
    SendFollowUpMessage {
        content: Vec<TextContent>,
    },
    ApiCall {
        name: String,
        arguments: Value,
    },
    OpenDetailPage {
        url: String,
    },
    ExpirePreviousCards {
        component_paths: Vec<String>,
        match_policy: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct CallOutcome {
    pub result: AtomicApiResult,
    pub model_visible: Value,
    pub render: Option<RenderOutcome>,
}

pub struct Orchestrator<H, C, E, R, A> {
    skill: LoadedSkill,
    registry: ApiRegistry,
    host: H,
    consent: C,
    executor: E,
    renderer: R,
    audit: A,
}

impl<H, C, E, R, A> Orchestrator<H, C, E, R, A>
where
    H: RuntimeHost,
    C: ConsentGate,
    E: ApiExecutor,
    R: RenderRouter,
    A: AuditSink,
{
    pub fn load_skill(
        skill: LoadedSkill,
        host: H,
        consent: C,
        executor: E,
        renderer: R,
        audit: A,
    ) -> Self {
        let registry = ApiRegistry::from_manifest(&skill.manifest);
        Self {
            skill,
            registry,
            host,
            consent,
            executor,
            renderer,
            audit,
        }
    }

    pub fn call_api(&self, context: ApiCallContext) -> Result<CallOutcome, DockCoreError> {
        let registered = self.registry.get(&context.api_name)?;
        let risk_level = RiskPolicy::new().infer_api_risk(&registered.declaration);
        let input_report = validate_input(&registered.declaration.input_schema, &context.arguments);
        if !input_report.is_valid() {
            self.record_audit(&context, risk_level, None, AuditOutcome::ValidationFailed)?;
            return Err(DockCoreError::validation(
                format!("arguments for `{}` failed inputSchema", context.api_name),
                input_report,
            ));
        }

        match self.host.check_permission(&context)? {
            PermissionDecision::Allow => {}
            PermissionDecision::Deny(reason) => {
                self.record_audit(
                    &context,
                    risk_level,
                    None,
                    AuditOutcome::BlockedPermissionDenied,
                )?;
                return Err(DockCoreError::core(ErrorCode::PermissionDenied, reason));
            }
        }

        let consent_request = build_consent_request(ConsentRequestInput {
            user_did: context.user_did.clone(),
            agent_did: context.agent_did.clone(),
            merchant_did: context.merchant_did.clone(),
            skill_id: context.skill_id.clone(),
            session_id: context.session_id.clone(),
            api_name: context.api_name.clone(),
            risk_level,
            arguments: &context.arguments,
        });
        let mut proof = None;
        if risk_level.requires_consent() {
            match self.consent.check_consent(&context, &consent_request)? {
                ConsentDecision::Approved => {
                    proof = Some(consent_proof(
                        &consent_request,
                        "dock-core-consent-gate",
                        parameter_digest(&consent_request.parameter_summary),
                    ));
                }
                ConsentDecision::Required(reason) => {
                    self.record_audit(
                        &context,
                        risk_level,
                        None,
                        AuditOutcome::BlockedConsentRequired,
                    )?;
                    return Err(DockCoreError::core(ErrorCode::ConsentRequired, reason));
                }
            }
        }

        let component_path = registered.declaration.component_path();
        let result = match self.executor.execute(&context, component_path) {
            Ok(result) => result,
            Err(error) => {
                self.record_audit(&context, risk_level, proof, AuditOutcome::Error)?;
                return Err(error);
            }
        };
        let result_report = validate_api_result(&result);
        if !result_report.is_valid() {
            self.record_audit(&context, risk_level, proof, AuditOutcome::ValidationFailed)?;
            return Err(DockCoreError::validation(
                format!(
                    "API `{}` returned invalid AtomicApiResult",
                    context.api_name
                ),
                result_report,
            ));
        }

        let render = self.route_result(&context, &result, component_path);
        self.record_audit(&context, risk_level, proof, AuditOutcome::Ok)?;

        Ok(CallOutcome {
            model_visible: serde_json::to_value(result.model_visible()).map_err(|error| {
                DockCoreError::core(
                    ErrorCode::ValidationFailed,
                    format!("failed to serialize model-visible result: {error}"),
                )
            })?,
            result,
            render,
        })
    }

    pub fn handle_component_action(
        &self,
        base_context: &ApiCallContext,
        action: ComponentAction,
    ) -> Result<Option<CallOutcome>, DockCoreError> {
        match action {
            ComponentAction::ApiCall { name, arguments } => self
                .call_api(base_context.for_api(name, arguments))
                .map(Some),
            ComponentAction::SendFollowUpMessage { .. }
            | ComponentAction::OpenDetailPage { .. }
            | ComponentAction::ExpirePreviousCards { .. } => Ok(None),
        }
    }

    pub fn registry(&self) -> &ApiRegistry {
        &self.registry
    }

    pub fn skill(&self) -> &LoadedSkill {
        &self.skill
    }

    fn route_result(
        &self,
        context: &ApiCallContext,
        result: &AtomicApiResult,
        component_path: Option<&str>,
    ) -> Option<RenderOutcome> {
        if result.is_error {
            return None;
        }

        let Some(component_path) = component_path else {
            return Some(self.renderer.fallback(context, result, "no_component_path"));
        };

        let input = ComponentRenderInput {
            api_name: context.api_name.clone(),
            arguments: context.arguments.clone(),
            content: result.content.clone(),
            structured_content: result.structured_content.clone(),
            meta: result.meta.clone(),
            component_path: component_path.to_owned(),
        };

        match self.renderer.render(context, &input) {
            Ok(render) => Some(render),
            Err(error) => Some(self.renderer.fallback(
                context,
                result,
                &format!("render_failed: {error}"),
            )),
        }
    }

    fn record_audit(
        &self,
        context: &ApiCallContext,
        risk_level: RiskLevel,
        consent_proof: Option<ConsentProof>,
        outcome: AuditOutcome,
    ) -> Result<(), DockCoreError> {
        self.audit.record(AuditEvent {
            user_did: context.user_did.clone(),
            agent_did: context.agent_did.clone(),
            merchant_did: context.merchant_did.clone(),
            session_id: context.session_id.clone(),
            skill_id: context.skill_id.clone(),
            api_name: context.api_name.clone(),
            risk_level,
            parameter_summary: redact_value(&context.arguments),
            consent_proof,
            outcome: outcome.to_string(),
        })
    }
}
