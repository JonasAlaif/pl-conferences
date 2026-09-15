mod model;
#[allow(dead_code)]
mod tests;

pub use model::*;

use std::sync::OnceLock;

use ollama_rs::Ollama;

pub fn ollama() -> &'static Ollama {
    static OLLAMA: OnceLock<Ollama> = OnceLock::new();
    OLLAMA.get_or_init(|| Ollama::default())
}

pub async fn model() -> Model {
    llama3_2().await
}
