use relm::Widget;
use relm_derive::{widget, Msg};

use super::Selection;

pub struct Model {
    selection: Option<Selection>,
}

#[derive(Msg)]
pub enum Msg {
    NewSelection(Option<Selection>),
}

#[widget]
impl Widget for FilePreview {
    fn model() -> Model {
        Model { selection: None }
    }

    fn update(&mut self, event: Msg) {
        match event {
            Msg::NewSelection(selection) => {
                self.model.selection = selection;
            }
        }
    }

    view! {
        gtk::Box {}
    }
}
