use crate::{assets::{Clip, Pack}, config::{Config, IdleAlternatesConfig}};
use anyhow::Result;
use std::sync::Arc;
use tokio::time::{Duration, Instant};

/// One random wait per confirmed normal-idle period. Playback itself is timed
/// by the existing writer acknowledgement and one-shot controller.
pub struct IdleSchedule {
    settings: IdleAlternatesConfig,
    clips: Vec<Arc<Clip>>,
    random: fastrand::Rng,
    next_at: Option<Instant>,
    previous: Option<usize>,
}

impl IdleSchedule {
    pub fn new(config: &Config, pack: &Pack) -> Result<Self> {
        Ok(Self {
            settings: config.idle_alternates.clone(),
            clips: config.idle_alternate_clips(pack)?,
            random: fastrand::Rng::new(),
            next_at: None,
            previous: None,
        })
    }

    pub fn update(&mut self, normal_idle_ready: bool, now: Instant) {
        if !normal_idle_ready {
            self.next_at = None;
        } else if self.next_at.is_none() && !self.clips.is_empty() {
            let delay = self.random.u64(self.settings.min_interval_ms..=self.settings.max_interval_ms);
            self.next_at = Some(now + Duration::from_millis(delay));
        }
    }

    pub fn deadline(&self) -> Option<Instant> { self.next_at }

    pub fn take_due(&mut self, now: Instant) -> Option<Arc<Clip>> {
        if self.next_at.is_none_or(|deadline| now < deadline) { return None; }
        self.next_at = None;
        let skip = self.previous.filter(|_| self.settings.avoid_immediate_repeat && self.clips.len() > 1);
        let mut index = self.random.usize(..self.clips.len() - usize::from(skip.is_some()));
        if skip.is_some_and(|previous| index >= previous) { index += 1; }
        self.previous = Some(index);
        Some(self.clips[index].clone())
    }

    pub fn status(&self, now: Instant, playing: Option<&str>) -> serde_json::Value {
        serde_json::json!({
            "enabled": !self.clips.is_empty(),
            "animations": self.clips.iter().map(|clip| clip.name.as_str()).collect::<Vec<_>>(),
            "min_interval_ms": self.settings.min_interval_ms,
            "max_interval_ms": self.settings.max_interval_ms,
            "avoid_immediate_repeat": self.settings.avoid_immediate_repeat,
            "playing": playing,
            "next_in_ms": self.next_at.map(|deadline| deadline.saturating_duration_since(now).as_millis()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn fixture(names: &[&str]) -> (Config, Pack) {
        let clips = ["idle"].into_iter().chain(names.iter().copied()).map(|name| {
            (name.into(), Arc::new(Clip {
                name: name.into(), frames: vec!["frame".into()], frame_ms: 83,
                looping: true, entry: None, exit: None, variants: BTreeMap::new(),
            }))
        }).collect();
        let mut config = Config::default();
        config.idle_alternates.animations = names.iter().map(|name| (*name).into()).collect();
        let pack = Pack {
            manifest: crate::assets::Manifest {
                schema_version: 1, id: "test".into(), canvas_size: 64, background: "#000000".into(),
                default_animation: Some("idle".into()), animations: BTreeMap::new(),
            }, clips, encoded_bytes: 0,
        };
        (config, pack)
    }

    #[test]
    fn idle_events_do_not_extend_a_wait_and_activity_resets_it() {
        let (config, pack) = fixture(&["alternate"]);
        let mut schedule = IdleSchedule::new(&config, &pack).unwrap();
        let now = Instant::now();
        schedule.update(true, now);
        let deadline = schedule.deadline().unwrap();
        assert!((Duration::from_secs(45)..=Duration::from_secs(60)).contains(&(deadline - now)));
        schedule.update(true, now + Duration::from_secs(20));
        assert_eq!(schedule.deadline(), Some(deadline));
        assert!(schedule.take_due(deadline - Duration::from_millis(1)).is_none());
        schedule.update(false, now + Duration::from_secs(30));
        assert!(schedule.deadline().is_none());
        schedule.update(true, now + Duration::from_secs(40));
        assert!(schedule.deadline().unwrap() >= now + Duration::from_secs(85));
    }

    #[test]
    fn random_selection_avoids_repeats_and_rearms_only_after_normal_idle() {
        let (config, pack) = fixture(&["one", "two", "three", "four"]);
        let mut schedule = IdleSchedule::new(&config, &pack).unwrap();
        schedule.random = fastrand::Rng::with_seed(7);
        let mut now = Instant::now();
        let mut previous = None;
        let mut seen = std::collections::BTreeSet::new();
        for _ in 0..100 {
            schedule.update(true, now);
            let due = schedule.deadline().unwrap();
            let chosen = schedule.take_due(due).unwrap();
            assert_ne!(previous.as_deref(), Some(chosen.name.as_str()));
            seen.insert(chosen.name.clone());
            previous = Some(chosen.name.clone());
            assert!(schedule.deadline().is_none());
            assert!(schedule.take_due(due).is_none());
            schedule.update(false, due); // alternate is playing or idle is not acknowledged yet
            assert!(schedule.deadline().is_none());
            now = due + Duration::from_millis(chosen.duration_ms());
        }
        assert_eq!(seen.len(), 4);
    }

    #[test]
    fn fixed_interval_and_single_alternate_remain_usable() {
        let (mut config, pack) = fixture(&["one"]);
        config.idle_alternates.min_interval_ms = 1000;
        config.idle_alternates.max_interval_ms = 1000;
        let mut schedule = IdleSchedule::new(&config, &pack).unwrap();
        let now = Instant::now();
        for _ in 0..2 {
            schedule.update(true, now);
            assert_eq!(schedule.deadline(), Some(now + Duration::from_secs(1)));
            assert_eq!(schedule.take_due(now + Duration::from_secs(1)).unwrap().name, "one");
        }
    }

    #[test]
    fn empty_configuration_is_disabled_and_invalid_references_fail_early() {
        let (mut config, mut pack) = fixture(&[]);
        let mut schedule = IdleSchedule::new(&config, &pack).unwrap();
        schedule.update(true, Instant::now());
        assert!(schedule.deadline().is_none());
        config.idle_alternates.animations = vec!["missing".into()];
        assert!(IdleSchedule::new(&config, &pack).is_err());
        config.idle_alternates.animations = vec!["idle".into()];
        assert!(IdleSchedule::new(&config, &pack).is_err());
        config.idle_alternates.animations = vec!["alternate".into()];
        pack.clips.remove("idle");
        assert!(IdleSchedule::new(&config, &pack).is_err());
    }
}
