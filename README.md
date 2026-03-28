# agent-speak

[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-2024_edition-orange.svg)](https://www.rust-lang.org/)
[![macOS](https://img.shields.io/badge/platform-macOS-lightgrey.svg)]()
[![Built for AI Agents](https://img.shields.io/badge/built_for-AI_agents-blueviolet.svg)]()

Give your AI agent a voice. A tiny, pluggable TTS CLI that lets agents (or you) speak text aloud from the terminal.

```
agent-speak "Your deploy just finished"
```

Built for AI agent workflows — Claude Code, NanoClaw, or any agent that can run shell commands. Drop it in your PATH and your agent can alert you by voice when something important happens, without you staring at a screen.

## Features

- **Auto language detection** — detects Chinese, Japanese, Korean, English, French, German, Russian, and more from the text content, then picks the right voice automatically
- **Smart voice selection** — uses your system's high-quality Siri voice for your default language, falls back to the best available voice for other languages
- **Pluggable engine system** — ships with macOS `say`, designed for adding more (Edge TTS, OpenAI, ElevenLabs...)
- **Persistent config** — set your default engine/voice once, every call after that is just `agent-speak "text"`
- **Zero extra runtime dependencies** — single binary, no background service needed

## Install

```bash
cargo install --path .
```

Or build and copy manually:

```bash
cargo build --release
cp target/release/agent-speak ~/.local/bin/
```

## Usage

```bash
# Just speak
agent-speak "该开会了"

# Override voice for one call
agent-speak -v Samantha "Time for the meeting"

# Speed up
agent-speak -r 1.5 "Hurry up"

# List available engines
agent-speak engines

# List voices for current engine
agent-speak voices
```

## Configuration

```bash
# Show current config
agent-speak config show

# Set default voice for an engine
agent-speak config set-voice say Kyoko

# Change default engine (when you add more)
agent-speak config set-engine edge

# Set speech rate (1.0 = normal)
agent-speak config set-rate 1.2

# Reset everything
agent-speak config reset
```

Config lives at `~/.config/agent-speak/config.toml`.

## How language detection works

agent-speak scans the text for Unicode character ranges:

| Characters | Detected as |
|---|---|
| Hiragana / Katakana | Japanese |
| CJK Ideographs (no kana) | Chinese |
| Hangul | Korean |
| Cyrillic | Russian |
| Latin | English |
| Arabic script | Arabic |

If the text doesn't look like language at all (paths, SHAs, emoji, numbers), agent-speak doesn't force a language-specific voice and falls back to the system default behavior.

When the detected language matches your system language, agent-speak uses no `-v` flag — this lets macOS use the high-quality Siri neural voice, which sounds significantly better than the named voices from `say -v ?`.

For other languages, it picks the best available voice automatically (e.g. Kyoko for Japanese, Samantha for English, Thomas for French), skipping novelty voices like Grandma or Bells.

## Adding engines

Engines implement a simple trait:

```rust
#[async_trait]
pub trait Engine: Send + Sync {
    fn info(&self) -> EngineInfo;
    async fn check_availability(&self) -> Result<(), EngineError>;
    async fn speak(&self, request: &SpeakRequest) -> Result<(), EngineError>;
    async fn voices(&self) -> Result<Vec<VoiceInfo>, EngineError> { Ok(vec![]) }
}
```

Add your engine in `src/engines/`, register it in `src/engines/mod.rs`, done. Users switch with `agent-speak config set-engine <name>`. For the current interface, engine template, and testing expectations, see `CONTRIBUTING.md`.

## Use with AI agents

### Claude Code

The agent can call it directly:

```bash
agent-speak "Build failed — check the logs"
```

### NanoClaw

Container agents write an IPC file, the host picks it up and runs `agent-speak`:

```bash
cat > /workspace/ipc/commands/speak-$(date +%s).json << 'EOF'
{"type": "speak", "text": "PR review requested"}
EOF
```

### Any agent

If your agent can run shell commands, it can use agent-speak. No SDK, no API, no setup beyond putting the binary in PATH.

## License

MIT
