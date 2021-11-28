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

                        self.root().set_opacity(1.0);
                    }
                    None => {
                        // If the selection is a directory, hide the contents of the file preview.
                        // Set opacity instead of visibility to preserve the space allocated
                        // to the preview widget.
                        self.root().set_opacity(0.0);
                    }
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
