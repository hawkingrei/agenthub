use std::collections::HashSet;

use base64::{Engine, engine::general_purpose::STANDARD};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde_json::Value;

use crate::{McpTransportError, protocol::ProtocolVersion};

#[derive(Clone)]
pub struct ToolHeaderPlan {
    fields: Vec<HeaderField>,
}

#[derive(Clone)]
struct HeaderField {
    name: HeaderName,
    path: Vec<String>,
    kind: Primitive,
}

#[derive(Clone, Copy)]
enum Primitive {
    String,
    Integer,
    Boolean,
}

impl ToolHeaderPlan {
    /// Validate once during discovery. Invalid header annotations exclude this tool over HTTP;
    /// callers must not silently remove the annotation and forward an inconsistent request.
    pub fn from_schema(schema: &Value) -> Result<Self, McpTransportError> {
        if !schema.is_object() {
            return Err(McpTransportError::InvalidMetadata);
        }
        let mut fields = Vec::new();
        collect_fields(schema, &mut Vec::new(), true, &mut fields)?;
        let mut names = HashSet::new();
        if fields.len() > 64 || fields.iter().any(|field| !names.insert(field.name.clone())) {
            return Err(McpTransportError::InvalidMetadata);
        }
        Ok(Self { fields })
    }

    fn append(&self, headers: &mut HeaderMap, arguments: &Value) -> Result<(), McpTransportError> {
        let mut bytes = 0usize;
        for field in &self.fields {
            let value = field
                .path
                .iter()
                .try_fold(arguments, |value, key| value.as_object()?.get(key));
            let Some(value) = value.filter(|value| !value.is_null()) else {
                continue;
            };
            let text = match field.kind {
                Primitive::String => value.as_str().map(str::to_owned),
                Primitive::Boolean => value.as_bool().map(|value| value.to_string()),
                Primitive::Integer => value
                    .as_f64()
                    .filter(|number| {
                        number.fract() == 0.0
                            && (-9_007_199_254_740_991.0..=9_007_199_254_740_991.0).contains(number)
                    })
                    .map(|number| (number as i64).to_string()),
            }
            .ok_or(McpTransportError::InvalidMetadata)?;
            let value = encode_value(&text)?;
            bytes = bytes
                .saturating_add(field.name.as_str().len())
                .saturating_add(value.as_bytes().len());
            if bytes > 16_384 {
                return Err(McpTransportError::MessageTooLarge);
            }
            headers.insert(field.name.clone(), value);
        }
        Ok(())
    }
}

fn collect_fields(
    schema: &Value,
    path: &mut Vec<String>,
    reachable: bool,
    fields: &mut Vec<HeaderField>,
) -> Result<(), McpTransportError> {
    if path.len() > 64 {
        return Err(McpTransportError::InvalidMetadata);
    }
    let Some(object) = schema.as_object() else {
        return Ok(());
    };
    if let Some(name) = object.get("x-mcp-header") {
        if !reachable || path.is_empty() || fields.len() >= 64 {
            return Err(McpTransportError::InvalidMetadata);
        }
        let name = name
            .as_str()
            .filter(|value| !value.is_empty() && value.len() <= 128)
            .ok_or(McpTransportError::InvalidMetadata)?;
        let name = HeaderName::from_bytes(format!("mcp-param-{name}").as_bytes())
            .map_err(|_| McpTransportError::InvalidMetadata)?;
        let kind = match object.get("type").and_then(Value::as_str) {
            Some("string") => Primitive::String,
            Some("integer") => Primitive::Integer,
            Some("boolean") => Primitive::Boolean,
            _ => return Err(McpTransportError::InvalidMetadata),
        };
        fields.push(HeaderField {
            name,
            path: path.clone(),
            kind,
        });
    }
    if let Some(properties) = object.get("properties").and_then(Value::as_object) {
        for (key, property) in properties {
            path.push(key.clone());
            collect_fields(property, path, reachable, fields)?;
            path.pop();
        }
    }
    // These values are subschemas, unlike examples/default/enum, which are instance data.
    for key in [
        "items",
        "additionalItems",
        "contains",
        "additionalProperties",
        "unevaluatedProperties",
        "unevaluatedItems",
        "propertyNames",
        "not",
        "if",
        "then",
        "else",
        "contentSchema",
    ] {
        if let Some(subschema) = object.get(key) {
            if let Some(subschemas) = subschema.as_array() {
                for subschema in subschemas {
                    collect_fields(subschema, path, false, fields)?;
                }
            } else {
                collect_fields(subschema, path, false, fields)?;
            }
        }
    }
    for key in ["allOf", "anyOf", "oneOf", "prefixItems"] {
        if let Some(subschemas) = object.get(key).and_then(Value::as_array) {
            for subschema in subschemas {
                collect_fields(subschema, path, false, fields)?;
            }
        }
    }
    for key in [
        "$defs",
        "definitions",
        "patternProperties",
        "dependentSchemas",
        "dependencies",
    ] {
        if let Some(subschemas) = object.get(key).and_then(Value::as_object) {
            for subschema in subschemas.values() {
                collect_fields(subschema, path, false, fields)?;
            }
        }
    }
    Ok(())
}

