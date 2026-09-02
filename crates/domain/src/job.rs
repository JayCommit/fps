use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum JobKind {
    Install,
    Start,
    Stop,
    Backup,
    Restore,
    FilesList,
    FilesRead,
    FilesWrite,
    Exec,
}

impl JobKind {
    pub fn parse(value: &str) -> Self {
        match value {
            "start" => Self::Start,
            "stop" => Self::Stop,
            "backup" => Self::Backup,
            "restore" => Self::Restore,
            "files_list" => Self::FilesList,
            "files_read" => Self::FilesRead,
            "files_write" => Self::FilesWrite,
            "exec" => Self::Exec,
            _ => Self::Install,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Install => "install",
            Self::Start => "start",
            Self::Stop => "stop",
            Self::Backup => "backup",
            Self::Restore => "restore",
            Self::FilesList => "files_list",
            Self::FilesRead => "files_read",
            Self::FilesWrite => "files_write",
            Self::Exec => "exec",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    Queued,
    Dispatched,
    Succeeded,
    Failed,
}

impl JobStatus {
    pub fn parse(value: &str) -> Self {
        match value {
            "dispatched" => Self::Dispatched,
            "succeeded" => Self::Succeeded,
            "failed" => Self::Failed,
            _ => Self::Queued,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Dispatched => "dispatched",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
        }
    }
}
