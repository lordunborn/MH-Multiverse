use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::tuning::live_tuning_dir;

// ── Public types ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventDefinition {
    /// Map key from Events.json
    pub id: String,
    pub display_name: String,
    /// Path relative to the LiveTuning/ directory root
    pub file_path: String,
    pub daily_gift: Option<String>,
    pub instanced_missions: Option<Vec<String>>,
    pub is_hidden: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduleRule {
    pub name: String,
    pub is_enabled: bool,
    /// "AlwaysOn" | "WeeklyRotation" | "DayOfWeek" | "SpecialDate" | "SpecialDateLunar"
    pub rule_type: String,
    /// For WeeklyRotation and DayOfWeek
    pub start_day_of_week: Option<String>,
    /// For SpecialDate and SpecialDateLunar
    pub start_month: Option<u32>,
    pub start_day: Option<u32>,
    pub duration_days: Option<u32>,
    pub events: Vec<String>,
}

/// Return type for load_events — carries which file was read.
#[derive(Debug, Serialize)]
pub struct EventsData {
    pub definitions: Vec<EventDefinition>,
    pub using_override: bool,
}

/// Return type for load_event_schedule — carries which file was read.
#[derive(Debug, Serialize)]
pub struct ScheduleData {
    pub rules: Vec<ScheduleRule>,
    pub using_override: bool,
}

// ── Raw serde types ───────────────────────────────────────────────────────────

/// Matches one value in the Events.json object (the key is the event ID).
#[derive(Debug, Deserialize)]
struct RawEventDefIn {
    #[serde(rename = "DisplayName")]
    display_name: String,
    #[serde(rename = "FilePath")]
    file_path: String,
    #[serde(rename = "DailyGift")]
    daily_gift: Option<String>,
    #[serde(rename = "InstancedMissions")]
    instanced_missions: Option<Vec<String>>,
    #[serde(rename = "IsHidden")]
    is_hidden: Option<bool>,
}

/// The fields of an Events.json entry the editor changes. Every other field of an entry (Leaderboards, Waypoints and
/// anything added later) is kept exactly as the file has it: the server reads EventsOverride.json instead of
/// Events.json when it exists, so a field left out of the override is lost to the server.
const EDITED_FIELDS: [&str; 5] = ["DisplayName", "FilePath", "DailyGift", "InstancedMissions", "IsHidden"];

/// Matches one entry in the EventSchedule.json array.
#[derive(Debug, Deserialize, Serialize)]
struct RawScheduleRule {
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "IsEnabled")]
    is_enabled: bool,
    #[serde(rename = "Type")]
    rule_type: String,
    #[serde(rename = "StartDayOfWeek", skip_serializing_if = "Option::is_none")]
    start_day_of_week: Option<String>,
    #[serde(rename = "StartMonth", skip_serializing_if = "Option::is_none")]
    start_month: Option<u32>,
    #[serde(rename = "StartDay", skip_serializing_if = "Option::is_none")]
    start_day: Option<u32>,
    #[serde(rename = "DurationDays", skip_serializing_if = "Option::is_none")]
    duration_days: Option<u32>,
    #[serde(rename = "Events")]
    events: Vec<String>,
}

// ── Path helpers ──────────────────────────────────────────────────────────────

fn events_file_paths(
    dir: &Path,
) -> (std::path::PathBuf, std::path::PathBuf) {
    (dir.join("EventsOverride.json"), dir.join("Events.json"))
}

fn schedule_file_paths(
    dir: &Path,
) -> (std::path::PathBuf, std::path::PathBuf) {
    (
        dir.join("EventScheduleOverride.json"),
        dir.join("EventSchedule.json"),
    )
}

// ── I/O helpers ───────────────────────────────────────────────────────────────

