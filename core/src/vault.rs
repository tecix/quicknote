use std::collections::HashMap;
use std::fs;
use std::path::{Component, Path, PathBuf};

use chrono::{DateTime, Local};
use serde_json::Value;

use crate::edit::{apply_capture, InsertAfter};
use crate::format::format_text;

#[derive(Debug, Clone)]
pub struct Vault {
    pub name: String,
    pub path: PathBuf,
    pub open: bool,
}

/// Vaults known to the local Obsidian installation (deb/AppImage or Flatpak config).
pub fn detect_vaults() -> Vec<Vault> {
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
    let cfg = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".config"));
    let candidates = [
        cfg.join("obsidian/obsidian.json"),
        home.join(".var/app/md.obsidian.Obsidian/config/obsidian/obsidian.json"),
    ];
    for c in candidates {
        let Ok(text) = fs::read_to_string(&c) else { continue };
        let Ok(json) = serde_json::from_str::<Value>(&text) else { continue };
        let Some(map) = json.get("vaults").and_then(Value::as_object) else { continue };
        let mut out: Vec<Vault> = map
            .values()
            .filter_map(|v| {
                let p = PathBuf::from(v.get("path")?.as_str()?);
                Some(Vault {
                    name: p.file_name()?.to_string_lossy().into_owned(),
                    path: p,
                    open: v.get("open").and_then(Value::as_bool).unwrap_or(false),
                })
            })
            .collect();
        out.sort_by(|a, b| a.name.cmp(&b.name));
        return out;
    }
    Vec::new()
}

#[derive(Debug, Clone, Default)]
pub struct Choice {
    pub name: String,
    pub kind: String,
    pub capture_to: String,
    pub capture_to_active_file: bool,
    pub task: bool,
    pub prepend: bool,
    pub format_enabled: bool,
    pub format: String,
    pub create_enabled: bool,
    pub create_template: Option<String>,
    pub insert_after: InsertAfter,
}

impl Choice {
    fn from_value(v: &Value) -> Choice {
        let s = |x: &Value, k: &str| x.get(k).and_then(Value::as_str).unwrap_or("").to_string();
        let b = |x: &Value, k: &str| x.get(k).and_then(Value::as_bool).unwrap_or(false);
        let fmt = v.get("format").cloned().unwrap_or(Value::Null);
        let cr = v.get("createFileIfItDoesntExist").cloned().unwrap_or(Value::Null);
        let ia = v.get("insertAfter").cloned().unwrap_or(Value::Null);
        Choice {
            name: s(v, "name"),
            kind: s(v, "type"),
            capture_to: s(v, "captureTo"),
            capture_to_active_file: b(v, "captureToActiveFile"),
            task: b(v, "task"),
            prepend: b(v, "prepend"),
            format_enabled: b(&fmt, "enabled"),
            format: s(&fmt, "format"),
            create_enabled: b(&cr, "enabled"),
            create_template: (b(&cr, "createWithTemplate") && !s(&cr, "template").is_empty())
                .then(|| s(&cr, "template")),
            insert_after: InsertAfter {
                enabled: b(&ia, "enabled"),
                after: s(&ia, "after"),
                insert_at_end: b(&ia, "insertAtEnd"),
                consider_subsections: b(&ia, "considerSubsections"),
                create_if_not_found: b(&ia, "createIfNotFound"),
                create_at_top: ia.get("createIfNotFoundLocation").and_then(Value::as_str)
                    == Some("top"),
            },
        }
    }
}

fn flatten(list: &Value, out: &mut Vec<Value>) {
    for c in list.as_array().into_iter().flatten() {
        if c.get("type").and_then(Value::as_str) == Some("Multi") {
            flatten(c.get("choices").unwrap_or(&Value::Null), out);
        } else {
            out.push(c.clone());
        }
    }
}

fn load(vault: &Path) -> Result<(Vec<Value>, HashMap<String, String>), String> {
    let p = vault.join(".obsidian/plugins/quickadd/data.json");
    let text = fs::read_to_string(&p).map_err(|_| "QuickAdd data.json not found in the vault".to_string())?;
    let json: Value = serde_json::from_str(&text).map_err(|e| format!("Invalid QuickAdd data: {e}"))?;
    let mut choices = Vec::new();
    flatten(json.get("choices").unwrap_or(&Value::Null), &mut choices);
    let globals = json
        .get("globalVariables")
        .and_then(Value::as_object)
        .map(|m| {
            m.iter()
                .filter_map(|(k, v)| Some((k.clone(), v.as_str()?.to_string())))
                .collect()
        })
        .unwrap_or_default();
    Ok((choices, globals))
}

