use crate::{
    assets::{Clip, Pack},
    config::{Config, Transport},
    device::{self, Ack, Target},
    engine::{Engine, Outcome},
    event::{Event, Kind},
    idle::IdleSchedule,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Role {
    WorkEntry,
    DelegationEntry,
    DelegationExit,
    Reaction,
    IdleAlternate,
}

struct Playback {
    selected_base: Option<String>,
    role: Option<Role>,
}

impl Playback {
    fn on_event(&mut self, outcome: &Outcome, engine: &Engine, config: &Config, pack: &Pack)
        -> Option<(Option<Arc<Clip>>, bool)> {
        let chosen = base(engine, config, pack).or_else(|| {
            // Even a missing higher-priority asset must end an idle vignette.
            if self.role == Some(Role::IdleAlternate) { named("idle", engine, config, pack) }
            else { None }
        });
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
            Some(Role::IdleAlternate) => engine.candidates()[0] == "idle",
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
    let mut idle_schedule = IdleSchedule::new(&config, &pack)?;
    let normal_idle = named("idle", &Engine::default(), &config, &pack)
        .map(|clip| clip.name.clone());
    let catalog = if config.device.transport == Transport::StoredGif {
        Some(Arc::new(Catalog::prepare(&config, &pack)?))
    } else {
        None
    };
    let status = Arc::new(Mutex::new(json!({
        "pack":pack.manifest.id, "observation":"events received since bridge start",
        "device":{"dry_run":config.dry_run}, "prepared_bytes":pack.encoded_bytes,
        "process_id":std::process::id()
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
    let mut acknowledged_serial = None;
    eprintln!(
        "bridge listening on {}; dry_run={}",
        config.pipe, config.dry_run
    );
    loop {
        let now = Instant::now();
        let normal_idle_ready = engine.candidates()[0] == "idle" && playback.role.is_none()
            && normal_idle.as_deref().is_some_and(|name| target.key() == Some(name))
            && acknowledged_serial == Some(target.serial);
        idle_schedule.update(normal_idle_ready, now);
        status.lock().unwrap()["state"] = serde_json::to_value(engine.snapshot())?;
        status.lock().unwrap()["desired_clip"] = json!(target.key());
        let playing_alternate = if playback.role == Some(Role::IdleAlternate) { target.key() } else { None };
        status.lock().unwrap()["idle_alternates"] = idle_schedule.status(now, playing_alternate);
        let timer = async {
            if let Some(deadline) = deadline {
                tokio::time::sleep_until(deadline).await;
            } else {
                std::future::pending::<()>().await;
            }
        };
        let idle_timer = async {
            if let Some(deadline) = idle_schedule.deadline() {
                tokio::time::sleep_until(deadline).await;
            } else {
                std::future::pending::<()>().await;
            }
        };
        tokio::select! {
            biased; // Observed activity wins over an idle deadline ready in the same iteration.
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
                if ack.serial == target.serial { acknowledged_serial = Some(ack.serial); }
                acknowledge_playback(&target, ack, &mut deadline);
            }
            _ = timer => {
                deadline = None;
                let next = playback.after_clip(&engine, &config, &pack, target.clip.as_ref());
                target = Target {serial:target.serial+1,clip:next,single_play:false,created:Instant::now()};
                target_tx.send_replace(target.clone());
            }
            _ = idle_timer => {
                if normal_idle_ready && let Some(clip) = idle_schedule.take_due(Instant::now()) {
                    eprintln!("idle alternate: {}", clip.name);
                    playback.role = Some(Role::IdleAlternate);
                    target = Target { serial:target.serial+1,clip:Some(clip),single_play:true,created:Instant::now() };
                    deadline = None;
                    target_tx.send_replace(target.clone());
                }
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
    fn idle_alternates_survive_idle_events_return_to_idle_and_yield_to_activity() {
        let pack = delegation_pack(true);
        for (kind, expected) in [(Kind::Prompt, "working-enter"), (Kind::Approval, "needs-input"),
            (Kind::CompactStart, "compacting"), (Kind::AgentStart, "delegating-start")] {
            let mut engine = Engine::default();
            let mut playback = Playback { selected_base: Some("idle".into()), role: Some(Role::IdleAlternate) };
            assert!(signal(&mut playback, &mut engine, &pack, Kind::SessionStart, None).is_none());
            assert_eq!(playback.role, Some(Role::IdleAlternate));
            assert_eq!(signal(&mut playback, &mut engine, &pack, kind, (kind == Kind::AgentStart).then_some("child"))
                .unwrap().0, expected);
            assert!(playback.role != Some(Role::IdleAlternate));
        }
        let mut playback = Playback { selected_base: Some("idle".into()), role: Some(Role::IdleAlternate) };
        assert_eq!(playback.after_clip(&Engine::default(), &Config::default(), &pack, None).unwrap().name, "idle");
        assert!(playback.role.is_none());
    }

    #[test]
    fn missing_priority_assets_still_cancel_an_idle_alternate() {
        let mut pack = delegation_pack(false);
        pack.clips.remove("compacting");
        let mut engine = Engine::default();
        let mut playback = Playback { selected_base: Some("idle".into()), role: Some(Role::IdleAlternate) };
        assert_eq!(signal(&mut playback, &mut engine, &pack, Kind::CompactStart, None), Some(("idle".into(), false)));
        assert_eq!(engine.candidates()[0], "compacting");
        assert!(playback.role.is_none());
    }

    #[tokio::test]
    async fn deduplicated_idle_target_is_acknowledged_without_replaying_it() {
        let pack = delegation_pack(false);
        let first = Target { serial: 1, clip: Some(pack.clips["idle"].clone()), single_play: false, created: Instant::now() };
        let (sender, receiver) = watch::channel(first.clone());
        let (ack_sender, mut acks) = mpsc::channel(4);
        let status = Arc::new(Mutex::new(json!({})));
        let config = crate::config::DeviceConfig { transport: Transport::Frames, ..Default::default() };
        let writer = tokio::spawn(device::writer(config, true, None, receiver, ack_sender, status));
        assert_eq!(timeout(Duration::from_secs(1), acks.recv()).await.unwrap().unwrap().serial, 1);
        sender.send_replace(Target { serial: 2, clip: Some(pack.clips["delegating"].clone()), single_play: true, created: Instant::now() });
        sender.send_replace(Target { serial: 3, ..first });
        assert_eq!(timeout(Duration::from_secs(1), acks.recv()).await.unwrap().unwrap().serial, 3);
        writer.abort();
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn live_controller_cycles_alternates_preempts_and_resets_after_compaction() {
        async fn wait_for(pipe: &str, predicate: impl Fn(&serde_json::Value) -> bool) -> serde_json::Value {
            let end = Instant::now() + Duration::from_secs(3);
            loop {
                let last = match ipc::send(pipe, ipc::Request::Status, 100).await {
                    Ok(Some(state)) if predicate(&state) => return state,
                    Ok(state) => format!("{state:?}"),
                    Err(error) => format!("{error:#}"),
                };
                assert!(Instant::now() < end, "controller status wait expired: {last}");
                sleep(Duration::from_millis(10)).await;
            }
        }
        async fn emit(pipe: &str, kind: Kind, session: &str) {
            ipc::send(pipe, ipc::Request::Event(Event {
                kind, session_id: session.into(), turn_id: Some("turn".into()), agent_id: None,
                tool_name: None, tool_use_id: None, observed_at_ms: 1,
            }), 1000).await.unwrap();
        }
        let mut pack = delegation_pack(true);
        for name in ["idle-one", "idle-two"] {
            pack.clips.insert(name.into(), Arc::new(Clip {
                name: name.into(), frames: vec!["frame-a".into(), "frame-b".into()], frame_ms: 200,
                looping: true, entry: None, exit: None, variants: Default::default(),
            }));
        }
        let config = Config {
            dry_run: true,
            device: crate::config::DeviceConfig { transport: Transport::Frames, ..Default::default() },
            pipe: format!(r"\\.\pipe\pixoo-idle-test-{}-{}", std::process::id(), fastrand::u64(..)),
            idle_alternates: crate::config::IdleAlternatesConfig {
                animations: vec!["idle-one".into(), "idle-two".into()],
                min_interval_ms: 250,
                max_interval_ms: 250,
                ..Default::default()
            },
            ..Default::default()
        };
        let pipe = config.pipe.clone();
        let bridge = tokio::spawn(run(config, pack));
        let first = wait_for(&pipe, |s| s["idle_alternates"]["playing"].is_string()).await;
        assert_eq!(first["state"]["logical_state"], "idle");
        wait_for(&pipe, |s| s["device"]["last_clip"] == "idle" && s["idle_alternates"]["next_in_ms"].is_number()).await;
        let second = wait_for(&pipe, |s| s["idle_alternates"]["playing"].is_string()).await;
        assert_ne!(first["idle_alternates"]["playing"], second["idle_alternates"]["playing"]);
        emit(&pipe, Kind::Prompt, "work").await;
        wait_for(&pipe, |s| s["device"]["last_clip"] == "working").await;
        sleep(Duration::from_millis(400)).await;
        let working = wait_for(&pipe, |s| s["state"]["logical_state"] == "working").await;
        assert!(working["idle_alternates"]["playing"].is_null());
        assert!(working["idle_alternates"]["next_in_ms"].is_null());
        emit(&pipe, Kind::Complete, "work").await;
        let reaction = wait_for(&pipe, |s| s["device"]["last_clip"] == "finished").await;
        assert!(reaction["idle_alternates"]["next_in_ms"].is_null());
        wait_for(&pipe, |s| s["device"]["last_clip"] == "idle" && s["idle_alternates"]["next_in_ms"].is_number()).await;
        emit(&pipe, Kind::CompactStart, "manual").await;
        wait_for(&pipe, |s| s["device"]["last_clip"] == "compacting").await;
        sleep(Duration::from_millis(400)).await;
        let compacting = wait_for(&pipe, |s| s["state"]["logical_state"] == "compacting").await;
        assert_eq!(compacting["state"]["working"], 0);
        assert!(compacting["idle_alternates"]["next_in_ms"].is_null());
        emit(&pipe, Kind::CompactEnd, "manual").await;
        let idle = wait_for(&pipe, |s| s["device"]["last_clip"] == "idle" && s["idle_alternates"]["next_in_ms"].is_number()).await;
        assert!(idle["idle_alternates"]["next_in_ms"].as_u64().unwrap() > 100);
        bridge.abort();
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