fn read_events_from_path(path: &Path) -> Result<Vec<EventDefinition>, String> {
    let contents = std::fs::read_to_string(path)
        .map_err(|e| format!("Cannot read {}: {e}", path.display()))?;

    let raw: HashMap<String, RawEventDefIn> = serde_json::from_str(&contents)
        .map_err(|e| format!("JSON parse error in {}: {e}", path.display()))?;

    Ok(raw
        .into_iter()
        .map(|(id, def)| EventDefinition {
            id,
            display_name: def.display_name,
            file_path: def.file_path,
            daily_gift: def.daily_gift,
            instanced_missions: def.instanced_missions,
            is_hidden: def.is_hidden,
        })
        .collect())
}

fn read_schedule_from_path(path: &Path) -> Result<Vec<ScheduleRule>, String> {
    let contents = std::fs::read_to_string(path)
        .map_err(|e| format!("Cannot read {}: {e}", path.display()))?;

    let raw: Vec<RawScheduleRule> = serde_json::from_str(&contents)
        .map_err(|e| format!("JSON parse error in {}: {e}", path.display()))?;

    Ok(raw
        .into_iter()
        .map(|r| ScheduleRule {
            name: r.name,
            is_enabled: r.is_enabled,
            rule_type: r.rule_type,
            start_day_of_week: r.start_day_of_week,
            start_month: r.start_month,
            start_day: r.start_day,
            duration_days: r.duration_days,
            events: r.events,
        })
        .collect())
}

/// Reads Events.json or EventsOverride.json as raw JSON, keeping every field of every entry.
fn read_raw_events(path: &Path) -> Result<Map<String, Value>, String> {
    let contents = std::fs::read_to_string(path)
        .map_err(|e| format!("Cannot read {}: {e}", path.display()))?;

    serde_json::from_str::<Map<String, Value>>(contents.trim_start_matches('\u{feff}'))
        .map_err(|e| format!("JSON parse error in {}: {e}", path.display()))
}

fn set_or_remove(entry: &mut Map<String, Value>, key: &str, value: Option<Value>) {
    match value {
        Some(v) => {
            entry.insert(key.to_string(), v);
        }
        None => {
            entry.remove(key);
        }
    }
}

/// Builds the entries to write: for each definition, the entry already in the file (or the default Events.json entry
/// when the file has none) with only the edited fields updated. Fields the editor does not edit are carried over
/// untouched, and any such field the existing file lacks but the default has (an override written by an older version
/// of this app dropped Leaderboards and Waypoints) is restored from the default.
fn build_events_map(
    definitions: &[EventDefinition],
    existing: &Map<String, Value>,
    defaults: &Map<String, Value>,
) -> Map<String, Value> {
    let mut out = Map::new();

    for def in definitions {
        let mut entry: Map<String, Value> = existing
            .get(&def.id)
            .and_then(|v| v.as_object())
            .cloned()
            .unwrap_or_default();

        if let Some(default_entry) = defaults.get(&def.id).and_then(|v| v.as_object()) {
            for (key, value) in default_entry {
                if !EDITED_FIELDS.contains(&key.as_str()) && !entry.contains_key(key) {
                    entry.insert(key.clone(), value.clone());
                }
            }
        }

        entry.insert("DisplayName".to_string(), Value::String(def.display_name.clone()));
        entry.insert("FilePath".to_string(), Value::String(def.file_path.clone()));
        set_or_remove(&mut entry, "DailyGift", def.daily_gift.clone().map(Value::String));
        set_or_remove(
            &mut entry,
            "InstancedMissions",
            def.instanced_missions
                .clone()
                .map(|list| Value::Array(list.into_iter().map(Value::String).collect())),
        );
        set_or_remove(&mut entry, "IsHidden", def.is_hidden.map(Value::Bool));

        out.insert(def.id.clone(), Value::Object(entry));
    }

    out
}

fn write_events_to_path(
    path: &Path,
    default_path: &Path,
    definitions: &[EventDefinition],
) -> Result<(), String> {
    let existing = if path.exists() { read_raw_events(path)? } else { Map::new() };
    let defaults = if default_path.exists() { read_raw_events(default_path)? } else { Map::new() };

    let map = build_events_map(definitions, &existing, &defaults);

    let contents = serde_json::to_string_pretty(&map)
        .map_err(|e| format!("JSON serialisation error: {e}"))?;

    std::fs::write(path, contents)
        .map_err(|e| format!("Cannot write {}: {e}", path.display()))?;

    Ok(())
}

