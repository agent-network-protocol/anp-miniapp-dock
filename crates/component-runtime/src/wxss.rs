use crate::render_ir::RenderStyle;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Default)]
pub struct WxssStyleSheet {
    classes: BTreeMap<String, RenderStyle>,
    ids: BTreeMap<String, RenderStyle>,
    tags: BTreeMap<String, RenderStyle>,
    descendants: Vec<DescendantStyleRule>,
    warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
struct DescendantStyleRule {
    ancestor: SimpleSelector,
    target: SimpleSelector,
    style: RenderStyle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SimpleSelector {
    Class(String),
    Id(String),
    Tag(String),
}

impl WxssStyleSheet {
    pub fn parse(source: &str) -> Self {
        let mut sheet = Self::default();
        let mut rest = source;
        while let Some(start) = rest.find('{') {
            let selector = rest[..start].trim();
            let after_start = &rest[start + 1..];
            let Some(end) = after_start.find('}') else {
                sheet.warnings.push("unterminated WXSS rule".to_owned());
                break;
            };
            let body = &after_start[..end];
            rest = &after_start[end + 1..];

            let style = parse_declarations(body, &mut sheet.warnings);
            if let Some((ancestor, target)) = parse_descendant_selector(selector) {
                sheet.descendants.push(DescendantStyleRule {
                    ancestor,
                    target,
                    style,
                });
            } else if let Some(class_name) = selector.strip_prefix('.') {
                if let Some(class_name) = non_empty_selector(class_name) {
                    sheet.classes.insert(class_name.to_owned(), style);
                } else {
                    sheet
                        .warnings
                        .push(format!("unsupported selector `{}`", selector.trim()));
                }
            } else if let Some(id) = selector.strip_prefix('#') {
                if let Some(id) = non_empty_selector(id) {
                    sheet.ids.insert(id.to_owned(), style);
                } else {
                    sheet
                        .warnings
                        .push(format!("unsupported selector `{}`", selector.trim()));
                }
            } else if is_simple_tag_selector(selector.trim()) {
                sheet.tags.insert(selector.trim().to_owned(), style);
            } else if !selector.trim().is_empty() {
                sheet
                    .warnings
                    .push(format!("unsupported selector `{}`", selector.trim()));
            }
        }
        sheet
    }

    pub fn class_style(&self, class_name: &str) -> Option<&RenderStyle> {
        self.classes.get(class_name)
    }

    pub fn id_style(&self, id: &str) -> Option<&RenderStyle> {
        self.ids.get(id)
    }

    pub fn tag_style(&self, tag: &str) -> Option<&RenderStyle> {
        self.tags.get(tag)
    }

    pub fn matching_descendant_styles<'a>(
        &'a self,
        ancestors: &'a [SimpleSelector],
        target: &'a [SimpleSelector],
    ) -> impl Iterator<Item = &'a RenderStyle> + 'a {
        self.descendants
            .iter()
            .filter(move |rule| {
                ancestors.iter().any(|selector| selector == &rule.ancestor)
                    && target.iter().any(|selector| selector == &rule.target)
            })
            .map(|rule| &rule.style)
    }

    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }
}

pub fn parse_inline_style(source: &str) -> (RenderStyle, Vec<String>) {
    let mut warnings = Vec::new();
    let style = parse_declarations(source, &mut warnings);
    (style, warnings)
}

pub fn merge_styles(base: &mut RenderStyle, overlay: &RenderStyle) {
    merge_optional(&mut base.display, &overlay.display);
    merge_optional(&mut base.flex_direction, &overlay.flex_direction);
    merge_optional(&mut base.gap, &overlay.gap);
    merge_optional(&mut base.justify_content, &overlay.justify_content);
    merge_optional(&mut base.align_items, &overlay.align_items);
    merge_optional(&mut base.width, &overlay.width);
    merge_optional(&mut base.height, &overlay.height);
    merge_optional(&mut base.min_width, &overlay.min_width);
    merge_optional(&mut base.max_width, &overlay.max_width);
    merge_optional(&mut base.min_height, &overlay.min_height);
    merge_optional(&mut base.max_height, &overlay.max_height);
    merge_optional(&mut base.margin, &overlay.margin);
    merge_optional(&mut base.padding, &overlay.padding);
    merge_optional(&mut base.color, &overlay.color);
    merge_optional(&mut base.background, &overlay.background);
    merge_optional(&mut base.opacity, &overlay.opacity);
    merge_optional(&mut base.font_size, &overlay.font_size);
    merge_optional(&mut base.font_weight, &overlay.font_weight);
    merge_optional(&mut base.line_height, &overlay.line_height);
    merge_optional(&mut base.border, &overlay.border);
    merge_optional(&mut base.border_radius, &overlay.border_radius);
    merge_optional(&mut base.box_shadow, &overlay.box_shadow);
    merge_optional(&mut base.text_align, &overlay.text_align);
    merge_optional(&mut base.overflow_x, &overlay.overflow_x);
    base.extra.extend(overlay.extra.clone());
}

