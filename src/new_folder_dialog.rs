use gtk::prelude::*;
use gtk::{gio, glib};
use relm4::prelude::*;

use crate::alert::AlertMsg;
use crate::ERROR_BROKER;

#[derive(Debug)]
pub struct NewFolderDialog {
    parent: gio::File,
    entry_buffer: gtk::EntryBuffer,
    visible: bool,
}

#[derive(Debug)]
pub enum NewFolderDialogMsg {
    Show,
    Response(gtk::ResponseType),
    Hide,
}

#[relm4::component(pub)]
impl SimpleComponent for NewFolderDialog {
    type Init = gio::File;
    type Input = NewFolderDialogMsg;
    type Output = ();

    view! {
        dialog = gtk::Dialog::builder()
            .title("New Folder")
            .use_header_bar(gtk::Settings::default().unwrap().is_gtk_dialogs_use_header() as i32)
            .build() {

            #[chain(add_css_class("suggested-action"))]
            add_button: ("Create", gtk::ResponseType::Accept),
            add_button: ("Cancel", gtk::ResponseType::Cancel),

            #[watch]
            set_visible: model.visible,
            set_modal: true,

            gtk::Box {
                set_margin_all: 5,

                gtk::Entry {
                    set_buffer: &model.entry_buffer,
                    set_hexpand: true,
                },
            },

            connect_response[sender] => move |_, response| {
                sender.input(NewFolderDialogMsg::Response(response));
            },

            connect_close_request[sender] => move |_| {
                sender.input(NewFolderDialogMsg::Hide);
                gtk::Inhibit(true)
            },
        }
    }

    fn init(
        parent: gio::File,
        root: &Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = NewFolderDialog {
            parent,
            entry_buffer: gtk::EntryBuffer::default(),
            visible: false,
        };
        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, _: ComponentSender<Self>) {
        match msg {
            NewFolderDialogMsg::Show => self.visible = true,
            NewFolderDialogMsg::Hide | NewFolderDialogMsg::Response(gtk::ResponseType::Cancel) => {
                self.visible = false
            }
            NewFolderDialogMsg::Response(gtk::ResponseType::Accept) => {
                let child = match self
                    .parent
                    .child_for_display_name(&self.entry_buffer.text())
                {
                    Ok(child) => child,
                    Err(e) => {
                        ERROR_BROKER.send(AlertMsg::Show {
                            text: e.to_string(),
                        });
                        return;
                    }
                };

                relm4::spawn_local(async move {
                    if let Err(e) = child.make_directory_future(glib::Priority::default()).await {
                        ERROR_BROKER.send(AlertMsg::Show {
                            text: e.to_string(),
                        });
                    }
                });

                self.visible = false;
            }
            _ => (),
        }
    }
}
