//! Bounded registered tool declarations. Credentials and runtime grants live outside manifests.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::app_events::{APP_EVENT_MAX_CLASSES, AppEventDeclaration};

mod connection;
#[cfg(test)]
mod tests;
pub use connection::{AppConnection, valid_credential_reference};

pub const APP_MANIFEST_MAX_BYTES: usize = 262_144;
pub const APP_ARGUMENT_MAX_BYTES: usize = 1_048_576;
const MAX_TOOLS: usize = 64;
const MAX_SCOPES: usize = 64;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AppManifest {
    pub schema_version: u32,
    pub scopes: BTreeSet<String>,
    pub tools: Vec<AppTool>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub events: Vec<AppEventDeclaration>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AppTool {
    pub name: String,
    pub input_schema: Value,
    pub output_schema: Option<Value>,
    pub required_scopes: BTreeSet<String>,
    pub replay: AppReplayPolicy,
}

/// A write is replayable only through an explicit stable identity, never an upstream annotation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum AppReplayPolicy {
    ReadOnly,
    NonIdempotent,
    StableIdentity { property_path: Vec<String> },
}

pub struct CompiledAppManifest {
    manifest: AppManifest,
    tools: BTreeMap<String, CompiledTool>,
}

struct CompiledTool {
    input: jsonschema::Validator,
    output: Option<jsonschema::Validator>,
    index: usize,
}

impl AppManifest {
    pub fn compile(&self) -> anyhow::Result<CompiledAppManifest> {
        anyhow::ensure!(
            self.schema_version == 1,
            "unsupported app manifest schema version"
        );
        anyhow::ensure!(
            !self.tools.is_empty() && self.tools.len() <= MAX_TOOLS,
            "invalid app tool count"
        );
        anyhow::ensure!(
            !self.scopes.is_empty()
                && self.scopes.len() <= MAX_SCOPES
                && self.scopes.iter().all(|scope| valid_name(scope)),
            "invalid app scopes"
        );
        anyhow::ensure!(
            self.events.len() <= APP_EVENT_MAX_CLASSES,
            "invalid app event count"
        );
        let mut event_names = BTreeSet::new();
        for event in &self.events {
            anyhow::ensure!(
                valid_name(&event.name)
                    && event_names.insert(&event.name)
                    && !event.required_scopes.is_empty()
                    && event.required_scopes.is_subset(&self.scopes),
                "invalid app event declaration"
            );
        }
        // Bound nested values before serializing or cloning the complete declaration.
        for tool in &self.tools {
            anyhow::ensure!(valid_name(&tool.name), "invalid app tool name");
            anyhow::ensure!(
                !tool.required_scopes.is_empty() && tool.required_scopes.is_subset(&self.scopes),
                "app tool requires undeclared scopes"
            );
            if let AppReplayPolicy::StableIdentity { property_path } = &tool.replay {
                anyhow::ensure!(
                    !property_path.is_empty()
                        && property_path.len() <= 8
                        && property_path.iter().all(|part| valid_name(part)),
                    "invalid app stable identity path"
                );
                validate_identity_schema(&tool.input_schema, property_path)?;
            }
            bounded_value(&tool.input_schema, APP_MANIFEST_MAX_BYTES)?;
            if let Some(output) = &tool.output_schema {
                bounded_value(output, APP_MANIFEST_MAX_BYTES)?;
            }
        }
        bounded_value(&serde_json::to_value(self)?, APP_MANIFEST_MAX_BYTES)?;
        let mut tools = BTreeMap::new();
        for (index, tool) in self.tools.iter().enumerate() {
            let compiled = CompiledTool {
                input: compile_schema(&tool.input_schema)?,
                output: tool
                    .output_schema
                    .as_ref()
                    .map(compile_schema)
                    .transpose()?,
                index,
            };
            anyhow::ensure!(
                tools.insert(tool.name.clone(), compiled).is_none(),
                "duplicate app tool name"
            );
        }
        Ok(CompiledAppManifest {
            manifest: self.clone(),
            tools,
        })
    }
}

impl CompiledAppManifest {
    pub fn manifest(&self) -> &AppManifest {
        &self.manifest
    }

    pub fn allowed_events(&self, granted: &BTreeSet<String>) -> anyhow::Result<BTreeSet<String>> {
        anyhow::ensure!(
            granted.is_subset(&self.manifest.scopes),
            "app grant includes undeclared scopes"
        );
        Ok(self
            .manifest
            .events
            .iter()
            .filter(|event| event.required_scopes.is_subset(granted))
            .map(|event| event.name.clone())
            .collect())
    }

    pub fn allowed_tools(&self, granted: &BTreeSet<String>) -> anyhow::Result<BTreeSet<String>> {
        anyhow::ensure!(
            granted.is_subset(&self.manifest.scopes),
            "app grant includes undeclared scopes"
        );
        Ok(self
            .manifest
            .tools
            .iter()
            .filter(|tool| tool.required_scopes.is_subset(granted))
            .map(|tool| tool.name.clone())
            .collect())
    }

