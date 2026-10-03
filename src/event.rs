use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    SessionStart,
    SessionEnd,
    Prompt,
    Approval,
    ToolComplete,
    CompactStart,
    CompactEnd,
    AgentStart,
    AgentStop,
    Complete,
    Interrupt,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Event {
    pub kind: Kind,
    pub session_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_use_id: Option<String>,
    #[serde(default)]
    pub observed_at_ms: u64,
}

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

impl Event {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            !self.session_id.is_empty() && self.session_id.len() <= 256,
            "invalid session ID"
        );
        for id in [
            &self.turn_id,
            &self.agent_id,
            &self.tool_name,
            &self.tool_use_id,
        ]
        .into_iter()
        .flatten()
        {
            ensure!(
                !id.is_empty() && id.len() <= 512,
                "invalid event identifier"
            );
        }
        Ok(())
    }

    pub fn normalize(value: &Value) -> Result<Option<Self>> {
        let name = value
            .get("hook_event_name")
            .and_then(Value::as_str)
            .or_else(|| value.get("type").and_then(Value::as_str))
            .unwrap_or("");
        let kind = match name {
            "SessionStart" => Kind::SessionStart,
            "SessionEnd" => Kind::SessionEnd,
            "UserPromptSubmit" => Kind::Prompt,
            "PermissionRequest" => Kind::Approval,
            "PostToolUse" => Kind::ToolComplete,
            "PreCompact" => Kind::CompactStart,
            "PostCompact" => Kind::CompactEnd,
            "SubagentStart" => Kind::AgentStart,
            "SubagentStop" => Kind::AgentStop,
            "agent-turn-complete" => Kind::Complete,
            "Interrupt" => Kind::Interrupt,
            // Stop is a continuation checkpoint, not completion.
            _ => return Ok(None),
        };
        let string = |key: &str| value.get(key).and_then(Value::as_str).map(str::to_owned);
        let event = Self {
            kind,
            session_id: string("session_id")
                .or_else(|| string("thread-id"))
                .unwrap_or_default(),
            turn_id: string("turn_id").or_else(|| string("turn-id")),
            agent_id: string("agent_id"),
            tool_name: string("tool_name"),
            tool_use_id: string("tool_use_id"),
            observed_at_ms: now_ms(),
        };
        event.validate()?;
        Ok(Some(event))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn strips_content_and_ignores_stop() {
        let event = Event::normalize(&serde_json::json!({
            "hook_event_name":"UserPromptSubmit", "session_id":"a", "turn_id":"b",
            "prompt":"private text", "transcript_path":"private file"
        }))
        .unwrap()
        .unwrap();
        let serialized = serde_json::to_string(&event).unwrap();
        assert!(!serialized.contains("private"));
        assert!(
            Event::normalize(&serde_json::json!({"hook_event_name":"Stop"}))
                .unwrap()
                .is_none()
        );
    }
    #[test]
    fn completion_notifier_fields_are_normalized() {
        let event = Event::normalize(&serde_json::json!({
            "type":"agent-turn-complete", "thread-id":"a", "turn-id":"b"
        }))
        .unwrap()
        .unwrap();
        assert_eq!(event.kind, Kind::Complete);
        assert_eq!(event.turn_id.as_deref(), Some("b"));
    }
}
