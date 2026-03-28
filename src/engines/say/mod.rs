mod lang;
mod voices;

use async_trait::async_trait;
use tokio::process::Command;

use crate::engine::{
    Engine, EngineCapabilities, EngineError, EngineInfo, EngineProvider, SpeakRequest, VoiceInfo,
};

/// macOS native TTS via the `say` command.
pub struct SayEngine;

#[async_trait]
impl Engine for SayEngine {
    fn info(&self) -> EngineInfo {
        EngineInfo {
            id: "say",
            display_name: "macOS say",
            description: "macOS built-in speech synthesis (zero dependencies, auto language detection)",
            provider: EngineProvider::LocalCommand,
            capabilities: EngineCapabilities::new(true, true, true, true),
        }
    }

    async fn check_availability(&self) -> Result<(), EngineError> {
        let output = Command::new("which")
            .arg("say")
            .output()
            .await
            .map_err(|error| EngineError::unavailable(format!("failed to check `say`: {error}")))?;

        if output.status.success() {
            Ok(())
        } else {
            Err(EngineError::unavailable(
                "macOS `say` command was not found in PATH",
            ))
        }
    }

    async fn speak(&self, request: &SpeakRequest) -> Result<(), EngineError> {
        let mut cmd = Command::new("say");

        if let Some(voice) = &request.voice {
            cmd.args(["-v", voice]);
        } else if let Some(lang) = crate::lang::detect_lang(&request.text) {
            let sys_lang = lang::system_lang().await;
            if lang != sys_lang {
                let voice_entries = load_voice_entries().await?;
                if let Some(voice) = voices::voice_for_lang(&voice_entries, lang) {
                    cmd.args(["-v", &voice]);
                }
            }
        }

        if let Some(rate) = request.rate {
            let wpm = (175.0 * rate) as u32;
            cmd.args(["-r", &wpm.to_string()]);
        }

        cmd.arg(&request.text);

        let output = cmd
            .output()
            .await
            .map_err(|error| EngineError::execution(format!("failed to execute `say`: {error}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(EngineError::provider(format!(
                "`say` exited with error: {stderr}"
            )));
        }

        Ok(())
    }

    async fn voices(&self) -> Result<Vec<VoiceInfo>, EngineError> {
        let entries = load_voice_entries().await?;
        Ok(voices::to_voice_infos(&entries))
    }
}

async fn load_voice_entries() -> Result<Vec<voices::VoiceEntry>, EngineError> {
    let output = Command::new("say")
        .args(["-v", "?"])
        .output()
        .await
        .map_err(|error| EngineError::execution(format!("failed to query voices: {error}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(EngineError::provider(format!(
            "`say -v ?` exited with error: {stderr}"
        )));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(voices::parse_voice_entries(&stdout))
}
