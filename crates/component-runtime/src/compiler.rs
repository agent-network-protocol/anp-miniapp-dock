use crate::loader::ComponentPackage;
use crate::render_ir::{
    RenderEventBinding, RenderEventKind, RenderNode, RenderNodeKind, RENDER_IR_SCHEMA_VERSION,
};
use crate::wxml::{parse_wxml, WxmlElement, WxmlNode, WxmlParseError};
use crate::wxss::{merge_styles, parse_inline_style, SimpleSelector, WxssStyleSheet};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

#[derive(Debug, Clone, PartialEq)]
pub struct BindingContext {
    data: Value,
    locals: Map<String, Value>,
}

impl BindingContext {
    pub fn new(data: Value) -> Self {
        Self {
            data,
            locals: Map::new(),
        }
    }

    fn with_local(&self, key: impl Into<String>, value: Value) -> Self {
        let mut next = self.clone();
        next.locals.insert(key.into(), value);
        next
    }

    fn resolve_path(&self, path: &str) -> Option<Value> {
        let mut segments = path
            .split('.')
            .map(str::trim)
            .filter(|part| !part.is_empty());
        let first = segments.next()?;
        let mut current = self
            .locals
            .get(first)
            .cloned()
            .or_else(|| self.data.get(first).cloned())?;

        for segment in segments {
            if segment == "length" {
                current = match current {
                    Value::Array(items) => Value::from(items.len()),
                    Value::String(text) => Value::from(text.chars().count()),
                    Value::Object(map) => Value::from(map.len()),
                    _ => return None,
                };
                continue;
            }
            current = current.get(segment).cloned()?;
        }

        Some(current)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComponentRenderOutput {
    pub schema_version: String,
    pub root: RenderNode,
    pub warnings: Vec<String>,
}

impl ComponentRenderOutput {
    pub fn new(root: RenderNode, warnings: Vec<String>) -> Self {
        Self {
            schema_version: RENDER_IR_SCHEMA_VERSION.to_owned(),
            root,
            warnings,
        }
    }
}

#[derive(Debug)]
pub enum ComponentCompileError {
    Wxml(WxmlParseError),
    MissingRoot,
}

impl std::fmt::Display for ComponentCompileError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Wxml(error) => write!(formatter, "WXML parse failed: {error}"),
            Self::MissingRoot => formatter.write_str("component did not produce a render root"),
        }
    }
}

impl std::error::Error for ComponentCompileError {}

pub fn compile_component_to_render_ir(
    package: &ComponentPackage,
    data: &Value,
) -> Result<ComponentRenderOutput, ComponentCompileError> {
    compile_wxml_to_render_ir(
        &package.wxml,
        package.wxss.as_deref().unwrap_or_default(),
        data,
    )
}

pub fn compile_wxml_to_render_ir(
    wxml: &str,
    wxss: &str,
    data: &Value,
) -> Result<ComponentRenderOutput, ComponentCompileError> {
    let ast = parse_wxml(wxml).map_err(ComponentCompileError::Wxml)?;
    let sheet = WxssStyleSheet::parse(wxss);
    let mut warnings = sheet.warnings().to_vec();
    let context = BindingContext::new(data.clone());
    let mut counter = 0_usize;
    let Some(root) = compile_node(&ast, &context, &sheet, &mut warnings, &mut counter, &[])?
        .into_iter()
        .next()
    else {
        return Err(ComponentCompileError::MissingRoot);
    };
    Ok(ComponentRenderOutput::new(root, warnings))
}

fn compile_node(
    node: &WxmlNode,
    context: &BindingContext,
    sheet: &WxssStyleSheet,
    warnings: &mut Vec<String>,
    counter: &mut usize,
    ancestors: &[SimpleSelector],
) -> Result<Vec<RenderNode>, ComponentCompileError> {
    match node {
        WxmlNode::Text(text) => {
            let text = interpolate_text(text, context, warnings);
            if text.trim().is_empty() {
                Ok(Vec::new())
            } else {
                Ok(vec![RenderNode::text(next_id(counter, "text"), text)])
            }
        }
        WxmlNode::Element(element) => {
            compile_element(element, context, sheet, warnings, counter, ancestors)
        }
    }
}