pub(super) fn add_request_metadata(
    headers: &mut HeaderMap,
    version: ProtocolVersion,
    message: &Value,
    tool_headers: Option<&ToolHeaderPlan>,
) -> Result<(), McpTransportError> {
    let params = message
        .get("params")
        .and_then(Value::as_object)
        .ok_or(McpTransportError::InvalidMetadata)?;
    let metadata = params
        .get("_meta")
        .and_then(Value::as_object)
        .ok_or(McpTransportError::InvalidMetadata)?;
    if metadata
        .get("io.modelcontextprotocol/protocolVersion")
        .and_then(Value::as_str)
        != Some(version.as_str())
        || !metadata
            .get("io.modelcontextprotocol/clientInfo")
            .is_some_and(Value::is_object)
        || !metadata
            .get("io.modelcontextprotocol/clientCapabilities")
            .is_some_and(Value::is_object)
    {
        return Err(McpTransportError::InvalidMetadata);
    }
    let method = message["method"]
        .as_str()
        .ok_or(McpTransportError::InvalidMessage)?;
    headers.insert(
        "mcp-method",
        HeaderValue::from_str(method).map_err(|_| McpTransportError::InvalidMetadata)?,
    );
    let name_field = match method {
        "tools/call" | "prompts/get" => Some("name"),
        "resources/read" => Some("uri"),
        _ => None,
    };
    if let Some(field) = name_field {
        let name = params
            .get(field)
            .and_then(Value::as_str)
            .ok_or(McpTransportError::InvalidMetadata)?;
        if name.len() > 4096 {
            return Err(McpTransportError::MessageTooLarge);
        }
        headers.insert("mcp-name", encode_value(name)?);
    }
    if method == "tools/call" {
        let plan = tool_headers.ok_or(McpTransportError::InvalidMetadata)?;
        plan.append(headers, params.get("arguments").unwrap_or(&Value::Null))?;
    }
    Ok(())
}

fn encode_value(text: &str) -> Result<HeaderValue, McpTransportError> {
    if text.len() > 16_384 {
        return Err(McpTransportError::MessageTooLarge);
    }
    let plain = text.trim() == text
        && text
            .bytes()
            .all(|byte| byte == b'\t' || (0x20..=0x7e).contains(&byte))
        && !(text.starts_with("=?base64?") && text.ends_with("?="));
    let encoded = if plain {
        text.to_owned()
    } else {
        format!("=?base64?{}?=", STANDARD.encode(text.as_bytes()))
    };
    let mut value =
        HeaderValue::from_str(&encoded).map_err(|_| McpTransportError::InvalidMetadata)?;
    value.set_sensitive(true);
    Ok(value)
}
