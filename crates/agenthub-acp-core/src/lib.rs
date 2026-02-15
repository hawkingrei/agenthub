use std::collections::HashMap;
use std::path::PathBuf;

use agent_client_protocol::{
    ContentBlock, EnvVariable, HttpHeader, McpCapabilities, McpServer, McpServerHttp,
    McpServerStdio, TextContent,
};
use serde::Deserialize;
use serde_json::{Map, Value};

#[derive(Debug, Clone)]
pub struct AcpSkill {
    pub name: String,
    pub path: String,
    pub instructions: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpConfigFile {
    #[serde(default, rename = "mcpServers")]
    pub mcp_servers: HashMap<String, McpServerConfigJson>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerConfigJson {
    pub command: Option<String>,
    pub args: Option<Vec<String>>,
    pub env: Option<HashMap<String, String>>,
    pub url: Option<String>,
    pub headers: Option<HashMap<String, String>>,
}

#[derive(Debug, Deserialize)]
pub struct SkillsConfigFile {
    #[serde(default)]
    pub skills: Vec<SkillEntryJson>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum SkillEntryJson {
    Path(String),
    Detailed { path: String, name: Option<String> },
}

#[derive(Debug, Clone)]
pub struct SkillEntry {
    pub path: String,
    pub name: Option<String>,
}

pub fn expand_tilde(path: &str) -> String {
    if path == "~" {
        std::env::var("HOME").unwrap_or_else(|_| path.to_string())
    } else if let Some(stripped) = path.strip_prefix("~/") {
        if let Ok(home) = std::env::var("HOME") {
            PathBuf::from(home)
                .join(stripped)
                .to_string_lossy()
                .to_string()
        } else {
            path.to_string()
        }
    } else {
        path.to_string()
    }
}

pub fn parse_mcp_config(contents: &str) -> Result<Vec<McpServer>, serde_json::Error> {
    let config: McpConfigFile = serde_json::from_str(contents)?;
    Ok(config
        .mcp_servers
        .into_iter()
        .filter_map(|(name, entry)| build_mcp_server(&name, &entry))
        .collect())
}

pub fn build_mcp_server(name: &str, entry: &McpServerConfigJson) -> Option<McpServer> {
    if let Some(command) = entry.command.as_deref() {
        let command = expand_tilde(command);
        let args = entry.args.clone().unwrap_or_default();
        let env = entry
            .env
            .clone()
            .unwrap_or_default()
            .into_iter()
            .map(|(key, value)| EnvVariable::new(key, value))
            .collect::<Vec<_>>();
        let server = McpServerStdio::new(name.to_string(), PathBuf::from(command))
            .args(args)
            .env(env);
        return Some(McpServer::Stdio(server));
    }

    if let Some(url) = entry.url.as_deref() {
        let headers = entry
            .headers
            .clone()
            .unwrap_or_default()
            .into_iter()
            .map(|(key, value)| HttpHeader::new(key, value))
            .collect::<Vec<_>>();
        let server = McpServerHttp::new(name.to_string(), url.to_string()).headers(headers);
        return Some(McpServer::Http(server));
    }

    tracing::warn!(
        "mcp server skipped: name={} reason=missing command/url",
        name
    );
    None
}

pub fn filter_mcp_servers(mcp_servers: Vec<McpServer>, caps: &McpCapabilities) -> Vec<McpServer> {
    mcp_servers
        .into_iter()
        .filter(|server| match server {
            McpServer::Http(_) => caps.http,
            McpServer::Sse(_) => caps.sse,
            McpServer::Stdio(_) => true,
            _ => false,
        })
        .collect()
}

pub fn parse_skills_config(contents: &str) -> Result<Vec<SkillEntry>, serde_json::Error> {
    let config: SkillsConfigFile = serde_json::from_str(contents)?;
    Ok(config
        .skills
        .into_iter()
        .map(|entry| match entry {
            SkillEntryJson::Path(path) => SkillEntry { path, name: None },
            SkillEntryJson::Detailed { path, name } => SkillEntry { path, name },
        })
        .collect())
}

pub fn extract_skill_name(contents: &str) -> Option<String> {
    let mut lines = contents.lines();
    let first = lines.next()?.trim();
    if first != "---" {
        return None;
    }
    for line in lines {
        let trimmed = line.trim();
        if trimmed == "---" || trimmed == "..." {
            break;
        }
        if let Some(rest) = trimmed.strip_prefix("name:") {
            let raw = rest.trim();
            let value = raw
                .strip_prefix('"')
                .and_then(|item| item.strip_suffix('"'))
                .or_else(|| {
                    raw.strip_prefix('\'')
                        .and_then(|item| item.strip_suffix('\''))
                })
                .unwrap_or(raw);
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

pub fn escape_skill_meta(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

pub fn sanitize_skill_contents(contents: &str) -> String {
    let lower = contents.to_ascii_lowercase();
    let mut output = String::with_capacity(contents.len());
    let mut last = 0;
    for (idx, _) in lower.match_indices("</skill") {
        output.push_str(&contents[last..idx]);
        output.push_str("<\\/skill");
        last = idx + "</skill".len();
    }
    output.push_str(&contents[last..]);
    output
}

pub fn build_skill(name: String, path: String, contents: &str) -> AcpSkill {
    let escaped_name = escape_skill_meta(&name);
    let escaped_path = escape_skill_meta(&path);
    let safe_contents = sanitize_skill_contents(contents);
    let instructions = format!(
        "<skill>\n<name>{}</name>\n<path>{}</path>\n{}\n</skill>",
        escaped_name, escaped_path, safe_contents
    );
    AcpSkill {
        name,
        path,
        instructions,
    }
}

pub fn build_skill_blocks(skills: &[AcpSkill]) -> Vec<ContentBlock> {
    skills
        .iter()
        .map(|skill| ContentBlock::Text(TextContent::new(skill.instructions.clone())))
        .collect()
}

pub fn build_skills_meta(skills: &[AcpSkill]) -> Option<Map<String, Value>> {
    if skills.is_empty() {
        return None;
    }
    let skill_items = skills
        .iter()
        .map(|skill| {
            serde_json::json!({
                "name": skill.name,
                "path": skill.path,
            })
        })
        .collect::<Vec<_>>();
    let mut agenthub = Map::new();
    agenthub.insert("skills".to_string(), Value::Array(skill_items));
    let mut meta = Map::new();
    meta.insert("agenthub".to_string(), Value::Object(agenthub));
    Some(meta)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn server_name(server: &McpServer) -> &str {
        match server {
            McpServer::Http(cfg) => cfg.name.as_str(),
            McpServer::Sse(cfg) => cfg.name.as_str(),
            McpServer::Stdio(cfg) => cfg.name.as_str(),
            _ => "unknown",
        }
    }

    #[test]
    fn parse_mcp_config_supports_stdio_and_http() {
        let json = r#"
        {
          "mcpServers": {
            "stdio": {
              "command": "node",
              "args": ["server.js"],
              "env": { "TOKEN": "abc" }
            },
            "http": {
              "url": "http://localhost:7777",
              "headers": { "Authorization": "Bearer xyz" }
            }
          }
        }
        "#;
        let servers = parse_mcp_config(json).expect("parse mcp config");
        assert_eq!(servers.len(), 2);

        let mut by_name = HashMap::new();
        for server in servers {
            by_name.insert(server_name(&server).to_string(), server);
        }

        match by_name.get("stdio") {
            Some(McpServer::Stdio(cfg)) => {
                assert_eq!(cfg.command, PathBuf::from("node"));
                assert_eq!(cfg.args, vec!["server.js".to_string()]);
                assert_eq!(cfg.env.len(), 1);
            }
            _ => panic!("missing stdio"),
        }

        match by_name.get("http") {
            Some(McpServer::Http(cfg)) => {
                assert_eq!(cfg.url, "http://localhost:7777".to_string());
                assert_eq!(cfg.headers.len(), 1);
            }
            _ => panic!("missing http"),
        }
    }

    #[test]
    fn parse_skills_config_supports_strings_and_objects() {
        let json = r#"
        {
          "skills": [
            "/tmp/skill-a.md",
            { "path": "/tmp/skill-b.md", "name": "Custom" }
          ]
        }
        "#;
        let entries = parse_skills_config(json).expect("parse skills config");
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].path, "/tmp/skill-a.md");
        assert_eq!(entries[0].name, None);
        assert_eq!(entries[1].path, "/tmp/skill-b.md");
        assert_eq!(entries[1].name, Some("Custom".to_string()));
    }

    #[test]
    fn extract_skill_name_reads_front_matter() {
        let contents = r#"---
name: demo-skill
description: sample
---
# Body
"#;
        assert_eq!(extract_skill_name(contents), Some("demo-skill".to_string()));
        assert_eq!(extract_skill_name("no front matter"), None);
    }
}