fn compile_children(
    children: &[WxmlNode],
    context: &BindingContext,
    sheet: &WxssStyleSheet,
    warnings: &mut Vec<String>,
    counter: &mut usize,
    ancestors: &[SimpleSelector],
) -> Result<Vec<RenderNode>, ComponentCompileError> {
    let mut output = Vec::new();
    let mut condition_chain_matched: Option<bool> = None;
    for child in children {
        let WxmlNode::Element(element) = child else {
            if !matches!(child, WxmlNode::Text(text) if text.trim().is_empty()) {
                condition_chain_matched = None;
            }
            output.extend(compile_node(
                child, context, sheet, warnings, counter, ancestors,
            )?);
            continue;
        };

        if let Some(condition) = element.attrs.get("wx:elif") {
            if condition_chain_matched == Some(true) {
                continue;
            }
            let matched = evaluate_condition(condition, context, warnings, "wx:elif");
            condition_chain_matched = Some(matched);
            if !matched {
                continue;
            }
            let mut clone = element.clone();
            clone.attrs.remove("wx:elif");
            output.extend(compile_element(
                &clone, context, sheet, warnings, counter, ancestors,
            )?);
            continue;
        }

        if element.attrs.contains_key("wx:else") {
            if condition_chain_matched == Some(true) {
                condition_chain_matched = None;
                continue;
            }
            condition_chain_matched = None;
            let mut clone = element.clone();
            clone.attrs.remove("wx:else");
            output.extend(compile_element(
                &clone, context, sheet, warnings, counter, ancestors,
            )?);
            continue;
        }

        if let Some(condition) = element.attrs.get("wx:if") {
            let matched = evaluate_condition(condition, context, warnings, "wx:if");
            condition_chain_matched = Some(matched);
            if !matched {
                continue;
            }
            let mut clone = element.clone();
            clone.attrs.remove("wx:if");
            output.extend(compile_element(
                &clone, context, sheet, warnings, counter, ancestors,
            )?);
            continue;
        }

        condition_chain_matched = None;
        output.extend(compile_element(
            element, context, sheet, warnings, counter, ancestors,
        )?);
    }
    Ok(output)
}

fn compile_element(
    element: &WxmlElement,
    context: &BindingContext,
    sheet: &WxssStyleSheet,
    warnings: &mut Vec<String>,
    counter: &mut usize,
    ancestors: &[SimpleSelector],
) -> Result<Vec<RenderNode>, ComponentCompileError> {
    if let Some(condition) = element.attrs.get("wx:if") {
        if !evaluate_condition(condition, context, warnings, "wx:if") {
            return Ok(Vec::new());
        }
    }

    if let Some(for_expr) = element.attrs.get("wx:for") {
        let Some(path) = single_binding_path(for_expr) else {
            warnings.push(format!("unsupported wx:for expression `{for_expr}`"));
            return Ok(Vec::new());
        };
        let item_name = element
            .attrs
            .get("wx:for-item")
            .map(String::as_str)
            .unwrap_or("item");
        let index_name = element
            .attrs
            .get("wx:for-index")
            .map(String::as_str)
            .unwrap_or("index");
        let Some(Value::Array(items)) = context.resolve_path(path) else {
            return Ok(Vec::new());
        };
        let mut nodes = Vec::new();
        for (index, item) in items.into_iter().enumerate() {
            let loop_context = context
                .with_local(item_name, item)
                .with_local(index_name, Value::from(index));
            let mut clone = element.clone();
            if let Some(key_value) = element
                .attrs
                .get("wx:key")
                .and_then(|key| resolve_wx_key(key, item_name, &loop_context))
            {
                clone
                    .attrs
                    .insert("data-render-key".to_owned(), key_to_string(key_value));
            }
            clone.attrs.remove("wx:for");
            clone.attrs.remove("wx:for-item");
            clone.attrs.remove("wx:for-index");
            nodes.extend(compile_element(
                &clone,
                &loop_context,
                sheet,
                warnings,
                counter,
                ancestors,
            )?);
        }
        return Ok(nodes);
    }

    let kind = match element.tag.as_str() {
        "view" => RenderNodeKind::View,
        "text" => RenderNodeKind::Text,
        "image" => RenderNodeKind::Image,
        "button" => RenderNodeKind::Button,
        "scroll-view" => RenderNodeKind::ScrollView,
        "input" => RenderNodeKind::Input,
        "textarea" => RenderNodeKind::Textarea,
        "radio" => RenderNodeKind::Radio,
        "checkbox" => RenderNodeKind::Checkbox,
        "picker" => RenderNodeKind::Picker,
        "map" => RenderNodeKind::MapPreview,
        "canvas" => RenderNodeKind::CanvasStatic,
        other => {
            warnings.push(format!("unsupported WXML tag `{other}`"));
            RenderNodeKind::View
        }
    };

    let mut node = RenderNode::new(next_id(counter, &element.tag), kind);
    apply_attrs(&mut node, element, context, sheet, warnings, ancestors);

    let mut child_ancestors = ancestors.to_vec();
    child_ancestors.extend(element_selectors(element));
    node.children = compile_children(
        &element.children,
        context,
        sheet,
        warnings,
        counter,
        &child_ancestors,
    )?;

    if node.kind == RenderNodeKind::Text && node.text.is_none() && !node.children.is_empty() {
        let text = node
            .children
            .iter()
            .filter_map(|child| child.text.as_deref())
            .collect::<String>();
        node.text = Some(text);
        node.children.clear();
    }

    Ok(vec![node])
}

