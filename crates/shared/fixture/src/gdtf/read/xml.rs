use crate::ProfileError;
use quick_xml::{Reader, XmlVersion, events::Event};
use std::collections::BTreeMap;

#[derive(Debug)]
pub(super) struct Node {
    pub name: String,
    pub attributes: BTreeMap<String, String>,
    pub children: Vec<Node>,
}

impl Node {
    pub fn attr(&self, key: &str) -> Option<&str> {
        self.attributes.get(key).map(String::as_str)
    }

    pub fn required(&self, key: &str) -> Result<&str, ProfileError> {
        self.attr(key)
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| ProfileError::Invalid(format!("GDTF {} requires {key}", self.name)))
    }

    pub fn children_named<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Node> {
        self.children.iter().filter(move |node| node.name == name)
    }

    pub fn child(&self, name: &str) -> Option<&Node> {
        self.children.iter().find(|node| node.name == name)
    }
}

pub(super) fn parse(xml: &str) -> Result<Node, ProfileError> {
    let invalid = |message: String| ProfileError::Invalid(format!("invalid GDTF XML: {message}"));
    let mut reader = Reader::from_str(xml);
    let mut stack: Vec<Node> = Vec::new();
    let mut root = None;
    let mut count = 0_usize;
    loop {
        let event = reader
            .read_event()
            .map_err(|error| invalid(error.to_string()))?;
        let empty = matches!(&event, Event::Empty(_));
        match event {
            Event::Start(element) | Event::Empty(element) => {
                count += 1;
                if stack.len() >= 128 || count > 100_000 {
                    return Err(invalid("element/depth limit exceeded".into()));
                }
                let mut attributes = BTreeMap::new();
                for attribute in element.attributes() {
                    let attribute = attribute.map_err(|error| invalid(error.to_string()))?;
                    let key = std::str::from_utf8(attribute.key.as_ref())
                        .map_err(|error| invalid(error.to_string()))?;
                    let value = attribute
                        .decoded_and_normalized_value(XmlVersion::Implicit1_0, reader.decoder())
                        .map_err(|error| invalid(error.to_string()))?;
                    attributes.insert(key.to_owned(), value.into_owned());
                }
                let node = Node {
                    name: std::str::from_utf8(element.name().as_ref())
                        .map_err(|error| invalid(error.to_string()))?
                        .to_owned(),
                    attributes,
                    children: Vec::new(),
                };
                if empty {
                    attach(node, &mut stack, &mut root)?;
                } else {
                    stack.push(node);
                }
            }
            Event::End(_) => {
                let node = stack
                    .pop()
                    .ok_or_else(|| invalid("unmatched closing tag".into()))?;
                attach(node, &mut stack, &mut root)?;
            }
            Event::DocType(_) => {
                return Err(invalid(
                    "document type declarations are not supported".into(),
                ));
            }
            Event::Eof => break,
            _ => {}
        }
    }
    if !stack.is_empty() {
        return Err(invalid("unclosed element".into()));
    }
    root.ok_or_else(|| invalid("missing document root".into()))
}

fn attach(node: Node, stack: &mut [Node], root: &mut Option<Node>) -> Result<(), ProfileError> {
    if let Some(parent) = stack.last_mut() {
        parent.children.push(node);
    } else if root.replace(node).is_some() {
        return Err(ProfileError::Invalid("GDTF XML has multiple roots".into()));
    }
    Ok(())
}
