use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use crate::calligraphy::{ensure_catalogue_loaded, CatalogueState};

// ── Embedded display names ────────────────────────────────────────────────────

static DISPLAY_NAMES_JSON: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/display_names.json"));

// ── Display name state ────────────────────────────────────────────────────────

/// Holds the embedded prototype path → display name map plus an optional
/// per-server-directory override map loaded from `display_names_custom.json`.
///
/// The embedded map is immutable after construction. The custom map is loaded
/// lazily on first use and refreshed if the server directory changes.
pub struct DisplayNameState {
    embedded: HashMap<String, String>,
    custom: Mutex<Option<(String, HashMap<String, String>)>>,
}

impl DisplayNameState {
    pub fn new() -> Self {
        let embedded: HashMap<String, String> =
            serde_json::from_str(DISPLAY_NAMES_JSON).unwrap_or_default();
        Self {
            embedded,
            custom: Mutex::new(None),
        }
    }

    /// Resolve a prototype path to a display name.
    ///
    /// Resolution order:
    /// 1. `display_names_custom.json` in `server_dir` (if present and non-empty)
    /// 2. Embedded `display_names.json`
    /// 3. The prototype path itself (verbatim fallback)
    pub(crate) fn lookup(&self, server_dir: &str, path: &str) -> String {
        if let Ok(mut guard) = self.custom.lock() {
            let needs_load = guard
                .as_ref()
                .map(|(cached_dir, _)| cached_dir != server_dir)
                .unwrap_or(true);

            if needs_load {
                let custom_path = Path::new(server_dir).join("display_names_custom.json");
                let map: HashMap<String, String> = fs::read_to_string(&custom_path)
                    .ok()
                    .and_then(|s| serde_json::from_str(&s).ok())
                    .unwrap_or_default();
                *guard = Some((server_dir.to_string(), map));
            }

            if let Some((_, ref custom_map)) = *guard {
                if let Some(name) = custom_map.get(path) {
                    if name != "N/A" && !name.is_empty() {
                        return name.clone();
                    }
                }
            }
        }

        self.embedded
            .get(path)
            .filter(|n| n.as_str() != "N/A" && !n.is_empty())
            .cloned()
            .unwrap_or_else(|| path.to_string())
    }
}