fn apply_attrs(
    node: &mut RenderNode,
    element: &WxmlElement,
    context: &BindingContext,
    sheet: &WxssStyleSheet,
    warnings: &mut Vec<String>,
    ancestors: &[SimpleSelector],
) {
    if let Some(style) = sheet.tag_style(&element.tag) {
        merge_styles(&mut node.style, style);
    }

    if let Some(class_names) = element.attrs.get("class") {
        for class_name in class_names.split_whitespace() {
            if let Some(style) = sheet.class_style(class_name) {
                merge_styles(&mut node.style, style);
            }
        }
    }

    if let Some(id) = element.attrs.get("id") {
        if let Some(style) = sheet.id_style(id) {
            merge_styles(&mut node.style, style);
        }
    }

    let selectors = element_selectors(element);
    for style in sheet.matching_descendant_styles(ancestors, &selectors) {
        merge_styles(&mut node.style, style);
    }

    if let Some(inline_style) = element.attrs.get("style") {
        let (style, mut style_warnings) = parse_inline_style(inline_style);
        warnings.append(&mut style_warnings);
        merge_styles(&mut node.style, &style);
    }

    let disabled =
        supports_disabled(&element.tag) && disabled_attr_value(element, context, warnings);
    if disabled {
        node.props.insert("disabled".to_owned(), Value::Bool(true));
    }

    for (name, value) in &element.attrs {
        match name.as_str() {
            "class" | "style" | "wx:if" | "wx:elif" | "wx:else" | "wx:for" | "wx:key"
            | "wx:for-item" | "wx:for-index" => {}
            "id" => {
                node.props
                    .insert("id".to_owned(), Value::String(value.clone()));
            }
            "disabled" if supports_disabled(&element.tag) => {}
            "data-render-key" => {
                node.props
                    .insert("key".to_owned(), Value::String(value.clone()));
            }
            "bindtap" if !disabled => node
                .events
                .push(RenderEventBinding::new(RenderEventKind::Tap, value)),
            "catchtap" if !disabled => node
                .events
                .push(RenderEventBinding::new(RenderEventKind::CatchTap, value)),
            "bindinput" if supports_input_event(&element.tag) && !disabled => node
                .events
                .push(RenderEventBinding::new(RenderEventKind::Input, value)),
            "bindchange" if supports_change_event(&element.tag) && !disabled => node
                .events
                .push(RenderEventBinding::new(RenderEventKind::Change, value)),
            "bindload" if element.tag == "image" => node
                .events
                .push(RenderEventBinding::new(RenderEventKind::ImageLoad, value)),
            "binderror" if element.tag == "image" => node
                .events
                .push(RenderEventBinding::new(RenderEventKind::ImageError, value)),
            "src" if element.tag == "image" => {
                node.props.insert(
                    "src".to_owned(),
                    interpolate_value(value, context, warnings),
                );
            }
            "scroll-x" if element.tag == "scroll-view" => {
                node.props
                    .insert("scrollX".to_owned(), Value::Bool(value != "false"));
            }
            "scroll-y" if element.tag == "scroll-view" => {
                node.props
                    .insert("scrollY".to_owned(), Value::Bool(value != "false"));
            }
            attr if set_safe_form_prop(node, element, attr, value, context, warnings) => {}
            attr if set_safe_static_media_prop(node, element, attr, value, context, warnings) => {}
            attr if attr.starts_with("data-") => {
                let key = attr.trim_start_matches("data-").to_owned();
                for event in &mut node.events {
                    event
                        .dataset
                        .insert(key.clone(), interpolate_value(value, context, warnings));
                }
            }
            attr if should_warn_unsupported_attr(&element.tag, attr) => {
                warnings.push(format!(
                    "unsupported `{}` attribute `{attr}`",
                    public_component_tag(&element.tag)
                ));
            }
            _ => {}
        }
    }
}

