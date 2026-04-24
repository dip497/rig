//! Shared app state: one instance of each first-party adapter.

use rig_core::adapter::Adapter;

pub struct AppState {
    pub claude: Box<dyn Adapter>,
    pub codex: Box<dyn Adapter>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            claude: Box::new(rig_adapter_claude::ClaudeAdapter::new()),
            codex: Box::new(rig_adapter_codex::CodexAdapter::new()),
        }
    }

    pub fn adapter_by_id(&self, id: &str) -> Option<&dyn Adapter> {
        match id {
            rig_adapter_claude::AGENT_ID => Some(self.claude.as_ref()),
            rig_adapter_codex::AGENT_ID => Some(self.codex.as_ref()),
            _ => None,
        }
    }

    pub fn agents(&self) -> [&dyn Adapter; 2] {
        [self.claude.as_ref(), self.codex.as_ref()]
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
