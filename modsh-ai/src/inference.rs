//! Local inference — Ollama and llama.cpp integration

use serde::{Deserialize, Serialize};
use std::process::{Command, Stdio};
use std::time::Duration;
use thiserror::Error;

/// Inference errors
#[derive(Error, Debug)]
pub enum InferenceError {
    /// Model not available
    #[error("model not available: {0}")]
    ModelNotAvailable(String),
    /// Inference failed
    #[error("inference failed: {0}")]
    InferenceFailed(String),
    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    /// Timeout
    #[error("timeout")]
    Timeout,
}

/// Inference backend
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Backend {
    /// Ollama API
    Ollama,
    /// llama.cpp HTTP server
    LlamaCpp,
}

/// Inference configuration
#[derive(Debug, Clone)]
pub struct InferenceConfig {
    /// Backend to use
    pub backend: Backend,
    /// Model name
    pub model: String,
    /// API endpoint URL
    pub endpoint: String,
    /// Timeout
    pub timeout: Duration,
    /// Max tokens to generate
    pub max_tokens: u32,
    /// Temperature
    pub temperature: f32,
}

impl Default for InferenceConfig {
    fn default() -> Self {
        Self {
            backend: Backend::Ollama,
            model: "auto".to_string(),
            endpoint: "http://localhost:11434".to_string(),
            timeout: Duration::from_secs(30),
            max_tokens: 150,
            temperature: 0.7,
        }
    }
}

/// Inference engine
pub struct InferenceEngine {
    config: InferenceConfig,
}

impl InferenceEngine {
    /// Create a new inference engine
    #[must_use]
    pub fn new(config: InferenceConfig) -> Self {
        Self { config }
    }

    /// Detect available models
    ///
    /// # Errors
    /// Returns an error if model detection fails
    pub fn detect_models(&self) -> Result<Vec<String>, InferenceError> {
        match self.config.backend {
            Backend::Ollama => Self::list_ollama_models(),
            Backend::LlamaCpp => Self::list_llamacpp_models(),
        }
    }

    fn list_ollama_models() -> Result<Vec<String>, InferenceError> {
        let output = Command::new("ollama").args(["list"]).output()?;

        if !output.status.success() {
            return Err(InferenceError::ModelNotAvailable(
                "Ollama not available".to_string(),
            ));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut models = Vec::new();

        // Parse output: NAME	ID	SIZE	MODIFIED
        for line in stdout.lines().skip(1) {
            if let Some(name) = line.split_whitespace().next() {
                models.push(name.to_string());
            }
        }

        Ok(models)
    }

    #[allow(clippy::unnecessary_wraps)]
    fn list_llamacpp_models() -> Result<Vec<String>, InferenceError> {
        // llama.cpp models are just files in a directory
        // TODO: Scan configured model directory
        Ok(Vec::new())
    }

    /// Generate a completion
    ///
    /// # Errors
    /// Returns an error if inference fails
    pub async fn complete(&self, prompt: &str, context: &str) -> Result<String, InferenceError> {
        let full_prompt = format!("Context: {context}\n\nUser: {prompt}\n\nAssistant:");

        match self.config.backend {
            Backend::Ollama => self.ollama_generate(&full_prompt).await,
            Backend::LlamaCpp => Self::llamacpp_generate(&full_prompt),
        }
    }

    async fn ollama_generate(&self, prompt: &str) -> Result<String, InferenceError> {
        let model = if self.config.model == "auto" {
            // Pick a reasonable default
            Self::list_ollama_models()?
                .into_iter()
                .next()
                .ok_or_else(|| {
                    InferenceError::ModelNotAvailable("No models found in Ollama".to_string())
                })?
        } else {
            self.config.model.clone()
        };

        let request = OllamaRequest {
            model,
            prompt: prompt.to_string(),
            stream: false,
            options: OllamaOptions {
                temperature: self.config.temperature,
                num_predict: i32::try_from(self.config.max_tokens).unwrap_or(i32::MAX),
            },
        };

        let client = reqwest::Client::new();
        let url = format!("{}/api/generate", self.config.endpoint);

        let response =
            tokio::time::timeout(self.config.timeout, client.post(&url).json(&request).send())
                .await;

        let response = match response {
            Ok(Ok(r)) => r,
            Ok(Err(e)) => return Err(InferenceError::InferenceFailed(e.to_string())),
            Err(_) => return Err(InferenceError::Timeout),
        };

        if !response.status().is_success() {
            return Err(InferenceError::InferenceFailed(format!(
                "HTTP {}",
                response.status()
            )));
        }

        let body: OllamaResponse = response
            .json()
            .await
            .map_err(|e: reqwest::Error| InferenceError::InferenceFailed(e.to_string()))?;

        Ok(body.response)
    }

    fn llamacpp_generate(_prompt: &str) -> Result<String, InferenceError> {
        // TODO: Implement llama.cpp HTTP API
        Err(InferenceError::InferenceFailed(
            "llama.cpp backend not yet implemented".to_string(),
        ))
    }
}

/// Ollama API request
#[derive(Serialize)]
struct OllamaRequest {
    model: String,
    prompt: String,
    stream: bool,
    options: OllamaOptions,
}

/// Ollama options
#[derive(Serialize)]
struct OllamaOptions {
    temperature: f32,
    num_predict: i32,
}

/// Ollama API response
#[derive(Deserialize)]
struct OllamaResponse {
    response: String,
}

/// Check if local inference is available
#[must_use]
pub fn is_inference_available() -> bool {
    // Check for Ollama
    if Command::new("ollama")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
    {
        return true;
    }

    // TODO: Check for llama.cpp

    false
}
