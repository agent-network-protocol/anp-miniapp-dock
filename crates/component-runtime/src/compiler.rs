use crate::loader::ComponentPackage;
use crate::render_ir::{RenderEventBinding, RenderEventKind, RenderNode, RenderNodeKind};
use crate::wxml::{parse_wxml, WxmlElement, WxmlNode, WxmlParseError};
use crate::wxss::{merge_styles, parse_inline_style, WxssStyleSheet};
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

    fn truthy(&self, path: &str) -> bool {
        match self.resolve_path(path) {
            Some(Value::Bool(value)) => value,
            Some(Value::Number(value)) => value.as_f64().map(|value| value != 0.0).unwrap_or(false),
            Some(Value::String(value)) => !value.is_empty(),
            Some(Value::Array(value)) => !value.is_empty(),
            Some(Value::Object(value)) => !value.is_empty(),
            Some(Value::Null) | None => false,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ComponentRenderOutput {
    pub root: RenderNode,
    pub warnings: Vec<String>,
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
    let Some(root) = compile_node(&ast, &context, &sheet, &mut warnings, &mut counter)?
        .into_iter()
        .next()
    else {
        return Err(ComponentCompileError::MissingRoot);
    };
    Ok(ComponentRenderOutput { root, warnings })
}

fn compile_node(
    node: &WxmlNode,
    context: &BindingContext,
    sheet: &WxssStyleSheet,
    warnings: &mut Vec<String>,
    counter: &mut usize,
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
        WxmlNode::Element(element) => compile_element(element, context, sheet, warnings, counter),
    }
}

fn compile_element(
    element: &WxmlElement,
    context: &BindingContext,
    sheet: &WxssStyleSheet,
    warnings: &mut Vec<String>,
    counter: &mut usize,
) -> Result<Vec<RenderNode>, ComponentCompileError> {
    if let Some(condition) = element.attrs.get("wx:if") {
        let Some(path) = single_binding_path(condition) else {
            warnings.push(format!("unsupported wx:if expression `{condition}`"));
            return Ok(Vec::new());
        };
        if !context.truthy(path) {
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
        other => {
            warnings.push(format!("unsupported WXML tag `{other}`"));
            RenderNodeKind::View
        }
    };

    let mut node = RenderNode::new(next_id(counter, &element.tag), kind);
    apply_attrs(&mut node, element, context, sheet, warnings);

    for child in &element.children {
        node.children
            .extend(compile_node(child, context, sheet, warnings, counter)?);
    }

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
) {
    if let Some(class_names) = element.attrs.get("class") {
        for class_name in class_names.split_whitespace() {
            if let Some(style) = sheet.class_style(class_name) {
                merge_styles(&mut node.style, style);
            }
        }
    }

    if let Some(inline_style) = element.attrs.get("style") {
        let (style, mut style_warnings) = parse_inline_style(inline_style);
        warnings.append(&mut style_warnings);
        merge_styles(&mut node.style, &style);
    }

    for (name, value) in &element.attrs {
        match name.as_str() {
            "class" | "style" | "wx:if" | "wx:for" | "wx:key" | "wx:for-item" | "wx:for-index" => {}
            "data-render-key" => {
                node.props
                    .insert("key".to_owned(), Value::String(value.clone()));
            }
            "bindtap" => node
                .events
                .push(RenderEventBinding::new(RenderEventKind::Tap, value)),
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
            attr if attr.starts_with("data-") => {
                let key = attr.trim_start_matches("data-").to_owned();
                for event in &mut node.events {
                    event
                        .dataset
                        .insert(key.clone(), interpolate_value(value, context, warnings));
                }
            }
            _ => {}
        }
    }
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
    if let Some(path) = single_binding_path(source) {
        return context.resolve_path(path).unwrap_or(Value::Null);
    }
    Value::String(interpolate_text(source, context, warnings))
}

fn resolve_binding_as_string(
    expression: &str,
    context: &BindingContext,
    warnings: &mut Vec<String>,
) -> String {
    if !is_supported_path(expression) {
        warnings.push(format!("unsupported binding expression `{expression}`"));
        return String::new();
    }

    match context.resolve_path(expression) {
        Some(Value::String(value)) => value,
        Some(Value::Number(value)) => value.to_string(),
        Some(Value::Bool(value)) => value.to_string(),
        Some(Value::Null) | None => String::new(),
        Some(value) => value.to_string(),
    }
}

fn single_binding_path(source: &str) -> Option<&str> {
    let source = source.trim();
    let expression = source.strip_prefix("{{")?.strip_suffix("}}")?.trim();
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

        assert_eq!(output.root.children[0].text.as_deref(), Some("Ada"));
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
