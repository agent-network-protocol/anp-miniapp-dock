use crate::render_ir::RenderStyle;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Default)]
pub struct WxssStyleSheet {
    classes: BTreeMap<String, RenderStyle>,
    warnings: Vec<String>,
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

            if let Some(class_name) = selector.strip_prefix('.') {
                let style = parse_declarations(body, &mut sheet.warnings);
                sheet.classes.insert(class_name.trim().to_owned(), style);
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
    merge_optional(&mut base.width, &overlay.width);
    merge_optional(&mut base.height, &overlay.height);
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
    merge_optional(&mut base.text_align, &overlay.text_align);
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
        "width" => style.width = Some(value),
        "height" => style.height = Some(value),
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
        "text-align" => style.text_align = Some(value),
        other => warnings.push(format!("unsupported style property `{other}`")),
    }
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
