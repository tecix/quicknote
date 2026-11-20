//! Capture-window styling.
//!
//! Every QuickAdd choice gets a colour from [`PALETTE`] by position (the window
//! carries `qn-c<N>`). The colour shows as a gradient line along the top edge, the
//! choice dot (an orb in dark mode), the pill tint, the file name and the caret.
//! Animations are kept to a minimum so the window feels instant.

use std::cell::RefCell;

use gtk::gdk;

thread_local! {
    static PROVIDER: RefCell<Option<gtk::CssProvider>> = const { RefCell::new(None) };
}

struct Swatch {
    /// Light theme: accent fill, and a darker "ink" for text on light backgrounds.
    fill: &'static str,
    ink: &'static str,
    /// Dark theme: the two gradient colours, and a light ink for text.
    dark_fill: &'static str,
    dark_fill2: &'static str,
    dark_ink: &'static str,
}

/// GNOME palette colours, in the order choices are assigned.
const PALETTE: [Swatch; 8] = [
    Swatch { fill: "#3584e4", ink: "#1a5fb4", dark_fill: "#3584e4", dark_fill2: "#26a269", dark_ink: "#99c1f1" }, // blue
    Swatch { fill: "#e66100", ink: "#c64600", dark_fill: "#e66100", dark_fill2: "#e5a50a", dark_ink: "#ffbe6f" }, // orange
    Swatch { fill: "#9141ac", ink: "#813d9c", dark_fill: "#9141ac", dark_fill2: "#c64f7f", dark_ink: "#dc8add" }, // purple
    Swatch { fill: "#2ec27e", ink: "#1b7a4b", dark_fill: "#26a269", dark_fill2: "#2190a4", dark_ink: "#8ff0a4" }, // green
    Swatch { fill: "#e01b24", ink: "#a51d2d", dark_fill: "#c01c28", dark_fill2: "#e66100", dark_ink: "#f66151" }, // red
    Swatch { fill: "#2190a4", ink: "#156674", dark_fill: "#2190a4", dark_fill2: "#26a269", dark_ink: "#93ddea" }, // teal
    Swatch { fill: "#e5a50a", ink: "#865e00", dark_fill: "#c88800", dark_fill2: "#e66100", dark_ink: "#f9f06b" }, // yellow
    Swatch { fill: "#e66197", ink: "#a8335f", dark_fill: "#c64f7f", dark_fill2: "#9141ac", dark_ink: "#f8a5c2" }, // pink
];

pub const PALETTE_LEN: usize = PALETTE.len();

/// Text colour for the choice at `index` that reads well on the window background.
pub fn ink(index: u32, dark: bool) -> &'static str {
    let s = &PALETTE[index as usize % PALETTE_LEN];
    if dark { s.dark_ink } else { s.ink }
}

const BASE: &str = r#"
window.quicknote { background-color: alpha(@window_bg_color, $A); }
window.quicknote scrolledwindow, window.quicknote textview, window.quicknote textview > text { background: none; }
window.quicknote .quicknote-text { font-size: 1.12em; }
window.quicknote.qn-dark .quicknote-text { font-size: 1.3em; }
window.quicknote .quicknote-placeholder { opacity: 0.6; }

/* The only gradient: a glowing line along the top edge. No intro animation. */
window.quicknote .qn-glow { min-height: 3px; }

/* Saving: the text flicks away, then the window closes as soon as the note is written. */
window.quicknote.saving .qn-scroller { animation: qn-out 120ms ease-in forwards; }
@keyframes qn-out { to { transform: translateY(-6px); opacity: 0; } }

/* Footer */
window.quicknote .quicknote-footer { border-top: 1px solid alpha(currentColor, 0.08); }
window.quicknote.qn-dark .quicknote-footer { background-color: alpha(black, 0.28); }
window.quicknote .quicknote-footer dropdown > button { padding: 2px 10px 2px 8px; border-radius: 999px; transition: background-color 150ms ease; }
window.quicknote.qn-dark .quicknote-footer dropdown > button { padding: 2px 10px 2px 4px; background-color: alpha(white, 0.07); box-shadow: inset 0 0 0 1px alpha(white, 0.1); }
window.quicknote.qn-dark .quicknote-footer dropdown > button:hover { background-color: alpha(white, 0.12); }

/* Choice dot (light) / gradient orb (dark) */
.qn-dot { min-width: 9px; min-height: 9px; border-radius: 999px; -gtk-icon-size: 9px; color: transparent; margin: 0 3px; }
window.quicknote.qn-dark .qn-dot { -gtk-icon-size: 12px; padding: 5px; margin: 0; }

/* "Ctrl 2" key hint after switching choice from the keyboard */
.qn-kbd { opacity: 0; transition: opacity 120ms ease; }
.qn-kbd.show { opacity: 1; }
.qn-kbd > label { font-size: 0.85em; font-weight: 500; padding: 2px 8px; border-radius: 6px; background-color: @view_bg_color; box-shadow: 0 0 0 1px alpha(currentColor, 0.12), 0 2px 0 alpha(currentColor, 0.12); }
"#;

const PER_SWATCH: &str = r#"
.qn-dot-$N { background-color: $FILL; box-shadow: 0 0 0 3px alpha($FILL, 0.28); }
window.quicknote.qn-dark .qn-dot-$N { background-color: transparent; background-image: linear-gradient(135deg, $DFILL, $DFILL2); box-shadow: none; }
window.quicknote.qn-light.qn-c$N .qn-glow { background-image: linear-gradient(to right, transparent, $FILL 22%, $FILL 78%, transparent); }
window.quicknote.qn-dark.qn-c$N .qn-glow { background-image: linear-gradient(to right, transparent, $DFILL 20%, $DFILL2 80%, transparent); }
window.quicknote.qn-light.qn-c$N .quicknote-footer dropdown > button { background-color: alpha($FILL, 0.15); }
window.quicknote.qn-light.qn-c$N .quicknote-footer dropdown > button:hover { background-color: alpha($FILL, 0.24); }
window.quicknote.qn-light.qn-c$N textview > text { caret-color: $FILL; }
window.quicknote.qn-light.qn-c$N textview > text > selection { background-color: alpha($FILL, 0.25); }
window.quicknote.qn-dark.qn-c$N textview > text { caret-color: $DINK; }
window.quicknote.qn-dark.qn-c$N textview > text > selection { background-color: alpha($DFILL, 0.4); }
"#;

fn stylesheet(percent: u32) -> String {
    let a = percent.clamp(30, 100) as f64 / 100.0;
    let mut css = BASE.replace("$A", &a.to_string());
    for (i, s) in PALETTE.iter().enumerate() {
        css.push_str(
            &PER_SWATCH
                .replace("$N", &i.to_string())
                .replace("$DFILL2", s.dark_fill2)
                .replace("$DFILL", s.dark_fill)
                .replace("$DINK", s.dark_ink)
                .replace("$FILL", s.fill)
                .replace("$INK", s.ink),
        );
    }
    css
}

/// (Re)loads the capture-window stylesheet. `percent` makes the background
/// translucent (text stays opaque).
pub fn apply(percent: u32) {
    let Some(display) = gdk::Display::default() else { return };
    let css = stylesheet(percent);
    PROVIDER.with(|p| {
        let mut p = p.borrow_mut();
        let provider = p.get_or_insert_with(|| {
            let pr = gtk::CssProvider::new();
            gtk::style_context_add_provider_for_display(&display, &pr, gtk::STYLE_PROVIDER_PRIORITY_USER);
            pr
        });
        provider.load_from_string(&css);
    });
}
