mod application;
#[rustfmt::skip]
mod config;
mod login;
mod session;
mod utils;
mod window;

use std::env;
use std::str::FromStr;

use self::application::Application;
use self::login::Login;
use self::session::Session;
use self::window::Window;

use config::{GETTEXT_PACKAGE, LOCALEDIR, RESOURCES_FILE};
use gettextrs::LocaleCategory;
use gtk::{gio, glib};
use log::LevelFilter;
use once_cell::sync::Lazy;
use syslog::Facility;

pub static RUNTIME: Lazy<tokio::runtime::Runtime> =
    Lazy::new(|| tokio::runtime::Runtime::new().unwrap());

fn main() {
    // Initialize logger
    syslog::init(
        Facility::LOG_USER,
        LevelFilter::from_str(&env::var("RUST_LOG").unwrap_or_else(|_| String::from("info")))
            .expect("error on parsing log-level"),
        Some("telegrand"),
    )
    .expect("could not initialize logging");

    // Prepare i18n
    gettextrs::setlocale(LocaleCategory::LcAll, "");
    gettextrs::bindtextdomain(GETTEXT_PACKAGE, LOCALEDIR).expect("Unable to bind the text domain");
    gettextrs::textdomain(GETTEXT_PACKAGE).expect("Unable to switch to the text domain");

    glib::set_application_name("Telegrand");

    gtk::init().expect("Unable to start GTK4");
    adw::init();

    let res = gio::Resource::load(RESOURCES_FILE).expect("Could not load gresource file");
    gio::resources_register(&res);

    let app = Application::new();
    app.run();
}
