use std::env;
use std::io::Write;

use async_trait::async_trait;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use tempfile::Builder;
use tokio::process::Command;

use crate::engine::{
    Engine, EngineCapabilities, EngineError, EngineInfo, EngineProvider, SpeakRequest, VoiceInfo,
};

const DEFAULT_BASE_URL: &str = "https://api.minimaxi.com";
const DEFAULT_MODEL: &str = "speech-2.8-hd";
const DEFAULT_PLAYER: &str = "afplay";

pub struct MinimaxEngine {
    client: reqwest::Client,
    #[cfg(test)]
    api_key_override: Option<String>,
    #[cfg(test)]
    base_url_override: Option<String>,
    #[cfg(test)]
    player_override: Option<String>,
}

impl MinimaxEngine {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
            #[cfg(test)]
            api_key_override: None,
            #[cfg(test)]
            base_url_override: None,
            #[cfg(test)]
            player_override: None,
        }
    }

    #[cfg(test)]
    fn with_test_overrides(base_url: impl Into<String>, player: impl Into<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_key_override: Some("test-key".to_string()),
            base_url_override: Some(base_url.into()),
            player_override: Some(player.into()),
        }
    }

    fn api_key(&self) -> Result<String, EngineError> {
        #[cfg(test)]
        if let Some(api_key) = &self.api_key_override {
            return Ok(api_key.clone());
        }

        let api_key = env::var("MINIMAX_API_KEY")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());

        api_key.ok_or_else(|| EngineError::config("MINIMAX_API_KEY is not set"))
    }

    fn base_url(&self) -> String {
        #[cfg(test)]
        if let Some(base_url) = &self.base_url_override {
            return trim_trailing_slash(base_url);
        }

        trim_trailing_slash(
            &env::var("MINIMAX_TTS_BASE_URL").unwrap_or_else(|_| DEFAULT_BASE_URL.to_string()),
        )
    }

    fn model(&self) -> String {
        env::var("MINIMAX_TTS_MODEL")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| DEFAULT_MODEL.to_string())
    }

    fn player_command(&self) -> &str {
        #[cfg(test)]
        if let Some(player) = &self.player_override {
            return player.as_str();
        }

        DEFAULT_PLAYER
    }

    async fn fetch_voice_catalog(
        &self,
        voice_type: &str,
    ) -> Result<VoiceCatalogResponse, EngineError> {
        self.post_json("/v1/get_voice", &GetVoiceRequest { voice_type })
            .await
    }

    async fn synthesize(&self, request: &SpeakRequest) -> Result<Vec<u8>, EngineError> {
        let rate = validate_rate(request.rate)?;
        let voice_id = match &request.voice {
            Some(voice) => voice.clone(),
            None => self.resolve_default_voice(&request.text).await?,
        };

        let response: T2aResponse = self
            .post_json(
                "/v1/t2a_v2",
                &build_t2a_request(&self.model(), &request.text, &voice_id, rate),
            )
            .await?;

        extract_audio_bytes(response)
    }

    async fn resolve_default_voice(&self, text: &str) -> Result<String, EngineError> {
        let catalog = self.fetch_voice_catalog("all").await?;
        ensure_base_resp_success(&catalog.base_resp)?;
        let mut system_voices = catalog.system_voice.unwrap_or_default();
        if system_voices.is_empty() {
            return Err(EngineError::config(
                "MiniMax returned no system voices; set an explicit voice with `agent-speak config set-voice minimax <voice_id>`",
            ));
        }

        if let Some(prefix) = language_prefix(crate::lang::detect_lang(text))
            && let Some(voice) = system_voices
                .iter()
                .find(|voice| voice.voice_id.starts_with(prefix))
            {
                return Ok(voice.voice_id.clone());
            }

        Ok(system_voices.remove(0).voice_id)
    }

    async fn play_audio_bytes(&self, audio_bytes: &[u8]) -> Result<(), EngineError> {
        let mut file = Builder::new()
            .prefix("agent-speak-minimax-")
            .suffix(".mp3")
            .tempfile()
            .map_err(|error| {
                EngineError::execution(format!("failed to create temporary audio file: {error}"))
            })?;

        file.write_all(audio_bytes).map_err(|error| {
            EngineError::execution(format!("failed to write temporary audio file: {error}"))
        })?;
        file.flush().map_err(|error| {
            EngineError::execution(format!("failed to flush temporary audio file: {error}"))
        })?;

        let output = Command::new(self.player_command())
            .arg(file.path())
            .output()
            .await
            .map_err(|error| {
                EngineError::execution(format!(
                    "failed to execute `{}`: {error}",
                    self.player_command()
                ))
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let message = stderr.trim();
            return Err(EngineError::execution(if message.is_empty() {
                format!(
                    "`{}` exited with status {}",
                    self.player_command(),
                    output.status
                )
            } else {
                format!("`{}` failed: {message}", self.player_command())
            }));
        }

        Ok(())
    }

    async fn post_json<TReq, TResp>(&self, path: &str, body: &TReq) -> Result<TResp, EngineError>
    where
        TReq: Serialize + ?Sized,
        TResp: for<'de> Deserialize<'de>,
    {
        let url = format!("{}/{}", self.base_url(), path.trim_start_matches('/'));
        let response = self
            .client
            .post(url)
            .bearer_auth(self.api_key()?)
            .json(body)
            .send()
            .await
            .map_err(classify_transport_error)?;

        classify_status_error(response.status(), response).await
    }
}

