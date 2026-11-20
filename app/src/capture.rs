use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, Instant};

use adw::prelude::*;
use chrono::Local;
use gtk::{gdk, gio, glib, pango};
use libadwaita as adw;
use quicknote_core::{run_capture, CaptureSet};

use crate::config::Config;
use crate::settings;
use crate::style;

const WIDTH: i32 = 600;
const MIN_TEXT_HEIGHT: i32 = 72;
const MAX_TEXT_HEIGHT: i32 = 340;
const MARGIN_X: i32 = 18;
const MARGIN_TOP: i32 = 16;
const CSS_CLASS: &str = "quicknote";
/// Length of the "text flicks away" save animation; the window closes right after it
/// (or as soon as the note is written, if that takes longer).
const SAVE_FLICK: Duration = Duration::from_millis(120);

fn buffer_text(b: &gtk::TextBuffer) -> String {
    b.text(&b.start_iter(), &b.end_iter(), false).to_string()
}

#[derive(Clone, Copy, PartialEq)]
enum SaveState {
    Idle,
    Saving,
    Saved,
}

/// Vault + parsed QuickAdd choices. Loaded when the window opens and again when it
/// regains focus (e.g. after Settings), never per keystroke.
struct Source {
    vault: PathBuf,
    set: CaptureSet,
}

impl Source {
    fn load() -> (Source, Config) {
        let cfg = Config::load();
        let vault = PathBuf::from(&cfg.vault_path);
        let set = if cfg.vault_path.is_empty() { CaptureSet::default() } else { CaptureSet::load(&vault).unwrap_or_default() };
        (Source { vault, set }, cfg)
    }

    fn names(&self) -> Vec<&str> {
        self.set.choices.iter().map(|c| c.name.as_str()).collect()
    }

    fn index_of(&self, name: &str) -> u32 {
        self.set.choices.iter().position(|c| c.name == name).unwrap_or(0) as u32
    }
}

fn placeholder_markup(choices: usize) -> String {
    let switch = if choices > 1 { " · Ctrl+1–9 switch" } else { "" };
    format!("Write a note…\n<span size=\"small\">Enter save · Shift+Enter new line{switch} · Esc close</span>")
}

/// Marks the window with the colour of the choice at `index`.
fn set_choice_class(win: &impl IsA<gtk::Widget>, index: u32) {
    for k in 0..style::PALETTE_LEN {
        win.remove_css_class(&format!("qn-c{k}"));
    }
    win.add_css_class(&format!("qn-c{}", index as usize % style::PALETTE_LEN));
}

fn set_theme_class(win: &impl IsA<gtk::Widget>, dark: bool) {
    let (on, off) = if dark { ("qn-dark", "qn-light") } else { ("qn-light", "qn-dark") };
    win.remove_css_class(off);
    win.add_css_class(on);
}

/// Drop-down rows: a colour dot (an orb in dark mode) followed by the choice name.
fn choice_factory() -> gtk::SignalListItemFactory {
    let factory = gtk::SignalListItemFactory::new();
    factory.connect_setup(|_, item| {
        let Some(item) = item.downcast_ref::<gtk::ListItem>() else { return };
        let dot = gtk::Image::from_icon_name("object-select-symbolic");
        dot.set_valign(gtk::Align::Center);
        let label = gtk::Label::builder().xalign(0.0).build();
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        row.append(&dot);
        row.append(&label);
        item.set_child(Some(&row));
    });
    factory.connect_bind(|_, item| {
        let Some(item) = item.downcast_ref::<gtk::ListItem>() else { return };
        let Some(row) = item.child() else { return };
        let (Some(dot), Some(label)) = (row.first_child(), row.last_child()) else { return };
        let name = item.item().and_downcast::<gtk::StringObject>().map(|s| s.string()).unwrap_or_default();
        if let Some(label) = label.downcast_ref::<gtk::Label>() {
            label.set_text(&name);
        }
        let n = item.position() as usize % style::PALETTE_LEN;
        dot.set_css_classes(&["qn-dot", &format!("qn-dot-{n}")]);
    });
    factory
}

thread_local! {
    /// The open capture window and its text, so a repeated shortcut can find them.
    static OPEN: RefCell<Option<(glib::WeakRef<adw::ApplicationWindow>, gtk::TextBuffer)>> = const { RefCell::new(None) };
}

