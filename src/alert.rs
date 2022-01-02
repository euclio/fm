//! Alert dialog for displaying arbitrary errors.
//!
//! Inspired by [`relm4_components::alert`], but allows sending the dialog text as part of the
//! `Show` message, and supports displaying only a single button to dismiss.

use relm4::gtk::{self, prelude::*};
use relm4::{send, ComponentUpdate, Model, Sender, Widgets};
use relm4_components::ParentWindow;

use super::AppModel;

pub struct AlertModel {
    is_active: bool,
    text: String,
}

pub enum AlertMsg {
    Show { text: String },
    Response(gtk::ResponseType),
}

impl Model for AlertModel {
    type Msg = AlertMsg;
    type Widgets = AlertWidgets;
    type Components = ();
}

impl ComponentUpdate<AppModel> for AlertModel {
    fn init_model(_parent_model: &AppModel) -> Self {
        AlertModel {
            is_active: false,
            text: String::default(),
        }
    }

    fn update(
        &mut self,
        msg: AlertMsg,
        _components: &(),
        _sender: Sender<AlertMsg>,
        _parent_sender: Sender<<AppModel as Model>::Msg>,
    ) {
        match msg {
            AlertMsg::Show { text: message } => {
                self.is_active = true;
                self.text = message;
            }
            AlertMsg::Response(_) => {
                self.is_active = false;
            }
        }
    }
}

#[relm4_macros::widget(pub)]
impl Widgets<AlertModel, AppModel> for AlertWidgets {
    view! {
        dialog = gtk::MessageDialog {
            set_transient_for: parent!(parent_widgets.parent_window().as_ref()),
            set_message_type: gtk::MessageType::Error,
            set_visible: watch!(model.is_active),
            connect_response(sender) => move |_, response| {
                send!(sender, AlertMsg::Response(response));
            },
            set_text: Some("Something went wrong"),
            set_secondary_text: watch!(Some(&model.text)),
            set_modal: true,
            add_button: args!("OK", gtk::ResponseType::Accept),
        }
    }
}