fn supports_disabled(tag: &str) -> bool {
    matches!(
        tag,
        "button" | "input" | "textarea" | "radio" | "checkbox" | "picker"
    )
}

fn supports_input_event(tag: &str) -> bool {
    matches!(tag, "input" | "textarea")
}

fn supports_change_event(tag: &str) -> bool {
    matches!(tag, "input" | "textarea" | "radio" | "checkbox" | "picker")
}

fn set_safe_form_prop(
    node: &mut RenderNode,
    element: &WxmlElement,
    attr: &str,
    value: &str,
    context: &BindingContext,
    warnings: &mut Vec<String>,
) -> bool {
    let prop = match element.tag.as_str() {
        "input" | "textarea" => match attr {
            "name" => Some("name"),
            "value" => Some("value"),
            "placeholder" => Some("placeholder"),
            "type" => Some("inputType"),
            "maxlength" => Some("maxLength"),
            "confirm-type" => Some("confirmType"),
            _ => None,
        },
        "radio" | "checkbox" => match attr {
            "name" => Some("name"),
            "value" => Some("value"),
            "checked" => Some("checked"),
            _ => None,
        },
        "picker" => match attr {
            "name" => Some("name"),
            "value" => Some("value"),
            "range" => Some("options"),
            "range-key" => Some("rangeKey"),
            "mode" => Some("mode"),
            _ => None,
        },
        _ => None,
    };
    let Some(prop) = prop else {
        return false;
    };

    let value = match prop {
        "checked" if value.trim().is_empty() => Value::Bool(true),
        "checked" => Value::Bool(evaluate_condition(value, context, warnings, "checked")),
        "maxLength" => normalize_numeric_prop(interpolate_value(value, context, warnings)),
        "options" => normalize_picker_options(interpolate_value(value, context, warnings)),
        _ => interpolate_value(value, context, warnings),
    };
    node.props.insert(prop.to_owned(), value);
    true
}

fn set_safe_static_media_prop(
    node: &mut RenderNode,
    element: &WxmlElement,
    attr: &str,
    value: &str,
    context: &BindingContext,
    warnings: &mut Vec<String>,
) -> bool {
    let prop = match element.tag.as_str() {
        "map" => match attr {
            "id" => Some("id"),
            "region" => Some("region"),
            "location-token" => Some("locationToken"),
            "scale" => Some("scale"),
            "title" => Some("title"),
            "description" => Some("description"),
            "latitude" | "longitude" | "markers" | "polyline" | "controls" => {
                warnings.push(format!("unsupported `map-preview` attribute `{attr}`"));
                return true;
            }
            _ => None,
        },
        "canvas" => match attr {
            "id" => Some("id"),
            "canvas-id" => Some("canvasId"),
            "poster" => Some("poster"),
            "description" => Some("description"),
            "width" => Some("width"),
            "height" => Some("height"),
            "script" | "draw" | "bindtouchstart" | "bindtouchmove" | "bindtouchend" => {
                warnings.push(format!("unsupported `canvas-static` attribute `{attr}`"));
                return true;
            }
            _ => None,
        },
        _ => None,
    };
    let Some(prop) = prop else {
        return false;
    };

    let value = match prop {
        "scale" | "width" | "height" => {
            normalize_numeric_prop(interpolate_value(value, context, warnings))
        }
        _ => interpolate_value(value, context, warnings),
    };
    node.props.insert(prop.to_owned(), value);
    true
}

fn normalize_numeric_prop(value: Value) -> Value {
    match value {
        Value::String(value) => {
            if let Ok(number) = value.parse::<i64>() {
                return Value::from(number);
            }
            if let Ok(number) = value.parse::<f64>() {
                if let Some(number) = serde_json::Number::from_f64(number) {
                    return Value::Number(number);
                }
            }
            Value::String(value)
        }
        value => value,
    }
}

fn normalize_picker_options(value: Value) -> Value {
    match value {
        Value::Array(items) => Value::Array(
            items
                .into_iter()
                .map(|item| match item {
                    Value::String(_) | Value::Number(_) | Value::Bool(_) | Value::Object(_) => item,
                    Value::Null => Value::String(String::new()),
                    other => Value::String(other.to_string()),
                })
                .collect(),
        ),
        Value::String(value) => Value::Array(
            value
                .split(',')
                .map(str::trim)
                .filter(|item| !item.is_empty())
                .map(|item| Value::String(item.to_owned()))
                .collect(),
        ),
        value => value,
    }
}

