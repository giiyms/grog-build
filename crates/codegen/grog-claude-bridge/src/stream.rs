//! Parse Claude Code `--output-format stream-json` lines into grog events.

use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamEvent {
    TextDelta(String),
    ToolUse {
        id: String,
        name: String,
        input: Value,
    },
    ToolResult {
        id: String,
    },
    Result {
        session_id: Option<String>,
        text: String,
    },
    Error(String),
    Ignored,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssistantDelta {
    pub text: String,
}

#[derive(Deserialize)]
struct Envelope {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    message: Option<Message>,
    #[serde(default)]
    result: Option<String>,
    #[serde(default)]
    session_id: Option<String>,
    #[serde(default)]
    error: Option<Value>,
}

#[derive(Deserialize)]
struct Message {
    #[serde(default)]
    content: Vec<ContentBlock>,
}

#[derive(Deserialize)]
struct ContentBlock {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    input: Option<Value>,
}

pub fn parse_stream_json_line(line: &str) -> Result<StreamEvent, serde_json::Error> {
    let line = line.trim();
    if line.is_empty() {
        return Ok(StreamEvent::Ignored);
    }
    let env: Envelope = serde_json::from_str(line)?;
    Ok(match env.kind.as_str() {
        "assistant" => {
            let mut text = String::new();
            if let Some(msg) = env.message {
                for block in msg.content {
                    if block.kind == "text" {
                        if let Some(t) = block.text {
                            text.push_str(&t);
                        }
                    } else if block.kind == "tool_use" {
                        return Ok(StreamEvent::ToolUse {
                            id: block.id.unwrap_or_default(),
                            name: block.name.unwrap_or_default(),
                            input: block.input.unwrap_or(Value::Null),
                        });
                    }
                }
            }
            if text.is_empty() {
                StreamEvent::Ignored
            } else {
                StreamEvent::TextDelta(text)
            }
        }
        "result" => StreamEvent::Result {
            session_id: env.session_id,
            text: env.result.unwrap_or_default(),
        },
        "error" => StreamEvent::Error(
            env.error
                .map(|e| e.to_string())
                .unwrap_or_else(|| "error".into()),
        ),
        _ => StreamEvent::Ignored,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_assistant_text() {
        let line = r#"{"type":"assistant","message":{"content":[{"type":"text","text":"Hello"}]}}"#;
        assert_eq!(
            parse_stream_json_line(line).unwrap(),
            StreamEvent::TextDelta("Hello".into())
        );
    }

    #[test]
    fn parses_tool_use() {
        let line = r#"{"type":"assistant","message":{"content":[{"type":"tool_use","id":"t1","name":"Read","input":{"path":"a.rs"}}]}}"#;
        match parse_stream_json_line(line).unwrap() {
            StreamEvent::ToolUse { id, name, .. } => {
                assert_eq!(id, "t1");
                assert_eq!(name, "Read");
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn parses_result_with_session() {
        let line = r#"{"type":"result","session_id":"sess-1","result":"done"}"#;
        match parse_stream_json_line(line).unwrap() {
            StreamEvent::Result { session_id, text } => {
                assert_eq!(session_id.as_deref(), Some("sess-1"));
                assert_eq!(text, "done");
            }
            other => panic!("{other:?}"),
        }
    }
}
