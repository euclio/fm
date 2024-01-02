//! Dialog component for mounting a new mountable.

use std::time::Duration;

use adw::prelude::*;
use futures::prelude::*;
use futures::select;
use futures::stream::{AbortHandle, Abortable, Aborted};
use gtk::{gio, glib};
use relm4::prelude::*;

use super::app::AppMsg;
use crate::util::GResultExt;

/// The duration between progress pulses of the URI entry while a mount operation is underway.
const PROGRESS_PULSE_DURATION: Duration = Duration::from_millis(100);

#[derive(Debug)]
pub struct Mount {
    uri_buffer: gtk::EntryBuffer,
    visible: bool,
    abort_handle: Option<AbortHandle>,
}

#[derive(Debug)]
pub enum MountMsg {
    /// Mount a new mountable.
    Mount,

    /// User clicked an action.
    Response(gtk::ResponseType),

    /// Close the dialog.
    Close,

    /// Pulse the progress indicator.
    Pulse,

    /// Abort any in-progress mount operation and reset the progress indicator.
    Finish,
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
            uri_buffer: gtk::EntryBuffer::default(),
            visible: false,
            abort_handle: None,
        };

        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update_with_view(
        &mut self,
        widgets: &mut Self::Widgets,
        msg: Self::Input,
        sender: ComponentSender<Self>,
        root: &Self::Root,
    ) {
        match msg {
            MountMsg::Mount => {
                self.visible = true;
            }
            MountMsg::Response(gtk::ResponseType::Accept) => {
                let uri_file = gio::File::for_uri(&self.uri_buffer.text());
                let mount_operation =
                    gtk::MountOperation::new(Some(root.upcast_ref::<gtk::Window>()));

                let mount_fut = uri_file.mount_enclosing_volume_future(
                    gio::MountMountFlags::NONE,
                    Some(&mount_operation),
                );

                let (abort_handle, abort_registration) = AbortHandle::new_pair();
                self.abort_handle.replace(abort_handle);
                let mut mount_fut = Abortable::new(mount_fut, abort_registration).fuse();

                widgets.uri_entry.set_progress_fraction(0.1);
                widgets.uri_entry.progress_pulse();

                let sender = sender.clone();
                relm4::spawn_local(async move {
                    loop {
                        select! {
                            _ = glib::timeout_future(PROGRESS_PULSE_DURATION).fuse() => {
                                sender.input(MountMsg::Pulse);
                            }
                            res = mount_fut => {
                                let res = res.map(|r| r.filter_handled());

                                match res {
                                    Ok(Ok(_)) | Err(Aborted) => sender.input(MountMsg::Close),
                                    Ok(Err(e)) => {
                                        sender.input(MountMsg::Finish);
                                        sender.output(AppMsg::Error(Box::new(e))).unwrap();
                                    }
                                }

                                break;
                            }
                        }
                    }
                });
            }
            MountMsg::Response(gtk::ResponseType::Cancel) => {
                sender.input(MountMsg::Finish);
                self.visible = false;
            }
            MountMsg::Close => {
                sender.input(MountMsg::Finish);
                self.visible = false;
            }
            MountMsg::Finish => {
                widgets.uri_entry.set_progress_fraction(0.0);

                if let Some(handle) = self.abort_handle.take() {
                    handle.abort();
                }
            }
            MountMsg::Pulse => widgets.uri_entry.progress_pulse(),
            _ => (),
        }

        self.update_view(widgets, sender);
    }
}
