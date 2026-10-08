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
    pub entered_delegation: bool,
    pub finished_delegation: bool,
    pub reaction: Option<&'static str>,
    pub terminal_applied: bool,
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
    pub fn delegating(&self) -> bool {
        self.sessions.values().any(|s| !s.children.is_empty())
    }
    pub fn candidates(&self) -> Vec<&'static str> {
        let mut candidates = Vec::with_capacity(5);
        if self.attention() {
            candidates.push("needs-input");
        }
        if self.sessions.values().any(|s| s.compacting) {
            candidates.push("compacting");
        }
        if self.delegating() {
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
        let was_delegating = self.delegating();
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
        let mut terminal_applied = false;
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
            // Subagent hooks identify the parent session, but their turn ID can
            // belong to the child (or be absent). Keep the parent's turn intact.
            // A known, completed parent must not be revived by a late hook.
            Kind::AgentStart if session.running || session.turn.is_none() => {
                session.running = true;
                if let Some(agent) = &event.agent_id
                    && session.children.len() < MAX_CHILDREN
                {
                    session.children.insert(agent.clone());
                }
            }
            Kind::AgentStop => {
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
                    terminal_applied = true;
                    session.turn = Some(turn.into());
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
            entered_delegation: !was_delegating && self.delegating(),
            finished_delegation: event.kind == Kind::AgentStop && was_delegating && !self.delegating(),
            reaction,
            terminal_applied,
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
    fn completion_disposition_distinguishes_applied_stale_and_duplicate_events() {
        let mut engine = Engine::default();
        engine.apply(&event(Kind::Prompt, "a", "current", 10));
        assert!(!engine.apply(&event(Kind::Complete, "a", "old", 20)).terminal_applied);
        assert!(engine.busy());
        assert!(engine.apply(&event(Kind::Complete, "a", "current", 30)).terminal_applied);
        assert!(!engine.busy());
        let duplicate = engine.apply(&event(Kind::Complete, "a", "current", 30));
        assert!(!duplicate.terminal_applied);
        assert!(duplicate.reaction.is_none());
        engine.apply(&event(Kind::Prompt, "a", "next", 40));
        assert!(!engine.apply(&event(Kind::Complete, "a", "current", 30)).terminal_applied);
        assert!(engine.busy());
    }
    #[test]
    fn child_stop_does_not_end_parent() {
        let mut engine = Engine::default();
        let mut e = event(Kind::AgentStart, "a", "1", 1);
        e.agent_id = Some("child".into());
        assert!(engine.apply(&e).entered_delegation);
        assert_eq!(engine.candidates()[0], "delegating");
        e.kind = Kind::AgentStop;
        assert!(engine.apply(&e).finished_delegation);
        assert_eq!(engine.candidates()[0], "working");
        e.kind = Kind::Interrupt;
        assert_eq!(engine.apply(&e).reaction, Some("interrupted"));
        assert!(!engine.busy());
    }
    #[test]
    fn delegation_hooks_with_child_turn_ids_preserve_the_parent_turn() {
        let mut engine = Engine::default();
        engine.apply(&event(Kind::Prompt, "parent", "parent-turn", 10));
        let mut child = event(Kind::AgentStart, "parent", "child-turn", 20);
        child.agent_id = Some("child".into());
        assert!(engine.apply(&child).entered_delegation);
        assert_eq!(engine.snapshot().observed_children, 1);
        assert_eq!(engine.sessions["parent"].turn.as_deref(), Some("parent-turn"));
        child.kind = Kind::AgentStop;
        child.observed_at_ms = 30;
        assert!(engine.apply(&child).finished_delegation);
        assert!(engine.busy());
        assert_eq!(engine.candidates(), ["working"]);
        assert_eq!(
            engine.apply(&event(Kind::Complete, "parent", "parent-turn", 40)).reaction,
            Some("finished")
        );
        assert!(!engine.busy());
    }
    #[test]
    fn delegation_hooks_without_turn_ids_track_parallel_children() {
        let mut engine = Engine::default();
        engine.apply(&event(Kind::Prompt, "parent", "parent-turn", 10));
        let mut child = event(Kind::AgentStart, "parent", "unused", 20);
        child.turn_id = None;
        child.agent_id = Some("one".into());
        assert!(engine.apply(&child).entered_delegation);
        child.agent_id = Some("two".into());
        assert!(!engine.apply(&child).entered_delegation);
        assert_eq!(engine.snapshot().observed_children, 2);
        child.kind = Kind::AgentStop;
        child.agent_id = Some("one".into());
        assert!(!engine.apply(&child).finished_delegation);
        assert!(engine.delegating());
        child.agent_id = Some("two".into());
        assert!(engine.apply(&child).finished_delegation);
        assert!(engine.busy());
    }
    #[test]
    fn late_delegation_hooks_do_not_restart_a_completed_parent() {
        let mut engine = Engine::default();
        engine.apply(&event(Kind::Prompt, "parent", "parent-turn", 10));
        engine.apply(&event(Kind::Complete, "parent", "parent-turn", 20));
        let mut child = event(Kind::AgentStart, "parent", "child-turn", 30);
        child.agent_id = Some("child".into());
        assert!(!engine.apply(&child).entered_delegation);
        assert!(!engine.busy());
        assert!(!engine.delegating());
    }
    #[test]
    fn delegation_cannot_restart_a_parent_whose_prompt_was_unobserved() {
        let mut engine = Engine::default();
        let mut child = event(Kind::AgentStart, "parent", "child-turn", 10);
        child.agent_id = Some("child".into());
        assert!(engine.apply(&child).entered_delegation);
        assert!(engine.busy());
        engine.apply(&event(Kind::Complete, "parent", "parent-turn", 20));
        child.turn_id = Some("other-child-turn".into());
        child.observed_at_ms = 30;
        assert!(!engine.apply(&child).entered_delegation);
        assert!(!engine.busy());
        assert!(!engine.delegating());
    }
    #[test]
    fn delegation_edges_follow_first_and_last_child_across_chats() {
        let mut engine = Engine::default();
        let mut a = event(Kind::AgentStart, "a", "1", 1);
        a.agent_id = Some("child-a".into());
        let mut b = event(Kind::AgentStart, "b", "2", 2);
        b.agent_id = Some("child-b".into());
        assert!(engine.apply(&a).entered_delegation);
        assert!(!engine.apply(&a).entered_delegation);
        assert!(!engine.apply(&b).entered_delegation);
        a.kind = Kind::AgentStop;
        assert!(!engine.apply(&a).finished_delegation);
        b.kind = Kind::AgentStop;
        b.agent_id = Some("unknown".into());
        assert!(!engine.apply(&b).finished_delegation);
        b.agent_id = Some("child-b".into());
        assert!(engine.apply(&b).finished_delegation);
        assert!(!engine.apply(&b).finished_delegation);
        assert!(engine.busy());
    }
    #[test]
    fn clearing_children_on_parent_interrupt_is_not_delegation_finish() {
        let mut engine = Engine::default();
        let mut e = event(Kind::AgentStart, "a", "1", 1);
        e.agent_id = Some("child".into());
        engine.apply(&e);
        e.kind = Kind::Interrupt;
        let outcome = engine.apply(&e);
        assert!(!outcome.finished_delegation);
        assert_eq!(outcome.reaction, Some("interrupted"));
        e.kind = Kind::AgentStop;
        assert!(!engine.apply(&e).finished_delegation);
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
