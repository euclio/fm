//! Alert dialog for displaying arbitrary errors.
//!
//! Inspired by [`relm4_components::alert`], but allows sending the dialog text as part of the
//! `Show` message, and supports displaying only a single button to dismiss.

use gtk::prelude::*;
use relm4::prelude::*;
use relm4::MessageBroker;

pub static ERROR_BROKER: MessageBroker<AlertModel> = MessageBroker::new();

#[derive(Debug)]
pub struct AlertModel {
    is_active: bool,
    text: String,
}

#[derive(Debug)]
pub enum AlertMsg {
    Show { text: String },
    Response(gtk::ResponseType),
}

#[relm4::component(pub)]
impl SimpleComponent for AlertModel {
    type Widgets = AlertWidgets;
    type Init = ();
    type Input = AlertMsg;
    type Output = ();

    view! {
        dialog = gtk::MessageDialog {
            set_message_type: gtk::MessageType::Error,
            #[watch]
            set_visible: model.is_active,
            connect_response[sender] => move |_, response| {
                sender.input(AlertMsg::Response(response));
            },
            set_text: Some("Something went wrong"),
            #[watch]
            set_secondary_text: Some(&model.text),
            set_modal: true,
            add_button: ("OK", gtk::ResponseType::Accept),
        }
    }

    fn init(_: (), root: &Self::Root, sender: ComponentSender<Self>) -> ComponentParts<Self> {
        let model = AlertModel {
            is_active: false,
            text: String::default(),
        };

        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, input: Self::Input, _sender: ComponentSender<Self>) {
        match input {
            AlertMsg::Show { text } => {
                self.text = text;
                self.is_active = true;
            }
            AlertMsg::Response(_) => {
                self.is_active = false;
            }
        }
    }
}
