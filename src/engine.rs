use std::collections::HashMap;
use std::fmt;

use async_trait::async_trait;

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineProvider {
    LocalCommand,
    RemoteApi,
}

impl fmt::Display for EngineProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EngineProvider::LocalCommand => write!(f, "local-command"),
            EngineProvider::RemoteApi => write!(f, "remote-api"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EngineCapabilities {
    pub list_voices: bool,
    pub configurable_voice: bool,
    pub adjustable_rate: bool,
    pub auto_detect_language: bool,
}

impl EngineCapabilities {
    pub const fn new(
        list_voices: bool,
        configurable_voice: bool,
        adjustable_rate: bool,
        auto_detect_language: bool,
    ) -> Self {
        Self {
            list_voices,
            configurable_voice,
            adjustable_rate,
            auto_detect_language,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineInfo {
    pub id: &'static str,
    pub display_name: &'static str,
    pub description: &'static str,
    pub provider: EngineProvider,
    pub capabilities: EngineCapabilities,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SpeakRequest {
    pub text: String,
    pub voice: Option<String>,
    pub rate: Option<f32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoiceInfo {
    pub id: String,
    pub name: String,
    pub locale: Option<String>,
    pub description: Option<String>,
}

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineErrorKind {
    Unavailable,
    Config,
    #[allow(dead_code)]
    Authentication,
    Network,
    Unsupported,
    Provider,
    Execution,
}

impl fmt::Display for EngineErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EngineErrorKind::Unavailable => write!(f, "unavailable"),
            EngineErrorKind::Config => write!(f, "config"),
            EngineErrorKind::Authentication => write!(f, "authentication"),
            EngineErrorKind::Network => write!(f, "network"),
            EngineErrorKind::Unsupported => write!(f, "unsupported"),
            EngineErrorKind::Provider => write!(f, "provider"),
            EngineErrorKind::Execution => write!(f, "execution"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineError {
    kind: EngineErrorKind,
    message: String,
}

impl EngineError {
    pub fn new(kind: EngineErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    pub fn unavailable(message: impl Into<String>) -> Self {
        Self::new(EngineErrorKind::Unavailable, message)
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn config(message: impl Into<String>) -> Self {
        Self::new(EngineErrorKind::Config, message)
    }

    #[allow(dead_code)]
    pub fn authentication(message: impl Into<String>) -> Self {
        Self::new(EngineErrorKind::Authentication, message)
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn network(message: impl Into<String>) -> Self {
        Self::new(EngineErrorKind::Network, message)
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn unsupported(message: impl Into<String>) -> Self {
        Self::new(EngineErrorKind::Unsupported, message)
    }

    pub fn provider(message: impl Into<String>) -> Self {
        Self::new(EngineErrorKind::Provider, message)
    }

    pub fn execution(message: impl Into<String>) -> Self {
        Self::new(EngineErrorKind::Execution, message)
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn kind(&self) -> EngineErrorKind {
        self.kind
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} error: {}", self.kind, self.message)
    }
}

impl std::error::Error for EngineError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineStatus {
    pub info: EngineInfo,
    pub available: bool,
}

#[async_trait]
pub trait Engine: Send + Sync {
    fn info(&self) -> EngineInfo;

    async fn check_availability(&self) -> Result<(), EngineError>;

    async fn speak(&self, request: &SpeakRequest) -> Result<(), EngineError>;

    async fn voices(&self) -> Result<Vec<VoiceInfo>, EngineError> {
        Ok(vec![])
    }
}

pub struct Registry {
    engines: HashMap<String, Box<dyn Engine>>,
}

impl Registry {
    pub fn new() -> Self {
        Self {
            engines: HashMap::new(),
        }
    }

    pub fn register(&mut self, engine: Box<dyn Engine>) {
        let info = engine.info();
        self.engines.insert(info.id.to_string(), engine);
    }

    pub fn get(&self, id: &str) -> Option<&dyn Engine> {
        self.engines.get(id).map(|engine| engine.as_ref())
    }

    pub async fn list(&self) -> Vec<EngineStatus> {
        let mut entries = Vec::with_capacity(self.engines.len());

        for engine in self.engines.values() {
            let info = engine.info();
            let available = engine.check_availability().await.is_ok();
            entries.push(EngineStatus { info, available });
        }

        entries.sort_by_key(|entry| entry.info.id);
        entries
    }
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;

    use super::{
        Engine, EngineCapabilities, EngineError, EngineErrorKind, EngineInfo, EngineProvider,
        Registry, SpeakRequest,
    };

    struct FakeEngine {
        info: EngineInfo,
        available: bool,
    }

    #[async_trait]
    impl Engine for FakeEngine {
        fn info(&self) -> EngineInfo {
            self.info.clone()
        }

        async fn check_availability(&self) -> Result<(), EngineError> {
            if self.available {
                Ok(())
            } else {
                Err(EngineError::unavailable("engine is unavailable"))
            }
        }

        async fn speak(&self, _request: &SpeakRequest) -> Result<(), EngineError> {
            Ok(())
        }
    }

    fn fake_info(id: &'static str, description: &'static str) -> EngineInfo {
        EngineInfo {
            id,
            display_name: id,
            description,
            provider: EngineProvider::LocalCommand,
            capabilities: EngineCapabilities::new(true, true, true, false),
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn registry_gets_engines_by_name() {
        let mut registry = Registry::new();
        registry.register(Box::new(FakeEngine {
            info: fake_info("say", "built-in"),
            available: true,
        }));

        let engine = registry.get("say").unwrap();
        assert_eq!(engine.info().id, "say");
        assert_eq!(engine.info().description, "built-in");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn registry_lists_engines_in_sorted_order() {
        let mut registry = Registry::new();
        registry.register(Box::new(FakeEngine {
            info: fake_info("zeta", "last"),
            available: false,
        }));
        registry.register(Box::new(FakeEngine {
            info: fake_info("alpha", "first"),
            available: true,
        }));

        let entries = registry.list().await;
        assert_eq!(entries[0].info.id, "alpha");
        assert!(entries[0].available);
        assert_eq!(entries[1].info.id, "zeta");
        assert!(!entries[1].available);
    }

    #[test]
    fn engine_error_exposes_kind_and_message() {
        let error = EngineError::network("request timed out");
        assert_eq!(error.kind(), EngineErrorKind::Network);
        assert_eq!(error.message(), "request timed out");
        assert_eq!(error.to_string(), "network error: request timed out");
    }

    #[test]
    fn engine_error_supports_authentication_category() {
        let error = EngineError::authentication("missing api key");
        assert_eq!(error.kind(), EngineErrorKind::Authentication);
        assert_eq!(error.to_string(), "authentication error: missing api key");
    }

    #[test]
    fn engine_info_captures_provider_and_capabilities() {
        let info = EngineInfo {
            id: "template",
            display_name: "Template",
            description: "reference provider",
            provider: EngineProvider::RemoteApi,
            capabilities: EngineCapabilities::new(true, true, true, false),
        };

        assert_eq!(info.provider, EngineProvider::RemoteApi);
        assert!(info.capabilities.list_voices);
        assert!(info.capabilities.configurable_voice);
        assert!(info.capabilities.adjustable_rate);
        assert!(!info.capabilities.auto_detect_language);
    }
}
