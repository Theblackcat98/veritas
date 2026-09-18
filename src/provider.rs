use futures_util::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::UnboundedSender;

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
struct ChatRequest {
    model: String,
    messages: Vec<WireMessage>,
    stream: bool,
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
/// `base` like http://localhost:11434/v1, `model` like llama3.1 / gpt-4o-mini.
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
        temperature,
    };

    let mut req_builder = Client::new().post(&url).json(&req);
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
    let mut buf = String::new();

    while let Some(item) = stream.next().await {
        let chunk = match item {
            Ok(c) => c,
            Err(e) => {
                let _ = tx.send(StreamEvent::Error(format!("stream error: {e}")));
                return;
            }
        };
        buf.push_str(&String::from_utf8_lossy(&chunk));

        // Process complete lines; keep remainder in buf.
        let mut consumed_upto = 0usize;
        for line in buf.split_inclusive('\n') {
            if !line.ends_with('\n') {
                break;
            }
            consumed_upto += line.len();
            let line = line.trim();
            if line.is_empty() || !line.starts_with("data:") {
                continue;
            }
            let data = line.trim_start_matches("data:").trim();
            if data == "[DONE]" {
                let _ = tx.send(StreamEvent::Done);
                return;
            }
            if let Ok(parsed) = serde_json::from_str::<ChatChunk>(data) {
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
        }
        if consumed_upto > 0 {
            buf.drain(..consumed_upto);
        }
        // Guard against unbounded growth on a line without newline.
        if buf.len() > 1024 * 1024 {
            buf.clear();
        }
    }
    let _ = tx.send(StreamEvent::Done);
}

/// Best-effort probe of the model's context window from the inference
/// engine (D009). Ollama-only today — the charter's default local target:
/// `POST {root}/api/show` on the base URL with `/v1` stripped. Returns None
/// on any other engine or any error; the sidebar then shows `n/a`.
pub async fn fetch_context_length(base: String, model: String) -> Option<u64> {
    let trimmed = base.trim_end_matches('/');
    let root = trimmed.strip_suffix("/v1").unwrap_or(trimmed);
    let url = format!("{root}/api/show");
    let resp = Client::new()
        .post(&url)
        .json(&serde_json::json!({ "model": model }))
        .send()
        .await
        .ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let v: serde_json::Value = resp.json().await.ok()?;
    parse_context_length(&v)
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
}