impl Default for MinimaxEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Engine for MinimaxEngine {
    fn info(&self) -> EngineInfo {
        EngineInfo {
            id: "minimax",
            display_name: "MiniMax TTS",
            description: "MiniMax Speech API over HTTP with remote voice listing and local playback",
            provider: EngineProvider::RemoteApi,
            capabilities: EngineCapabilities::new(true, true, true, true),
        }
    }

    async fn check_availability(&self) -> Result<(), EngineError> {
        if !cfg!(target_os = "macos") {
            return Err(EngineError::unavailable(
                "MiniMax playback is currently supported only on macOS",
            ));
        }

        self.api_key()?;
        Ok(())
    }

    async fn speak(&self, request: &SpeakRequest) -> Result<(), EngineError> {
        let audio_bytes = self.synthesize(request).await?;
        self.play_audio_bytes(&audio_bytes).await
    }

    async fn voices(&self) -> Result<Vec<VoiceInfo>, EngineError> {
        let catalog = self.fetch_voice_catalog("all").await?;
        ensure_base_resp_success(&catalog.base_resp)?;
        Ok(flatten_voice_catalog(catalog))
    }
}

#[derive(Debug, Serialize, PartialEq)]
struct GetVoiceRequest<'a> {
    voice_type: &'a str,
}

#[derive(Debug, Serialize, PartialEq)]
struct T2aRequest<'a> {
    model: &'a str,
    text: &'a str,
    stream: bool,
    language_boost: &'a str,
    output_format: &'a str,
    voice_setting: VoiceSetting<'a>,
    audio_setting: AudioSetting<'a>,
}

#[derive(Debug, Serialize, PartialEq)]
struct VoiceSetting<'a> {
    voice_id: &'a str,
    speed: f32,
    vol: f32,
    pitch: i32,
}

#[derive(Debug, Serialize, PartialEq)]
struct AudioSetting<'a> {
    sample_rate: u32,
    bitrate: u32,
    format: &'a str,
    channel: u8,
}

