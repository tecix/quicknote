mod capture;
mod config;
mod settings;
mod shortcut;
mod style;

use adw::prelude::*;
use gtk::{gio, glib};
use libadwaita as adw;

const APP_ID: &str = "dev.tecix.QuickNote";

fn main() -> glib::ExitCode {
    // A small text popup gains nothing from GPU rendering; the default Vulkan
    // renderer loads Mesa + libLLVM and doubles memory (136 → 68 MB RSS measured).
    // Respect an explicit GSK_RENDERER so it can still be overridden.
    if std::env::var_os("GSK_RENDERER").is_none() {
        // SAFETY: first statement in main, before GTK or any other thread starts.
        unsafe { std::env::set_var("GSK_RENDERER", "cairo") };
    }
    let app = adw::Application::builder()
        .application_id(APP_ID)
        .flags(gio::ApplicationFlags::HANDLES_COMMAND_LINE)
        .build();
    app.add_main_option(
        "settings",
        glib::Char::from(b's'),
        glib::OptionFlags::NONE,
        glib::OptionArg::None,
        "Open the settings window",
        None,
    );
    app.connect_command_line(|app, cl| {
        if cl.options_dict().contains("settings") {
            settings::present(app);
        } else {
            capture::present(app);
        }
        glib::ExitCode::SUCCESS
    });
    app.run()
}
