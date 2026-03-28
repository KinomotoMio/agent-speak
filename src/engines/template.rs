use async_trait::async_trait;

use crate::engine::{
    Engine, EngineCapabilities, EngineError, EngineInfo, EngineProvider, SpeakRequest, VoiceInfo,
};

/// A compile-checked skeleton for future remote API providers.
///
/// Copy this module, rename the type, and replace the placeholder logic with
/// real provider-specific implementation and tests.
#[cfg_attr(not(test), allow(dead_code))]
#[derive(Default)]
pub struct TemplateEngine;

#[async_trait]
impl Engine for TemplateEngine {
    fn info(&self) -> EngineInfo {
        EngineInfo {
            id: "template",
            display_name: "Template API",
            description: "Reference skeleton for adding a remote TTS provider",
            provider: EngineProvider::RemoteApi,
            capabilities: EngineCapabilities::new(true, true, true, false),
        }
    }

    async fn check_availability(&self) -> Result<(), EngineError> {
        Err(EngineError::config(
            "template engine is a skeleton; configure credentials and provider logic before use",
        ))
    }

    async fn speak(&self, _request: &SpeakRequest) -> Result<(), EngineError> {
        Err(EngineError::unsupported(
            "template engine is documentation-only and is not runnable",
        ))
    }

    async fn voices(&self) -> Result<Vec<VoiceInfo>, EngineError> {
        Ok(vec![VoiceInfo {
            id: "example-voice".to_string(),
            name: "Example Voice".to_string(),
            locale: Some("en_US".to_string()),
            description: Some(
                "Replace this placeholder with provider-specific voice discovery".to_string(),
            ),
        }])
    }
}

#[cfg(test)]
mod tests {
    use crate::engine::{Engine, EngineErrorKind, SpeakRequest};

    use super::TemplateEngine;

    #[tokio::test(flavor = "current_thread")]
    async fn template_engine_exposes_expected_metadata() {
        let engine = TemplateEngine;
        let info = engine.info();

        assert_eq!(info.id, "template");
        assert!(info.capabilities.list_voices);
        assert!(info.capabilities.configurable_voice);
        assert_eq!(info.provider.to_string(), "remote-api");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn template_engine_reports_placeholder_behavior() {
        let engine = TemplateEngine;

        let availability = engine.check_availability().await.unwrap_err();
        assert_eq!(availability.kind(), EngineErrorKind::Config);

        let speak = engine
            .speak(&SpeakRequest {
                text: "hello".to_string(),
                voice: None,
                rate: None,
            })
            .await
            .unwrap_err();
        assert_eq!(speak.kind(), EngineErrorKind::Unsupported);
    }
}
