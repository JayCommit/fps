//! Native template schema, interpolation, catalogue helpers, and Egg import.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const NATIVE_TEMPLATE_SCHEMA_VERSION: &str = "1";
pub const NATIVE_TEMPLATE_KIND: &str = "fps.template";

pub const MIN_READ_SCHEMA: u32 = 1;
pub const MAX_WRITE_SCHEMA: u32 = 1;

pub fn schema_supported(version: u32) -> bool {
    version >= MIN_READ_SCHEMA && version <= MAX_WRITE_SCHEMA
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeTemplate {
    pub kind: String,
    pub schema_version: u32,
    pub name: String,
    pub slug: String,
    pub description: String,
    pub docker_image: String,
    #[serde(default)]
    pub startup: Option<String>,
    #[serde(default)]
    pub environment: BTreeMap<String, String>,
    #[serde(default)]
    pub ports: Vec<NativePort>,
    #[serde(default)]
    pub memory_mb: u32,
    #[serde(default)]
    pub cpu_shares: u32,
    #[serde(default)]
    pub volume_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativePort {
    pub name: String,
    pub protocol: String,
    pub container_port: u16,
}

impl NativeTemplate {
    pub fn validate(&self) -> Result<(), String> {
        if self.kind != NATIVE_TEMPLATE_KIND {
            return Err(format!("kind must be {NATIVE_TEMPLATE_KIND}"));
        }
        if !schema_supported(self.schema_version) {
            return Err(format!("unsupported schema {}", self.schema_version));
        }
        if self.name.trim().is_empty() || self.slug.trim().is_empty() {
            return Err("name and slug are required".into());
        }
        if self.docker_image.trim().is_empty() {
            return Err("docker_image is required".into());
        }
        if !self
            .slug
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        {
            return Err("slug must be lowercase alphanumeric plus hyphen".into());
        }
        Ok(())
    }

    pub fn with_defaults(mut self) -> Self {
        if self.memory_mb == 0 {
            self.memory_mb = 64;
        }
        if self.cpu_shares == 0 {
            self.cpu_shares = 1024;
        }
        if self.volume_path.trim().is_empty() {
            self.volume_path = "/data".into();
        }
        if self.kind.is_empty() {
            self.kind = NATIVE_TEMPLATE_KIND.into();
        }
        if self.schema_version == 0 {
            self.schema_version = 1;
        }
        self
    }
}

pub fn interpolate(input: &str, vars: &BTreeMap<String, String>) -> String {
    let mut out = input.to_string();
    for (key, value) in vars {
        out = out.replace(&format!("{{{{{key}}}}}"), value);
        out = out.replace(&format!("${{{key}}}"), value);
    }
    out
}

pub fn interpolate_map(
    map: &BTreeMap<String, String>,
    vars: &BTreeMap<String, String>,
) -> BTreeMap<String, String> {
    map.iter()
        .map(|(k, v)| (k.clone(), interpolate(v, vars)))
        .collect()
}

pub fn import_egg(egg: &Value) -> Result<NativeTemplate, String> {
    let name = egg
        .get("name")
        .and_then(Value::as_str)
        .ok_or("egg is missing name")?
        .to_string();
    let description = egg
        .get("description")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let docker_image = egg
        .get("docker_image")
        .and_then(Value::as_str)
        .or_else(|| {
            egg.get("docker_images")
                .and_then(Value::as_object)
                .and_then(|m| m.values().next())
                .and_then(Value::as_str)
        })
        .ok_or("egg is missing docker_image")?
        .to_string();
    let startup = egg
        .get("startup")
        .and_then(Value::as_str)
        .map(str::to_string);
    let mut environment = BTreeMap::new();
    if let Some(vars) = egg.get("variables").and_then(Value::as_array) {
        for var in vars {
            let key = var
                .get("env_variable")
                .and_then(Value::as_str)
                .ok_or("egg variable missing env_variable")?;
            let value = var
                .get("default_value")
                .and_then(Value::as_str)
                .unwrap_or("");
            environment.insert(key.to_string(), value.to_string());
        }
    }
    Ok(NativeTemplate {
        kind: NATIVE_TEMPLATE_KIND.into(),
        schema_version: 1,
        name: name.clone(),
        slug: slugify(&name),
        description,
        docker_image,
        startup,
        environment,
        ports: vec![NativePort {
            name: "game".into(),
            protocol: "tcp".into(),
            container_port: 25565,
        }],
        memory_mb: 1024,
        cpu_shares: 1024,
        volume_path: "/data".into(),
    }
    .with_defaults())
}

pub fn slugify(name: &str) -> String {
    let mut s = String::new();
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            s.push(ch.to_ascii_lowercase());
        } else if !s.ends_with('-') && !s.is_empty() {
            s.push('-');
        }
    }
    s.trim_matches('-').to_string()
}

pub fn http_echo_catalogue() -> NativeTemplate {
    NativeTemplate {
        kind: NATIVE_TEMPLATE_KIND.into(),
        schema_version: 1,
        name: "HTTP Echo".into(),
        slug: "http-echo".into(),
        description: "Tiny demo workload (hashicorp/http-echo) used for local and CI deploy tests."
            .into(),
        docker_image: "hashicorp/http-echo:1.0.0".into(),
        startup: None,
        environment: BTreeMap::from([("ECHO_TEXT".into(), "fps".into())]),
        ports: vec![NativePort {
            name: "http".into(),
            protocol: "tcp".into(),
            container_port: 5678,
        }],
        memory_mb: 64,
        cpu_shares: 256,
        volume_path: "/data".into(),
    }
}

pub fn minecraft_catalogue() -> NativeTemplate {
    NativeTemplate {
        kind: NATIVE_TEMPLATE_KIND.into(),
        schema_version: 1,
        name: "Minecraft (itzg)".into(),
        slug: "minecraft-itzg".into(),
        description:
            "Vanilla Minecraft via itzg/minecraft-server. EULA must be accepted in environment."
                .into(),
        docker_image: "itzg/minecraft-server:java21".into(),
        startup: None,
        environment: BTreeMap::from([
            ("EULA".into(), "TRUE".into()),
            ("TYPE".into(), "VANILLA".into()),
            ("MEMORY".into(), "1G".into()),
        ]),
        ports: vec![NativePort {
            name: "game".into(),
            protocol: "tcp".into(),
            container_port: 25565,
        }],
        memory_mb: 1024,
        cpu_shares: 1024,
        volume_path: "/data".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alpha1_supports_schema_v1_identity_only() {
        assert!(schema_supported(1));
        assert!(!schema_supported(0));
        assert!(!schema_supported(2));
    }

    #[test]
    fn interpolates_brace_and_dollar() {
        let mut vars = BTreeMap::new();
        vars.insert("PORT".into(), "25565".into());
        assert_eq!(interpolate("listen {{PORT}}", &vars), "listen 25565");
        assert_eq!(interpolate("listen ${PORT}", &vars), "listen 25565");
    }

    #[test]
    fn imports_minimal_egg() {
        let egg = serde_json::json!({
            "name": "Paper",
            "description": "Minecraft paper",
            "docker_images": {"Java 21": "ghcr.io/pterodactyl/yolks:java_21"},
            "startup": "java -jar {{SERVER_JARFILE}}",
            "variables": [{
                "env_variable": "SERVER_JARFILE",
                "default_value": "server.jar"
            }]
        });
        let native = import_egg(&egg).unwrap();
        assert_eq!(native.slug, "paper");
        native.validate().unwrap();
    }
}
