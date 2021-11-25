use gtk::prelude::*;
use log::*;
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
                info!("new selection: {:?}", selection);

                self.model.selection = selection;

                match &self.model.selection {
                    Some(selection) => {
                        self.widgets
                            .file_name
                            .set_text(&selection.item.file_name().unwrap().to_string_lossy());
                        self.widgets.image.set_from_file(&selection.item);

                        self.root().set_visible(true);
                    }
                    None => self.root().set_visible(false),
                }
            }
        }
    }

    view! {
        gtk::Box {
            halign: gtk::Align::Fill,
            hexpand: true,
            orientation: gtk::Orientation::Vertical,
            #[name = "image"]
            gtk::Image {},

            #[name = "file_name"]
            gtk::Label {
                justify: gtk::Justification::Center,
            },
        }
    }
}
