use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq)]
pub enum WxmlNode {
    Element(WxmlElement),
    Text(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct WxmlElement {
    pub tag: String,
    pub attrs: BTreeMap<String, String>,
    pub children: Vec<WxmlNode>,
}

pub fn parse_wxml(source: &str) -> Result<WxmlNode, WxmlParseError> {
    Parser::new(source).parse_document()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WxmlParseError {
    message: String,
}

impl WxmlParseError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for WxmlParseError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for WxmlParseError {}

struct Parser<'a> {
    source: &'a str,
    position: usize,
}

impl<'a> Parser<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            source,
            position: 0,
        }
    }

    fn parse_document(&mut self) -> Result<WxmlNode, WxmlParseError> {
        let children = self.parse_children(None)?;
        let mut meaningful = children
            .into_iter()
            .filter(|node| !matches!(node, WxmlNode::Text(text) if text.trim().is_empty()))
            .collect::<Vec<_>>();
        match meaningful.len() {
            1 => Ok(meaningful.remove(0)),
            0 => Err(WxmlParseError::new("WXML document is empty")),
            _ => Ok(WxmlNode::Element(WxmlElement {
                tag: "view".to_owned(),
                attrs: BTreeMap::new(),
                children: meaningful,
            })),
        }
    }

    fn parse_children(
        &mut self,
        closing_tag: Option<&str>,
    ) -> Result<Vec<WxmlNode>, WxmlParseError> {
        let mut children = Vec::new();
        while self.position < self.source.len() {
            if self.starts_with("</") {
                let tag = self.read_closing_tag()?;
                if closing_tag == Some(tag.as_str()) {
                    return Ok(children);
                }
                return Err(WxmlParseError::new(format!(
                    "unexpected closing tag `{tag}`"
                )));
            }

            if self.starts_with("<") {
                children.push(WxmlNode::Element(self.read_element()?));
            } else {
                children.push(WxmlNode::Text(self.read_text()));
            }
        }

        if let Some(tag) = closing_tag {
            return Err(WxmlParseError::new(format!("missing closing tag `{tag}`")));
        }

        Ok(children)
    }

    fn read_element(&mut self) -> Result<WxmlElement, WxmlParseError> {
        self.expect("<")?;
        let inside = self.read_until(">")?;
        let (inside, self_closing) = split_self_closing_tag(&inside);
        let (tag, attrs) = parse_tag_inside(inside)?;
        let children = if self_closing {
            Vec::new()
        } else {
            self.parse_children(Some(&tag))?
        };

        Ok(WxmlElement {
            tag,
            attrs,
            children,
        })
    }

    fn read_closing_tag(&mut self) -> Result<String, WxmlParseError> {
        self.expect("</")?;
        let tag = self.read_until(">")?;
        Ok(tag.trim().to_owned())
    }

    fn read_text(&mut self) -> String {
        let rest = &self.source[self.position..];
        let end = rest.find('<').unwrap_or(rest.len());
        self.position += end;
        rest[..end].to_owned()
    }

    fn read_until(&mut self, needle: &str) -> Result<String, WxmlParseError> {
        let rest = &self.source[self.position..];
        let Some(index) = rest.find(needle) else {
            return Err(WxmlParseError::new(format!("missing `{needle}`")));
        };
        let value = rest[..index].to_owned();
        self.position += index + needle.len();
        Ok(value)
    }

    fn expect(&mut self, expected: &str) -> Result<(), WxmlParseError> {
        if self.starts_with(expected) {
            self.position += expected.len();
            return Ok(());
        }
        Err(WxmlParseError::new(format!("expected `{expected}`")))
    }

    fn starts_with(&self, value: &str) -> bool {
        self.source[self.position..].starts_with(value)
    }
}

fn split_self_closing_tag(source: &str) -> (&str, bool) {
    let trimmed = source.trim_end();
    if let Some(inside) = trimmed.strip_suffix('/') {
        (inside.trim_end(), true)
    } else {
        (trimmed, false)
    }
}

fn parse_tag_inside(source: &str) -> Result<(String, BTreeMap<String, String>), WxmlParseError> {
    let source = source.trim();
    if source.is_empty() {
        return Err(WxmlParseError::new("empty tag"));
    }

    let tag_end = source.find(char::is_whitespace).unwrap_or(source.len());
    let tag = source[..tag_end].to_owned();
    let attrs = parse_attrs(&source[tag_end..])?;
    Ok((tag, attrs))
}

fn parse_attrs(source: &str) -> Result<BTreeMap<String, String>, WxmlParseError> {
    let mut attrs = BTreeMap::new();
    let mut index = 0;
    let bytes = source.as_bytes();
    while index < source.len() {
        while index < source.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        if index >= source.len() {
            break;
        }

        let name_start = index;
        while index < source.len() && !bytes[index].is_ascii_whitespace() && bytes[index] != b'=' {
            index += 1;
        }
        let name = source[name_start..index].trim();
        while index < source.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        let value = if index < source.len() && bytes[index] == b'=' {
            index += 1;
            while index < source.len() && bytes[index].is_ascii_whitespace() {
                index += 1;
            }
            read_attr_value(source, &mut index)?
        } else {
            String::new()
        };
        if !name.is_empty() {
            attrs.insert(name.to_owned(), value);
        }
    }
    Ok(attrs)
}

fn read_attr_value(source: &str, index: &mut usize) -> Result<String, WxmlParseError> {
    let bytes = source.as_bytes();
    if *index >= source.len() {
        return Ok(String::new());
    }
    let quote = bytes[*index];
    if quote == b'"' || quote == b'\'' {
        *index += 1;
        let start = *index;
        while *index < source.len() && bytes[*index] != quote {
            *index += 1;
        }
        if *index >= source.len() {
            return Err(WxmlParseError::new("unterminated attribute value"));
        }
        let value = source[start..*index].to_owned();
        *index += 1;
        return Ok(value);
    }

    let start = *index;
    while *index < source.len() && !bytes[*index].is_ascii_whitespace() {
        *index += 1;
    }
    Ok(source[start..*index].to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_nested_p0_tags_and_attrs() {
        let root = parse_wxml(
            r#"<view class="card"><text>{{title}}</text><image src="{{image}}" bindload="onLoad" /></view>"#,
        )
        .expect("wxml parses");

        let WxmlNode::Element(root) = root else {
            panic!("root should be element");
        };
        assert_eq!(root.tag, "view");
        assert_eq!(root.attrs.get("class").map(String::as_str), Some("card"));
        assert_eq!(root.children.len(), 2);
    }

    #[test]
    fn reports_missing_closing_tag() {
        let error = parse_wxml("<view><text>bad</view>").expect_err("invalid nesting fails");

        assert!(error.to_string().contains("unexpected closing tag"));
    }

    #[test]
    fn slash_inside_attribute_value_does_not_make_tag_self_closing() {
        let root = parse_wxml(r#"<view data-path="/menu/">inside</view>"#).expect("wxml parses");

        let WxmlNode::Element(root) = root else {
            panic!("root should be element");
        };
        assert_eq!(
            root.attrs.get("data-path").map(String::as_str),
            Some("/menu/")
        );
        assert_eq!(root.children, vec![WxmlNode::Text("inside".to_owned())]);
    }
}
