mod config;
mod engine;
mod engines;

use std::fmt;

use clap::{Parser, Subcommand};

use config::{Config, ConfigLoadError, ConfigSaveError};
use engine::{Engine, EngineError, Registry, SpeakRequest};

#[derive(Parser)]
#[command(name = "agent-speak", about = "Pluggable TTS for NanoClaw")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Text to speak (when no subcommand is given).
    #[arg(trailing_var_arg = true)]
    text: Vec<String>,

    /// Override engine for this invocation.
    #[arg(short, long, global = true)]
    engine: Option<String>,

    /// Override voice for this invocation.
    #[arg(short, long, global = true)]
    voice: Option<String>,

    /// Speech rate multiplier (1.0 = normal).
    #[arg(short, long, global = true)]
    rate: Option<f32>,
}

#[derive(Subcommand)]
enum Commands {
    /// List available engines.
    Engines,
    /// List available voices for an engine.
    Voices {
        /// Engine name. Defaults to the configured default engine.
        engine: Option<String>,
    },
    /// View or modify configuration.
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
}

#[derive(Subcommand)]
enum ConfigAction {
    /// Show current configuration.
    Show,
    /// Set the default engine.
    SetEngine { name: String },
    /// Set the default voice for an engine.
    SetVoice { engine: String, voice: String },
    /// Set the default speech rate.
    SetRate { rate: f32 },
    /// Reset to defaults.
    Reset,
}

#[derive(Debug)]
enum AppError {
    Usage,
    ConfigLoad(ConfigLoadError),
    ConfigSave(ConfigSaveError),
    UnknownEngine(String),
    EngineUnavailable { id: String, reason: String },
    Engine(EngineError),
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), AppError> {
    let cli = Cli::parse();

    let mut registry = Registry::new();
    engines::register_all(&mut registry);

    match cli.command {
        Some(Commands::Engines) => cmd_engines(&registry).await,
        Some(Commands::Voices { engine }) => {
            let engine_id = match engine {
                Some(engine_id) => engine_id,
                None => load_config()?.engine,
            };
            cmd_voices(&registry, &engine_id).await
        }
        Some(Commands::Config { action }) => cmd_config(action).await,
        None => {
            let text = cli.text.join(" ");
            if text.is_empty() {
                return Err(AppError::Usage);
            }

            let config = load_config()?;
            cmd_speak(&registry, &config, text, cli.engine, cli.voice, cli.rate).await
        }
    }
}

async fn cmd_speak(
    registry: &Registry,
    config: &Config,
    text: String,
    engine_override: Option<String>,
    voice_override: Option<String>,
    rate_override: Option<f32>,
) -> Result<(), AppError> {
    let engine_id = engine_override.unwrap_or_else(|| config.engine.clone());
    let engine = resolve_engine(registry, &engine_id)?;
    ensure_engine_available(&engine_id, engine).await?;

    let request = SpeakRequest {
        text,
        voice: voice_override.or_else(|| config.voices.get(&engine_id).cloned()),
        rate: rate_override.or(config.rate),
    };

    engine.speak(&request).await.map_err(AppError::Engine)
}

async fn cmd_engines(registry: &Registry) -> Result<(), AppError> {
    let engines = registry.list().await;
    if engines.is_empty() {
        println!("No engines registered.");
        return Ok(());
    }

    for entry in engines {
        let status = if entry.available { "✓" } else { "✗" };
        println!("  {status} {:12} {}", entry.info.id, entry.info.description);
    }

    Ok(())
}

async fn cmd_voices(registry: &Registry, engine_id: &str) -> Result<(), AppError> {
    let engine = resolve_engine(registry, engine_id)?;
    ensure_engine_available(engine_id, engine).await?;

    let voices = engine.voices().await.map_err(AppError::Engine)?;
    if voices.is_empty() {
        println!("No voices listed for engine '{engine_id}'.");
        return Ok(());
    }

    for voice in voices {
        println!("  {}", voice.name);
    }

    Ok(())
}

async fn cmd_config(action: ConfigAction) -> Result<(), AppError> {
    match action {
        ConfigAction::Show => {
            let config = load_config()?;
            println!("Config: {}", Config::path().display());
            println!("  engine: {}", config.engine);
            if config.voices.is_empty() {
                println!("  voices: (engine defaults)");
            } else {
                for (engine_id, voice) in &config.voices {
                    println!("  voice[{engine_id}]: {voice}");
                }
            }
            match config.rate {
                Some(rate) => println!("  rate: {rate}"),
                None => println!("  rate: (default)"),
            }
            Ok(())
        }
        ConfigAction::SetEngine { name } => {
            let mut config = load_config()?;
            config.engine = name.clone();
            save_config(&config)?;
            println!("Default engine set to: {name}");
            Ok(())
        }
        ConfigAction::SetVoice { engine, voice } => {
            let mut config = load_config()?;
            config.voices.insert(engine.clone(), voice.clone());
            save_config(&config)?;
            println!("Default voice for '{engine}' set to: {voice}");
            Ok(())
        }
        ConfigAction::SetRate { rate } => {
            let mut config = load_config()?;
            config.rate = Some(rate);
            save_config(&config)?;
            println!("Default rate set to: {rate}");
            Ok(())
        }
        ConfigAction::Reset => {
            let config = Config::default();
            save_config(&config)?;
            println!("Config reset to defaults.");
            Ok(())
        }
    }
}

fn load_config() -> Result<Config, AppError> {
    Config::load().map_err(AppError::ConfigLoad)
}

fn save_config(config: &Config) -> Result<(), AppError> {
    config.save().map_err(AppError::ConfigSave)
}

fn resolve_engine<'a>(registry: &'a Registry, engine_id: &str) -> Result<&'a dyn Engine, AppError> {
    registry
        .get(engine_id)
        .ok_or_else(|| AppError::UnknownEngine(engine_id.to_string()))
}

async fn ensure_engine_available(engine_id: &str, engine: &dyn Engine) -> Result<(), AppError> {
    engine
        .check_availability()
        .await
        .map_err(|error| AppError::EngineUnavailable {
            id: engine_id.to_string(),
            reason: error.message().to_string(),
        })
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::Usage => {
                writeln!(f, "Usage: agent-speak \"text to speak\"")?;
                write!(f, "       agent-speak --help")
            }
            AppError::ConfigLoad(error) => {
                writeln!(f, "Failed to load config: {error}")?;
                write!(
                    f,
                    "Fix {} or run `agent-speak config reset` to restore defaults.",
                    error.path().display()
                )
            }
            AppError::ConfigSave(error) => write!(f, "Error: {error}"),
            AppError::UnknownEngine(engine_id) => {
                writeln!(f, "Unknown engine: {engine_id}")?;
                write!(f, "Run `agent-speak engines` to see available engines.")
            }
            AppError::EngineUnavailable { id, reason } => {
                if reason.is_empty() {
                    write!(f, "Engine '{id}' is not available on this system.")
                } else {
                    writeln!(f, "Engine '{id}' is not available on this system.")?;
                    write!(f, "Reason: {reason}")
                }
            }
            AppError::Engine(error) => write!(f, "Error: {error}"),
        }
    }
}