fn parse_declarations(source: &str, warnings: &mut Vec<String>) -> RenderStyle {
    let mut style = RenderStyle::default();
    for declaration in source.split(';') {
        let declaration = declaration.trim();
        if declaration.is_empty() {
            continue;
        }
        let Some((name, value)) = declaration.split_once(':') else {
            warnings.push(format!("unsupported declaration `{declaration}`"));
            continue;
        };
        set_style_property(
            &mut style,
            name.trim(),
            normalize_unit(value.trim()),
            warnings,
        );
    }
    style
}

fn set_style_property(
    style: &mut RenderStyle,
    name: &str,
    value: String,
    warnings: &mut Vec<String>,
) {
    match name {
        "display" => style.display = Some(value),
        "flex-direction" => style.flex_direction = Some(value),
        "gap" => style.gap = Some(value),
        "justify-content" => style.justify_content = Some(value),
        "align-items" => style.align_items = Some(value),
        "width" => style.width = Some(value),
        "height" => style.height = Some(value),
        "min-width" => style.min_width = Some(value),
        "max-width" => style.max_width = Some(value),
        "min-height" => style.min_height = Some(value),
        "max-height" => style.max_height = Some(value),
        "margin" => style.margin = Some(value),
        "padding" => style.padding = Some(value),
        "color" => style.color = Some(value),
        "background" | "background-color" => style.background = Some(value),
        "opacity" => style.opacity = Some(value),
        "font-size" => style.font_size = Some(value),
        "font-weight" => style.font_weight = Some(value),
        "line-height" => style.line_height = Some(value),
        "border" => style.border = Some(value),
        "border-radius" => style.border_radius = Some(value),
        "box-shadow" => style.box_shadow = Some(value),
        "text-align" => style.text_align = Some(value),
        "overflow-x" => style.overflow_x = Some(value),
        other => warnings.push(format!("unsupported style property `{other}`")),
    }
}

fn parse_descendant_selector(selector: &str) -> Option<(SimpleSelector, SimpleSelector)> {
    let parts = selector.split_whitespace().collect::<Vec<_>>();
    if parts.len() != 2 {
        return None;
    }
    Some((
        parse_simple_selector(parts[0])?,
        parse_simple_selector(parts[1])?,
    ))
}

fn parse_simple_selector(selector: &str) -> Option<SimpleSelector> {
    if let Some(class_name) = selector.strip_prefix('.') {
        return non_empty_selector(class_name).map(|value| SimpleSelector::Class(value.to_owned()));
    }
    if let Some(id) = selector.strip_prefix('#') {
        return non_empty_selector(id).map(|value| SimpleSelector::Id(value.to_owned()));
    }
    if is_simple_tag_selector(selector) {
        return Some(SimpleSelector::Tag(selector.to_owned()));
    }
    None
}

fn non_empty_selector(selector: &str) -> Option<&str> {
    let selector = selector.trim();
    (!selector.is_empty() && selector.chars().all(is_selector_char)).then_some(selector)
}

fn is_simple_tag_selector(selector: &str) -> bool {
    !selector.is_empty()
        && selector
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch == '-')
}

fn is_selector_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '-' || ch == '_'
}

fn normalize_unit(value: &str) -> String {
    value.replace("rpx", "px")
}

fn merge_optional(target: &mut Option<String>, source: &Option<String>) {
    if let Some(source) = source {
        *target = Some(source.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wxss_class_styles_map_to_render_style() {
        let sheet = WxssStyleSheet::parse(
            ".card { display: flex; flex-direction: row; padding: 24rpx; color: #333; }",
        );

        let style = sheet.class_style("card").expect("class style exists");

        assert_eq!(style.display.as_deref(), Some("flex"));
        assert_eq!(style.flex_direction.as_deref(), Some("row"));
        assert_eq!(style.padding.as_deref(), Some("24px"));
        assert_eq!(style.color.as_deref(), Some("#333"));
        assert!(sheet.warnings().is_empty());
    }

    #[test]
    fn unsupported_wxss_property_is_warning() {
        let sheet = WxssStyleSheet::parse(".card { transform: scale(1); }");

        assert!(sheet.warnings()[0].contains("transform"));
    }
}
