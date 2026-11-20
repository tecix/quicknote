//! Global shortcut = a GNOME custom keybinding that runs this binary.
//! (Wayland has no app-level global key grabs; the shell must own the binding.)

use gtk::gio;
use gtk::prelude::*;

const MEDIA_KEYS: &str = "org.gnome.settings-daemon.plugins.media-keys";
const CUSTOM: &str = "org.gnome.settings-daemon.plugins.media-keys.custom-keybinding";
const PATH: &str = "/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/quicknote/";

fn schema_ok() -> bool {
    gio::SettingsSchemaSource::default()
        .map(|s| s.lookup(MEDIA_KEYS, true).is_some() && s.lookup(CUSTOM, true).is_some())
        .unwrap_or(false)
}

fn paths(s: &gio::Settings) -> Vec<String> {
    s.strv("custom-keybindings").iter().map(|g| g.to_string()).collect()
}

fn err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

/// Registers (or updates) the shortcut. `accel` is a GTK accelerator string.
pub fn install(accel: &str) -> Result<(), String> {
    if !schema_ok() {
        return Err("GNOME settings-daemon keybinding schema not found (needs GNOME)".into());
    }
    let exe = std::env::current_exe().map_err(err)?;
    let s = gio::Settings::new(MEDIA_KEYS);
    let mut list = paths(&s);
    if !list.iter().any(|p| p == PATH) {
        list.push(PATH.to_string());
        let refs: Vec<&str> = list.iter().map(String::as_str).collect();
        s.set_strv("custom-keybindings", refs.as_slice()).map_err(err)?;
    }
    let k = gio::Settings::with_path(CUSTOM, PATH);
    k.set_string("name", "Quick Note").map_err(err)?;
    k.set_string("command", &format!("'{}'", exe.display())).map_err(err)?;
    k.set_string("binding", accel).map_err(err)?;
    gio::Settings::sync();
    Ok(())
}

pub fn remove() -> Result<(), String> {
    if !schema_ok() {
        return Ok(());
    }
    let s = gio::Settings::new(MEDIA_KEYS);
    let list: Vec<String> = paths(&s).into_iter().filter(|p| p != PATH).collect();
    let refs: Vec<&str> = list.iter().map(String::as_str).collect();
    s.set_strv("custom-keybindings", refs.as_slice()).map_err(err)?;
    let k = gio::Settings::with_path(CUSTOM, PATH);
    k.reset("name");
    k.reset("command");
    k.reset("binding");
    gio::Settings::sync();
    Ok(())
}
