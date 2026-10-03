use crate::event::{Event, Kind};
use serde::Serialize;
use std::collections::{HashMap, HashSet, VecDeque};

const MAX_SESSIONS: usize = 1024;
const MAX_CHILDREN: usize = 128;
const MAX_TERMINALS: usize = 64;

#[derive(Default)]
struct Session {
    turn: Option<String>,
    running: bool,
    compacting: bool,
    approvals: HashSet<String>,
    children: HashSet<String>,
    terminals: VecDeque<(String, u64)>,
}

#[derive(Default)]
pub struct Engine {
    sessions: HashMap<String, Session>,
}

#[derive(Debug, Default)]
pub struct Outcome {
    pub entered_work: bool,
    pub reaction: Option<&'static str>,
}

#[derive(Serialize)]
pub struct Snapshot {
    pub sessions: usize,
    pub working: usize,
    pub needs_input: usize,
    pub compacting: usize,
    pub observed_children: usize,
    pub logical_state: &'static str,
}

impl Engine {
    pub fn busy(&self) -> bool {
        self.sessions.values().any(|s| s.running)
    }
    pub fn attention(&self) -> bool {
        self.sessions.values().any(|s| !s.approvals.is_empty())
    }
    pub fn candidates(&self) -> Vec<&'static str> {
        let mut candidates = Vec::with_capacity(5);
        if self.attention() {
            candidates.push("needs-input");
        }
        if self.sessions.values().any(|s| s.compacting) {
            candidates.push("compacting");
        }
        if self.sessions.values().any(|s| !s.children.is_empty()) {
            candidates.push("delegating");
        }
        candidates.push(if self.busy() { "working" } else { "idle" });
        candidates
    }
    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            sessions: self.sessions.len(),
            working: self.sessions.values().filter(|s| s.running).count(),
            needs_input: self
                .sessions
                .values()
                .filter(|s| !s.approvals.is_empty())
                .count(),
            compacting: self.sessions.values().filter(|s| s.compacting).count(),
            observed_children: self.sessions.values().map(|s| s.children.len()).sum(),
            logical_state: self.candidates()[0],
        }
    }
    pub fn apply(&mut self, event: &Event) -> Outcome {
        let was_busy = self.busy();
        if event.kind == Kind::SessionEnd {
            self.sessions.remove(&event.session_id);
            return Outcome::default();
        }
        if !self.sessions.contains_key(&event.session_id) && self.sessions.len() >= MAX_SESSIONS {
            return Outcome::default();
        }
        let session = self.sessions.entry(event.session_id.clone()).or_default();
        if event.kind == Kind::SessionStart {
            return Outcome::default();
        }

        let turn = event.turn_id.as_deref().unwrap_or("unidentified-turn");
        let terminal = session
            .terminals
            .iter()
            .find(|(id, _)| id == turn)
            .map(|(_, time)| *time);
        if terminal.is_some() && event.kind != Kind::Prompt {
            return Outcome::default();
        }
        if event.kind == Kind::Prompt && terminal.is_some_and(|time| event.observed_at_ms <= time) {
            return Outcome::default();
        }
        let matches = session.turn.as_deref().is_none_or(|id| id == turn);
        let mut reaction = None;
        match event.kind {
            Kind::Prompt => {
                if session.turn.as_deref() != Some(turn) || !session.running {
                    session.approvals.clear();
                    session.children.clear();
                    session.compacting = false;
                }
                session.terminals.retain(|(id, _)| id != turn);
                session.turn = Some(turn.into());
                session.running = true;
            }
            Kind::Approval if matches => {
                session.turn = Some(turn.into());
                session.running = true;
                if session.approvals.len() < 128 {
                    session
                        .approvals
                        .insert(event.tool_name.clone().unwrap_or_default());
                }
            }
            Kind::ToolComplete if matches => {
                // This is observed tool resumption, not a universal approval-resolution API.
                if let Some(tool) = &event.tool_name {
                    session.approvals.remove(tool);
                }
            }
            Kind::CompactStart if matches => {
                session.turn = Some(turn.into());
                session.compacting = true;
                // Manual compaction doesn't force the already-idle base pose into working.
            }
            Kind::CompactEnd if matches => session.compacting = false,
            Kind::AgentStart if matches => {
                session.turn = Some(turn.into());
                session.running = true;
                if let Some(agent) = &event.agent_id
                    && session.children.len() < MAX_CHILDREN
                {
                    session.children.insert(agent.clone());
                }
            }
            Kind::AgentStop if matches => {
                if let Some(agent) = &event.agent_id {
                    session.children.remove(agent);
                }
            }
            Kind::Complete | Kind::Interrupt => {
                session
                    .terminals
                    .push_back((turn.into(), event.observed_at_ms));
                while session.terminals.len() > MAX_TERMINALS {
                    session.terminals.pop_front();
                }
                if matches {
                    session.running = false;
                    session.approvals.clear();
                    session.children.clear();
                    session.compacting = false;
                    reaction = Some(if event.kind == Kind::Complete {
                        "finished"
                    } else {
                        "interrupted"
                    });
                }
            }
            _ => {}
        }
        // One chat ending does not jack the aggregate pet out of another active chat.
        if self.busy() || self.attention() {
            reaction = None;
        }
        Outcome {
            entered_work: !was_busy && self.busy(),
            reaction,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn event(kind: Kind, session: &str, turn: &str, at: u64) -> Event {
        Event {
            kind,
            session_id: session.into(),
            turn_id: Some(turn.into()),
            agent_id: None,
            tool_name: None,
            tool_use_id: None,
            observed_at_ms: at,
        }
    }
    #[test]
    fn simultaneous_chats_and_attention_priority() {
        let mut engine = Engine::default();
        assert!(engine.apply(&event(Kind::Prompt, "a", "1", 1)).entered_work);
        assert!(!engine.apply(&event(Kind::Prompt, "b", "2", 2)).entered_work);
        engine.apply(&event(Kind::Approval, "b", "2", 3));
        assert_eq!(engine.candidates()[0], "needs-input");
        assert!(
            engine
                .apply(&event(Kind::Complete, "a", "1", 4))
                .reaction
                .is_none()
        );
        assert_eq!(engine.snapshot().working, 1);
        assert_eq!(
            engine.apply(&event(Kind::Complete, "b", "2", 5)).reaction,
            Some("finished")
        );
        assert_eq!(engine.candidates()[0], "idle");
    }
    #[test]
    fn late_events_cannot_resurrect_a_completed_turn() {
        let mut engine = Engine::default();
        engine.apply(&event(Kind::Prompt, "a", "1", 10));
        engine.apply(&event(Kind::Complete, "a", "1", 20));
        engine.apply(&event(Kind::AgentStart, "a", "1", 15));
        engine.apply(&event(Kind::Prompt, "a", "1", 15));
        assert!(!engine.busy());
        assert!(
            engine
                .apply(&event(Kind::Prompt, "a", "2", 30))
                .entered_work
        );
        engine.apply(&event(Kind::Complete, "a", "1", 40));
        assert!(engine.busy());
    }
    #[test]
    fn compaction_preserves_idle_or_working_pose() {
        let mut engine = Engine::default();
        engine.apply(&event(Kind::CompactStart, "a", "manual", 1));
        assert!(!engine.busy());
        assert_eq!(engine.candidates(), ["compacting", "idle"]);
        engine.apply(&event(Kind::CompactEnd, "a", "manual", 2));
        engine.apply(&event(Kind::Prompt, "a", "work", 3));
        engine.apply(&event(Kind::CompactStart, "a", "work", 4));
        engine.apply(&event(Kind::CompactEnd, "a", "work", 5));
        assert_eq!(engine.candidates(), ["working"]);
    }
    #[test]
    fn child_stop_does_not_end_parent() {
        let mut engine = Engine::default();
        let mut e = event(Kind::AgentStart, "a", "1", 1);
        e.agent_id = Some("child".into());
        engine.apply(&e);
        assert_eq!(engine.candidates()[0], "delegating");
        e.kind = Kind::AgentStop;
        engine.apply(&e);
        assert_eq!(engine.candidates()[0], "working");
        e.kind = Kind::Interrupt;
        assert_eq!(engine.apply(&e).reaction, Some("interrupted"));
        assert!(!engine.busy());
    }
    #[test]
    fn only_matching_tool_resumes_observed_approval() {
        let mut engine = Engine::default();
        let mut e = event(Kind::Approval, "a", "1", 1);
        e.tool_name = Some("Bash".into());
        engine.apply(&e);
        e.kind = Kind::ToolComplete;
        e.tool_name = Some("apply_patch".into());
        engine.apply(&e);
        assert!(engine.attention());
        e.tool_name = Some("Bash".into());
        engine.apply(&e);
        assert!(!engine.attention());
        assert!(engine.busy());
    }
}
