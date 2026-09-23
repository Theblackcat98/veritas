use std::time::Duration;

use futures_util::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::UnboundedSender;
use tokio::time::timeout;

/// No bytes for this long = hung server; fail instead of spinning forever.
const IDLE_TIMEOUT: Duration = Duration::from_secs(60);
/// Guard against unbounded growth on a line without a newline (D004).
const MAX_LINE_BUF: usize = 1024 * 1024;

#[derive(Debug, Clone)]
pub enum StreamEvent {
    Token(String),
    Usage {
        prompt_tokens: u64,
        completion_tokens: u64,
    },
    Done,
    Error(String),
}

#[derive(Debug, Clone, Serialize)]
pub struct WireMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Serialize)]
struct StreamOptions {
    include_usage: bool,
}

#[derive(Debug, Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<WireMessage>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    stream_options: Option<StreamOptions>,
    /// Omitted entirely unless the user overrides via /temp — the engine's
    /// own temperature then applies (D009).
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
}

#[derive(Debug, Deserialize)]
struct ChatChunk {
    #[serde(default)]
    choices: Vec<ChunkChoice>,
    usage: Option<Usage>,
}

#[derive(Debug, Deserialize)]
struct ChunkChoice {
    delta: ChunkDelta,
}

#[derive(Debug, Deserialize)]
struct ChunkDelta {
    content: Option<String>,
}

/// Token usage as reported by the server in a stream chunk, when present (D009).
#[derive(Debug, Deserialize)]
struct Usage {
    #[serde(default)]
    prompt_tokens: u64,
    #[serde(default)]
    completion_tokens: u64,
}

/// POST {base}/chat/completions with SSE, forwarding delta.content as Token events.
/// `base` like http://localhost:11434/v1, `model` an engine-specific id (D030).
pub async fn stream_chat(
    base: String,
    api_key: String,
    model: String,
    temperature: Option<f32>,
    history: Vec<WireMessage>,
    tx: UnboundedSender<StreamEvent>,
) {
    let url = format!("{}/chat/completions", base.trim_end_matches('/'));
    let req = ChatRequest {
        model,
        messages: history,
        stream: true,
        stream_options: Some(StreamOptions {
            include_usage: true,
        }),
        temperature,
    };

    let client = Client::builder()
        .connect_timeout(std::time::Duration::from_secs(5))
        .build()
        .unwrap_or_else(|_| Client::new());

    let mut req_builder = client.post(&url).json(&req);
    if !api_key.is_empty() {
        req_builder = req_builder.bearer_auth(api_key);
    }

    let resp = match req_builder.send().await {
        Ok(r) => r,
        Err(e) => {
            let _ = tx.send(StreamEvent::Error(format!("request failed: {e}")));
            return;
        }
    };
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        let body_short: String = body.chars().take(300).collect();
        let _ = tx.send(StreamEvent::Error(format!("HTTP {status}: {body_short}")));
        return;
    }

    let mut stream = resp.bytes_stream();
    // Bytes, not String: multi-byte UTF-8 may split across network chunks.
    // Decoding only complete lines keeps char boundaries safe — `\n` never
    // occurs inside a multi-byte sequence.
    let mut buf: Vec<u8> = Vec::new();

    loop {
        let item = match timeout(IDLE_TIMEOUT, stream.next()).await {
            Ok(Some(item)) => item,
            Ok(None) => break,
            Err(_) => {
                let _ = tx.send(StreamEvent::Error(format!(
                    "stream idle timeout ({}s)",
                    IDLE_TIMEOUT.as_secs()
                )));
                return;
            }
        };
        let chunk = match item {
            Ok(c) => c,
            Err(e) => {
                let _ = tx.send(StreamEvent::Error(format!("stream error: {e}")));
                return;
            }
        };
        buf.extend_from_slice(&chunk);

        // Process complete lines; keep the byte remainder in buf.
        let mut consumed_upto = 0usize;
        let mut done = false;
        let mut err: Option<String> = None;
        for line in buf.split_inclusive(|&b| b == b'\n') {
            if !line.ends_with(b"\n") {
                break;
            }
            consumed_upto += line.len();
            let line = String::from_utf8_lossy(line);
            let line = line.trim();
            if line.is_empty() || !line.starts_with("data:") {
                continue;
            }
            let data = line.trim_start_matches("data:").trim();
            if data == "[DONE]" {
                done = true;
                break;
            }
            match serde_json::from_str::<ChatChunk>(data) {
                Ok(parsed) => {
                    if let Some(u) = parsed.usage {
                        let _ = tx.send(StreamEvent::Usage {
                            prompt_tokens: u.prompt_tokens,
                            completion_tokens: u.completion_tokens,
                        });
                    }
                    for choice in parsed.choices {
                        if let Some(text) = choice.delta.content {
                            if !text.is_empty() {
                                let _ = tx.send(StreamEvent::Token(text));
                            }
                        }
                    }
                }
                Err(_) => {
                    let snippet: String = data.chars().take(120).collect();
                    err = Some(format!("bad stream chunk: {snippet}"));
                    break;
                }
            }
        }
        if consumed_upto > 0 {
            buf.drain(..consumed_upto);
        }
        if let Some(e) = err {
            let _ = tx.send(StreamEvent::Error(e));
            return;
        }
        if done {
            let _ = tx.send(StreamEvent::Done);
            return;
        }
        if buf.len() > MAX_LINE_BUF {
            let _ = tx.send(StreamEvent::Error(format!(
                "stream line exceeded {} KiB",
                MAX_LINE_BUF / 1024
            )));
            return;
        }
    }
    let _ = tx.send(StreamEvent::Done);
}

