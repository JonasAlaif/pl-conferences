//! Minimal Ollama client: one chat call, schema-constrained JSON out.

use anyhow::{Context, Result, anyhow};

/// Errors from the model server, split so callers can tell "the server is
/// gone" (fatal for the run) from "this answer was unusable" (skip the page).
#[derive(Debug)]
pub enum LlmError {
    Transport(anyhow::Error),
    BadOutput(String),
}

impl std::fmt::Display for LlmError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LlmError::Transport(e) => write!(f, "{e:#}"),
            LlmError::BadOutput(s) => write!(f, "unusable model output: {s}"),
        }
    }
}

impl std::error::Error for LlmError {}
use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use std::time::Duration;

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        format!("{}… [{} chars total]", s.chars().take(n).collect::<String>(), s.chars().count())
    }
}

/// The prompt as shown in logs: everything before the page in full, the
/// page itself reduced to a short preview and its size.
fn prompt_preview(user: &str) -> String {
    match user.split_once("PAGE:\n") {
        Some((head, page)) => format!("{head}PAGE: [{} chars] {}", page.chars().count(), truncate(page, 300).replace('\n', " ")),
        None => truncate(user, 2500),
    }
}

#[derive(Debug, Clone)]
pub struct Llm {
    pub base: String,
    pub model: String,
    pub think: bool,
    pub num_ctx: u32,
    pub temperature: f32,
    /// `PLC_NUM_GPU=0` forces CPU inference (used to measure memory locally).
    pub num_gpu: Option<u32>,
}

#[derive(Debug, Clone, Default)]
pub struct Usage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub prompt_ms: u64,
    pub completion_ms: u64,
    pub total_ms: u64,
}

impl Llm {
    /// Configuration from the environment: `OLLAMA_HOST`, `PLC_MODEL`,
    /// `PLC_THINK`, `PLC_NUM_CTX`.
    pub fn from_env() -> Self {
        let base = std::env::var("OLLAMA_HOST").unwrap_or_else(|_| "http://localhost:11434".into());
        let base = if base.starts_with("http") { base } else { format!("http://{base}") };
        Self {
            base: base.trim_end_matches('/').to_string(),
            model: std::env::var("PLC_MODEL").unwrap_or_else(|_| "qwen3.5:4b".into()),
            think: matches!(std::env::var("PLC_THINK").as_deref(), Ok("1") | Ok("true")),
            num_ctx: std::env::var("PLC_NUM_CTX").ok().and_then(|s| s.parse().ok()).unwrap_or(16384),
            temperature: 0.0,
            num_gpu: std::env::var("PLC_NUM_GPU").ok().and_then(|s| s.parse().ok()),
        }
    }

    /// Ask for a JSON value matching `T`'s schema. Returns the parsed value,
    /// the raw text and usage statistics.
    pub fn extract<T: DeserializeOwned + JsonSchema>(&self, system: &str, user: &str) -> Result<(T, String, Usage), LlmError> {
        let schema = serde_json::to_value(schemars::schema_for!(T)).map_err(|e| LlmError::Transport(e.into()))?;
        let schema_text = serde_json::to_string_pretty(&schema).map_err(|e| LlmError::Transport(e.into()))?;
        let schema_name = std::any::type_name::<T>().rsplit("::").next().unwrap_or("?").to_string();
        log::info!("llm >>> system: {system}\nllm >>> user: {}\nllm >>> format: JSON schema for {schema_name}", prompt_preview(user));
        let user = format!("{user}\n\nRespond with JSON matching this schema:\n{schema_text}");
        let (raw, usage) = self.chat(system, &user, Some(schema)).map_err(LlmError::Transport)?;
        let compact = serde_json::from_str::<Value>(&raw).map(|v| v.to_string()).unwrap_or_else(|_| raw.clone());
        log::info!("llm <<< {}", truncate(&compact, 3000));
        let value: T = serde_json::from_str(&raw).map_err(|e| LlmError::BadOutput(format!("{e}: {}", raw.chars().take(300).collect::<String>())))?;
        Ok((value, raw, usage))
    }

    /// Unload the model so its server process is started afresh by the next
    /// request. The runner process was seen growing from 5 GB to 13 GB of
    /// resident memory over a 90-minute run; a reload from the page cache
    /// costs seconds and resets that. Failures are only logged.
    pub fn unload(&self) {
        let body = json!({ "model": self.model, "keep_alive": 0 });
        let res = reqwest::blocking::Client::builder().timeout(Duration::from_secs(60)).build().and_then(|c| c.post(format!("{}/api/generate", self.base)).json(&body).send());
        match res {
            Ok(r) if r.status().is_success() => log::info!("model unloaded to free memory"),
            Ok(r) => log::warn!("unloading the model failed: HTTP {}", r.status()),
            Err(e) => log::warn!("unloading the model failed: {e}"),
        }
    }

    /// One chat completion. `format` constrains decoding to a JSON schema.
    pub fn chat(&self, system: &str, user: &str, format: Option<Value>) -> Result<(String, Usage)> {
        let mut body = json!({
            "model": self.model,
            "stream": false,
            "think": self.think,
            "keep_alive": "30m",
            "messages": [
                {"role": "system", "content": system},
                {"role": "user", "content": user},
            ],
            "options": {
                "temperature": self.temperature,
                "num_ctx": self.num_ctx,
                "num_predict": 2048,
            }
        });
        if let Some(f) = format {
            body["format"] = f;
        }
        if let Some(g) = self.num_gpu {
            body["options"]["num_gpu"] = json!(g);
        }
        let client = reqwest::blocking::Client::builder().timeout(Duration::from_secs(60 * 60)).build()?;
        let resp = client
            .post(format!("{}/api/chat", self.base))
            .json(&body)
            .send()
            .with_context(|| format!("POST {}/api/chat", self.base))?;
        let status = resp.status();
        let text = resp.text()?;
        if !status.is_success() {
            return Err(anyhow!("ollama HTTP {status}: {text}"));
        }
        let v: Value = serde_json::from_str(&text).context("ollama response is not JSON")?;
        let content = v["message"]["content"].as_str().unwrap_or("").to_string();
        let ns = |k: &str| v[k].as_u64().unwrap_or(0) / 1_000_000;
        let usage = Usage {
            prompt_tokens: v["prompt_eval_count"].as_u64().unwrap_or(0),
            completion_tokens: v["eval_count"].as_u64().unwrap_or(0),
            prompt_ms: ns("prompt_eval_duration"),
            completion_ms: ns("eval_duration"),
            total_ms: ns("total_duration"),
        };
        log::info!(
            "llm {}: prompt {} tok in {:.1}s, output {} tok in {:.1}s",
            self.model, usage.prompt_tokens, usage.prompt_ms as f64 / 1000.0,
            usage.completion_tokens, usage.completion_ms as f64 / 1000.0
        );
        Ok((content, usage))
    }

    /// True when the server answers and the model is present.
    pub fn check(&self) -> Result<()> {
        let v: Value = reqwest::blocking::get(format!("{}/api/tags", self.base))?.json()?;
        let names: Vec<&str> = v["models"].as_array().map(|a| a.iter().filter_map(|m| m["name"].as_str()).collect()).unwrap_or_default();
        let wanted = if self.model.contains(':') { self.model.clone() } else { format!("{}:latest", self.model) };
        if names.iter().any(|n| *n == wanted) {
            Ok(())
        } else {
            Err(anyhow!("model {} not found in ollama (have: {})", self.model, names.join(", ")))
        }
    }
}
