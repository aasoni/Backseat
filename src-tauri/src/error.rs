use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("not a git repository: {0}")]
    NotARepo(String),
    #[error("git: {0}")]
    Git(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("no project is open")]
    NoProject,
    #[error("a review round is already in progress")]
    RoundInFlight,
    #[error("agent command not found. Is Claude Code installed and on your PATH?")]
    AgentNotFound,
    #[error("the proposed change no longer matches the file; it may be outdated")]
    SuggestionOutdated,
    #[error("{0}")]
    Other(String),
}

impl Serialize for Error {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

pub type Result<T> = std::result::Result<T, Error>;
