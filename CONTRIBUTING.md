# Contributing

`agent-speak` is organized around a shared async engine abstraction. The CLI should stay small; provider-specific complexity belongs in engine modules.

## Architecture

Core concepts live in `src/engine.rs`:

- `Engine`: async trait implemented by every provider
- `EngineInfo`: stable metadata for listing and documentation
- `EngineCapabilities`: feature flags the CLI and docs can rely on
- `SpeakRequest`: normalized per-invocation request
- `VoiceInfo`: structured voice metadata
- `EngineError`: categorized failure model
- `Registry`: runtime registry for installed engines

Provider modules live under `src/engines/`.

Current layout:

- `src/engines/say/`: built-in macOS engine
- `src/engines/template.rs`: compile-checked skeleton for future API providers

## Provider categories

The abstraction is designed for two main classes of engines:

### Local command engines

Examples:

- macOS `say`
- Linux speech-dispatcher wrappers
- bundled CLI tools

Typical characteristics:

- availability depends on binaries, OS support, or PATH
- voice discovery usually shells out to a local command
- execution errors come from process spawn/exit status

### Remote API engines

Examples:

- OpenAI
- Edge TTS
- ElevenLabs
- self-hosted HTTP TTS services

Typical characteristics:

- availability depends on credentials, network reachability, or endpoint configuration
- voice catalogs may come from remote APIs
- failures must distinguish config/auth/network/provider issues

## Adding a new engine

Use `src/engines/template.rs` as the starting point.

Implementation steps:

1. Create a new module under `src/engines/`.
2. Implement `Engine` for your provider type.
3. Return a stable `EngineInfo`.
4. Model supported features accurately in `EngineCapabilities`.
5. Implement `check_availability()`.
6. Implement `voices()` if the provider can enumerate voices.
7. Implement `speak()` using `SpeakRequest`.
8. Register the provider in `src/engines/mod.rs`.
9. Add tests.
10. Update `README.md` if the provider becomes user-facing.

## Implementation rules

- Keep the CLI behavior provider-agnostic. Do not add provider-specific branching in `main.rs` unless there is no shared alternative.
- Keep pure logic separate from system/network I/O when practical.
- Return structured `EngineError` kinds instead of collapsing everything into a single string.
- Use stable engine ids. These ids are stored in user config.
- Treat `VoiceInfo.id` as the value users may persist; `name` is for display.
- If a provider cannot support a feature, return a categorized unsupported/config error instead of silently doing nothing.

## Error guidance

Use these categories intentionally:

- `Unavailable`: binary missing, unsupported OS, provider cannot run here
- `Config`: missing keys, invalid endpoint, incomplete local setup
- `Authentication`: invalid API token or auth failure
- `Network`: timeout, DNS, connection, transport failure
- `Unsupported`: feature intentionally not implemented by the provider
- `Provider`: provider returned an error response or non-zero exit with meaningful provider output
- `Execution`: generic command/process/runtime failure on our side

## Config and secrets

- Non-secret defaults belong in `config.toml`.
- Secrets should not be stored in project docs or committed fixtures.
- Remote API providers should prefer environment variables or explicit credential config, then surface missing config through `EngineError::config(...)`.

## Testing requirements

Every engine contribution should include:

- unit tests for pure logic
- tests for provider metadata/capabilities
- at least one availability-path test
- voice-list tests if `voices()` is implemented
- failure-path tests for the main provider-specific error cases

Avoid tests that require real audio playback or real remote credentials in CI unless the repository explicitly adds dedicated infrastructure for them.

## Using the template skeleton

`src/engines/template.rs` is not a runnable provider. It exists to show:

- the expected async trait shape
- the metadata and capability model
- the shape of `VoiceInfo`
- the style of structured error returns
- the minimum testing pattern

The intended workflow is:

1. Copy the file to a new module.
2. Rename the type and ids.
3. Replace placeholder errors with real implementation.
4. Register the engine.
5. Expand tests from the skeleton into real provider tests.

## Documentation expectations

When a new engine becomes user-facing:

- mention it in `README.md`
- document setup requirements
- document any required environment variables or platform limits
- document whether voice discovery is supported
- document any rate/voice/language-detection caveats
