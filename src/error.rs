use thiserror::Error;

#[derive(Error, Debug)]
pub enum CeceError {
    #[error("workspace '{0}' not found")]
    WorkspaceNotFound(String),
    #[error("agent '{0}' not found")]
    AgentNotFound(String),
    #[error("template '{0}' not found")]
    TemplateNotFound(String),
    #[error("workspace '{0}' already exists")]
    WorkspaceExists(String),
    #[error("agent '{0}' already exists")]
    AgentExists(String),
    #[error("cece is not initialized — run `cece init` first")]
    NotInitialized,
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("git error: {0}")]
    Git(String),
    #[error("cmux error: {0}")]
    Cmux(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, CeceError>;
