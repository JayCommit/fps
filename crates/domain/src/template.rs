use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::TemplateId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum TemplateSource {
    Native,
    EggImport,
}

impl TemplateSource {
    pub fn parse(value: &str) -> Self {
        match value {
            "egg_import" => Self::EggImport,
            _ => Self::Native,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Native => "native",
            Self::EggImport => "egg_import",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PortMapping {
    pub name: String,
    pub protocol: String,
    pub container_port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TemplateSummary {
    pub id: TemplateId,
    pub name: String,
    pub slug: String,
    #[serde(default)]
    pub game: String,
    pub description: String,
    pub docker_image: String,
    pub startup_command: Option<String>,
    pub memory_mb: i32,
    pub cpu_shares: i32,
    pub volume_path: String,
    pub source: TemplateSource,
    pub ports: Vec<PortMapping>,
    #[serde(default)]
    pub environment: BTreeMap<String, String>,
    pub created_at: DateTime<Utc>,
}