// ── Disk-only catalog types ───────────────────────────────────────────────────
//
// These types map directly to the on-disk JSON format. `ItemPrototypeRuntimeIdForClient`
// is a raw u64 which serialises as a JSON number. They are private to this module;
// the public "view" types below are used for all Tauri command boundaries.

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
struct GuidItemDisk {
    prototype_guid: i32,
    item_prototype_runtime_id_for_client: u64,
    quantity: i32,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
struct CatalogEntryDisk {
    sku_id: u64,
    guid_items: Vec<GuidItemDisk>,
    additional_guid_items: Vec<GuidItemDisk>,
    localized_entries: Vec<LocalizedEntry>,
    info_urls: Vec<UrlEntry>,
    content_data: Vec<UrlEntry>,
    #[serde(rename = "Type")]
    item_type: NamedItem,
    type_modifiers: Vec<NamedItem>,
}

// ── Public (frontend-facing) catalog types ────────────────────────────────────
//
// `ItemPrototypeRuntimeIdForClient` is represented as a String here to prevent
// JavaScript from silently losing precision on u64 values above Number.MAX_SAFE_INTEGER
// (9,007,199,254,740,991). Values in the catalog such as 16313827754993193555 would
// be corrupted if passed as a JSON number to the JS runtime.

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct GuidItem {
    pub prototype_guid: i32,
    /// Stored as a decimal string to preserve full u64 precision across the JS boundary.
    pub item_prototype_runtime_id_for_client: String,
    pub quantity: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LocalizedEntry {
    #[serde(default = "default_lang")]
    pub language_id: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub release_date: String,
    pub item_price: i32,
}

fn default_lang() -> String {
    "en_us".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct UrlEntry {
    #[serde(default = "default_lang")]
    pub language_id: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub image_data: String,
}

/// Shared shape for `Type` and each entry in `TypeModifiers`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct NamedItem {
    pub name: String,
    pub order: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct CatalogEntry {
    pub sku_id: u64,
    pub guid_items: Vec<GuidItem>,
    pub additional_guid_items: Vec<GuidItem>,
    pub localized_entries: Vec<LocalizedEntry>,
    pub info_urls: Vec<UrlEntry>,
    pub content_data: Vec<UrlEntry>,
    #[serde(rename = "Type")]
    pub item_type: NamedItem,
    pub type_modifiers: Vec<NamedItem>,
}

/// `CatalogEntry` with file provenance, returned by `load_catalog_entries`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogEntryWithMeta {
    #[serde(flatten)]
    pub entry: CatalogEntry,
    /// Base catalog filename (no directory), e.g. `"CatalogBundle.json"`.
    pub source_file: String,
    /// `true` when this entry's effective version came from the `*MODIFIED.json` sibling.
    pub from_modified: bool,
}

// ── Disk ↔ view conversions ───────────────────────────────────────────────────

fn disk_to_view(d: CatalogEntryDisk) -> CatalogEntry {
    CatalogEntry {
        sku_id: d.sku_id,
        guid_items: d.guid_items.into_iter().map(guid_disk_to_view).collect(),
        additional_guid_items: d
            .additional_guid_items
            .into_iter()
            .map(guid_disk_to_view)
            .collect(),
        localized_entries: d.localized_entries,
        info_urls: d.info_urls,
        content_data: d.content_data,
        item_type: d.item_type,
        type_modifiers: d.type_modifiers,
    }
}

fn guid_disk_to_view(g: GuidItemDisk) -> GuidItem {
    GuidItem {
        prototype_guid: g.prototype_guid,
        item_prototype_runtime_id_for_client: g.item_prototype_runtime_id_for_client.to_string(),
        quantity: g.quantity,
    }
}

fn view_to_disk(v: CatalogEntry) -> Result<CatalogEntryDisk, String> {
    let guid_items = v
        .guid_items
        .into_iter()
        .map(guid_view_to_disk)
        .collect::<Result<Vec<_>, _>>()?;
    let additional_guid_items = v
        .additional_guid_items
        .into_iter()
        .map(guid_view_to_disk)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(CatalogEntryDisk {
        sku_id: v.sku_id,
        guid_items,
        additional_guid_items,
        localized_entries: v.localized_entries,
        info_urls: v.info_urls,
        content_data: v.content_data,
        item_type: v.item_type,
        type_modifiers: v.type_modifiers,
    })
}

fn guid_view_to_disk(g: GuidItem) -> Result<GuidItemDisk, String> {
    let id = g
        .item_prototype_runtime_id_for_client
        .parse::<u64>()
        .map_err(|e| {
            format!(
                "Invalid prototype ID '{}': {e}",
                g.item_prototype_runtime_id_for_client
            )
        })?;
    Ok(GuidItemDisk {
        prototype_guid: g.prototype_guid,
        item_prototype_runtime_id_for_client: id,
        quantity: g.quantity,
    })
}

// ── Path helpers ──────────────────────────────────────────────────────────────

fn server_dir_of(server_exe: &str) -> Result<PathBuf, String> {
    Path::new(server_exe)
        .parent()
        .map(|p| p.to_path_buf())
        .ok_or_else(|| "Cannot determine server directory from exe path".to_string())
}

fn mtxstore_dir(server_exe: &str) -> Result<PathBuf, String> {
    Ok(server_dir_of(server_exe)?
        .join("Data")
        .join("Game")
        .join("MTXStore"))
}

/// Returns sorted paths of base `Catalog*.json` files, excluding `*MODIFIED*` variants.
fn base_catalog_files(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    for entry in
        fs::read_dir(dir).map_err(|e| format!("Cannot read MTXStore directory: {e}"))?
    {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with("Catalog")
            && name.ends_with(".json")
            && !name.to_ascii_uppercase().contains("MODIFIED")
        {
            files.push(entry.path());
        }
    }
    files.sort();
    Ok(files)
}

/// Derives the `*MODIFIED.json` path for a given base catalog path.
fn modified_path_for(base: &Path) -> PathBuf {
    let stem = base.file_stem().unwrap_or_default().to_string_lossy();
    base.with_file_name(format!("{stem}MODIFIED.json"))
}

/// Read and deserialise disk catalog entries from `path`, returning an empty
/// vec on any error (missing file, invalid JSON, wrong schema) rather than
/// propagating — callers decide whether absence is an error.
fn load_disk_entries(path: &Path) -> Vec<CatalogEntryDisk> {
    fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

// ── Direct catalog file editing ───────────────────────────────────────────────

/// Line-ending and BOM style of a catalog file, so a rewrite does not change them.
struct FileStyle {
    bom: bool,
    crlf: bool,
    trailing_newline: bool,
}

fn sku_of(entry: &Value) -> Option<u64> {
    entry.get("SkuId").and_then(Value::as_u64)
}

/// Read a catalog file as raw JSON values (nothing is dropped), plus its style.
fn read_catalog_file(path: &Path) -> Result<(Vec<Value>, FileStyle), String> {
    let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
    let bytes = fs::read(path).map_err(|e| format!("Cannot read {name}: {e}"))?;
    let bom = bytes.starts_with(&[0xEF, 0xBB, 0xBF]);
    let text = std::str::from_utf8(if bom { &bytes[3..] } else { &bytes })
        .map_err(|e| format!("{name} is not valid UTF-8: {e}"))?;
    let crlf = text.contains("\r\n");
    let trailing_newline = text.ends_with('\n');
    let value: Value =
        serde_json::from_str(text).map_err(|e| format!("{name} is not valid JSON: {e}"))?;
    match value {
        Value::Array(entries) => Ok((entries, FileStyle { bom, crlf, trailing_newline })),
        _ => Err(format!("{name} is not a JSON array of catalog entries")),
    }
}

/// Pretty formatter that escapes strings the way the server's .NET writer did, so a rewrite
/// leaves untouched text byte-for-byte the same: `"` `'` `&` `+` `<` `>` `` ` `` and every
/// non-ASCII character become `\uXXXX` (upper-case hex).
struct DotNetFormatter<'a>(serde_json::ser::PrettyFormatter<'a>);

impl serde_json::ser::Formatter for DotNetFormatter<'_> {
    fn begin_array<W: ?Sized + io::Write>(&mut self, w: &mut W) -> io::Result<()> {
        self.0.begin_array(w)
    }
    fn end_array<W: ?Sized + io::Write>(&mut self, w: &mut W) -> io::Result<()> {
        self.0.end_array(w)
    }
    fn begin_array_value<W: ?Sized + io::Write>(&mut self, w: &mut W, first: bool) -> io::Result<()> {
        self.0.begin_array_value(w, first)
    }
    fn end_array_value<W: ?Sized + io::Write>(&mut self, w: &mut W) -> io::Result<()> {
        self.0.end_array_value(w)
    }
    fn begin_object<W: ?Sized + io::Write>(&mut self, w: &mut W) -> io::Result<()> {
        self.0.begin_object(w)
    }
    fn end_object<W: ?Sized + io::Write>(&mut self, w: &mut W) -> io::Result<()> {
        self.0.end_object(w)
    }
    fn begin_object_key<W: ?Sized + io::Write>(&mut self, w: &mut W, first: bool) -> io::Result<()> {
        self.0.begin_object_key(w, first)
    }
    fn begin_object_value<W: ?Sized + io::Write>(&mut self, w: &mut W) -> io::Result<()> {
        self.0.begin_object_value(w)
    }
    fn end_object_value<W: ?Sized + io::Write>(&mut self, w: &mut W) -> io::Result<()> {
        self.0.end_object_value(w)
    }
    fn write_string_fragment<W: ?Sized + io::Write>(&mut self, w: &mut W, fragment: &str) -> io::Result<()> {
        for c in fragment.chars() {
            if c.is_ascii() && !matches!(c, '\'' | '&' | '+' | '<' | '>' | '`') {
                w.write_all(&[c as u8])?;
            } else {
                for unit in c.encode_utf16(&mut [0u16; 2]) {
                    write!(w, "\\u{:04X}", unit)?;
                }
            }
        }
        Ok(())
    }
    fn write_char_escape<W: ?Sized + io::Write>(&mut self, w: &mut W, esc: serde_json::ser::CharEscape) -> io::Result<()> {
        match esc {
            serde_json::ser::CharEscape::Quote => w.write_all(b"\\u0022"),
            other => self.0.write_char_escape(w, other),
        }
    }
}

/// Write the entries back with the original style (2-space indent, BOM and line endings kept).
fn write_catalog_file(path: &Path, entries: &[Value], style: &FileStyle) -> Result<(), String> {
    let mut buf: Vec<u8> = Vec::new();
    let formatter = DotNetFormatter(serde_json::ser::PrettyFormatter::with_indent(b"  "));
    let mut serializer = serde_json::Serializer::with_formatter(&mut buf, formatter);
    entries
        .serialize(&mut serializer)
        .map_err(|e| format!("Serialisation failed: {e}"))?;
    let mut text = String::from_utf8(buf).map_err(|e| format!("Serialisation failed: {e}"))?;
    if style.crlf {
        text = text.replace('\n', "\r\n");
    }
    if style.trailing_newline {
        text.push_str(if style.crlf { "\r\n" } else { "\n" });
    }
    let mut bytes = Vec::with_capacity(text.len() + 3);
    if style.bom {
        bytes.extend_from_slice(&[0xEF, 0xBB, 0xBF]);
    }
    bytes.extend_from_slice(text.as_bytes());
    fs::write(path, bytes).map_err(|e| format!("Write failed: {e}"))
}

/// Overwrite `old` with `new`, but keep anything `old` has that `new` does not mention:
/// object keys the editor does not know about, and the extra keys of array elements that
/// stay at the same position. Arrays take the length of `new`.
fn merge_preserving(old: &mut Value, new: &Value) {
    match (old, new) {
        (Value::Object(o), Value::Object(n)) => {
            for (key, value) in n {
                match o.get_mut(key) {
                    Some(existing) => merge_preserving(existing, value),
                    None => {
                        o.insert(key.clone(), value.clone());
                    }
                }
            }
        }
        (Value::Array(o), Value::Array(n)) => {
            o.truncate(n.len());
            for (i, value) in n.iter().enumerate() {
                match o.get_mut(i) {
                    Some(existing) => merge_preserving(existing, value),
                    None => o.push(value.clone()),
                }
            }
        }
        (old, new) => *old = new.clone(),
    }
}

/// Copy `path` to `<path>.bak` the first time it is written in this session, so the backup
/// is the file as it was before a whole editing session (a batch edit saves many times).
fn backup_once(path: &Path) -> Result<(), String> {
    static DONE: OnceLock<Mutex<HashSet<PathBuf>>> = OnceLock::new();
    let done = DONE.get_or_init(|| Mutex::new(HashSet::new()));
    let mut done = done.lock().map_err(|e| e.to_string())?;
    if done.contains(path) {
        return Ok(());
    }
    let mut bak = path.as_os_str().to_owned();
    bak.push(".bak");
    fs::copy(path, PathBuf::from(bak)).map_err(|e| format!("Backup failed: {e}"))?;
    done.insert(path.to_path_buf());
    Ok(())
}

// ── Display name resolution (internal) ───────────────────────────────────────

/// Resolve a prototype runtime ID (as a decimal string) to a display name,
/// using the catalogue and display name state. Falls back gracefully at each step.
fn name_for_id(
    dn: &DisplayNameState,
    cat: &CatalogueState,
    server_dir: &str,
    id_str: &str,
) -> String {
    let id: u64 = match id_str.parse() {
        Ok(v) => v,
        Err(_) => return id_str.to_string(),
    };

    // The MutexGuard `g` must stay live until after path_for_id returns its &str,
    // so we clone to an owned String inside the closure before `g` is dropped.
    let maybe_path: Option<String> = cat
        .0
        .lock()
        .ok()
        .and_then(|g| {
            g.as_ref()
                .and_then(|(_, c)| crate::calligraphy::path_for_id(c, id))
                .map(|s| s.to_string())
        });

    match maybe_path {
        Some(path) => dn.lookup(server_dir, &path),
        None => id_str.to_string(),
    }
}

// ── Tauri commands ────────────────────────────────────────────────────────────

/// Return the absolute path to `{server_dir}/Data/Game/MTXStore`.
#[tauri::command]
pub fn get_mtxstore_dir(server_exe: String) -> Result<String, String> {
    Ok(mtxstore_dir(&server_exe)?.to_string_lossy().to_string())
}

/// Return sorted base `Catalog*.json` filenames (no directory, no MODIFIED variants)
/// present in the MTXStore directory.
#[tauri::command]
pub fn list_catalog_files(server_exe: String) -> Result<Vec<String>, String> {
    let dir = mtxstore_dir(&server_exe)?;
    if !dir.exists() {
        return Ok(vec![]);
    }
    base_catalog_files(&dir).map(|paths| {
        paths
            .into_iter()
            .filter_map(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
            .collect()
    })
}

/// Load all catalog entries from the MTXStore directory.
///
/// For each base `Catalog*.json` the corresponding `*MODIFIED.json` sibling is also
/// read. MODIFIED entries override base entries with the same `SkuId`. Each returned
/// entry carries `source_file` (the base filename) and `from_modified` provenance so
/// the frontend can route save/delete operations correctly.
#[tauri::command]
pub fn load_catalog_entries(server_exe: String) -> Result<Vec<CatalogEntryWithMeta>, String> {
    let dir = mtxstore_dir(&server_exe)?;
    if !dir.exists() {
        return Ok(vec![]);
    }

    let base_files = base_catalog_files(&dir)?;
    let mut result: Vec<CatalogEntryWithMeta> = Vec::new();
    let mut sku_index: HashMap<u64, usize> = HashMap::new();

    let mut upsert = |entry: CatalogEntryDisk, base_name: &str, from_modified: bool| {
        let sku = entry.sku_id;
        let meta = CatalogEntryWithMeta {
            entry: disk_to_view(entry),
            source_file: base_name.to_string(),
            from_modified,
        };
        if let Some(&idx) = sku_index.get(&sku) {
            result[idx] = meta;
        } else {
            sku_index.insert(sku, result.len());
            result.push(meta);
        }
    };

    for base_path in &base_files {
        let base_name = base_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        for entry in load_disk_entries(base_path) {
            upsert(entry, &base_name, false);
        }

        let modified = modified_path_for(base_path);
        if modified.exists() {
            for entry in load_disk_entries(&modified) {
                upsert(entry, &base_name, true);
            }
        }
    }

    Ok(result)
}

/// Upsert a catalog entry directly into the existing catalog file.
///
/// `target_file` is the base filename (e.g. `"CatalogBundle.json"`). The entry is written
/// into that file, replacing the entry with the same `SkuId` or appending a new one. No
/// `*MODIFIED.json` file is ever created. If the SKU also lives in a legacy `*MODIFIED.json`
/// sibling (made by older versions), that copy is updated too, because the server loads
/// both files and the sibling would otherwise keep overriding the edit.
///
/// The file is edited as raw JSON, not through the typed structs: every field the editor
/// does not know about is kept, the key order is kept, and so are the file's line endings
/// and BOM. A `.bak` copy of each file is made the first time it is written in a session.
#[tauri::command]
pub fn save_catalog_entry(
    server_exe: String,
    entry: CatalogEntry,
    target_file: String,
) -> Result<(), String> {
    let dir = mtxstore_dir(&server_exe)?;
    let base_path = dir.join(&target_file);
    let legacy_path = modified_path_for(&base_path);

    if !base_path.is_file() {
        return Err(format!("{target_file} not found in the MTXStore folder"));
    }

    let sku = entry.sku_id;
    let new_value = serde_json::to_value(view_to_disk(entry)?)
        .map_err(|e| format!("Serialisation failed: {e}"))?;

    // Legacy sidecar first: only touched when it already holds this SKU.
    if legacy_path.is_file() {
        let (mut entries, style) = read_catalog_file(&legacy_path)?;
        if let Some(existing) = entries.iter_mut().find(|e| sku_of(e) == Some(sku)) {
            merge_preserving(existing, &new_value);
            backup_once(&legacy_path)?;
            write_catalog_file(&legacy_path, &entries, &style)?;
        }
    }

    let (mut entries, style) = read_catalog_file(&base_path)?;
    match entries.iter_mut().find(|e| sku_of(e) == Some(sku)) {
        Some(existing) => merge_preserving(existing, &new_value),
        None => entries.push(new_value),
    }
    backup_once(&base_path)?;
    write_catalog_file(&base_path, &entries, &style)
}

/// Delete a catalog entry by SKU from the existing catalog file(s).
///
/// `source_file` is the base filename (e.g. `"CatalogBundle.json"`). The entry is removed
/// from that file and from its legacy `*MODIFIED.json` sibling when that holds it too, so
/// a stale sidecar copy cannot keep the entry in the store. `from_modified` is kept for
/// compatibility with the frontend and is no longer needed to choose the file.
#[tauri::command]
pub fn delete_catalog_entry(
    server_exe: String,
    sku_id: u64,
    source_file: String,
    from_modified: bool,
) -> Result<(), String> {
    let _ = from_modified;
    let dir = mtxstore_dir(&server_exe)?;
    let base_path = dir.join(&source_file);
    let legacy_path = modified_path_for(&base_path);

    let mut removed = false;
    for path in [&base_path, &legacy_path] {
        if !path.is_file() {
            continue;
        }
        let (mut entries, style) = read_catalog_file(path)?;
        let before = entries.len();
        entries.retain(|e| sku_of(e) != Some(sku_id));
        if entries.len() != before {
            backup_once(path)?;
            write_catalog_file(path, &entries, &style)?;
            removed = true;
        }
    }

    if removed {
        Ok(())
    } else {
        Err(format!("SKU {sku_id} not found in {source_file}"))
    }
}

/// Scan all JSON files in the MTXStore directory (base and MODIFIED) and return the
/// next available SKU ID: `max_found + 1`, floored at 1001.
#[tauri::command]
pub fn get_next_sku_id(server_exe: String) -> Result<u64, String> {
    let dir = mtxstore_dir(&server_exe)?;
    if !dir.exists() {
        return Ok(1001);
    }

    let mut max_sku: u64 = 1000;

    for entry in
        fs::read_dir(&dir).map_err(|e| format!("Cannot read MTXStore directory: {e}"))?
    {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.ends_with(".json") {
            continue;
        }
        for e in load_disk_entries(&entry.path()) {
            if e.sku_id > max_sku {
                max_sku = e.sku_id;
            }
        }
    }

    Ok(max_sku + 1)
}

/// Resolve a prototype runtime ID (decimal string) to a human-readable display name.
///
/// The ID is a string parameter to safely transport the full u64 range from TypeScript.
/// Falls back through: custom override → embedded map → prototype path → raw ID.
#[tauri::command]
pub fn resolve_display_name(
    dn_state: tauri::State<DisplayNameState>,
    cat_state: tauri::State<CatalogueState>,
    server_exe: String,
    prototype_runtime_id: String,
) -> String {
    let server_dir = server_dir_of(&server_exe)
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();

    let _ = ensure_catalogue_loaded(&cat_state, &server_exe);

    name_for_id(&dn_state, &cat_state, &server_dir, &prototype_runtime_id)
}

/// Generate the HTML page for a bundle catalog entry and write it to disk.
///
/// Always writes a backup to:
///   `{server_dir}/Data/Web/MH-Multiverse-Bundles/html/{title_slug}_{sku}_en_bundle.html`
///
/// Also writes a backup CSS seed to:
///   `{server_dir}/Data/Web/MH-Multiverse-Bundles/css/style.css`
///   (only if the file does not already exist, preserving manual edits)
///
/// If `save_to_apache` is true, additionally writes the HTML to:
///   `{server_dir}/../Apache24/htdocs/bundles/{title_slug}_{sku}_en_bundle.html`
///   Returns an error if that `bundles/` subdirectory does not exist.
///
/// Returns the absolute path of the backup HTML file.
#[tauri::command]
pub fn generate_bundle_html(
    dn_state: tauri::State<DisplayNameState>,
    cat_state: tauri::State<CatalogueState>,
    server_exe: String,
    entry: CatalogEntry,
    save_to_apache: bool,
) -> Result<String, String> {
    let server_dir = server_dir_of(&server_exe)
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();

    let loc = entry
        .localized_entries
        .iter()
        .find(|e| e.language_id == "en_us")
        .or_else(|| entry.localized_entries.first())
        .ok_or_else(|| "Entry has no localized entries".to_string())?;

    let title = &loc.title;
    let price = loc.item_price;
    let sku_hex = format!("0x{:X}", entry.sku_id);

    let mut items_html = String::new();
    for item in &entry.guid_items {
        let name = name_for_id(
            &dn_state,
            &cat_state,
            &server_dir,
            &item.item_prototype_runtime_id_for_client,
        );
        if item.quantity == 1 {
            items_html.push_str(&format!("            <li>{name}</li>\n"));
        } else {
            items_html.push_str(&format!(
                "            <li>{name} x{}</li>\n",
                item.quantity
            ));
        }
    }

    let html = HTML_TEMPLATE
        .replace("{title}", title)
        .replace("{items}", &items_html)
        .replace("{price}", &price.to_string())
        .replace("{sku_hex}", &sku_hex);

    let slug = title.to_lowercase().replace(' ', "_");
    let filename = format!("{slug}_{}_en_bundle.html", entry.sku_id);

    // ── Unconditional backup ──────────────────────────────────────────────────

    let backup_root = server_dir_of(&server_exe)?
        .join("Data")
        .join("Web")
        .join("MH-Multiverse-Bundles");

    let backup_html_dir = backup_root.join("html");
    let backup_css_dir  = backup_root.join("css");

    fs::create_dir_all(&backup_html_dir)
        .map_err(|e| format!("Cannot create backup html directory: {e}"))?;
    fs::create_dir_all(&backup_css_dir)
        .map_err(|e| format!("Cannot create backup css directory: {e}"))?;

    // Only seed CSS if absent — preserves user edits made after first generation
    let backup_css_path = backup_css_dir.join("style.css");
    if !backup_css_path.exists() {
        fs::write(&backup_css_path, STORE_CSS)
            .map_err(|e| format!("Cannot write backup style.css: {e}"))?;
    }

    let backup_html_path = backup_html_dir.join(&filename);
    fs::write(&backup_html_path, &html)
        .map_err(|e| format!("Cannot write backup HTML: {e}"))?;

    // ── Optional Apache htdocs write ──────────────────────────────────────────

    if save_to_apache {
        let apache_bundles_dir = server_dir_of(&server_exe)?
            .parent()
            .ok_or_else(|| "Cannot determine root directory from server exe path".to_string())?
            .join("Apache24")
            .join("htdocs")
            .join("bundles");
        if !apache_bundles_dir.exists() {
            return Err(format!(
                "Apache bundles directory not found: {}",
                apache_bundles_dir.display()
            ));
        }
        let apache_html_path = apache_bundles_dir.join(&filename);
        fs::write(&apache_html_path, &html)
            .map_err(|e| format!("Cannot write Apache HTML: {e}"))?;
    }

    Ok(backup_html_path.to_string_lossy().to_string())
}

// ── CSS template ──────────────────────────────────────────────────────────────

static STORE_CSS: &str = r#"/* MH Multiverse — bundle store page */

html {
    background: #0d0d0b;
    color: #d7d7d7;
    font: 18px/1.385 Verdana, sans-serif;
}

*:focus { outline-color: #00e8ff; }

h1, h2, h3, h4, h5, h6 {
    color: #00aaff;
    font-family: "Bebas Neue", "Trebuchet MS", Verdana, sans-serif;
    font-weight: normal;
}

button, input {
    box-sizing: border-box;
    background: #1c1c1c;
    border: 1px solid #000;
    box-shadow: 0 0 1px 1px #00aaff;
    color: inherit;
    font: inherit;
}

button { padding: 8px; }
button:hover { box-shadow: 0 0 1px 1px #fff; }

div.content {
    position: absolute;
    inset: 10px 10px 80px;
    padding: 20px;
    border: 1px solid #00717c;
    background: #00090e;
    overflow-y: auto;
}

div.buttons {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 10px 20px;
    top: auto;
    bottom: 10px;
}

.content::-webkit-scrollbar { width: 8px; }
.content::-webkit-scrollbar-track { background: transparent; }
.content::-webkit-scrollbar-thumb {
    background: #13aaf4;
    border: 1px solid #46ddf0;
    border-radius: 10px;
}

div.buttons button {
    padding: 8px 40px 6px;
    font: normal 23px/20px "Bebas Neue", "Trebuchet MS", Verdana, sans-serif;
    color: #fff;
    text-transform: uppercase;
    text-shadow: 1px 1px 2px #000;
    border: 1px solid #65e872;
    border-radius: 4px;
    background: linear-gradient(337deg, #087a10 49%, #299f2c 51%, #0d7d16 70%, #087a10 100%);
    box-shadow: inset 0 0 8px #39cb42, 0 0 4px #39cb42;
}

div.buttons button:hover {
    background: linear-gradient(337deg, #009e08 49%, #30df39 51%, #24992b 70%, #009e08 100%);
    box-shadow: inset 0 0 8px #03ff0d, 0 0 4px #08cf10;
}

span.price {
    display: inline-block;
    position: relative;
    padding: 5px 25px 0 0;
    color: #fff;
    font: normal 32px/32px "Bebas Neue", "Trebuchet MS", Verdana, sans-serif;
}

.g {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    vertical-align: middle;
    width: 24px;
    height: 24px;
    margin-left: 6px;
    color: #00090e;
    font: bold 18px/1 Arial, sans-serif;
    text-shadow: 1px 2px 1px #00b2fe;
    background: linear-gradient(-35deg, #007fff 45%, #03c2f7 54%, #00b2fe 70%, #007fff 100%);
    border: 1px solid #00b2fe;
    border-radius: 50%;
}
"#;

// ── HTML template ─────────────────────────────────────────────────────────────

static HTML_TEMPLATE: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{title}</title>
    <link rel="preconnect" href="https://fonts.googleapis.com">
    <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
    <link href="https://fonts.googleapis.com/css2?family=Bebas+Neue&display=swap" rel="stylesheet">
    <link rel="stylesheet" href="css/style.css">
</head>
<body>
    <div class="content">
        <h1>{title}</h1>
        <p>What's included:</p>
        <ul>
{items}
        </ul>
    </div>
    <div class="buttons content">
        <span class="price">{price} <span class="g">G</span></span>
        <button onclick="myApi.BuyBundleFromJS('{sku_hex}')">Buy Now</button>
    </div>
</body>
</html>
"#;
/// Generate a 344×128 PNG thumbnail for a bundle store page and write it to disk.
///
/// Always writes a backup to:
///   `{server_dir}/Data/Web/MH-Multiverse-Bundles/images/{title_slug}_{sku}.png`
///
/// If `save_to_apache` is true, additionally writes to:
///   `{server_dir}/../Apache24/htdocs/bundles/images/{title_slug}_{sku}.png`
///   Returns an error if that directory does not exist.
///
/// `png_base64` is a standard Base64-encoded PNG produced by the frontend canvas.
///
/// Returns the absolute path of the backup image file.
#[tauri::command]
pub fn save_thumbnail(
    server_exe: String,
    slug: String,
    sku_id: u64,
    png_base64: String,
    save_to_apache: bool,
) -> Result<String, String> {
    use base64::{engine::general_purpose::STANDARD, Engine};

    let png_bytes = STANDARD
        .decode(&png_base64)
        .map_err(|e| format!("Invalid base64 thumbnail data: {e}"))?;

    let filename = format!("MTXStore_{slug}_{sku_id}.png");

    // ── Unconditional backup ──────────────────────────────────────────────────

    let backup_dir = server_dir_of(&server_exe)?
        .join("Data")
        .join("Web")
        .join("MH-Multiverse-Bundles")
        .join("images");

    fs::create_dir_all(&backup_dir)
        .map_err(|e| format!("Cannot create backup images directory: {e}"))?;

    let backup_path = backup_dir.join(&filename);
    fs::write(&backup_path, &png_bytes)
        .map_err(|e| format!("Cannot write backup thumbnail: {e}"))?;

    // ── Optional Apache htdocs write ──────────────────────────────────────────

    if save_to_apache {
        let apache_images_dir = server_dir_of(&server_exe)?
            .parent()
            .ok_or_else(|| "Cannot determine root directory from server exe path".to_string())?
            .join("Apache24")
            .join("htdocs")
            .join("bundles")
            .join("images");

        if !apache_images_dir.exists() {
            return Err(format!(
                "Apache images directory not found: {}",
                apache_images_dir.display()
            ));
        }

        let apache_path = apache_images_dir.join(&filename);
        fs::write(&apache_path, &png_bytes)
            .map_err(|e| format!("Cannot write Apache thumbnail: {e}"))?;
    }

    Ok(backup_path.to_string_lossy().to_string())
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn temp_server(tag: &str) -> (PathBuf, String) {
        let root = std::env::temp_dir().join(format!("mhm_store_test_{tag}_{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("Data").join("Game").join("MTXStore")).unwrap();
        let exe = root.join("MHServerEmu.exe").to_string_lossy().to_string();
        (root, exe)
    }

    fn view_entry(sku: u64, title: &str) -> CatalogEntry {
        CatalogEntry {
            sku_id: sku,
            guid_items: vec![GuidItem {
                prototype_guid: 0,
                item_prototype_runtime_id_for_client: "16313827754993193555".into(),
                quantity: 1,
            }],
            additional_guid_items: vec![],
            localized_entries: vec![LocalizedEntry {
                language_id: "en_us".into(),
                description: title.into(),
                title: title.into(),
                release_date: String::new(),
                item_price: 900,
            }],
            info_urls: vec![],
            content_data: vec![],
            item_type: NamedItem { name: "Hero".into(), order: 0 },
            type_modifiers: vec![],
        }
    }

    #[test]
    fn merge_keeps_unknown_keys_and_takes_new_values() {
        let mut old = json!({"SkuId": 1, "Extra": "keep", "LocalizedEntries": [{"Title": "old", "Note": "keep"}, {"Title": "gone"}]});
        let new = json!({"SkuId": 1, "LocalizedEntries": [{"Title": "new"}]});
        merge_preserving(&mut old, &new);
        assert_eq!(old["Extra"], "keep");
        assert_eq!(old["LocalizedEntries"][0]["Title"], "new");
        assert_eq!(old["LocalizedEntries"][0]["Note"], "keep");
        assert_eq!(old["LocalizedEntries"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn save_writes_the_existing_file_and_never_creates_a_modified_file() {
        let (root, exe) = temp_server("save");
        let dir = root.join("Data").join("Game").join("MTXStore");
        let base = dir.join("CatalogBundle.json");
        // CRLF file with a key the editor does not know about, as the real server files can carry.
        let original = "[\r\n  {\r\n    \"SkuId\": 5,\r\n    \"Unknown\": {\"a\": 1},\r\n    \"GuidItems\": [],\r\n    \"AdditionalGuidItems\": [],\r\n    \"LocalizedEntries\": [],\r\n    \"InfoUrls\": [],\r\n    \"ContentData\": [],\r\n    \"Type\": {\"Name\": \"Hero\", \"Order\": 0},\r\n    \"TypeModifiers\": []\r\n  }\r\n]";
        fs::write(&base, original).unwrap();

        save_catalog_entry(exe.clone(), view_entry(5, "Edited"), "CatalogBundle.json".into()).unwrap();
        save_catalog_entry(exe.clone(), view_entry(6, "Brand new"), "CatalogBundle.json".into()).unwrap();

        let text = fs::read_to_string(&base).unwrap();
        assert!(text.contains("\r\n"), "line endings must be kept");
        assert!(text.contains("\"Unknown\""), "fields the editor does not know must survive");
        assert!(text.contains("Edited") && text.contains("Brand new"));
        assert!(!dir.join("CatalogBundleMODIFIED.json").exists(), "no MODIFIED file may be created");
        assert!(dir.join("CatalogBundle.json.bak").exists(), "a backup of the file is made");
        let (entries, _) = read_catalog_file(&base).unwrap();
        assert_eq!(entries.len(), 2);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn save_updates_a_legacy_sidecar_that_holds_the_same_sku_and_delete_clears_both() {
        let (root, exe) = temp_server("legacy");
        let dir = root.join("Data").join("Game").join("MTXStore");
        let entry = |t: &str| json!([{"SkuId": 7, "GuidItems": [], "AdditionalGuidItems": [], "LocalizedEntries": [{"LanguageId": "en_us", "Description": t, "Title": t, "ReleaseDate": "", "ItemPrice": 1}], "InfoUrls": [], "ContentData": [], "Type": {"Name": "Hero", "Order": 0}, "TypeModifiers": []}]).to_string();
        fs::write(dir.join("Catalog.json"), entry("base")).unwrap();
        fs::write(dir.join("CatalogMODIFIED.json"), entry("sidecar")).unwrap();

        save_catalog_entry(exe.clone(), view_entry(7, "Both"), "Catalog.json".into()).unwrap();
        for file in ["Catalog.json", "CatalogMODIFIED.json"] {
            assert!(fs::read_to_string(dir.join(file)).unwrap().contains("Both"), "{file} must carry the edit");
        }

        delete_catalog_entry(exe.clone(), 7, "Catalog.json".into(), true).unwrap();
        for file in ["Catalog.json", "CatalogMODIFIED.json"] {
            assert_eq!(read_catalog_file(&dir.join(file)).unwrap().0.len(), 0, "{file} must be empty");
        }
        assert!(delete_catalog_entry(exe, 7, "Catalog.json".into(), false).is_err());
        let _ = fs::remove_dir_all(&root);
    }

    /// Run by hand against the real files:  MTX_CATALOG_DIR=<...>/Data/Game/MTXStore cargo test real_files -- --ignored --nocapture
    /// Proves a read and rewrite changes nothing.
    #[test]
    #[ignore]
    fn real_files_round_trip_byte_for_byte() {
        let dir = PathBuf::from(std::env::var("MTX_CATALOG_DIR").expect("set MTX_CATALOG_DIR"));
        for file in base_catalog_files(&dir).unwrap() {
            let original = fs::read(&file).unwrap();
            let (entries, style) = read_catalog_file(&file).unwrap();
            let out = std::env::temp_dir().join("mhm_roundtrip.json");
            write_catalog_file(&out, &entries, &style).unwrap();
            let same = fs::read(&out).unwrap() == original;
            println!("{} : {}", file.file_name().unwrap().to_string_lossy(), if same { "identical" } else { "DIFFERENT" });
            assert!(same, "{} changed on a plain rewrite", file.display());
        }
    }

    /// Run by hand: MTX_CATALOG_DIR=<...>/Data/Game/MTXStore cargo test real_edit -- --ignored --nocapture
    /// Edits one real entry in a COPY of Catalog.json and proves every other line is unchanged.
    #[test]
    #[ignore]
    fn real_edit_changes_only_that_entry() {
        let src = PathBuf::from(std::env::var("MTX_CATALOG_DIR").expect("set MTX_CATALOG_DIR")).join("Catalog.json");
        let (root, exe) = temp_server("realedit");
        let dir = root.join("Data").join("Game").join("MTXStore");
        fs::copy(&src, dir.join("Catalog.json")).unwrap();
        let original = fs::read_to_string(dir.join("Catalog.json")).unwrap();

        let (entries, _) = read_catalog_file(&dir.join("Catalog.json")).unwrap();
        let first: CatalogEntryDisk = serde_json::from_value(entries[10].clone()).unwrap();
        let mut view = disk_to_view(first);
        view.localized_entries[0].item_price += 1;
        save_catalog_entry(exe, view, "Catalog.json".into()).unwrap();

        let edited = fs::read_to_string(dir.join("Catalog.json")).unwrap();
        let a: Vec<&str> = original.split("
").collect();
        let b: Vec<&str> = edited.split("
").collect();
        assert_eq!(a.len(), b.len(), "line count must not change");
        let changed: Vec<usize> = (0..a.len()).filter(|&i| a[i] != b[i]).collect();
        println!("lines changed: {:?}", changed.iter().map(|i| format!("{} -> {}", a[*i].trim(), b[*i].trim())).collect::<Vec<_>>());
        assert_eq!(changed.len(), 1, "only the price line may change");
        assert!(!dir.join("CatalogMODIFIED.json").exists());
        let _ = fs::remove_dir_all(&root);
    }
}