fn should_warn_unsupported_attr(tag: &str, attr: &str) -> bool {
    matches!(
        tag,
        "input" | "textarea" | "radio" | "checkbox" | "picker" | "map" | "canvas"
    ) && !attr.starts_with("data-")
        && !attr.starts_with("wx:")
        && !matches!(attr, "bindtap" | "catchtap" | "bindinput" | "bindchange")
}

fn public_component_tag(tag: &str) -> &str {
    match tag {
        "map" => "map-preview",
        "canvas" => "canvas-static",
        tag => tag,
    }
}

fn element_selectors(element: &WxmlElement) -> Vec<SimpleSelector> {
    let mut selectors = vec![SimpleSelector::Tag(element.tag.clone())];
    if let Some(id) = element
        .attrs
        .get("id")
        .filter(|value| !value.trim().is_empty())
    {
        selectors.push(SimpleSelector::Id(id.clone()));
    }
    if let Some(class_names) = element.attrs.get("class") {
        selectors.extend(
            class_names
                .split_whitespace()
                .filter(|class_name| !class_name.is_empty())
                .map(|class_name| SimpleSelector::Class(class_name.to_owned())),
        );
    }
    selectors
}

fn disabled_attr_value(
    element: &WxmlElement,
    context: &BindingContext,
    warnings: &mut Vec<String>,
) -> bool {
    let Some(value) = element.attrs.get("disabled") else {
        return false;
    };
    if value.trim().is_empty() {
        return true;
    }
    evaluate_condition(value, context, warnings, "disabled")
}

fn resolve_wx_key(key: &str, item_name: &str, context: &BindingContext) -> Option<Value> {
    if key == "*this" {
        return context.resolve_path(item_name);
    }
    if let Some(path) = single_binding_path(key) {
        return context.resolve_path(path);
    }
    if is_supported_path(key) {
        return context
            .resolve_path(key)
            .or_else(|| context.resolve_path(&format!("{item_name}.{key}")));
    }
    None
}

fn key_to_string(value: Value) -> String {
    match value {
        Value::String(value) => value,
        Value::Number(value) => value.to_string(),
        Value::Bool(value) => value.to_string(),
        Value::Null => String::new(),
        value => value.to_string(),
    }
}

fn interpolate_text(source: &str, context: &BindingContext, warnings: &mut Vec<String>) -> String {
    let mut output = String::new();
    let mut rest = source;
    while let Some(start) = rest.find("{{") {
        output.push_str(&rest[..start]);
        let after_start = &rest[start + 2..];
        let Some(end) = after_start.find("}}") else {
            warnings.push(format!("unterminated binding in `{source}`"));
            output.push_str(&rest[start..]);
            return output;
        };
        let expression = after_start[..end].trim();
        output.push_str(&resolve_binding_as_string(expression, context, warnings));
        rest = &after_start[end + 2..];
    }
    output.push_str(rest);
    output
}

fn interpolate_value(source: &str, context: &BindingContext, warnings: &mut Vec<String>) -> Value {
    if let Some(expression) = binding_expression(source) {
        return match evaluate_expression(expression, context) {
            Ok(value) => value,
            Err(()) => {
                warnings.push(format!("unsupported binding expression `{expression}`"));
                Value::Null
            }
        };
    }
    Value::String(interpolate_text(source, context, warnings))
}

fn resolve_binding_as_string(
    expression: &str,
    context: &BindingContext,
    warnings: &mut Vec<String>,
) -> String {
    match evaluate_expression(expression, context) {
        Ok(Value::String(value)) => value,
        Ok(Value::Number(value)) => value.to_string(),
        Ok(Value::Bool(value)) => value.to_string(),
        Ok(Value::Null) | Err(()) => {
            if evaluate_expression(expression, context).is_err() {
                warnings.push(format!("unsupported binding expression `{expression}`"));
            }
            String::new()
        }
        Ok(value) => value.to_string(),
    }
}

fn evaluate_condition(
    source: &str,
    context: &BindingContext,
    warnings: &mut Vec<String>,
    label: &str,
) -> bool {
    let expression = binding_expression(source).unwrap_or_else(|| source.trim());
    match evaluate_expression(expression, context) {
        Ok(value) => value_truthy(&value),
        Err(()) => {
            warnings.push(format!("unsupported {label} expression `{source}`"));
            false
        }
    }
}