/// The vault's Capture choices plus QuickAdd global variables, parsed once so the
/// UI can preview targets on every keystroke without touching the disk.
#[derive(Debug, Clone, Default)]
pub struct CaptureSet {
    pub choices: Vec<Choice>,
    pub globals: HashMap<String, String>,
}

impl CaptureSet {
    pub fn load(vault: &Path) -> Result<CaptureSet, String> {
        let (raw, globals) = load(vault)?;
        let choices = raw.iter().map(Choice::from_value).filter(|c| c.kind == "Capture").collect();
        Ok(CaptureSet { choices, globals })
    }

    /// Relative note path the choice at `index` would write to for this text.
    pub fn preview(&self, index: usize, text: &str, now: &DateTime<Local>) -> Result<String, String> {
        let choice = self.choices.get(index).ok_or("No Capture choice selected")?;
        resolve_target(choice, text, now, &self.globals)
    }
}

pub fn list_capture_choices(vault: &Path) -> Vec<Choice> {
    CaptureSet::load(vault).map(|s| s.choices).unwrap_or_default()
}

/// Relative note path a capture would write to.
pub fn resolve_target(
    choice: &Choice,
    text: &str,
    now: &DateTime<Local>,
    globals: &HashMap<String, String>,
) -> Result<String, String> {
    if choice.capture_to_active_file {
        return Err("\"Capture to active file\" needs Obsidian; pick a choice with a file path".into());
    }
    let safe: String = text
        .chars()
        .map(|c| if "\\/:*?\"<>|\n".contains(c) { ' ' } else { c })
        .collect();
    let mut rel = format_text(choice.capture_to.trim(), safe.trim(), now, globals)
        .trim_start_matches('/')
        .to_string();
    if rel.is_empty() || rel.ends_with('/') {
        return Err("Folder captures are not supported; the choice needs a file path".into());
    }
    if Path::new(&rel).components().any(|c| matches!(c, Component::ParentDir)) {
        return Err("Capture path must stay inside the vault".into());
    }
    let lower = rel.to_lowercase();
    if lower.ends_with(".canvas") {
        return Err("Canvas captures are not supported".into());
    }
    if !lower.ends_with(".md") {
        rel.push_str(".md");
    }
    Ok(rel)
}

fn find_choice(vault: &Path, name: &str) -> Result<(Choice, HashMap<String, String>), String> {
    let (raw, globals) = load(vault)?;
    let wanted = name.trim().to_lowercase();
    let choice = raw
        .iter()
        .map(Choice::from_value)
        .find(|c| c.name.to_lowercase() == wanted)
        .ok_or_else(|| format!("QuickAdd choice \"{name}\" not found"))?;
    if choice.kind != "Capture" {
        return Err(format!("\"{}\" is a {} choice; only Capture is supported", choice.name, choice.kind));
    }
    Ok((choice, globals))
}

/// Runs a Capture choice against the vault files. Returns the relative path written.
pub fn run_capture(
    vault: &Path,
    choice_name: &str,
    text: &str,
    now: &DateTime<Local>,
) -> Result<String, String> {
    let (choice, globals) = find_choice(vault, choice_name)?;
    let rel = resolve_target(&choice, text, now, &globals)?;
    let full = vault.join(&rel);

    let mut content = match fs::read_to_string(&full) {
        Ok(c) => c,
        Err(_) if choice.create_enabled => {
            let base = match &choice.create_template {
                Some(t) => {
                    let t = t.trim_end_matches(".md");
                    let tpl = fs::read_to_string(vault.join(format!("{t}.md")))
                        .map_err(|_| format!("Template \"{t}\" not found"))?;
                    format_text(&tpl, text, now, &globals)
                }
                None => String::new(),
            };
            if let Some(dir) = full.parent() {
                fs::create_dir_all(dir).map_err(|e| e.to_string())?;
            }
            base
        }
        Err(_) => return Err(format!("File \"{rel}\" does not exist")),
    };

    let mut out = if choice.format_enabled {
        format_text(&choice.format, text, now, &globals)
    } else {
        text.to_string()
    };
    if choice.task {
        out = format!("- [ ] {out}");
    }
    content = apply_capture(&content, &out, choice.prepend, &choice.insert_after)?;

    // Atomic write: temp file in the same directory, then rename.
    let tmp = full.with_extension("md.quicknote-tmp");
    fs::write(&tmp, content).map_err(|e| e.to_string())?;
    fs::rename(&tmp, &full).map_err(|e| e.to_string())?;
    Ok(rel)
}
