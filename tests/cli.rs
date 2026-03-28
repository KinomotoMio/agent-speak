use std::fs;
use std::path::{Path, PathBuf};

use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;
use wiremock::matchers::{body_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

struct TestEnv {
    temp: TempDir,
    xdg_config_home: PathBuf,
    config_path: PathBuf,
}

impl TestEnv {
    fn new() -> Self {
        let temp = TempDir::new().unwrap();
        let xdg_config_home = temp.path().join("xdg");
        let output = Self::command_for_paths(temp.path(), &xdg_config_home)
            .args(["config", "show"])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        let stdout = String::from_utf8(output).unwrap();
        let config_path = stdout
            .lines()
            .find_map(|line| line.strip_prefix("Config: "))
            .map(PathBuf::from)
            .unwrap();

        Self {
            temp,
            xdg_config_home,
            config_path,
        }
    }

    fn cmd(&self) -> Command {
        Self::command_for_paths(self.temp.path(), &self.xdg_config_home)
    }

    fn write_invalid_config(&self) {
        if let Some(parent) = self.config_path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&self.config_path, "engine = [").unwrap();
    }

    fn command_for_paths(home: &Path, xdg_config_home: &Path) -> Command {
        let mut cmd = Command::cargo_bin("agent-speak").unwrap();
        cmd.env("HOME", home);
        cmd.env("XDG_CONFIG_HOME", xdg_config_home);
        cmd
    }
}

#[test]
fn engines_succeeds_with_invalid_config() {
    let env = TestEnv::new();
    env.write_invalid_config();

    env.cmd()
        .arg("engines")
        .assert()
        .success()
        .stdout(predicate::str::contains("say"));
}

#[test]
fn voices_with_explicit_engine_ignores_invalid_config() {
    if !cfg!(target_os = "macos") {
        return;
    }

    let env = TestEnv::new();
    env.write_invalid_config();

    env.cmd()
        .args(["voices", "say"])
        .assert()
        .success()
        .stderr(predicate::str::is_empty());
}

#[test]
fn voices_without_engine_fails_on_invalid_config() {
    let env = TestEnv::new();
    env.write_invalid_config();

    env.cmd()
        .arg("voices")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Failed to load config"))
        .stderr(predicate::str::contains("config reset"));
}

#[test]
fn config_show_fails_on_invalid_config() {
    let env = TestEnv::new();
    env.write_invalid_config();

    env.cmd()
        .args(["config", "show"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Failed to load config"));
}

#[test]
fn config_set_engine_fails_on_invalid_config() {
    let env = TestEnv::new();
    env.write_invalid_config();

    env.cmd()
        .args(["config", "set-engine", "edge"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Failed to load config"));
}

#[test]
fn config_reset_recovers_from_invalid_config() {
    let env = TestEnv::new();
    env.write_invalid_config();

    env.cmd()
        .args(["config", "reset"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Config reset to defaults."));

    env.cmd()
        .args(["config", "show"])
        .assert()
        .success()
        .stdout(predicate::str::contains("engine: say"));

    let saved = fs::read_to_string(&env.config_path).unwrap();
    assert!(saved.contains("engine = \"say\""));
}

#[test]
fn speak_fails_on_invalid_config_before_engine_execution() {
    let env = TestEnv::new();
    env.write_invalid_config();

    env.cmd()
        .arg("hello")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Failed to load config"));
}

#[test]
fn empty_text_prints_usage() {
    let env = TestEnv::new();

    env.cmd()
        .assert()
        .failure()
        .stderr(predicate::str::contains("Usage: agent-speak"));
}

#[test]
fn unknown_engine_still_errors_cleanly() {
    let env = TestEnv::new();

    env.cmd()
        .args(["voices", "nope"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Unknown engine: nope"));
}

#[tokio::test(flavor = "current_thread")]
async fn voices_minimax_prints_id_and_label() {
    if !cfg!(target_os = "macos") {
        return;
    }

    let env = TestEnv::new();
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

    env.cmd()
        .env("MINIMAX_API_KEY", "test-key")
        .env("MINIMAX_TTS_BASE_URL", server.uri())
        .args(["voices", "minimax"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "English_expressive_narrator  Expressive Narrator - system: Warm and articulate",
        ));
}