/// Shows the capture window, or brings the open one to the front.
pub fn present(app: &adw::Application) {
    let open = OPEN.with(|o| o.borrow().as_ref().and_then(|(w, b)| Some((w.upgrade()?, b.clone()))));
    let Some((win, buffer)) = open else {
        build(app, None);
        return;
    };
    if win.is_active() {
        win.present();
        return;
    }
    // GNOME custom shortcuts launch without an activation token, so Mutter refuses to
    // raise an existing window (measured: present() leaves it unfocused). A window
    // created in response to the key press does get focus, so swap in a fresh one
    // holding the same text and cursor.
    let text = buffer_text(&buffer);
    let (start, end) = buffer.selection_bounds().unwrap_or_else(|| {
        let i = buffer.iter_at_mark(&buffer.get_insert());
        (i, i)
    });
    build(app, Some((text, (start.offset(), end.offset()))));
    win.destroy();
}

fn build(app: &adw::Application, carry: Option<(String, (i32, i32))>) {
    let (src, cfg) = Source::load();
    if src.set.choices.is_empty() {
        settings::present(app);
        return;
    }

    // ── Text area with placeholder ───────────────────────────────────────────
    let view = gtk::TextView::builder()
        .wrap_mode(gtk::WrapMode::WordChar)
        .accepts_tab(false)
        .top_margin(MARGIN_TOP)
        .bottom_margin(12)
        .left_margin(MARGIN_X)
        .right_margin(MARGIN_X)
        .build();
    view.add_css_class("quicknote-text");
    let buffer = view.buffer();

    // Natural height propagates up, so the (non-resizable) window grows with the text.
    let scroller = gtk::ScrolledWindow::builder()
        .child(&view)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .propagate_natural_height(true)
        .min_content_height(MIN_TEXT_HEIGHT)
        .max_content_height(MAX_TEXT_HEIGHT)
        .vexpand(true)
        .build();
    scroller.add_css_class("qn-scroller");

    let placeholder = gtk::Label::builder()
        .use_markup(true)
        .label(placeholder_markup(src.set.choices.len()))
        .xalign(0.0)
        .halign(gtk::Align::Fill)
        .valign(gtk::Align::Start)
        .margin_top(MARGIN_TOP)
        .margin_start(MARGIN_X)
        .margin_end(MARGIN_X)
        .can_target(false)
        .build();
    placeholder.add_css_class("quicknote-text");
    placeholder.add_css_class("quicknote-placeholder");

    let text_area = gtk::Overlay::builder().child(&scroller).build();
    text_area.add_overlay(&placeholder);

    // ── Errors ───────────────────────────────────────────────────────────────
    let banner = adw::Banner::builder().button_label("Settings").use_markup(false).build();

    // ── Footer: choice pill · destination · settings ─────────────────────────
    let model = gtk::StringList::new(&src.names());
    let dd = gtk::DropDown::builder()
        .model(&model)
        .selected(src.index_of(&cfg.choice))
        .enable_search(true)
        .expression(gtk::PropertyExpression::new(gtk::StringObject::static_type(), None::<gtk::Expression>, "string"))
        .tooltip_text("QuickAdd Capture choice (Ctrl+1–9)")
        .valign(gtk::Align::Center)
        .build();
    // Set after construction: given to the builder next to `expression`, GTK kept its default factory.
    dd.set_factory(Some(&choice_factory()));

    // "→ folder/file.md": the folder is dimmed, the file name takes the choice colour,
    // and long paths lose their start so the file name stays visible.
    let target = gtk::Label::builder()
        .use_markup(true)
        .xalign(0.0)
        .hexpand(true)
        .max_width_chars(1) // keep the label from widening the window; it still fills the row
        .ellipsize(pango::EllipsizeMode::Start)
        .build();
    target.add_css_class("caption");

    let gear = gtk::Button::builder()
        .icon_name("emblem-system-symbolic")
        .tooltip_text("Settings (Ctrl+,)")
        .valign(gtk::Align::Center)
        .build();
    gear.add_css_class("flat");
    gear.add_css_class("circular");

    let footer = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(8)
        .margin_top(6)
        .margin_bottom(6)
        .margin_start(10)
        .margin_end(6)
        .build();
    footer.append(&dd);
    footer.append(&target);
    footer.append(&gear);
    // The window has no title bar, so the footer doubles as the drag handle.
    let handle = gtk::WindowHandle::builder().child(&footer).build();
    handle.add_css_class("quicknote-footer");

    // ── Layout ───────────────────────────────────────────────────────────────
    let glow = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    glow.add_css_class("qn-glow");

    let body = gtk::Box::new(gtk::Orientation::Vertical, 0);
    body.append(&glow);
    body.append(&banner);
    body.append(&text_area);
    body.append(&handle);

    let kbd_key = gtk::Label::new(None);
    let kbd = gtk::Box::builder()
        .spacing(4)
        .halign(gtk::Align::Start)
        .valign(gtk::Align::End)
        .margin_start(14)
        .margin_bottom(54)
        .can_target(false)
        .build();
    kbd.append(&gtk::Label::new(Some("Ctrl")));
    kbd.append(&kbd_key);
    kbd.add_css_class("qn-kbd");

    let root = gtk::Overlay::builder().child(&body).build();
    root.add_overlay(&kbd);

    let win = adw::ApplicationWindow::builder()
        .application(app)
        .title("Quick Note")
        .default_width(WIDTH)
        .resizable(false)
        .content(&root)
        .build();
    // Replace libadwaita's 360×200 minimum so short notes give a compact window.
    win.set_size_request(WIDTH, -1);
    win.add_css_class(CSS_CLASS);
    style::apply(cfg.opacity);
    set_choice_class(&win, dd.selected());

    // The theme class picks light or dark accents; it follows the system style live.
    let styles = adw::StyleManager::default();
    set_theme_class(&win, styles.is_dark());

    let src = Rc::new(RefCell::new(src));
    let state = Rc::new(Cell::new(SaveState::Idle));
    let reloading = Rc::new(Cell::new(false));

    let refresh = {
        let (src, buffer, dd, target, banner, state, styles) =
            (src.clone(), buffer.clone(), dd.clone(), target.clone(), banner.clone(), state.clone(), styles.clone());
        Rc::new(move || {
            if state.get() != SaveState::Idle {
                return;
            }
            match src.borrow().set.preview(dd.selected() as usize, buffer_text(&buffer).trim(), &Local::now()) {
                Ok(rel) => {
                    let (folder, name) = match rel.rfind('/') {
                        Some(i) => (&rel[..=i], &rel[i + 1..]),
                        None => ("", rel.as_str()),
                    };
                    let ink = style::ink(dd.selected(), styles.is_dark());
                    target.set_markup(&format!(
                        "<span alpha=\"62%\">→ {}</span><span weight=\"500\" foreground=\"{ink}\">{}</span>",
                        glib::markup_escape_text(folder),
                        glib::markup_escape_text(name)
                    ));
                    target.set_tooltip_text(Some(&rel));
                    banner.set_revealed(false);
                }
                Err(e) => {
                    target.set_text("");
                    target.set_tooltip_text(None);
                    banner.set_title(&e);
                    banner.set_revealed(true);
                }
            }
        })
    };

    if let Some((text, (a, b))) = carry {
        buffer.set_text(&text);
        buffer.select_range(&buffer.iter_at_offset(a), &buffer.iter_at_offset(b));
        placeholder.set_visible(text.is_empty());
    }
    refresh();
    OPEN.with(|o| *o.borrow_mut() = Some((win.downgrade(), buffer.clone())));

    // Follow the system light/dark switch live.
    {
        let (w, r) = (win.downgrade(), refresh.clone());
        let id = styles.connect_dark_notify(move |sm| {
            if let Some(win) = w.upgrade() {
                set_theme_class(&win, sm.is_dark());
                r();
            }
        });
        let id = RefCell::new(Some(id));
        win.connect_destroy(move |_| {
            if let Some(id) = id.borrow_mut().take() {
                styles.disconnect(id);
            }
        });
    }

    {
        let (r, placeholder) = (refresh.clone(), placeholder.clone());
        buffer.connect_changed(move |b| {
            placeholder.set_visible(b.char_count() == 0);
            r();
        });
    }
    {
        let (r, src, reloading, win) = (refresh.clone(), src.clone(), reloading.clone(), win.clone());
        dd.connect_selected_notify(move |dd| {
            set_choice_class(&win, dd.selected());
            if reloading.get() {
                return;
            }
            if let Some(c) = src.borrow().set.choices.get(dd.selected() as usize) {
                let mut cfg = Config::load();
                cfg.choice = c.name.clone();
                cfg.save();
            }
            r();
        });
    }

    // Pick up vault/choice changes made in Settings (or in Obsidian) when the window
    // regains focus.
    {
        let (src, dd, placeholder, r, reloading) =
            (src.clone(), dd.clone(), placeholder.clone(), refresh.clone(), reloading.clone());
        let was_inactive = Cell::new(false);
        win.connect_is_active_notify(move |w| {
            if !w.is_active() {
                was_inactive.set(true);
                return;
            }
            if !was_inactive.replace(false) {
                return;
            }
            let (fresh, cfg) = Source::load();
            if fresh.set.choices.is_empty() {
                return; // keep what we have; the banner/save path reports problems
            }
            if fresh.names() != src.borrow().names() {
                reloading.set(true);
                dd.set_model(Some(&gtk::StringList::new(&fresh.names())));
                dd.set_selected(fresh.index_of(&cfg.choice));
                placeholder.set_markup(&placeholder_markup(fresh.set.choices.len()));
                reloading.set(false);
            }
            *src.borrow_mut() = fresh;
            r();
        });
    }

    let open_settings = {
        let app = app.clone();
        move || settings::present(&app)
    };
    {
        let open_settings = open_settings.clone();
        gear.connect_clicked(move |_| open_settings());
    }
    {
        let open_settings = open_settings.clone();
        banner.connect_button_clicked(move |_| open_settings());
    }

    // Closing without saving discards the text. The window stays put while a save is
    // in flight so an error can still be shown in it.
    {
        let state = state.clone();
        win.connect_close_request(move |_| match state.get() {
            SaveState::Saving => glib::Propagation::Stop,
            SaveState::Idle | SaveState::Saved => glib::Propagation::Proceed,
        });
    }

    // Enter: the text flicks away while the note is written in the background, then the
    // window closes. On failure the window comes back with the text and the error.
    let submit = {
        let (win, buffer, dd, state, src, banner) =
            (win.clone(), buffer.clone(), dd.clone(), state.clone(), src.clone(), banner.clone());
        Rc::new(move || {
            let text = buffer_text(&buffer).trim().to_string();
            if text.is_empty() || state.get() != SaveState::Idle {
                return;
            }
            let (vault, choice) = {
                let s = src.borrow();
                let Some(c) = s.set.choices.get(dd.selected() as usize) else { return };
                (s.vault.clone(), c.name.clone())
            };
            state.set(SaveState::Saving);
            banner.set_revealed(false);
            win.add_css_class("saving");
            let (win, state, banner) = (win.clone(), state.clone(), banner.clone());
            glib::spawn_future_local(async move {
                let started = Instant::now();
                let res = gio::spawn_blocking(move || run_capture(&vault, &choice, &text, &Local::now()))
                    .await
                    .unwrap_or_else(|_| Err("internal error while saving".to_string()));
                if let Some(rest) = SAVE_FLICK.checked_sub(started.elapsed()) {
                    glib::timeout_future(rest).await;
                }
                match res {
                    Ok(_) => {
                        state.set(SaveState::Saved);
                        win.close();
                    }
                    Err(e) => {
                        win.remove_css_class("saving");
                        state.set(SaveState::Idle);
                        banner.set_title(&format!("Couldn't save: {e}"));
                        banner.set_revealed(true);
                    }
                }
            });
        })
    };

    let enter = gtk::EventControllerKey::new();
    {
        let submit = submit.clone();
        enter.connect_key_pressed(move |_, k, _, st| {
            if matches!(k, gdk::Key::Return | gdk::Key::KP_Enter) && !st.contains(gdk::ModifierType::SHIFT_MASK) {
                submit();
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
    }
    view.add_controller(enter);

    // Bubble phase, so the input method can still use Esc to cancel a pre-edit.
    let esc = gtk::EventControllerKey::new();
    {
        let win = win.clone();
        esc.connect_key_pressed(move |_, k, _, _| {
            if k == gdk::Key::Escape {
                win.close();
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
    }
    win.add_controller(esc);

    // Ctrl+1–9 picks a choice (with a brief key hint), Ctrl+, opens Settings.
    let ctrl = gtk::EventControllerKey::new();
    ctrl.set_propagation_phase(gtk::PropagationPhase::Capture);
    {
        let (dd, kbd, kbd_key) = (dd.clone(), kbd.clone(), kbd_key.clone());
        let hint_gen = Rc::new(Cell::new(0u32));
        ctrl.connect_key_pressed(move |_, k, _, st| {
            let mods = st & gtk::accelerator_get_default_mod_mask();
            if mods != gdk::ModifierType::CONTROL_MASK {
                return glib::Propagation::Proceed;
            }
            if k == gdk::Key::comma {
                open_settings();
                return glib::Propagation::Stop;
            }
            if let Some(n) = k.to_unicode().and_then(|c| c.to_digit(10)).filter(|n| (1..=9).contains(n)) {
                let i = n - 1;
                if i < dd.model().map_or(0, |m| m.n_items()) {
                    dd.set_selected(i);
                    kbd_key.set_text(&n.to_string());
                    kbd.add_css_class("show");
                    let id = hint_gen.get().wrapping_add(1);
                    hint_gen.set(id);
                    let (kbd, hint_gen) = (kbd.clone(), hint_gen.clone());
                    glib::timeout_add_local_once(Duration::from_millis(900), move || {
                        if hint_gen.get() == id {
                            kbd.remove_css_class("show");
                        }
                    });
                }
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
    }
    win.add_controller(ctrl);

    win.present();
    view.grab_focus();
}
