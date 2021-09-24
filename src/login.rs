use glib::clone;
use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use tdgrand::{enums::AuthorizationState, functions, types};

use crate::config;
use crate::utils::do_async;

mod imp {
    use super::*;
    use adw::subclass::prelude::BinImpl;
    use glib::subclass::Signal;
    use gtk::{gio, CompositeTemplate, PasswordEntry};
    use once_cell::sync::Lazy;
    use std::cell::Cell;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/com/github/melix99/telegrand/ui/login.ui")]
    pub struct Login {
        pub client_id: Cell<i32>,
        #[template_child]
        pub previous_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub next_stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub next_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub next_spinner: TemplateChild<gtk::Spinner>,
        #[template_child]
        pub content: TemplateChild<adw::Leaflet>,
        #[template_child]
        pub phone_number_entry: TemplateChild<gtk::Entry>,
        #[template_child]
        pub welcome_page_error_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub custom_encryption_key_entry: TemplateChild<gtk::PasswordEntry>,
        #[template_child]
        pub use_test_dc_switch: TemplateChild<gtk::Switch>,
        #[template_child]
        pub code_entry: TemplateChild<gtk::Entry>,
        #[template_child]
        pub code_error_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub password_entry: TemplateChild<gtk::PasswordEntry>,
        #[template_child]
        pub password_error_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub encryption_key_entry: TemplateChild<gtk::PasswordEntry>,
        #[template_child]
        pub encryption_key_error_label: TemplateChild<gtk::Label>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Login {
        const NAME: &'static str = "Login";
        type Type = super::Login;
        type ParentType = adw::Bin;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
            klass.install_action("login.previous", None, move |widget, _, _| {
                widget.previous()
            });
            klass.install_action("login.next", None, move |widget, _, _| widget.next());
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for Login {
        fn signals() -> &'static [Signal] {
            static SIGNALS: Lazy<Vec<Signal>> = Lazy::new(|| {
                vec![Signal::builder("new-session", &[], <()>::static_type().into()).build()]
            });
            SIGNALS.as_ref()
        }

        fn constructed(&self, obj: &Self::Type) {
            self.parent_constructed(obj);

            // Show the previous button on all pages except in the
            // "phone number" and "encryption key" pages
            let self_ = imp::Login::from_instance(obj);
            let previous_button = &*self_.previous_button;
            self_.content.connect_visible_child_name_notify(clone!(@weak previous_button => move |content| {
                let visible_page = content.visible_child_name().unwrap();
                if visible_page == "phone-number-page" || visible_page == "encryption-key-page" {
                    previous_button.set_visible(false);
                } else {
                    previous_button.set_visible(true);
                }
            }));

            // Bind the use-test-dc setting to the relative switch
            let use_test_dc_switch = &*self_.use_test_dc_switch;
            let settings = gio::Settings::new(config::APP_ID);
            settings
                .bind("use-test-dc", use_test_dc_switch, "state")
                .build();

            connect_password_entry_activate(&*self_.custom_encryption_key_entry, "login.next");
            connect_entry_activate(&*self_.phone_number_entry, "login.next");
            connect_entry_activate(&*self_.code_entry, "login.next");
            connect_entry_activate(&*self_.phone_number_entry, "login.next");
        }
    }

    // PasswordEntry does not seem to be a subclass of Entry and also does not implement EntryExt
    // So, unfortunetely this redunant function is necessary.
    fn connect_password_entry_activate(entry: &PasswordEntry, action: &'static str) {
        entry.connect_activate(move |this| {
            this.activate_action(action, None);
        });
    }

    fn connect_entry_activate<W: WidgetExt + EntryExt>(entry: &W, action: &'static str) {
        entry.connect_activate(move |this| {
            this.activate_action(action, None);
        });
    }

    impl WidgetImpl for Login {}
    impl BinImpl for Login {}
}

glib::wrapper! {
    pub struct Login(ObjectSubclass<imp::Login>)
        @extends gtk::Widget, adw::Bin;
}

impl Default for Login {
    fn default() -> Self {
        Self::new()
    }
}

impl Login {
    pub fn new() -> Self {
        glib::Object::new(&[]).expect("Failed to create Login")
    }

    pub fn login_client(&self, client_id: i32) {
        let self_ = imp::Login::from_instance(self);
        self_.client_id.set(client_id);

        self_.phone_number_entry.set_text("");
        self_.custom_encryption_key_entry.set_text("");
        self_.code_entry.set_text("");
        self_.password_entry.set_text("");
        self_.encryption_key_entry.set_text("");

        self.unfreeze();
        self.action_set_enabled("login.next", false);
    }