/// Probe `GET {base}/models` and return the first model id (D030). None on
/// any error or empty list — the sidebar then shows `n/a`.
pub async fn fetch_models(base: String, api_key: String) -> Option<String> {
    let url = format!("{}/models", base.trim_end_matches('/'));
    let client = Client::builder()
        .timeout(Duration::from_secs(3))
        .build()
        .unwrap_or_else(|_| Client::new());
    let mut req = client.get(&url);
    if !api_key.is_empty() {
        req = req.bearer_auth(api_key);
    }
    let resp = req.send().await.ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let v: serde_json::Value = resp.json().await.ok()?;
    parse_first_model(&v)
}

/// OpenAI-shaped `{ "data": [ { "id": ... } ] }` — shared by Ollama,
/// llama.cpp server, vLLM, and OpenRouter.
fn parse_first_model(v: &serde_json::Value) -> Option<String> {
    v.get("data")?
        .as_array()?
        .first()?
        .get("id")?
        .as_str()
        .map(str::to_string)
}

/// Best-effort probe of the model's context window from the inference
/// engine (D009). Probes Ollama (`POST {root}/api/show`) with fallback to
/// llama.cpp server (`GET {root}/props`). Returns None on any other engine or
/// any error; the sidebar then shows `n/a`.
pub async fn fetch_context_length(base: String, model: String) -> Option<u64> {
    let trimmed = base.trim_end_matches('/');
    let root = trimmed.strip_suffix("/v1").unwrap_or(trimmed);
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .build()
        .unwrap_or_else(|_| Client::new());

    // 1. Try Ollama: POST {root}/api/show
    let ollama_url = format!("{root}/api/show");
    if let Ok(resp) = client
        .post(&ollama_url)
        .json(&serde_json::json!({ "model": model }))
        .send()
        .await
    {
        if resp.status().is_success() {
            if let Ok(v) = resp.json::<serde_json::Value>().await {
                if let Some(ctx) = parse_context_length(&v) {
                    return Some(ctx);
                }
            }
        }
    }

    // 2. Try llama.cpp server: GET {root}/props
    let props_url = format!("{root}/props");
    if let Ok(resp) = client.get(&props_url).send().await {
        if resp.status().is_success() {
            if let Ok(v) = resp.json::<serde_json::Value>().await {
                if let Some(ctx) = parse_llama_cpp_props(&v) {
                    return Some(ctx);
                }
            }
        }
    }

    None
}

fn parse_llama_cpp_props(v: &serde_json::Value) -> Option<u64> {
    if let Some(n) = v
        .get("default_generation_settings")
        .and_then(|s| s.get("n_ctx"))
        .and_then(|x| x.as_u64())
    {
        return Some(n);
    }
    if let Some(n) = v.get("n_ctx").and_then(|x| x.as_u64()) {
        return Some(n);
    }
    None
}

