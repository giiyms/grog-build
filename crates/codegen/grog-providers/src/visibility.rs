//! Hidden ids stay in the catalog so `-m` still works.

use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

static SHOW_HIDDEN: AtomicBool = AtomicBool::new(false);

const HIDDEN_MODELS_FILE: &str = "hidden-models";

pub fn hidden_models_path(home: &Path) -> PathBuf {
    home.join(HIDDEN_MODELS_FILE)
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HiddenSet {
    ids: BTreeSet<String>,
}

impl HiddenSet {
    pub fn load(home: &Path) -> Self {
        Self::from_text(&fs::read_to_string(hidden_models_path(home)).unwrap_or_default())
    }

    pub fn from_text(text: &str) -> Self {
        let mut ids = BTreeSet::new();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            ids.insert(line.to_string());
        }
        Self { ids }
    }

    pub fn save(&self, home: &Path) -> io::Result<()> {
        let path = hidden_models_path(home);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        if self.ids.is_empty() {
            match fs::remove_file(&path) {
                Ok(()) => Ok(()),
                Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
                Err(err) => Err(err),
            }
        } else {
            let mut body = String::from("# grog hidden picker models\n");
            for id in &self.ids {
                body.push_str(id);
                body.push('\n');
            }
            fs::write(path, body)
        }
    }

    pub fn hide(&mut self, id: &str) {
        let id = id.trim();
        if !id.is_empty() {
            self.ids.insert(id.to_string());
        }
    }

    pub fn unhide(&mut self, id: &str) {
        let id = id.trim();
        self.ids.remove(id);
        if let Some(slug) = id.split('/').next_back()
            && slug != id
        {
            self.ids.remove(slug);
        }
    }

    pub fn contains(&self, catalog_key: &str) -> bool {
        if self.ids.contains(catalog_key) {
            return true;
        }
        let slug = catalog_key.split('/').next_back().unwrap_or(catalog_key);
        self.ids.iter().any(|id| {
            id == slug
                || id
                    .split('/')
                    .next_back()
                    .is_some_and(|stored| stored == slug)
        })
    }

    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &str> {
        self.ids.iter().map(String::as_str)
    }
}

pub fn show_hidden() -> bool {
    SHOW_HIDDEN.load(Ordering::Relaxed)
}

pub fn set_show_hidden(value: bool) {
    SHOW_HIDDEN.store(value, Ordering::Relaxed);
}

pub fn toggle_show_hidden() -> bool {
    let next = !show_hidden();
    set_show_hidden(next);
    next
}

pub fn is_picker_visible(catalog_key: &str, hidden: &HiddenSet) -> bool {
    show_hidden() || !hidden.contains(catalog_key)
}

pub fn load_hidden_from_grog_home() -> HiddenSet {
    HiddenSet::load(&xai_dirs::grok_home())
}

pub fn persist_hidden_to_grog_home(hidden: &HiddenSet) -> io::Result<()> {
    hidden.save(&xai_dirs::grok_home())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static SHOW_HIDDEN_LOCK: Mutex<()> = Mutex::new(());

    fn isolated_show_hidden() -> std::sync::MutexGuard<'static, ()> {
        SHOW_HIDDEN_LOCK.lock().unwrap_or_else(|e| e.into_inner())
    }

    #[test]
    fn hide_persists_and_filters_then_unhide_restores() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        let mut hidden = HiddenSet::load(home);
        assert!(hidden.is_empty());
        hidden.hide("antigravity/gemini-3.6-flash");
        hidden.save(home).unwrap();
        assert!(hidden_models_path(home).is_file());

        let reloaded = HiddenSet::load(home);
        assert!(reloaded.contains("antigravity/gemini-3.6-flash"));
        assert!(reloaded.contains("gemini-3.6-flash"));
        assert!(!reloaded.contains("antigravity/gemini-3.8-flash-high"));

        let _guard = isolated_show_hidden();
        set_show_hidden(false);
        assert!(!is_picker_visible(
            "antigravity/gemini-3.6-flash",
            &reloaded
        ));
        assert!(is_picker_visible(
            "antigravity/gemini-3.8-flash-high",
            &reloaded
        ));

        let mut restored = reloaded;
        restored.unhide("antigravity/gemini-3.6-flash");
        restored.save(home).unwrap();
        let after = HiddenSet::load(home);
        assert!(!after.contains("antigravity/gemini-3.6-flash"));
        assert!(is_picker_visible("antigravity/gemini-3.6-flash", &after));
        assert!(
            !hidden_models_path(home).exists(),
            "empty hidden set removes the file"
        );
        set_show_hidden(false);
    }

    #[test]
    fn show_hidden_toggle_reveals_without_deleting() {
        let _guard = isolated_show_hidden();
        set_show_hidden(false);
        let hidden = HiddenSet::from_text("codex/gpt-5.6-sol\n");
        assert!(!is_picker_visible("codex/gpt-5.6-sol", &hidden));
        assert!(toggle_show_hidden());
        assert!(is_picker_visible("codex/gpt-5.6-sol", &hidden));
        assert!(!hidden.is_empty());
        set_show_hidden(false);
    }
}
