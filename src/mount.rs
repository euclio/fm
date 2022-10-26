//! Dialog component for mounting a new mountable.

use adw::prelude::*;
use gtk::prelude::*;
use relm4::prelude::*;

#[derive(Debug)]
pub struct Mount {
    visible: bool,
}

#[derive(Debug)]
pub enum MountMsg {
    /// Mount a new mountable.
    Mount,

    /// User clicked an action.
    Response(gtk::ResponseType),

    /// Close the dialog.
    Close,
}

#[relm4::component(pub)]
impl SimpleComponent for Mount {
    type Init = ();
    type Input = MountMsg;
    type Output = ();
    type Widgets = MountWidgets;

    view! {
        dialog = gtk::Dialog::builder()
            .title("Connect to remote folder")
            .use_header_bar(gtk::Settings::default().unwrap().is_gtk_dialogs_use_header() as i32)
            .build() {

            #[watch]
            set_visible: model.visible,

            #[chain(add_css_class("suggested-action"))]
            add_button: ("Connect", gtk::ResponseType::Accept),

            add_button: ("Cancel", gtk::ResponseType::Cancel),

            gtk::ListBox {
                add_css_class: "boxed-list",
                set_selection_mode: gtk::SelectionMode::None,

                adw::EntryRow {
                    set_title: "URI",
                },

                adw::EntryRow {
                    set_title: "User",
                },

                adw::PasswordEntryRow {
                    set_title: "Password",
                },
            },

            connect_response[sender] => move |_, response| {
                sender.input(MountMsg::Response(response));
            },

            connect_close_request[sender] => move |_| {
                sender.input(MountMsg::Close);
                gtk::Inhibit(true)
            },
        }
    }

    fn init(init: (), root: &Self::Root, sender: ComponentSender<Self>) -> ComponentParts<Self> {
        let model = Mount {
            visible: false,
        };

        let accept_button = gtk::Button::new();

        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            MountMsg::Mount => {
                self.visible = true;
            },
            MountMsg::Response(response) if response == gtk::ResponseType::Cancel => {
                self.visible = false;
            },
            MountMsg::Close => {
                self.visible = false;
            },
            _ => (),
        }
    }
}
