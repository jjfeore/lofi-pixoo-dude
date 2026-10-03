use crate::{assets::Pack, config::Config, shell_path};
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use std::{fs, path::Path};
use toml_edit::{Array, DocumentMut, Item, value};

pub fn hook_spec(config: &Config, pack: &Pack, exe: &Path) -> Result<Value> {
    for text in [exe.to_string_lossy(), config.pipe.as_str().into()] {
        ensure!(
            !text.contains(['"', '\n', '\r', '%']),
            "unsupported shell characters in hook path"
        );
    }
    let command = format!("\"{}\" emit --pipe \"{}\"", exe.display(), config.pipe);
    let mut hooks = serde_json::Map::new();
    let mut events = vec![
        "SessionStart",
        "SessionEnd",
        "UserPromptSubmit",
        "Interrupt",
    ];
    for (state, extra) in [
        ("compacting", ["PreCompact", "PostCompact"]),
        ("delegating", ["SubagentStart", "SubagentStop"]),
        ("needs-input", ["PermissionRequest", "PostToolUse"]),
    ] {
        if config
            .mapping(state)
            .is_some_and(|name| pack.clips.contains_key(name))
        {
            events.extend(extra);
        }
    }
    for event in events {
        let mut group = json!({"hooks":[{"type":"command","command":command,"timeout":1}]});
        if event == "PostToolUse" {
            group["matcher"] = json!("^(Bash|apply_patch|Edit|Write|mcp__.*)$");
        }
        hooks.insert(event.into(), json!([group]));
    }
    Ok(json!({"hooks":hooks}))
}

fn is_program_ours(program: &str, exe: &Path) -> bool {
    let name = program.rsplit(['\\', '/']).next().unwrap_or(program);
    program.eq_ignore_ascii_case(&exe.to_string_lossy())
        || name.eq_ignore_ascii_case("pixoo-pet.exe")
        || name == "pixoo-pet"
}

fn is_hook_ours(hook: &Value, exe: &Path) -> bool {
    hook.get("type").and_then(Value::as_str) == Some("command")
        && hook
            .get("command")
            .and_then(Value::as_str)
            .is_some_and(|command| {
                command
                    .strip_prefix('"')
                    .and_then(|s| s.split_once("\" emit --pipe "))
                    .is_some_and(|(program, _)| is_program_ours(program, exe))
            })
}

fn merge_hooks(mut old: Value, fragment: &Value, exe: &Path) -> Result<Value> {
    ensure!(old.is_object(), "existing hooks.json must be an object");
    let object = old.as_object_mut().unwrap();
    let hooks = object.entry("hooks").or_insert_with(|| json!({}));
    let hooks = hooks
        .as_object_mut()
        .context("hooks field must be an object")?;
    let additions = fragment["hooks"].as_object().unwrap();
    for (event, groups) in hooks.iter_mut() {
        let groups = groups
            .as_array_mut()
            .context("hook event groups must be arrays")?;
        let replacement = additions
            .get(event)
            .and_then(Value::as_array)
            .and_then(|g| g.first());
        let mut installed = false;
        let mut result = Vec::new();
        for mut group in std::mem::take(groups) {
            let commands = group
                .get_mut("hooks")
                .and_then(Value::as_array_mut)
                .context("each hook group must contain a hooks array")?;
            let has_ours = commands.iter().any(|h| is_hook_ours(h, exe));
            let only_ours = has_ours && commands.iter().all(|h| is_hook_ours(h, exe));
            if only_ours {
                if !installed && let Some(replacement) = replacement {
                    result.push(replacement.clone());
                    installed = true;
                }
            } else {
                commands.retain(|h| !is_hook_ours(h, exe));
                result.push(group);
            }
        }
        if !installed && let Some(replacement) = replacement {
            result.push(replacement.clone());
        }
        *groups = result;
    }
    for (event, groups) in additions {
        hooks.entry(event.clone()).or_insert_with(|| groups.clone());
    }
    Ok(old)
}

fn string_array(doc: &DocumentMut, key: &str) -> Result<Vec<String>> {
    let Some(item) = doc.get(key) else {
        return Ok(Vec::new());
    };
    let array = item
        .as_array()
        .with_context(|| format!("{key} must be an array"))?;
    array
        .iter()
        .map(|v| {
            v.as_str()
                .map(str::to_owned)
                .context("notify arguments must be strings")
        })
        .collect()
}

fn array_item(strings: &[String]) -> Item {
    let mut array = Array::new();
    for s in strings {
        array.push(s.as_str());
    }
    value(array)
}

fn merge_config(codex: &str, bridge: &str, exe: &Path, config: &Path) -> Result<(String, String)> {
    let mut codex: DocumentMut = codex
        .trim_start_matches('\u{feff}')
        .parse()
        .context("parse existing Codex TOML")?;
    let mut bridge: DocumentMut = bridge
        .trim_start_matches('\u{feff}')
        .parse()
        .context("parse bridge TOML")?;
    let old_notify = string_array(&codex, "notify")?;
    let forwarded = string_array(&bridge, "notification_forward")?;
    let is_adapter = old_notify.len() == 4
        && is_program_ours(&old_notify[0], exe)
        && old_notify[1] == "notify"
        && old_notify[2] == "--config";
    ensure!(
        !forwarded.first().is_some_and(|p| is_program_ours(p, exe)),
        "notification_forward must not point to the Pixoo adapter itself"
    );
    if !old_notify.is_empty() && !is_adapter {
        ensure!(
            forwarded.is_empty() || forwarded == old_notify,
            "existing notify and notification_forward differ; review the bridge forwarding field before preparing setup"
        );
        bridge["notification_forward"] = array_item(&old_notify);
    }
    codex["notify"] = array_item(&[
        exe.to_string_lossy().into_owned(),
        "notify".into(),
        "--config".into(),
        config.to_string_lossy().into_owned(),
    ]);
    if let Some(features) = codex.get("features") {
        ensure!(
            features.is_table() || features.is_inline_table(),
            "features must be a TOML table"
        );
    }
    codex["features"]["hooks"] = value(true);
    Ok((codex.to_string(), bridge.to_string()))
}