fn write_schedule_to_path(path: &Path, rules: &[ScheduleRule]) -> Result<(), String> {
    let raw: Vec<RawScheduleRule> = rules
        .iter()
        .map(|r| RawScheduleRule {
            name: r.name.clone(),
            is_enabled: r.is_enabled,
            rule_type: r.rule_type.clone(),
            start_day_of_week: r.start_day_of_week.clone(),
            start_month: r.start_month,
            start_day: r.start_day,
            duration_days: r.duration_days,
            events: r.events.clone(),
        })
        .collect();

    let contents = serde_json::to_string_pretty(&raw)
        .map_err(|e| format!("JSON serialisation error: {e}"))?;

    std::fs::write(path, contents)
        .map_err(|e| format!("Cannot write {}: {e}", path.display()))?;

    Ok(())
}

// ── Tauri commands ────────────────────────────────────────────────────────────

#[tauri::command]
pub fn load_events(server_exe: String) -> Result<EventsData, String> {
    let dir = live_tuning_dir(&server_exe)?;
    let (override_path, default_path) = events_file_paths(&dir);

    if override_path.exists() {
        let definitions = read_events_from_path(&override_path)?;
        Ok(EventsData {
            definitions,
            using_override: true,
        })
    } else if default_path.exists() {
        let definitions = read_events_from_path(&default_path)?;
        Ok(EventsData {
            definitions,
            using_override: false,
        })
    } else {
        Ok(EventsData {
            definitions: vec![],
            using_override: false,
        })
    }
}

#[tauri::command]
pub fn load_event_schedule(server_exe: String) -> Result<ScheduleData, String> {
    let dir = live_tuning_dir(&server_exe)?;
    let (override_path, default_path) = schedule_file_paths(&dir);

    if override_path.exists() {
        let rules = read_schedule_from_path(&override_path)?;
        Ok(ScheduleData {
            rules,
            using_override: true,
        })
    } else if default_path.exists() {
        let rules = read_schedule_from_path(&default_path)?;
        Ok(ScheduleData {
            rules,
            using_override: false,
        })
    } else {
        Ok(ScheduleData {
            rules: vec![],
            using_override: false,
        })
    }
}

#[tauri::command]
pub fn save_events_override(
    server_exe: String,
    definitions: Vec<EventDefinition>,
) -> Result<(), String> {
    let dir = live_tuning_dir(&server_exe)?;
    let (override_path, default_path) = events_file_paths(&dir);
    write_events_to_path(&override_path, &default_path, &definitions)
}

#[tauri::command]
pub fn save_schedule_override(
    server_exe: String,
    rules: Vec<ScheduleRule>,
) -> Result<(), String> {
    let dir = live_tuning_dir(&server_exe)?;
    write_schedule_to_path(&dir.join("EventScheduleOverride.json"), &rules)
}

#[tauri::command]
pub fn reset_events_override(server_exe: String) -> Result<EventsData, String> {
    let dir = live_tuning_dir(&server_exe)?;
    let (override_path, default_path) = events_file_paths(&dir);

    if !default_path.exists() {
        return Err("Events.json not found — cannot reset override".to_string());
    }

    std::fs::copy(&default_path, &override_path)
        .map_err(|e| format!("Cannot copy Events.json to override: {e}"))?;

    let definitions = read_events_from_path(&override_path)?;
    Ok(EventsData {
        definitions,
        using_override: true,
    })
}

#[tauri::command]
pub fn reset_schedule_override(server_exe: String) -> Result<ScheduleData, String> {
    let dir = live_tuning_dir(&server_exe)?;
    let (override_path, default_path) = schedule_file_paths(&dir);

    if !default_path.exists() {
        return Err("EventSchedule.json not found — cannot reset override".to_string());
    }

    std::fs::copy(&default_path, &override_path)
        .map_err(|e| format!("Cannot copy EventSchedule.json to override: {e}"))?;

    let rules = read_schedule_from_path(&override_path)?;
    Ok(ScheduleData {
        rules,
        using_override: true,
    })
}

