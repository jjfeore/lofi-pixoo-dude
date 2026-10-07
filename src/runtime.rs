use crate::{
    assets::{Clip, Pack},
    config::{Config, Transport},
    device::{self, Ack, Target},
    engine::{Engine, Outcome},
    event::{Event, Kind},
    ipc,
    storage::Catalog,
};
use anyhow::Result;
use serde_json::json;
use std::sync::{Arc, Mutex};
use tokio::{
    sync::{mpsc, watch},
    time::Instant,
};

pub fn base(engine: &Engine, config: &Config, pack: &Pack) -> Option<Arc<Clip>> {
    engine.candidates().iter().find_map(|state| {
        config
            .mapping(state)
            .and_then(|name| pack.resolve(name, engine.busy()))
    })
}

fn named(state: &str, engine: &Engine, config: &Config, pack: &Pack) -> Option<Arc<Clip>> {
    config
        .mapping(state)
        .and_then(|name| pack.resolve(name, engine.busy()))
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Role {
    WorkEntry,
    DelegationEntry,
    DelegationExit,
    Reaction,
}

struct Playback {
    selected_base: Option<String>,
    role: Option<Role>,
}

impl Playback {
    fn on_event(&mut self, outcome: &Outcome, engine: &Engine, config: &Config, pack: &Pack)
        -> Option<(Option<Arc<Clip>>, bool)> {
        let chosen = base(engine, config, pack);
        let chosen_name = chosen.as_ref().map(|clip| clip.name.clone());
        let higher_priority = engine.candidates().iter()
            .filter(|state| matches!(**state, "needs-input" | "compacting"))
            .any(|state| named(state, engine, config, pack)
                .is_some_and(|clip| Some(clip.name.as_str()) == chosen_name.as_deref()));
        let delegation = named("delegating", engine, config, pack);
        let delegation_visible = engine.delegating() && delegation.as_ref()
            .is_some_and(|clip| Some(clip.name.as_str()) == chosen_name.as_deref());
        let reaction = if higher_priority { None }
            else { outcome.reaction.and_then(|state| named(state, engine, config, pack)) };
        let exit = if outcome.finished_delegation && !higher_priority {
            delegation.as_ref().and_then(|clip| clip.exit.as_ref())
                .and_then(|name| pack.clips.get(name).cloned())
        } else { None };
        let entry = if outcome.entered_delegation && delegation_visible && !higher_priority {
            delegation.as_ref().and_then(|clip| clip.entry.as_ref())
                .and_then(|name| pack.clips.get(name).cloned())
                .map(|clip| (clip, Role::DelegationEntry))
        } else { None };
        let entry = entry.or_else(|| {
            if outcome.entered_work && !higher_priority {
                named("working", engine, config, pack)
                    .and_then(|clip| clip.entry.as_ref().and_then(|name| pack.clips.get(name).cloned()))
                    .map(|clip| (clip, Role::WorkEntry))
            } else { None }
        });
        let protected = !higher_priority && !outcome.entered_work && match self.role {
            Some(Role::WorkEntry) => engine.busy(),
            Some(Role::DelegationEntry) => engine.delegating(),
            Some(Role::DelegationExit) => !engine.delegating() && engine.busy(),
            Some(Role::Reaction) => !engine.busy(),
            None => false,
        };
        if let Some(clip) = reaction {
            self.role = Some(Role::Reaction);
            Some((Some(clip), true))
        } else if let Some(clip) = exit {
            self.role = Some(Role::DelegationExit);
            Some((Some(clip), true))
        } else if let Some((clip, role)) = entry {
            self.role = Some(role);
            Some((Some(clip), true))
        } else if protected {
            None
        } else if chosen_name != self.selected_base || self.role.is_some() {
            self.role = None;
            self.selected_base = chosen_name;
            Some((chosen, false))
        } else { None }
    }

    fn after_clip(&mut self, engine: &Engine, config: &Config, pack: &Pack, current: Option<&Arc<Clip>>)
        -> Option<Arc<Clip>> {
        let next = if self.role.is_some() { base(engine, config, pack) } else { None };
        if self.role.is_some() { self.selected_base = next.as_ref().map(|clip| clip.name.clone()); }
        self.role = None;
        next.or_else(|| current.map(|clip| Arc::new(Clip {
            name: format!("{}:final", clip.name), frames: vec![clip.frames.last().unwrap().clone()],
            frame_ms: clip.frame_ms, looping: true, entry: None, exit: None, variants: Default::default(),
        })))
    }
}

fn acknowledge_playback(target: &Target, ack: Ack, deadline: &mut Option<Instant>) {
    if ack.serial == target.serial
        && (target.single_play
            || target.clip.as_ref().is_some_and(|clip| !clip.looping && clip.frames.len() > 1))
    {
        // Preserve the writer's deadline even if the acknowledgement sat in the
        // queue, or arrived after the clip should already have finished.
        *deadline = Some(ack.playback_ends_at);
    }
}

pub async fn run(config: Config, pack: Pack) -> Result<()> {
    let catalog = if config.device.transport == Transport::StoredGif {
        Some(Arc::new(Catalog::prepare(&config, &pack)?))
    } else {
        None
    };
    let status = Arc::new(Mutex::new(json!({
        "pack":pack.manifest.id, "observation":"events received since bridge start",
        "device":{"dry_run":config.dry_run}, "prepared_bytes":pack.encoded_bytes
    })));
    let mut engine = Engine::default();
    let initial = base(&engine, &config, &pack).or_else(|| {
        pack.manifest
            .default_animation
            .as_ref()
            .and_then(|name| pack.clips.get(name).cloned())
    });
    let mut target = Target {
        serial: 1,
        clip: initial,
        single_play: false,
        created: Instant::now(),
    };
    let mut playback = Playback { selected_base: target.key().map(str::to_owned), role: None };
    let (target_tx, target_rx) = watch::channel(target.clone());
    let (events_tx, mut events_rx) = mpsc::channel::<Event>(256);
    let (ack_tx, mut ack_rx) = mpsc::channel::<Ack>(16);
    let mut server = tokio::spawn(ipc::serve(config.pipe.clone(), events_tx, status.clone()));
    let mut writer = tokio::spawn(device::writer(
        config.device.clone(),
        config.dry_run,
        catalog,
        target_rx,
        ack_tx,
        status.clone(),
    ));
    let mut deadline: Option<Instant> = None;
    eprintln!(
        "bridge listening on {}; dry_run={}",
        config.pipe, config.dry_run
    );
    loop {
        status.lock().unwrap()["state"] = serde_json::to_value(engine.snapshot())?;
        status.lock().unwrap()["desired_clip"] = json!(target.key());
        let timer = async {
            if let Some(deadline) = deadline {
                tokio::time::sleep_until(deadline).await;
            } else {
                std::future::pending::<()>().await;
            }
        };
        tokio::select! {
            Some(event) = events_rx.recv() => {
                let outcome = engine.apply(&event);
                if matches!(event.kind, Kind::AgentStart | Kind::AgentStop) {
                    eprintln!(
                        "delegation event: {:?}; observed_children={}",
                        event.kind,
                        engine.snapshot().observed_children
                    );
                }
                let next = playback.on_event(&outcome, &engine, &config, &pack);
                if let Some((clip,single_play)) = next {
                    target = Target { serial:target.serial+1,clip,single_play,created:Instant::now() };
                    deadline = None;
                    target_tx.send_replace(target.clone());
                }
            }
            Some(ack) = ack_rx.recv() => {
                acknowledge_playback(&target, ack, &mut deadline);
            }
            _ = timer => {
                deadline = None;
                let next = playback.after_clip(&engine, &config, &pack, target.clip.as_ref());
                target = Target {serial:target.serial+1,clip:next,single_play:false,created:Instant::now()};
                target_tx.send_replace(target.clone());
            }
            result = &mut server => { result??; break; }
            result = &mut writer => { result??; break; }
            _ = tokio::signal::ctrl_c() => { eprintln!("bridge stopped; panel retains its current content"); break; }
        }
    }
    server.abort();
    writer.abort();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::Kind;
    use tokio::time::{Duration, sleep, timeout};

    #[tokio::test]
    async fn delayed_playback_acknowledgement_does_not_restart_the_one_shot_timer() {
        let target = Target { serial: 7, clip: None, single_play: true, created: Instant::now() };
        let playback_ends_at = Instant::now() + Duration::from_millis(20);
        let (sender, mut receiver) = mpsc::channel(1);
        sender.send(Ack { serial: target.serial, playback_ends_at }).await.unwrap();
        sleep(Duration::from_millis(40)).await;
        let mut deadline = None;
        acknowledge_playback(&target, receiver.recv().await.unwrap(), &mut deadline);
        assert_eq!(deadline, Some(playback_ends_at));
        timeout(Duration::from_millis(50), tokio::time::sleep_until(deadline.unwrap()))
            .await.unwrap();
    }

    #[test]
    fn stale_acknowledgements_and_native_loops_do_not_replace_a_one_shot_deadline() {
        let pack = delegation_pack(true);
        let mut target = Target { serial: 7, clip: Some(pack.clips["working-enter"].clone()),
            single_play: false, created: Instant::now() };
        let playback_ends_at = Instant::now() + Duration::from_secs(1);
        let mut deadline = None;
        acknowledge_playback(&target, Ack { serial: 7, playback_ends_at }, &mut deadline);
        assert_eq!(deadline, Some(playback_ends_at));
        acknowledge_playback(&target, Ack { serial: 6, playback_ends_at: Instant::now() }, &mut deadline);
        assert_eq!(deadline, Some(playback_ends_at));
        target.clip = Some(pack.clips["working"].clone());
        deadline = None;
        acknowledge_playback(&target, Ack { serial: 7, playback_ends_at }, &mut deadline);
        assert!(deadline.is_none());
    }

    fn delegation_pack(transitions: bool) -> Pack {
        let names = ["idle", "working", "working-enter", "delegating", "needs-input", "compacting", "finished", "interrupted"];
        let mut clips = std::collections::BTreeMap::new();
        for name in names.into_iter().chain(transitions.then_some("delegating-start"))
            .chain(transitions.then_some("delegating-finished")) {
            clips.insert(name.into(), Arc::new(Clip {
                name: name.into(), frames: vec!["frame-a".into(), "frame-b".into()], frame_ms: 83,
                looping: !matches!(name, "working-enter" | "delegating-start" | "delegating-finished" | "finished" | "interrupted"),
                entry: if name == "working" { Some("working-enter".into()) }
                    else if name == "delegating" && transitions { Some("delegating-start".into()) } else { None },
                exit: (name == "delegating" && transitions).then(|| "delegating-finished".into()),
                variants: Default::default(),
            }));
        }
        Pack { manifest: crate::assets::Manifest {
            schema_version: 1, id: "in-memory-playback-test".into(), canvas_size: 64,
            background: "#000000".into(), default_animation: Some("idle".into()), animations: Default::default(),
        }, clips, encoded_bytes: 0 }
    }

    fn signal(playback: &mut Playback, engine: &mut Engine, pack: &Pack, kind: Kind, agent: Option<&str>)
        -> Option<(String, bool)> {
        let event = Event {
            kind, session_id: "parent".into(),
            turn_id: Some(agent.map_or_else(|| "turn".into(), |id| format!("child-{id}"))),
            agent_id: agent.map(str::to_owned), tool_name: Some("Bash".into()), tool_use_id: None,
            observed_at_ms: 1,
        };
        let outcome = engine.apply(&event);
        playback.on_event(&outcome, engine, &Config::default(), pack)
            .map(|(clip, single)| (clip.unwrap().name.clone(), single))
    }

    fn working_playback(pack: &Pack) -> (Playback, Engine) {
        let mut playback = Playback { selected_base: Some("idle".into()), role: None };
        let mut engine = Engine::default();
        assert_eq!(signal(&mut playback, &mut engine, pack, Kind::Prompt, None), Some(("working-enter".into(), true)));
        assert_eq!(playback.after_clip(&engine, &Config::default(), pack, None).unwrap().name, "working");
        (playback, engine)
    }

    #[test]
    fn delegation_transitions_play_once_for_first_and_last_child() {
        let pack = delegation_pack(true);
        let (mut playback, mut engine) = working_playback(&pack);
        assert_eq!(signal(&mut playback, &mut engine, &pack, Kind::AgentStart, Some("one")), Some(("delegating-start".into(), true)));
        assert!(signal(&mut playback, &mut engine, &pack, Kind::AgentStart, Some("one")).is_none());
        assert!(signal(&mut playback, &mut engine, &pack, Kind::AgentStart, Some("two")).is_none());
        assert_eq!(playback.after_clip(&engine, &Config::default(), &pack, None).unwrap().name, "delegating");
        assert!(signal(&mut playback, &mut engine, &pack, Kind::AgentStop, Some("one")).is_none());
        assert_eq!(signal(&mut playback, &mut engine, &pack, Kind::AgentStop, Some("two")), Some(("delegating-finished".into(), true)));
        assert!(signal(&mut playback, &mut engine, &pack, Kind::ToolComplete, None).is_none());
        assert_eq!(playback.after_clip(&engine, &Config::default(), &pack, None).unwrap().name, "working");
        assert!(signal(&mut playback, &mut engine, &pack, Kind::AgentStop, Some("two")).is_none());
    }

    #[test]
    fn delegation_quick_stop_and_new_start_replace_pending_transitions() {
        let pack = delegation_pack(true);
        let (mut playback, mut engine) = working_playback(&pack);
        signal(&mut playback, &mut engine, &pack, Kind::AgentStart, Some("one"));
        assert_eq!(signal(&mut playback, &mut engine, &pack, Kind::AgentStop, Some("one")), Some(("delegating-finished".into(), true)));
        assert_eq!(signal(&mut playback, &mut engine, &pack, Kind::AgentStart, Some("two")), Some(("delegating-start".into(), true)));
    }

    #[test]
    fn delegation_transfers_yield_to_attention_and_compaction() {
        let pack = delegation_pack(true);
        let (mut playback, mut engine) = working_playback(&pack);
        signal(&mut playback, &mut engine, &pack, Kind::AgentStart, Some("one"));
        assert_eq!(signal(&mut playback, &mut engine, &pack, Kind::Approval, None), Some(("needs-input".into(), false)));
        assert!(signal(&mut playback, &mut engine, &pack, Kind::AgentStop, Some("one")).is_none());
        assert_eq!(signal(&mut playback, &mut engine, &pack, Kind::ToolComplete, None), Some(("working".into(), false)));
        assert_eq!(signal(&mut playback, &mut engine, &pack, Kind::CompactStart, None), Some(("compacting".into(), false)));
        assert!(signal(&mut playback, &mut engine, &pack, Kind::AgentStart, Some("two")).is_none());
        assert!(signal(&mut playback, &mut engine, &pack, Kind::AgentStop, Some("two")).is_none());
        assert_eq!(signal(&mut playback, &mut engine, &pack, Kind::CompactEnd, None), Some(("working".into(), false)));
        engine.apply(&Event {
            kind: Kind::CompactStart, session_id: "other-chat".into(), turn_id: Some("manual".into()),
            agent_id: None, tool_name: None, tool_use_id: None, observed_at_ms: 2,
        });
        assert_eq!(signal(&mut playback, &mut engine, &pack, Kind::Complete, None), Some(("compacting".into(), false)));
    }

    #[test]
    fn delegation_optional_transitions_keep_previous_loop_behavior() {
        let pack = delegation_pack(false);
        let (mut playback, mut engine) = working_playback(&pack);
        assert_eq!(signal(&mut playback, &mut engine, &pack, Kind::AgentStart, Some("one")), Some(("delegating".into(), false)));
        assert_eq!(signal(&mut playback, &mut engine, &pack, Kind::AgentStop, Some("one")), Some(("working".into(), false)));
    }

    #[test]
    fn delegation_missing_loop_ignores_visuals_and_keeps_child_bookkeeping() {
        let mut pack = delegation_pack(false);
        pack.clips.remove("delegating");
        let (mut playback, mut engine) = working_playback(&pack);
        assert!(signal(&mut playback, &mut engine, &pack, Kind::AgentStart, Some("one")).is_none());
        assert_eq!(engine.snapshot().observed_children, 1);
        assert!(signal(&mut playback, &mut engine, &pack, Kind::AgentStop, Some("one")).is_none());
        assert_eq!(engine.snapshot().observed_children, 0);
        assert!(engine.busy());
    }

    #[test]
    fn delegation_parent_interrupt_overrides_transfer_without_return_gesture() {
        let pack = delegation_pack(true);
        let (mut playback, mut engine) = working_playback(&pack);
        signal(&mut playback, &mut engine, &pack, Kind::AgentStart, Some("one"));
        assert_eq!(signal(&mut playback, &mut engine, &pack, Kind::Interrupt, None), Some(("interrupted".into(), true)));
        assert!(signal(&mut playback, &mut engine, &pack, Kind::AgentStop, Some("one")).is_none());
        assert_eq!(playback.after_clip(&engine, &Config::default(), &pack, None).unwrap().name, "idle");
    }

    #[test]
    fn missing_reactions_do_not_block_completion_or_idle() {
        let root = tempfile::tempdir().unwrap();
        image::RgbaImage::new(64, 64)
            .save(root.path().join("idle.png"))
            .unwrap();
        image::RgbaImage::new(64, 64)
            .save(root.path().join("working.png"))
            .unwrap();
        crate::assets::import_directory(root.path(), "partial", 83, false).unwrap();
        let pack = Pack::load(root.path(), 40).unwrap();
        let config = Config::default();
        let mut engine = Engine::default();
        let mut event = Event {
            kind: Kind::Prompt,
            session_id: "a".into(),
            turn_id: Some("1".into()),
            agent_id: None,
            tool_name: None,
            tool_use_id: None,
            observed_at_ms: 1,
        };
        engine.apply(&event);
        assert_eq!(base(&engine, &config, &pack).unwrap().name, "working");
        event.kind = Kind::Complete;
        event.observed_at_ms = 2;
        let outcome = engine.apply(&event);
        assert!(named(outcome.reaction.unwrap(), &engine, &config, &pack).is_none());
        assert_eq!(base(&engine, &config, &pack).unwrap().name, "idle");
    }
}