/// Stage snapshots and proposals. Destination files are never changed here.
pub fn prepare(config_path: &Path, output: &Path, codex_home: Option<&Path>) -> Result<()> {
    ensure!(
        !output.exists(),
        "setup output exists; choose a new review directory"
    );
    let config_path = shell_path(config_path.canonicalize()?);
    let config = Config::load(&config_path)?;
    let pack = Pack::load(&config.pack, config.device.max_frames)?;
    let exe = shell_path(std::env::current_exe()?.canonicalize()?);
    let home = match codex_home {
        Some(path) => path.to_path_buf(),
        None => match std::env::var_os("CODEX_HOME") {
            Some(path) => path.into(),
            None => std::env::var_os("USERPROFILE")
                .or_else(|| std::env::var_os("HOME"))
                .context("set CODEX_HOME or pass --codex-home")?
                .into(),
        },
    };
    let home = if codex_home.is_none() && std::env::var_os("CODEX_HOME").is_none() {
        home.join(".codex")
    } else {
        home
    };
    let home = if home.is_absolute() {
        home
    } else {
        std::env::current_dir()?.join(home)
    };
    let destinations = [
        home.join("config.toml"),
        home.join("hooks.json"),
        config_path.clone(),
    ];
    let mut originals = Vec::new();
    for path in &destinations {
        originals.push(match fs::read_to_string(path) {
            Ok(text) => Some(text),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => return Err(error).with_context(|| format!("read {}", path.display())),
        });
    }
    let hooks = match originals[1].as_deref() {
        Some(text) => serde_json::from_str(text.trim_start_matches('\u{feff}'))
            .context("parse existing hooks.json")?,
        None => json!({}),
    };
    let hooks = merge_hooks(hooks, &hook_spec(&config, &pack, &exe)?, &exe)?;
    let (codex, bridge) = merge_config(
        originals[0].as_deref().unwrap_or(""),
        originals[2]
            .as_deref()
            .context("bridge config disappeared")?,
        &exe,
        &config_path,
    )?;
    // The output location differs from the destination: validate the proposed
    // bridge structure without interpreting its relative pack in the staging dir.
    let _: Config = toml::from_str(&bridge).context("validate proposed bridge config")?;
    let _: toml::Value = toml::from_str(&codex).context("validate proposed Codex config")?;
    let proposals = [codex, serde_json::to_string_pretty(&hooks)?, bridge];
    fs::create_dir_all(output)?;
    let mut files = Vec::new();
    for (i, source) in ["config.toml", "hooks.json", "bridge.toml"]
        .into_iter()
        .enumerate()
    {
        fs::write(output.join(source), &proposals[i])?;
        let original = format!("original-{source}");
        if let Some(text) = &originals[i] {
            fs::write(output.join(&original), text)?;
        }
        files.push(json!({"source":source,"destination":destinations[i],
            "original":original,"existed":originals[i].is_some()}));
    }
    fs::write(
        output.join("metadata.json"),
        serde_json::to_vec_pretty(&json!({"version":1,"files":files}))?,
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_preserves_notifier_comments_and_other_settings() {
        let exe = Path::new(r"C:\pixoo\pixoo-pet.exe");
        let (codex, bridge) = merge_config(
            "# keep me\nnotify = ['helper.exe', 'done']\nmodel = 'x'\n[features]\nother = true\n",
            "# bridge note\npack = 'pets'\n[device]\naddress = '192.0.2.1'\n",
            exe,
            Path::new("bridge.toml"),
        )
        .unwrap();
        assert!(codex.contains("# keep me") && codex.contains("model = 'x'"));
        assert!(bridge.contains("# bridge note"));
        assert!(codex.contains("other = true"));
        let doc: DocumentMut = bridge.parse().unwrap();
        assert_eq!(
            string_array(&doc, "notification_forward").unwrap(),
            ["helper.exe", "done"]
        );
        let (second, bridge_again) =
            merge_config(&codex, &bridge, exe, Path::new("bridge.toml")).unwrap();
        assert_eq!(codex, second);
        assert_eq!(bridge, bridge_again);
    }

    #[test]
    fn hook_updates_are_idempotent_and_preserve_unrelated_groups() {
        let exe = Path::new("pixoo-pet.exe");
        let other = json!({"hooks":[{"type":"command","command":"security-check"}]});
        let old = json!({"unrelated":true,"hooks":{"UserPromptSubmit":[other.clone(),
            {"hooks":[{"type":"command","command":"\"C:\\old\\pixoo-pet.exe\" emit --pipe \"old\""}]}]}});
        let replacement =
            json!({"hooks":[{"type":"command","command":"\"pixoo-pet.exe\" emit --pipe \"new\""}]});
        let fragment = json!({"hooks":{"UserPromptSubmit":[replacement.clone()]}});
        let merged = merge_hooks(old, &fragment, exe).unwrap();
        assert_eq!(
            merged["hooks"]["UserPromptSubmit"],
            json!([other, replacement])
        );
        assert_eq!(merged["unrelated"], true);
        assert_eq!(merge_hooks(merged.clone(), &fragment, exe).unwrap(), merged);
    }
}
