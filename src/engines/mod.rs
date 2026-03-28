mod minimax;
mod say;
pub mod template;

use crate::engine::Registry;

/// Register all built-in engines.
pub fn register_all(registry: &mut Registry) {
    registry.register(Box::new(minimax::MinimaxEngine::new()));
    registry.register(Box::new(say::SayEngine));
    // Future engines:
    // registry.register(Box::new(edge::EdgeEngine));
    // registry.register(Box::new(openai::OpenAiEngine));
}