#[derive(Debug, Deserialize, Clone, PartialEq, Eq)]
struct ApiVoice {
    voice_id: String,
    #[serde(default)]
    voice_name: Option<String>,
    #[serde(default)]
    description: Vec<String>,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
struct VoiceCatalogResponse {
    #[serde(default)]
    system_voice: Option<Vec<ApiVoice>>,
    #[serde(default)]
    voice_cloning: Option<Vec<ApiVoice>>,
    #[serde(default)]
    voice_generation: Option<Vec<ApiVoice>>,
    base_resp: BaseResp,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
struct T2aResponse {
    data: Option<T2aData>,
    base_resp: BaseResp,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
struct T2aData {
    audio: Option<String>,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
struct BaseResp {
    status_code: i32,
    status_msg: String,
}

fn build_t2a_request<'a>(
    model: &'a str,
    text: &'a str,
    voice_id: &'a str,
    speed: f32,
) -> T2aRequest<'a> {
    T2aRequest {
        model,
        text,
        stream: false,
        language_boost: "auto",
        output_format: "hex",
        voice_setting: VoiceSetting {
            voice_id,
            speed,
            vol: 1.0,
            pitch: 0,
        },
        audio_setting: AudioSetting {
            sample_rate: 32_000,
            bitrate: 128_000,
            format: "mp3",
            channel: 1,
        },
    }
}

fn validate_rate(rate: Option<f32>) -> Result<f32, EngineError> {
    let rate = rate.unwrap_or(1.0);
    if rate <= 0.0 {
        return Err(EngineError::config(
            "speech rate must be greater than 0 for MiniMax",
        ));
    }
    Ok(rate)
}

fn trim_trailing_slash(url: &str) -> String {
    url.trim_end_matches('/').to_string()
}

fn language_prefix(locale: Option<&str>) -> Option<&'static str> {
    match locale {
        Some("en_US") => Some("English"),
        Some("zh_CN") => Some("Chinese (Mandarin)"),
        Some("ja_JP") => Some("Japanese"),
        Some("ko_KR") => Some("Korean"),
        Some("ru_RU") => Some("Russian"),
        Some("ar_SA") => Some("Arabic"),
        Some("th_TH") => Some("Thai"),
        _ => None,
    }
}

fn flatten_voice_catalog(catalog: VoiceCatalogResponse) -> Vec<VoiceInfo> {
    let mut voices = Vec::new();

    if let Some(entries) = catalog.system_voice {
        voices.extend(
            entries
                .into_iter()
                .map(|voice| to_voice_info("system", voice)),
        );
    }
    if let Some(entries) = catalog.voice_cloning {
        voices.extend(
            entries
                .into_iter()
                .map(|voice| to_voice_info("voice_cloning", voice)),
        );
    }
    if let Some(entries) = catalog.voice_generation {
        voices.extend(
            entries
                .into_iter()
                .map(|voice| to_voice_info("voice_generation", voice)),
        );
    }

    voices
}

fn to_voice_info(category: &str, voice: ApiVoice) -> VoiceInfo {
    let ApiVoice {
        voice_id,
        voice_name,
        description,
    } = voice;
    let description = description
        .iter()
        .find(|description| !description.trim().is_empty())
        .map(|description| format!("{category}: {description}"));

    VoiceInfo {
        id: voice_id.clone(),
        name: voice_name.unwrap_or_else(|| voice_id.clone()),
        locale: None,
        description,
    }
}

fn extract_audio_bytes(response: T2aResponse) -> Result<Vec<u8>, EngineError> {
    ensure_base_resp_success(&response.base_resp)?;

    let audio_hex = response
        .data
        .and_then(|data| data.audio)
        .filter(|audio| !audio.trim().is_empty())
        .ok_or_else(|| EngineError::provider("MiniMax response did not include audio data"))?;

    hex::decode(audio_hex.trim()).map_err(|error| {
        EngineError::execution(format!("failed to decode MiniMax audio payload: {error}"))
    })
}

fn ensure_base_resp_success(base_resp: &BaseResp) -> Result<(), EngineError> {
    if base_resp.status_code == 0 {
        Ok(())
    } else {
        Err(EngineError::provider(format!(
            "MiniMax API error {}: {}",
            base_resp.status_code, base_resp.status_msg
        )))
    }
}

fn classify_transport_error(error: reqwest::Error) -> EngineError {
    EngineError::network(format!("failed to reach MiniMax API: {error}"))
}

async fn classify_status_error<T>(
    status: StatusCode,
    response: reqwest::Response,
) -> Result<T, EngineError>
where
    T: for<'de> Deserialize<'de>,
{
    if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
        let body = response.text().await.unwrap_or_default();
        return Err(EngineError::authentication(format!(
            "MiniMax authentication failed (HTTP {}): {}",
            status.as_u16(),
            sanitize_error_body(&body)
        )));
    }

    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(EngineError::provider(format!(
            "MiniMax API request failed (HTTP {}): {}",
            status.as_u16(),
            sanitize_error_body(&body)
        )));
    }

    response.json::<T>().await.map_err(|error| {
        EngineError::provider(format!("failed to parse MiniMax response: {error}"))
    })
}

fn sanitize_error_body(body: &str) -> String {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        "empty response body".to_string()
    } else {
        trimmed.to_string()
    }
}

