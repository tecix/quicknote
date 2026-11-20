use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use adw::prelude::*;
use gtk::{gdk, gio};
use libadwaita as adw;
use quicknote_core::{detect_vaults, list_capture_choices};

use crate::config::Config;
use crate::shortcut;

fn accel_label(accel: &str) -> String {
    match gtk::accelerator_parse(accel) {
        Some((k, m)) if !accel.is_empty() => gtk::accelerator_get_label(k, m).to_string(),
        _ => "Not set".to_string(),
    }
}

fn is_function_key(k: gdk::Key) -> bool {
    k.name().is_some_and(|n| n.len() <= 3 && n.starts_with('F') && n[1..].chars().all(|c| c.is_ascii_digit()) && n.len() > 1)
}

#[allow(deprecated)]
pub fn present(app: &adw::Application) {
    // Reuse an already-open settings window.
    for w in app.windows() {
        if w.title().as_deref() == Some("Quick Note Settings") {
            w.present();
            return;
        }
    }
    let cfg = Rc::new(RefCell::new(Config::load()));
    let win = adw::PreferencesWindow::builder()
        .application(app)
        .title("Quick Note Settings")
        .default_width(560)
        .default_height(620)
        .build();
    let page = adw::PreferencesPage::new();

    // ── Vault ────────────────────────────────────────────────────────────────
    let vault_group = adw::PreferencesGroup::builder()
        .title("Obsidian vault")
        .description("Notes are written straight into the vault; Obsidian does not need to run.")
        .build();

    let entry = adw::EntryRow::builder().title("Vault folder").text(cfg.borrow().vault_path.as_str()).build();
    let browse = gtk::Button::builder().icon_name("folder-open-symbolic").valign(gtk::Align::Center).tooltip_text("Browse…").build();
    browse.add_css_class("flat");
    entry.add_suffix(&browse);

    let vaults = detect_vaults();
    let detected = adw::ComboRow::builder().title("Detected vaults").build();
    let vnames: Vec<&str> = vaults.iter().map(|v| v.name.as_str()).collect();
    detected.set_model(Some(&gtk::StringList::new(&vnames)));
    detected.set_selected(
        vaults.iter().position(|v| v.path.to_string_lossy() == cfg.borrow().vault_path).unwrap_or(gtk::INVALID_LIST_POSITION as usize) as u32,
    );
    if !vaults.is_empty() {
        vault_group.add(&detected);
    }
    vault_group.add(&entry);
    page.add(&vault_group);

    // ── Capture choice ───────────────────────────────────────────────────────
    let choice_group = adw::PreferencesGroup::builder().title("QuickAdd").build();
    let choice_row = adw::ComboRow::builder().title("Capture choice").build();
    choice_group.add(&choice_row);
    page.add(&choice_group);

    let names: Rc<RefCell<Vec<String>>> = Rc::default();
    let loading = Rc::new(Cell::new(false));
    let reload = {
        let (cfg, choice_row, names, loading) = (cfg.clone(), choice_row.clone(), names.clone(), loading.clone());
        Rc::new(move || {
            loading.set(true);
            let list: Vec<String> = list_capture_choices(Path::new(&cfg.borrow().vault_path))
                .into_iter()
                .map(|c| c.name)
                .collect();
            let refs: Vec<&str> = list.iter().map(String::as_str).collect();
            choice_row.set_model(Some(&gtk::StringList::new(&refs)));
            choice_row.set_subtitle(if list.is_empty() {
                "No Capture choices found — is QuickAdd installed in this vault?"
            } else {
                ""
            });
            if let Some(i) = list.iter().position(|n| *n == cfg.borrow().choice) {
                choice_row.set_selected(i as u32);
            } else if let Some(first) = list.first() {
                cfg.borrow_mut().choice = first.clone();
                cfg.borrow().save();
            }
            *names.borrow_mut() = list;
            loading.set(false);
        })
    };
    reload();

    {
        let (cfg, names, loading) = (cfg.clone(), names.clone(), loading.clone());
        choice_row.connect_selected_notify(move |row| {
            if loading.get() {
                return;
            }
            if let Some(n) = names.borrow().get(row.selected() as usize) {
                cfg.borrow_mut().choice = n.clone();
                cfg.borrow().save();
            }
        });
    }
    {
        let (cfg, reload) = (cfg.clone(), reload.clone());
        entry.connect_changed(move |e| {
            cfg.borrow_mut().vault_path = e.text().to_string();
            cfg.borrow().save();
            reload();
        });
    }
    {
        let entry = entry.clone();
        detected.connect_selected_notify(move |row| {
            if let Some(v) = vaults.get(row.selected() as usize) {
                entry.set_text(&v.path.to_string_lossy());
            }
        });
    }
    {
        let (win, entry) = (win.clone(), entry.clone());
        browse.connect_clicked(move |_| {
            let entry = entry.clone();
            gtk::FileDialog::new().select_folder(Some(&win), gio::Cancellable::NONE, move |res| {
                if let Some(p) = res.ok().and_then(|f| f.path()) {
                    entry.set_text(&p.to_string_lossy());
                }
            });
        });
    }

    // ── Appearance ───────────────────────────────────────────────────────────
    let look = adw::PreferencesGroup::builder().title("Appearance").build();
    let adj = gtk::Adjustment::new(cfg.borrow().opacity as f64, 30.0, 100.0, 5.0, 10.0, 0.0);
    let opacity = adw::SpinRow::builder()
        .title("Window opacity")
        .subtitle("Background of the quick-note window, in percent")
        .adjustment(&adj)
        .build();
    {
        let cfg = cfg.clone();
        opacity.connect_value_notify(move |r| {
            let v = r.value().round() as u32;
            cfg.borrow_mut().opacity = v;
            cfg.borrow().save();
            crate::style::apply(v);
        });
    }
    look.add(&opacity);
    page.add(&look);

    // ── Shortcut ─────────────────────────────────────────────────────────────
    let sc_group = adw::PreferencesGroup::builder()
        .title("Global shortcut")
        .description("Registered as a GNOME custom keyboard shortcut that launches this app.")
        .build();
    let sc_row = adw::ActionRow::builder().title("Open Quick Note").subtitle(accel_label(&cfg.borrow().shortcut)).build();
    let change = gtk::Button::builder().label("Change…").valign(gtk::Align::Center).build();
    let clear = gtk::Button::builder().icon_name("edit-clear-symbolic").valign(gtk::Align::Center).tooltip_text("Remove shortcut").build();
    clear.add_css_class("flat");
    sc_row.add_suffix(&change);
    sc_row.add_suffix(&clear);
    sc_group.add(&sc_row);
    page.add(&sc_group);

    {
        let (win, cfg, sc_row) = (win.clone(), cfg.clone(), sc_row.clone());
        clear.connect_clicked(move |_| match shortcut::remove() {
            Ok(()) => {
                cfg.borrow_mut().shortcut.clear();
                cfg.borrow().save();
                sc_row.set_subtitle("Not set");
                win.add_toast(adw::Toast::new("Shortcut removed"));
            }
            Err(e) => win.add_toast(adw::Toast::new(&e)),
        });
    }
    {
        let (win, cfg, sc_row) = (win.clone(), cfg.clone(), sc_row.clone());
        change.connect_clicked(move |_| {
            let dialog = adw::AlertDialog::new(Some("Press the new shortcut"), Some("Hold a modifier (Ctrl, Alt, Super) and press a key.\nEsc cancels."));
            dialog.add_response("cancel", "Cancel");
            let ctrl = gtk::EventControllerKey::new();
            ctrl.set_propagation_phase(gtk::PropagationPhase::Capture);
            {
                let (dialog, win, cfg, sc_row) = (dialog.clone(), win.clone(), cfg.clone(), sc_row.clone());
                ctrl.connect_key_pressed(move |_, key, _, state| {
                    let mods = state & gtk::accelerator_get_default_mod_mask();
                    if key == gdk::Key::Escape && mods.is_empty() {
                        dialog.close();
                        return gtk::glib::Propagation::Stop;
                    }
                    let key = key.to_lower();
                    if !gtk::accelerator_valid(key, mods) {
                        return gtk::glib::Propagation::Stop; // bare modifier press
                    }
                    let has_mod = mods.intersects(gdk::ModifierType::CONTROL_MASK | gdk::ModifierType::ALT_MASK | gdk::ModifierType::SUPER_MASK);
                    if !has_mod && !is_function_key(key) {
                        return gtk::glib::Propagation::Stop;
                    }
                    let accel = gtk::accelerator_name(key, mods).to_string();
                    match shortcut::install(&accel) {
                        Ok(()) => {
                            cfg.borrow_mut().shortcut = accel.clone();
                            cfg.borrow().save();
                            sc_row.set_subtitle(&accel_label(&accel));
                            win.add_toast(adw::Toast::new("Shortcut saved"));
                        }
                        Err(e) => win.add_toast(adw::Toast::new(&e)),
                    }
                    dialog.close();
                    gtk::glib::Propagation::Stop
                });
            }
            dialog.add_controller(ctrl);
            dialog.present(Some(&win));
        });
    }

    win.add(&page);
    win.present();
    let _ = PathBuf::new();
}
