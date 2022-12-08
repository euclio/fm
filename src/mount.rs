//! Dialog component for mounting a new mountable.

use adw::prelude::*;
use gtk::gio;
use relm4::prelude::*;

use crate::AppMsg;

#[derive(Debug)]
pub struct Mount {
    uri_buffer: gtk::EntryBuffer,
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
impl Component for Mount {
    type Init = ();
    type Input = MountMsg;
    type Output = AppMsg;
    type CommandOutput = ();
    type Widgets = MountWidgets;

    view! {
        gtk::Dialog::builder()
            .title("Connect to Server")
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
                set_margin_all: 5,

                #[name = "uri_entry"]
                gtk::Entry {
                    set_placeholder_text: Some("Enter server address..."),
                    set_buffer: &model.uri_buffer,
                    set_width_chars: 50,

                    connect_activate => MountMsg::Response(gtk::ResponseType::Accept),
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

    fn init(_: (), root: &Self::Root, sender: ComponentSender<Self>) -> ComponentParts<Self> {
        let model = Mount {
            uri_buffer: gtk::EntryBuffer::new(None),
            visible: false,
        };

        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>, root: &Self::Root) {
        match msg {
            MountMsg::Mount => {
                self.visible = true;
            }
            MountMsg::Response(gtk::ResponseType::Accept) => {
                let uri_file = gio::File::for_uri(&self.uri_buffer.text());
                let mount_operation =
                    gtk::MountOperation::new(Some(root.upcast_ref::<gtk::Window>()));

                relm4::spawn_local(async move {
                    match uri_file
                        .mount_enclosing_volume_future(
                            gio::MountMountFlags::NONE,
                            Some(&mount_operation),
                        )
                        .await
                    {
                        Ok(_) => (),
                        Err(e) => sender.output(AppMsg::Error(Box::new(e))).unwrap(),
                    }
                });

                self.visible = false;
            }
            MountMsg::Response(gtk::ResponseType::Cancel) => {
                self.visible = false;
            }
            MountMsg::Close => {
                self.visible = false;
            }
            _ => (),
        }
    }
}