#[cfg(test)]
mod tests {
    use wiremock::matchers::{body_json, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use crate::engine::{Engine, EngineErrorKind, SpeakRequest};

    use super::{
        ApiVoice, BaseResp, MinimaxEngine, T2aResponse, VoiceCatalogResponse, build_t2a_request,
        extract_audio_bytes, flatten_voice_catalog, language_prefix, validate_rate,
    };

    #[test]
    fn build_t2a_request_uses_expected_defaults() {
        let request = build_t2a_request("speech-2.8-hd", "hello", "voice-1", 1.0);

        assert_eq!(request.model, "speech-2.8-hd");
        assert_eq!(request.text, "hello");
        assert!(!request.stream);
        assert_eq!(request.language_boost, "auto");
        assert_eq!(request.output_format, "hex");
        assert_eq!(request.voice_setting.voice_id, "voice-1");
        assert_eq!(request.voice_setting.speed, 1.0);
        assert_eq!(request.audio_setting.format, "mp3");
    }

    #[test]
    fn validate_rate_rejects_non_positive_values() {
        let error = validate_rate(Some(0.0)).unwrap_err();
        assert_eq!(error.kind(), EngineErrorKind::Config);
    }

    #[test]
    fn validate_rate_defaults_to_one() {
        assert_eq!(validate_rate(None).unwrap(), 1.0);
    }

    #[test]
    fn flatten_voice_catalog_keeps_ids_and_labels() {
        let voices = flatten_voice_catalog(VoiceCatalogResponse {
            system_voice: Some(vec![ApiVoice {
                voice_id: "English_expressive_narrator".to_string(),
                voice_name: Some("Expressive Narrator".to_string()),
                description: vec!["Warm and articulate".to_string()],
            }]),
            voice_cloning: Some(vec![ApiVoice {
                voice_id: "clone-123".to_string(),
                voice_name: None,
                description: vec![],
            }]),
            voice_generation: None,
            base_resp: BaseResp {
                status_code: 0,
                status_msg: "success".to_string(),
            },
        });

        assert_eq!(voices[0].id, "English_expressive_narrator");
        assert_eq!(voices[0].name, "Expressive Narrator");
        assert_eq!(
            voices[0].description.as_deref(),
            Some("system: Warm and articulate")
        );
        assert_eq!(voices[1].id, "clone-123");
        assert_eq!(voices[1].name, "clone-123");
        assert_eq!(voices[1].description, None);
    }

    #[test]
    fn language_prefix_maps_supported_languages() {
        assert_eq!(language_prefix(Some("zh_CN")), Some("Chinese (Mandarin)"));
        assert_eq!(language_prefix(Some("en_US")), Some("English"));
        assert_eq!(language_prefix(Some("ja_JP")), Some("Japanese"));
        assert_eq!(language_prefix(Some("fr_FR")), None);
    }

    #[test]
    fn extract_audio_bytes_reads_hex_payload() {
        let audio = extract_audio_bytes(T2aResponse {
            data: Some(super::T2aData {
                audio: Some("68656c6c6f".to_string()),
            }),
            base_resp: BaseResp {
                status_code: 0,
                status_msg: "success".to_string(),
            },
        })
        .unwrap();

        assert_eq!(audio, b"hello");
    }

    #[test]
    fn extract_audio_bytes_returns_provider_error_for_api_failure() {
        let error = extract_audio_bytes(T2aResponse {
            data: None,
            base_resp: BaseResp {
                status_code: 1004,
                status_msg: "bad request".to_string(),
            },
        })
        .unwrap_err();

        assert_eq!(error.kind(), EngineErrorKind::Provider);
    }

    #[test]
    fn extract_audio_bytes_rejects_missing_audio() {
        let error = extract_audio_bytes(T2aResponse {
            data: Some(super::T2aData { audio: None }),
            base_resp: BaseResp {
                status_code: 0,
                status_msg: "success".to_string(),
            },
        })
        .unwrap_err();

        assert_eq!(error.kind(), EngineErrorKind::Provider);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn voices_reads_from_mock_api() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/get_voice"))
            .and(body_json(serde_json::json!({ "voice_type": "all" })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "system_voice": [
                    {
                        "voice_id": "English_expressive_narrator",
                        "voice_name": "Expressive Narrator",
                        "description": ["Warm and articulate"]
                    }
                ],
                "voice_cloning": [],
                "voice_generation": [],
                "base_resp": {
                    "status_code": 0,
                    "status_msg": "success"
                }
            })))
            .mount(&server)
            .await;