    pub fn set_authorization_state(&self, state: AuthorizationState) {
        let self_ = imp::Login::from_instance(self);

        match state {
            AuthorizationState::WaitTdlibParameters => {
                self.send_tdlib_parameters();
            }
            AuthorizationState::WaitEncryptionKey(_) => {
                self.send_encryption_key(true);
            }
            AuthorizationState::WaitPhoneNumber => {
                self_.content.set_visible_child_name("phone-number-page");
                self.unfreeze();
                self_.phone_number_entry.grab_focus();
            }
            AuthorizationState::WaitCode(_) => {
                self_.content.set_visible_child_name("code-page");
                self.unfreeze();
                self_.code_entry.grab_focus();
            }
            AuthorizationState::WaitOtherDeviceConfirmation(_) => {
                todo!()
            }
            AuthorizationState::WaitRegistration(_) => {
                todo!()
            }
            AuthorizationState::WaitPassword(_) => {
                self_.content.set_visible_child_name("password-page");
                self.unfreeze();
                self_.password_entry.grab_focus();
            }
            AuthorizationState::Ready => {
                self.emit_by_name("new-session", &[]).unwrap();
            }
            _ => {}
        }
    }

    fn previous(&self) {
        let self_ = imp::Login::from_instance(self);
        self_.content.set_visible_child_name("phone-number-page");

        // grab focus for entry after reset
        self_.phone_number_entry.grab_focus();
    }

    fn next(&self) {
        self.freeze();

        let self_ = imp::Login::from_instance(self);
        let visible_page = self_.content.visible_child_name().unwrap();
        if visible_page == "phone-number-page" {
            let encryption_key = self_.custom_encryption_key_entry.text().to_string();
            if !encryption_key.is_empty() {
                self.change_encryption_key();
            }

            self.send_phone_number();
        } else if visible_page == "code-page" {
            self.send_code();
        } else if visible_page == "password-page" {
            self.send_password();
        } else if visible_page == "encryption-key-page" {
            self.send_encryption_key(false);
        }
    }

    fn freeze(&self) {
        self.action_set_enabled("login.previous", false);
        self.action_set_enabled("login.next", false);

        let self_ = imp::Login::from_instance(self);
        self_
            .next_stack
            .set_visible_child(&self_.next_spinner.get());
        self_.content.set_sensitive(false);
    }

    fn unfreeze(&self) {
        self.action_set_enabled("login.previous", true);
        self.action_set_enabled("login.next", true);

        let self_ = imp::Login::from_instance(self);
        self_.next_stack.set_visible_child(&self_.next_label.get());
        self_.content.set_sensitive(true);
    }

    fn send_tdlib_parameters(&self) {
        let self_ = imp::Login::from_instance(self);
        let client_id = self_.client_id.get();
        let use_test_dc = self_.use_test_dc_switch.state();
        let database_directory =
            format!("{}/telegrand/db0", glib::user_data_dir().to_str().unwrap());
        let parameters = types::TdlibParameters {
            use_test_dc,
            database_directory,
            use_message_database: true,
            use_secret_chats: true,
            api_id: config::TG_API_ID,
            api_hash: config::TG_API_HASH.to_string(),
            system_language_code: "en-US".to_string(),
            device_model: "Desktop".to_string(),
            application_version: config::VERSION.to_string(),
            enable_storage_optimizer: true,
            ..types::TdlibParameters::default()
        };
        do_async(
            glib::PRIORITY_DEFAULT_IDLE,
            async move {
                functions::SetTdlibParameters::new()
                    .parameters(parameters)
                    .send(client_id)
                    .await
            },
            clone!(@weak self as obj => move |result| async move {
                if let Err(err) = result {
                    let self_ = imp::Login::from_instance(&obj);
                    self_.welcome_page_error_label.set_text(&err.message);
                    self_.welcome_page_error_label.set_visible(true);
                }
            }),
        );
    }

