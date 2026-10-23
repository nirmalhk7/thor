use serde::{Deserialize, Serialize};
use serde_json::Value;

macro_rules! identifier {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub(super) String);
        impl $name {
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
    };
}

identifier!(ProjectId);
identifier!(DocumentId);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DocumentKind {
    Collection,
    Request,
    Environment,
    Scenario,
    Finding,
    Run,
    Evidence,
}

impl DocumentKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Collection => "collection",
            Self::Request => "request",
            Self::Environment => "environment",
            Self::Scenario => "scenario",
            Self::Finding => "finding",
            Self::Run => "run",
            Self::Evidence => "evidence",
        }
    }

    pub(super) fn parse(kind: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_value(Value::String(kind.into()))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: ProjectId,
    pub name: String,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    pub id: DocumentId,
    pub project_id: ProjectId,
    pub kind: DocumentKind,
    pub title: String,
    pub payload: Value,
    pub revision: u64,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
}