    /// Discovery is an upstream claim; the pinned declaration is the authority for compatibility.
    pub fn validate_declaration(
        &self,
        declaration: &Value,
        granted: &BTreeSet<String>,
    ) -> anyhow::Result<()> {
        let name = declaration
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("invalid app discovery declaration"))?;
        let tool = self.authorized_tool(name, granted)?;
        anyhow::ensure!(
            declaration.get("inputSchema") == Some(&tool.input_schema)
                && declaration.get("outputSchema") == tool.output_schema.as_ref(),
            "app discovery schema differs from its pinned manifest"
        );
        Ok(())
    }

    pub fn validate_arguments(
        &self,
        name: &str,
        schema: &Value,
        arguments: &Value,
        granted: &BTreeSet<String>,
    ) -> anyhow::Result<()> {
        let tool = self.authorized_tool(name, granted)?;
        anyhow::ensure!(schema == &tool.input_schema, "app input schema changed");
        bounded_value(arguments, APP_ARGUMENT_MAX_BYTES)?;
        anyhow::ensure!(
            self.tools[name].input.is_valid(arguments),
            "app arguments violate the declared schema"
        );
        Ok(())
    }

    pub fn validate_output(&self, name: &str, output: &Value) -> anyhow::Result<()> {
        let tool = self
            .tools
            .get(name)
            .ok_or_else(|| anyhow::anyhow!("undeclared app tool"))?;
        bounded_value(output, APP_ARGUMENT_MAX_BYTES)?;
        if let Some(schema) = &tool.output {
            anyhow::ensure!(
                schema.is_valid(output),
                "app output violates the declared schema"
            );
        }
        Ok(())
    }

    fn authorized_tool(&self, name: &str, granted: &BTreeSet<String>) -> anyhow::Result<&AppTool> {
        anyhow::ensure!(
            granted.is_subset(&self.manifest.scopes),
            "app grant includes undeclared scopes"
        );
        let compiled = self
            .tools
            .get(name)
            .ok_or_else(|| anyhow::anyhow!("undeclared app tool"))?;
        let tool = &self.manifest.tools[compiled.index];
        anyhow::ensure!(
            tool.required_scopes.is_subset(granted),
            "app tool scope is not granted"
        );
        Ok(tool)
    }
}

pub fn valid_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-_.:".contains(&byte))
}

/// Bound the complete native payload, including content outside a declared structured output.
pub fn validate_app_payload(value: &Value) -> anyhow::Result<()> {
    bounded_value(value, APP_ARGUMENT_MAX_BYTES)
}

fn bounded_value(value: &Value, bytes: usize) -> anyhow::Result<()> {
    let mut pending = vec![(value, 0)];
    let mut nodes = 0;
    while let Some((value, depth)) = pending.pop() {
        nodes += 1;
        anyhow::ensure!(
            depth <= 64 && nodes <= 32_768,
            "app JSON structure exceeds its limit"
        );
        match value {
            Value::Object(map) => {
                anyhow::ensure!(
                    nodes + pending.len() + map.len() <= 32_768,
                    "app JSON structure exceeds its limit"
                );
                pending.extend(map.values().map(|value| (value, depth + 1)));
            }
            Value::Array(items) => {
                anyhow::ensure!(
                    nodes + pending.len() + items.len() <= 32_768,
                    "app JSON structure exceeds its limit"
                );
                pending.extend(items.iter().map(|value| (value, depth + 1)));
            }
            _ => {}
        }
    }
    serde_json::to_writer(JsonBudget(bytes), value)
        .map_err(|_| anyhow::anyhow!("app JSON exceeds its size limit"))?;
    Ok(())
}

struct JsonBudget(usize);

impl std::io::Write for JsonBudget {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0 = self
            .0
            .checked_sub(bytes.len())
            .ok_or_else(|| std::io::Error::other("app JSON size limit"))?;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn compile_schema(schema: &Value) -> anyhow::Result<jsonschema::Validator> {
    anyhow::ensure!(
        schema.is_object() && schema["type"] == "object",
        "app schemas must describe objects"
    );
    if let Some(draft) = schema.get("$schema") {
        anyhow::ensure!(
            draft == "https://json-schema.org/draft/2020-12/schema",
            "unsupported app JSON Schema draft"
        );
    }
    // The crate disables HTTP/file resolution. Linear-time patterns avoid untrusted backtracking.
    jsonschema::options()
        .with_draft(jsonschema::Draft::Draft202012)
        .with_pattern_options(jsonschema::PatternOptions::regex().size_limit(65_536))
        .should_validate_formats(true)
        .should_ignore_unknown_formats(false)
        .build(schema)
        .map_err(|_| anyhow::anyhow!("invalid or unsupported app JSON Schema"))
}

fn validate_identity_schema(schema: &Value, path: &[String]) -> anyhow::Result<()> {
    let mut current = schema;
    for part in path {
        anyhow::ensure!(
            current["type"] == "object"
                && current["required"]
                    .as_array()
                    .is_some_and(|required| required.iter().any(|value| value == part)),
            "app stable identity must be required by its schema"
        );
        current = current
            .get("properties")
            .and_then(|properties| properties.get(part))
            .ok_or_else(|| anyhow::anyhow!("app stable identity is absent from its schema"))?;
    }
    anyhow::ensure!(
        current["type"] == "string",
        "app stable identity must be a string"
    );
    Ok(())
}
