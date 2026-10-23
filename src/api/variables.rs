use std::collections::BTreeMap;
use serde::{Deserialize, Serialize};

use super::ApiError;

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Variables {
    pub project: BTreeMap<String, String>,
    pub environment: BTreeMap<String, String>,
    pub collection: BTreeMap<String, String>,
    pub request: BTreeMap<String, String>,
    pub overrides: BTreeMap<String, String>,
}

impl Variables {
    /// One pass: values never become executable templates themselves.
    pub fn resolve(&self, input: &str) -> Result<String, ApiError> {
        let mut output = String::new();
        let mut remaining = input;
        while let Some(start) = remaining.find("{{") {
            output.push_str(&remaining[..start]);
            let template = &remaining[start + 2..];
            let end = template.find("}}").ok_or(ApiError::InvalidTemplate)?;
            let name = template[..end].trim();
            let value = [&self.overrides, &self.request, &self.collection, &self.environment, &self.project]
                .iter().find_map(|scope| scope.get(name))
                .ok_or_else(|| ApiError::MissingVariable(name.into()))?;
            output.push_str(value);
            remaining = &template[end + 2..];
        }
        output.push_str(remaining);
        Ok(output)
    }
}