#[tauri::command]
pub fn merge_events_override(server_exe: String) -> Result<EventsData, String> {
    let dir = live_tuning_dir(&server_exe)?;
    let (override_path, default_path) = events_file_paths(&dir);

    if !default_path.exists() {
        return Err("Events.json not found — cannot merge".to_string());
    }

    let defaults = read_events_from_path(&default_path)?;

    let mut current = if override_path.exists() {
        read_events_from_path(&override_path)?
    } else {
        vec![]
    };

    let existing_ids: HashSet<String> = current.iter().map(|d| d.id.clone()).collect();

    for def in defaults {
        if !existing_ids.contains(&def.id) {
            current.push(def);
        }
    }

    write_events_to_path(&override_path, &default_path, &current)?;
    Ok(EventsData {
        definitions: current,
        using_override: true,
    })
}

#[tauri::command]
pub fn merge_schedule_override(server_exe: String) -> Result<ScheduleData, String> {
    let dir = live_tuning_dir(&server_exe)?;
    let (override_path, default_path) = schedule_file_paths(&dir);

    if !default_path.exists() {
        return Err("EventSchedule.json not found — cannot merge".to_string());
    }

    let defaults = read_schedule_from_path(&default_path)?;

    let mut current = if override_path.exists() {
        read_schedule_from_path(&override_path)?
    } else {
        vec![]
    };

    let existing_names: HashSet<String> = current.iter().map(|r| r.name.clone()).collect();

    for rule in defaults {
        if !existing_names.contains(&rule.name) {
            current.push(rule);
        }
    }

    write_schedule_to_path(&override_path, &current)?;
    Ok(ScheduleData {
        rules: current,
        using_override: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn def(id: &str, name: &str) -> EventDefinition {
        EventDefinition {
            id: id.to_string(),
            display_name: name.to_string(),
            file_path: format!("Events/{id}.json"),
            daily_gift: None,
            instanced_missions: None,
            is_hidden: None,
        }
    }

    fn obj(v: Value) -> Map<String, Value> {
        v.as_object().cloned().unwrap()
    }

    #[test]
    fn saving_keeps_fields_the_editor_does_not_know() {
        let existing = obj(json!({
            "CivilWar": {
                "DisplayName": "Civil War",
                "FilePath": "Events/CivilWar.json",
                "Leaderboards": ["a.prototype", "b.prototype"],
                "Waypoints": ["w.prototype"],
                "SomethingNew": {"x": 1}
            }
        }));

        let out = build_events_map(&[def("CivilWar", "Civil War Renamed")], &existing, &Map::new());
        let entry = &out["CivilWar"];

        assert_eq!(entry["DisplayName"], "Civil War Renamed");
        assert_eq!(entry["Leaderboards"], json!(["a.prototype", "b.prototype"]));
        assert_eq!(entry["Waypoints"], json!(["w.prototype"]));
        assert_eq!(entry["SomethingNew"], json!({"x": 1}));
    }

    #[test]
    fn saving_restores_fields_an_older_override_dropped() {
        let existing = obj(json!({
            "WealthOfKingsCounty": {"DisplayName": "Wealth", "FilePath": "Events/W.json"}
        }));
        let defaults = obj(json!({
            "WealthOfKingsCounty": {
                "DisplayName": "Wealth",
                "FilePath": "Events/W.json",
                "Leaderboards": ["ic.prototype", "icc.prototype"]
            }
        }));

        let out = build_events_map(&[def("WealthOfKingsCounty", "Wealth")], &existing, &defaults);

        assert_eq!(out["WealthOfKingsCounty"]["Leaderboards"], json!(["ic.prototype", "icc.prototype"]));
    }

    #[test]
    fn edited_fields_win_and_can_be_cleared() {
        let existing = obj(json!({
            "A": {"DisplayName": "A", "FilePath": "Events/A.json", "DailyGift": "gift", "IsHidden": true}
        }));
        let defaults = obj(json!({
            "A": {"DisplayName": "A", "FilePath": "Events/A.json", "DailyGift": "gift", "IsHidden": true}
        }));

        // The editor cleared the gift and the hidden flag: they must not come back from the default.
        let out = build_events_map(&[def("A", "A")], &existing, &defaults);
        assert!(out["A"].get("DailyGift").is_none());
        assert!(out["A"].get("IsHidden").is_none());

        let mut with_values = def("A", "A");
        with_values.daily_gift = Some("new gift".to_string());
        with_values.instanced_missions = Some(vec!["m1".to_string()]);
        with_values.is_hidden = Some(false);
        let out = build_events_map(&[with_values], &existing, &defaults);
        assert_eq!(out["A"]["DailyGift"], "new gift");
        assert_eq!(out["A"]["InstancedMissions"], json!(["m1"]));
        assert_eq!(out["A"]["IsHidden"], false);
    }

    #[test]
    fn new_and_deleted_events_follow_the_editor() {
        let existing = obj(json!({
            "Old": {"DisplayName": "Old", "FilePath": "Events/Old.json", "Leaderboards": ["x"]}
        }));

        let out = build_events_map(&[def("Brand", "Brand New")], &existing, &Map::new());

        assert!(out.get("Old").is_none());
        assert_eq!(out["Brand"]["DisplayName"], "Brand New");
        assert!(out["Brand"].get("Leaderboards").is_none());
    }

    /// Reads a real Events.json (set MHM_TEST_LIVETUNING_DIR to a server's Data/Game/LiveTuning folder), saves it through the
    /// editor path with no edits and checks the result equals the original. Run with: cargo test real_events -- --ignored
    #[test]
    #[ignore]
    fn real_events_json_survives_a_save_unchanged() {
        let dir = std::path::PathBuf::from(std::env::var("MHM_TEST_LIVETUNING_DIR").expect("set MHM_TEST_LIVETUNING_DIR"));
        let default_path = dir.join("Events.json");
        let out_path = std::env::temp_dir().join(format!("mhm-real-events-{}.json", std::process::id()));

        let definitions = read_events_from_path(&default_path).unwrap();
        write_events_to_path(&out_path, &default_path, &definitions).unwrap();

        let original = read_raw_events(&default_path).unwrap();
        let written = read_raw_events(&out_path).unwrap();
        let _ = std::fs::remove_file(&out_path);

        assert_eq!(Value::Object(original), Value::Object(written));
    }

    #[test]
    fn writing_a_file_round_trips_leaderboards() {
        let dir = std::env::temp_dir().join(format!("mhm-events-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let default_path = dir.join("Events.json");
        let override_path = dir.join("EventsOverride.json");

        std::fs::write(
            &default_path,
            r#"{ "Midtown": { "DisplayName": "Midtown", "FilePath": "Events/M.json", "Leaderboards": ["m1", "m2"], "Waypoints": ["w"] } }"#,
        )
        .unwrap();

        // No override yet: the first save must still carry the default's extra fields.
        write_events_to_path(&override_path, &default_path, &[def("Midtown", "Midtown Madness")]).unwrap();
        let written = read_raw_events(&override_path).unwrap();
        assert_eq!(written["Midtown"]["Leaderboards"], json!(["m1", "m2"]));
        assert_eq!(written["Midtown"]["Waypoints"], json!(["w"]));
        assert_eq!(written["Midtown"]["DisplayName"], "Midtown Madness");

        // And a second save keeps them.
        write_events_to_path(&override_path, &default_path, &[def("Midtown", "Midtown Madness 2")]).unwrap();
        let written = read_raw_events(&override_path).unwrap();
        assert_eq!(written["Midtown"]["Leaderboards"], json!(["m1", "m2"]));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