fn evaluate_expression(expression: &str, context: &BindingContext) -> Result<Value, ()> {
    let expression = expression.trim();
    if expression.is_empty()
        || expression.contains('(')
        || expression.contains(')')
        || expression.contains(';')
    {
        return Err(());
    }

    if let Some(inner) = expression.strip_prefix('!') {
        let value = evaluate_expression(inner, context)?;
        return Ok(Value::Bool(!value_truthy(&value)));
    }

    if let Some((left, right)) = split_binary_expression(expression, "===") {
        return Ok(Value::Bool(
            evaluate_expression(left, context)? == evaluate_expression(right, context)?,
        ));
    }

    if let Some((left, right)) = split_binary_expression(expression, "!==") {
        return Ok(Value::Bool(
            evaluate_expression(left, context)? != evaluate_expression(right, context)?,
        ));
    }

    if matches!(expression, "true" | "false") {
        return Ok(Value::Bool(expression == "true"));
    }
    if expression == "null" {
        return Ok(Value::Null);
    }
    if let Some(value) = quoted_literal(expression) {
        return Ok(Value::String(value.to_owned()));
    }
    if let Ok(number) = expression.parse::<i64>() {
        return Ok(Value::from(number));
    }
    if let Ok(number) = expression.parse::<f64>() {
        if let Some(number) = serde_json::Number::from_f64(number) {
            return Ok(Value::Number(number));
        }
    }
    if is_supported_path(expression) {
        return Ok(context.resolve_path(expression).unwrap_or(Value::Null));
    }
    Err(())
}

fn split_binary_expression<'a>(expression: &'a str, operator: &str) -> Option<(&'a str, &'a str)> {
    let (left, right) = expression.split_once(operator)?;
    if left.trim().is_empty() || right.trim().is_empty() || right.contains(operator) {
        return None;
    }
    Some((left.trim(), right.trim()))
}

fn quoted_literal(expression: &str) -> Option<&str> {
    expression
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .or_else(|| {
            expression
                .strip_prefix('\'')
                .and_then(|value| value.strip_suffix('\''))
        })
}

fn value_truthy(value: &Value) -> bool {
    match value {
        Value::Bool(value) => *value,
        Value::Number(value) => value.as_f64().map(|value| value != 0.0).unwrap_or(false),
        Value::String(value) => !value.is_empty(),
        Value::Array(value) => !value.is_empty(),
        Value::Object(value) => !value.is_empty(),
        Value::Null => false,
    }
}

fn binding_expression(source: &str) -> Option<&str> {
    let source = source.trim();
    source.strip_prefix("{{")?.strip_suffix("}}").map(str::trim)
}

fn single_binding_path(source: &str) -> Option<&str> {
    let expression = binding_expression(source)?;
    is_supported_path(expression).then_some(expression)
}

fn is_supported_path(expression: &str) -> bool {
    !expression.is_empty()
        && expression.split('.').all(|part| {
            !part.is_empty()
                && part
                    .chars()
                    .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
        })
}

fn next_id(counter: &mut usize, prefix: &str) -> String {
    let id = format!("{prefix}-{counter}");
    *counter += 1;
    id
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn simple_binding_resolves_path() {
        let output = compile_wxml_to_render_ir(
            "<view><text>{{ user.name }}</text></view>",
            "",
            &json!({"user": {"name": "Ada"}}),
        )
        .expect("compile succeeds");

        assert_eq!(output.schema_version, RENDER_IR_SCHEMA_VERSION);
        assert_eq!(output.root.children[0].text.as_deref(), Some("Ada"));
    }

    #[test]
    fn render_output_serializes_schema_version() {
        let output = compile_wxml_to_render_ir("<view><text>Ready</text></view>", "", &json!({}))
            .expect("compile succeeds");

        assert_eq!(
            serde_json::to_value(output).unwrap(),
            json!({
                "schemaVersion": "dock.render-ir.v1",
                "root": {
                    "id": "view-0",
                    "kind": "view",
                    "children": [{
                        "id": "text-1",
                        "kind": "text",
                        "text": "Ready"
                    }]
                },
                "warnings": []
            })
        );
    }

    #[test]
    fn unsupported_expression_is_warning() {
        let output = compile_wxml_to_render_ir(
            "<view><text>{{ price + tax }}</text></view>",
            "",
            &json!({"price": 1, "tax": 2}),
        )
        .expect("compile succeeds");

        assert!(output.warnings[0].contains("unsupported binding expression"));
    }
}