        let engine = MinimaxEngine::with_test_overrides(server.uri(), "true");
        let voices = engine.voices().await.unwrap();

        assert_eq!(voices.len(), 1);
        assert_eq!(voices[0].id, "English_expressive_narrator");
        assert_eq!(voices[0].name, "Expressive Narrator");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn speak_uses_explicit_voice_and_decodes_audio() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/t2a_v2"))
            .and(body_json(serde_json::json!({
                "model": "speech-2.8-hd",
                "text": "hello",
                "stream": false,
                "language_boost": "auto",
                "output_format": "hex",
                "voice_setting": {
                    "voice_id": "voice-1",
                    "speed": 1.0,
                    "vol": 1.0,
                    "pitch": 0
                },
                "audio_setting": {
                    "sample_rate": 32000,
                    "bitrate": 128000,
                    "format": "mp3",
                    "channel": 1
                }
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": {
                    "audio": "68656c6c6f"
                },
                "base_resp": {
                    "status_code": 0,
                    "status_msg": "success"
                }
            })))
            .mount(&server)
            .await;

        let engine = MinimaxEngine::with_test_overrides(server.uri(), "true");
        engine
            .speak(&SpeakRequest {
                text: "hello".to_string(),
                voice: Some("voice-1".to_string()),
                rate: None,
            })
            .await
            .unwrap();
    }

    #[tokio::test(flavor = "current_thread")]
    async fn speak_auto_selects_voice_by_language() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/get_voice"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "system_voice": [
                    {
                        "voice_id": "English_expressive_narrator",
                        "voice_name": "Expressive Narrator",
                        "description": ["Warm and articulate"]
                    },
                    {
                        "voice_id": "Chinese (Mandarin)_Reliable_Executive",
                        "voice_name": "Steady Executive",
                        "description": ["Steady and reliable Mandarin"]
                    }
                ],
                "voice_cloning": [],
                "voice_generation": [],
                "base_resp": {
                    "status_code": 0,
                    "status_msg": "success"
                }
            })))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/v1/t2a_v2"))
            .and(body_json(serde_json::json!({
                "model": "speech-2.8-hd",
                "text": "该开会了",
                "stream": false,
                "language_boost": "auto",
                "output_format": "hex",
                "voice_setting": {
                    "voice_id": "Chinese (Mandarin)_Reliable_Executive",
                    "speed": 1.0,
                    "vol": 1.0,
                    "pitch": 0
                },
                "audio_setting": {
                    "sample_rate": 32000,
                    "bitrate": 128000,
                    "format": "mp3",
                    "channel": 1
                }
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": {
                    "audio": "68656c6c6f"
                },
                "base_resp": {
                    "status_code": 0,
                    "status_msg": "success"
                }
            })))
            .mount(&server)
            .await;

        let engine = MinimaxEngine::with_test_overrides(server.uri(), "true");
        engine
            .speak(&SpeakRequest {
                text: "该开会了".to_string(),
                voice: None,
                rate: None,
            })
            .await
            .unwrap();
    }

    #[tokio::test(flavor = "current_thread")]
    async fn voices_maps_unauthorized_to_authentication_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/get_voice"))
            .respond_with(ResponseTemplate::new(401).set_body_string("nope"))
            .mount(&server)
            .await;

        let engine = MinimaxEngine::with_test_overrides(server.uri(), "true");
        let error = engine.voices().await.unwrap_err();

        assert_eq!(error.kind(), EngineErrorKind::Authentication);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn voices_maps_provider_failure_from_base_resp() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/get_voice"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "system_voice": [],
                "voice_cloning": [],
                "voice_generation": [],
                "base_resp": {
                    "status_code": 42,
                    "status_msg": "bad request"
                }
            })))
            .mount(&server)
            .await;

        let engine = MinimaxEngine::with_test_overrides(server.uri(), "true");
        let error = engine.voices().await.unwrap_err();

        assert_eq!(error.kind(), EngineErrorKind::Provider);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn voices_maps_network_failures() {
        let engine = MinimaxEngine::with_test_overrides("http://127.0.0.1:9", "true");
        let error = engine.voices().await.unwrap_err();

        assert_eq!(error.kind(), EngineErrorKind::Network);
    }
}