fn parse_context_length(v: &serde_json::Value) -> Option<u64> {
    // `num_ctx` in `parameters` is the engine's effective window when set;
    // it caps the model's native context length, so it wins.
    if let Some(params) = v.get("parameters").and_then(|p| p.as_str()) {
        for line in params.lines() {
            let mut parts = line.split_whitespace();
            if parts.next() == Some("num_ctx") {
                if let Some(n) = parts.next().and_then(|s| s.parse::<u64>().ok()) {
                    return Some(n);
                }
            }
        }
    }
    let info = v.get("model_info")?;
    if let Some(n) = info.get("context_length").and_then(|x| x.as_u64()) {
        return Some(n);
    }
    info.as_object()?
        .iter()
        .find(|(k, _)| k.ends_with(".context_length"))
        .and_then(|(_, val)| val.as_u64())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usage_only_chunk_parses_without_choices() {
        // Ollama-style final chunk: empty choices, token usage attached.
        let chunk: ChatChunk = serde_json::from_str(
            r#"{"choices":[],"usage":{"prompt_tokens":12,"completion_tokens":34}}"#,
        )
        .expect("usage chunk parses");
        let u = chunk.usage.expect("usage present");
        assert_eq!((u.prompt_tokens, u.completion_tokens), (12, 34));
        assert!(chunk.choices.is_empty());
    }

    #[test]
    fn chunk_without_usage_and_missing_choices_still_parses() {
        let chunk: ChatChunk = serde_json::from_str(r#"{"choices":[{"delta":{"content":"hi"}}]}"#)
            .expect("plain chunk parses");
        assert!(chunk.usage.is_none());
        assert_eq!(chunk.choices.len(), 1);
    }

    #[test]
    fn temperature_omitted_from_request_when_unset() {
        let req = ChatRequest {
            model: "m".to_string(),
            messages: vec![],
            stream: true,
            stream_options: None,
            temperature: None,
        };
        let v = serde_json::to_value(&req).expect("serializes");
        assert!(v.get("temperature").is_none());
    }

    #[test]
    fn temperature_included_when_overridden() {
        let req = ChatRequest {
            model: "m".to_string(),
            messages: vec![],
            stream: true,
            stream_options: None,
            temperature: Some(0.5),
        };
        let v = serde_json::to_value(&req).expect("serializes");
        assert_eq!(v.get("temperature"), Some(&serde_json::json!(0.5)));
    }

    #[test]
    fn context_length_from_arch_keyed_model_info() {
        let v = serde_json::json!({
            "model_info": {
                "general.architecture": "llama",
                "llama.context_length": 131072
            }
        });
        assert_eq!(parse_context_length(&v), Some(131072));
    }

    #[test]
    fn num_ctx_in_parameters_wins() {
        let v = serde_json::json!({
            "parameters": "stop <|im_end|>\nnum_ctx 8192\ntemperature 0.7",
            "model_info": { "llama.context_length": 131072 }
        });
        assert_eq!(parse_context_length(&v), Some(8192));
    }

    #[test]
    fn context_length_absent_is_none() {
        let v = serde_json::json!({ "model_info": {} });
        assert_eq!(parse_context_length(&v), None);
    }

    #[test]
    fn stream_options_included_in_chat_request() {
        let req = ChatRequest {
            model: "m".to_string(),
            messages: vec![],
            stream: true,
            stream_options: Some(StreamOptions {
                include_usage: true,
            }),
            temperature: None,
        };
        let v = serde_json::to_value(&req).expect("serializes");
        assert_eq!(
            v.get("stream_options"),
            Some(&serde_json::json!({ "include_usage": true }))
        );
    }

    #[test]
    fn parse_llama_cpp_props_extracts_n_ctx() {
        let v = serde_json::json!({
            "default_generation_settings": {
                "n_ctx": 4096
            }
        });
        assert_eq!(parse_llama_cpp_props(&v), Some(4096));

        let v2 = serde_json::json!({ "n_ctx": 8192 });
        assert_eq!(parse_llama_cpp_props(&v2), Some(8192));

        let v3 = serde_json::json!({});
        assert_eq!(parse_llama_cpp_props(&v3), None);
    }

    #[test]
    fn parse_first_model_reads_openai_shaped_data_array() {
        let v = serde_json::json!({
            "object": "list",
            "data": [
                { "id": "llama3.2", "object": "model" },
                { "id": "qwen2.5", "object": "model" }
            ]
        });
        assert_eq!(parse_first_model(&v).as_deref(), Some("llama3.2"));
        assert_eq!(parse_first_model(&serde_json::json!({})), None);
        assert_eq!(parse_first_model(&serde_json::json!({ "data": [] })), None);
        assert_eq!(
            parse_first_model(&serde_json::json!({ "data": [{ "object": "model" }] })),
            None
        );
    }
}
