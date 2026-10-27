use serde::Deserialize;
use serde_json::{json, Value};

use crate::api::{CancellationToken, HttpService, RequestSpec, Variables};
use crate::projects::{DocumentId, ProjectId, ProjectStore};

#[derive(Deserialize)]
#[serde(tag = "command", rename_all = "snake_case", deny_unknown_fields)]
pub enum WorkbenchCommand {
    FeatureList,
    FeatureInvoke {
        slug: String,
        #[serde(default)]
        payload: Value,
    },
    SecurityAnalyze {
        slug: String,
        #[serde(default)]
        payload: Value,
    },
    ApiSend {
        request: RequestSpec,
        #[serde(default)]
        variables: Variables,
    },
    SecretStore {
        value: String,
        #[serde(default)]
        persistent: bool,
    },
    ProjectList,
    ProjectCreate {
        name: String,
    },
    RequestList {
        project: ProjectId,
    },
    RequestOpen {
        id: DocumentId,
    },
    RequestSave {
        project: ProjectId,
        title: String,
        request: RequestSpec,
        #[serde(default)]
        id: Option<DocumentId>,
        #[serde(default)]
        revision: Option<u64>,
    },
}

pub struct Workbench<'a> {
    http: &'a HttpService,
    projects: &'a ProjectStore,
}

impl<'a> Workbench<'a> {
    pub fn new(http: &'a HttpService, projects: &'a ProjectStore) -> Self {
        Self { http, projects }
    }

    pub async fn execute(&self, command: WorkbenchCommand) -> Result<Value, String> {
        self.execute_cancellable(command, &CancellationToken::new())
            .await
    }

    pub async fn execute_cancellable(
        &self,
        command: WorkbenchCommand,
        cancel: &CancellationToken,
    ) -> Result<Value, String> {
        if cancel.is_cancelled() {
            return Err("Tool operation cancelled".into());
        }
        match command {
            WorkbenchCommand::FeatureList => {
                encode(crate::AppState::global().features().status_report())
            }
            WorkbenchCommand::FeatureInvoke { slug, payload } => {
                if payload.get("from_active_tab").and_then(Value::as_bool) == Some(true) {
                    return Err("Paste captured evidence as input; active-page evaluation is unavailable from this tool tab".into());
                }
                crate::features::orchestrator::FeatureOrchestrator::invoke(&slug, &payload)
            }
            WorkbenchCommand::SecurityAnalyze { slug, payload } => {
                if !matches!(
                    slug.as_str(),
                    "header-analyzer" | "jwt-deep" | "response-diff"
                ) {
                    return Err(
                        "This workspace supports local header, JWT, and response analysis only"
                            .into(),
                    );
                }
                crate::features::orchestrator::FeatureOrchestrator::invoke(&slug, &payload)
            }
            WorkbenchCommand::ApiSend { request, variables } => {
                let response = self
                    .http
                    .send_cancellable(&request, &variables, cancel)
                    .await
                    .map_err(|error| error.to_string())?;
                encode(response)
            }
            WorkbenchCommand::SecretStore { value, persistent } => {
                let http = self.http.clone();
                let value = zeroize::Zeroizing::new(value);
                let reference = tokio::task::spawn_blocking(move || {
                    if persistent {
                        http.secrets().store_persistent(&value)
                    } else {
                        http.secrets().store_temporary(&value)
                    }
                })
                .await
                .map_err(|_| "Credential storage worker failed".to_string())?
                .map_err(|error| error.to_string())?;
                Ok(json!({"reference":reference}))
            }
            command => super::persistence::execute(self.projects, command),
        }
    }
}

pub(super) fn encode(value: impl serde::Serialize) -> Result<Value, String> {
    serde_json::to_value(value).map_err(|_| "Tool result serialization failed".into())
}
