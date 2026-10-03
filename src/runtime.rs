use crate::{
    assets::{Clip, Pack},
    config::Config,
    device::{self, Ack, Target},
    engine::Engine,
    event::Event,
    ipc,
};
use anyhow::Result;
use serde_json::json;
use std::sync::{Arc, Mutex};
use tokio::{
    sync::{mpsc, watch},
    time::{Duration, Instant},
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

pub async fn run(config: Config, pack: Pack) -> Result<()> {
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
    let mut selected_base = target.key().map(str::to_owned);
    let (target_tx, target_rx) = watch::channel(target.clone());
    let (events_tx, mut events_rx) = mpsc::channel::<Event>(256);
    let (ack_tx, mut ack_rx) = mpsc::channel::<Ack>(16);
    let mut server = tokio::spawn(ipc::serve(config.pipe.clone(), events_tx, status.clone()));
    let mut writer = tokio::spawn(device::writer(
        config.device.clone(),
        config.dry_run,
        target_rx,
        ack_tx,
        status.clone(),
    ));
    let mut deadline: Option<Instant> = None;
    let mut role: Option<&'static str> = None;
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
                let chosen = base(&engine,&config,&pack);
                let chosen_name = chosen.as_ref().map(|clip| clip.name.clone());
                let urgent = engine.attention() && chosen_name.as_deref().is_some_and(|name|
                    named("needs-input",&engine,&config,&pack).is_some_and(|c| c.name == name));
                let reaction = outcome.reaction.and_then(|state| named(state,&engine,&config,&pack));
                let entry = if outcome.entered_work && !urgent {
                    named("working",&engine,&config,&pack).and_then(|clip|
                        clip.entry.as_ref().and_then(|name| pack.clips.get(name).cloned()))
                } else { None };
                let next = if let Some(clip) = reaction {
                    role = Some("reaction");
                    Some((Some(clip),true))
                } else if let Some(clip) = entry {
                    role = Some("entry");
                    Some((Some(clip),true))
                } else if role.is_some() && !urgent &&
                    (!engine.busy() || role == Some("entry")) && !outcome.entered_work {
                    None
                } else if chosen_name != selected_base || role.is_some() {
                    role = None;
                    selected_base = chosen_name;
                    Some((chosen,false))
                } else { None };
                if let Some((clip,single_play)) = next {
                    target = Target { serial:target.serial+1,clip,single_play,created:Instant::now() };
                    deadline = None;
                    target_tx.send_replace(target.clone());
                }
            }
            Some(ack) = ack_rx.recv() => {
                if ack.serial == target.serial &&
                    (target.single_play || target.clip.as_ref().is_some_and(|c| !c.looping && c.frames.len()>1)) {
                    deadline = Some(Instant::now()+Duration::from_millis(ack.hold_ms));
                }
            }
            _ = timer => {
                deadline = None;
                let next = if role.is_some() { base(&engine,&config,&pack) } else { None };
                selected_base = if role.is_some() { next.as_ref().map(|c| c.name.clone()) } else { selected_base };
                role = None;
                let next = next.or_else(|| target.clip.as_ref().map(|clip| Arc::new(Clip {
                    name:format!("{}:final",clip.name),frames:vec![clip.frames.last().unwrap().clone()],
                    frame_ms:clip.frame_ms,looping:true,entry:None,variants:Default::default(),
                })));
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
