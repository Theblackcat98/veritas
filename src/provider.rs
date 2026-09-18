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
    temperature: f32,
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

/// Token usage as reported by the server in a stream chunk, when present (D011).
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
    temperature: f32,
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
        let chunk: ChatChunk =
            serde_json::from_str(r#"{"choices":[{"delta":{"content":"hi"}}]}"#)
                .expect("plain chunk parses");
        assert!(chunk.usage.is_none());
        assert_eq!(chunk.choices.len(), 1);
    }
}
