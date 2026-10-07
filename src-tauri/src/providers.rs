use serde::{Deserialize, Serialize};

/// Wire protocols supported by the local gateway.  A provider can expose more
/// than one capability, but a request is always sent through one adapter.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum WireApi {
    Responses,
    ChatCompletions,
    AnthropicMessages,
    Custom,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ProviderCapabilities {
    pub responses: bool,
    pub chat_completions: bool,
    pub anthropic_messages: bool,
    pub vision: bool,
    pub image_generation: bool,
    pub image_edit: bool,
    pub streaming: bool,
    pub compact: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderAdapter {
    pub id: String,
    pub name: String,
    pub wire_api: WireApi,
    pub base_url: String,
    #[serde(default)]
    pub models: Vec<String>,
    #[serde(default)]
    pub capabilities: ProviderCapabilities,
    #[serde(default)]
    pub headers: Vec<(String, String)>,
}

impl ProviderAdapter {
    pub fn endpoint(&self, path: &str) -> String {
        format!(
            "{}/{}",
            self.base_url.trim_end_matches('/'),
            path.trim_start_matches('/')
        )
    }

    pub fn supports_model(&self, model: &str) -> bool {
        self.models.is_empty() || self.models.iter().any(|candidate| candidate == model)
    }
}