    fn send_encryption_key(&self, use_empty_key: bool) {
        let self_ = imp::Login::from_instance(self);
        let client_id = self_.client_id.get();
        let encryption_key = {
            if use_empty_key {
                "".to_string()
            } else {
                self_.encryption_key_entry.text().to_string()
            }
        };
        do_async(
            glib::PRIORITY_DEFAULT_IDLE,
            async move {
                functions::CheckDatabaseEncryptionKey::new()
                    .encryption_key(encryption_key)
                    .send(client_id)
                    .await
            },
            clone!(@weak self as obj => move |result| async move {
                if let Err(err) = result {
                    let self_ = imp::Login::from_instance(&obj);

                    // If we were trying an empty key, we now show the
                    // encryption key page to let the user input its key.
                    // Otherwise just show the error in the relative label.
                    if use_empty_key {
                        self_.content.set_visible_child_name("encryption-key-page");
                        obj.action_set_enabled("login.next", true);
                    } else {
                        let encryption_key_error_label = &self_.encryption_key_error_label;
                        encryption_key_error_label.set_text(&err.message);
                        encryption_key_error_label.set_visible(true);

                        obj.unfreeze();

                        // grab focus for entry again after error
                        self_.encryption_key_entry.grab_focus();
                    }
                }
            }),
        );
    }

    fn change_encryption_key(&self) {
        let self_ = imp::Login::from_instance(self);
        let client_id = self_.client_id.get();
        let encryption_key = self_.custom_encryption_key_entry.text().to_string();
        do_async(
            glib::PRIORITY_DEFAULT_IDLE,
            async move {
                functions::SetDatabaseEncryptionKey::new()
                    .new_encryption_key(encryption_key)
                    .send(client_id)
                    .await
            },
            clone!(@weak self as obj => move |result| async move {
                if let Err(err) = result {
                    let self_ = imp::Login::from_instance(&obj);
                    self_.welcome_page_error_label.set_text(&err.message);
                    self_.welcome_page_error_label.set_visible(true);

                    // grab focus for entry again after error
                    self_.custom_encryption_key_entry.grab_focus();
                }
            }),
        );
    }

    fn send_phone_number(&self) {
        let self_ = imp::Login::from_instance(self);
        let client_id = self_.client_id.get();
        let phone_number = self_.phone_number_entry.text().to_string();
        do_async(
            glib::PRIORITY_DEFAULT_IDLE,
            async move {
                functions::SetAuthenticationPhoneNumber::new()
                    .phone_number(phone_number)
                    .send(client_id)
                    .await
            },
            clone!(@weak self as obj => move |result| async move {
                if let Err(err) = result {
                    let self_ = imp::Login::from_instance(&obj);
                    self_.welcome_page_error_label.set_text(&err.message);
                    self_.welcome_page_error_label.set_visible(true);

                    obj.unfreeze();

                    // grab focus for entry again after error
                    self_.phone_number_entry.grab_focus();
                }
            }),
        );
    }

    fn send_code(&self) {
        let self_ = imp::Login::from_instance(self);
        let client_id = self_.client_id.get();
        let code = self_.code_entry.text().to_string();
        do_async(
            glib::PRIORITY_DEFAULT_IDLE,
            async move {
                functions::CheckAuthenticationCode::new()
                    .code(code)
                    .send(client_id)
                    .await
            },
            clone!(@weak self as obj => move |result| async move {
                if let Err(err) = result {
                    let self_ = imp::Login::from_instance(&obj);
                    self_.code_error_label.set_text(&err.message);
                    self_.code_error_label.set_visible(true);

                    obj.unfreeze();

                    // grab focus for entry again after error
                    self_.code_entry.grab_focus();
                }
            }),
        );
    }

    fn send_password(&self) {
        let self_ = imp::Login::from_instance(self);
        let client_id = self_.client_id.get();
        let password = self_.password_entry.text().to_string();
        do_async(
            glib::PRIORITY_DEFAULT_IDLE,
            async move {
                functions::CheckAuthenticationPassword::new()
                    .password(password)
                    .send(client_id)
                    .await
            },
            clone!(@weak self as obj => move |result| async move {
                if let Err(err) = result {
                    let self_ = imp::Login::from_instance(&obj);
                    self_.password_error_label.set_text(&err.message);
                    self_.password_error_label.set_visible(true);

                    obj.unfreeze();

                    // grab focus for entry again after error
                    self_.password_entry.grab_focus();
                }
            }),
        );
    }

    pub fn client_id(&self) -> i32 {
        let self_ = imp::Login::from_instance(self);
        self_.client_id.get()
    }

    pub fn connect_new_session<F: Fn(&Self) + 'static>(&self, f: F) -> glib::SignalHandlerId {
        self.connect_local("new-session", true, move |values| {
            let obj = values[0].get::<Self>().unwrap();
            f(&obj);

            None
        })
        .unwrap()
    }
}
