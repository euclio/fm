use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

use gtk::prelude::*;
use log::*;
use mime_guess::mime::{self, Mime};
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

                        let block = read_block(&selection.item).unwrap_or_default();
                        let (mime, uncertain) = gio::content_type_guess(
                            Some(&selection.item.to_string_lossy()),
                            &block,
                        );
                        info!("guessed mime: {}, uncertain: {}", mime, uncertain);

                        let mime = mime
                            .parse::<Mime>()
                            .expect("could not parse guessed mime type");

                        match (mime.type_(), mime.subtype()) {
                            (mime::IMAGE, _) => self.widgets.image.set_from_file(&selection.item),
                            _ => self.widgets.image.set_from_gicon(
                                &gio::content_type_get_icon(mime.essence_str()),
                                gtk::IconSize::Dialog,
                            ),
                        }

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

/// Reads the first block of a file.
fn read_block(path: &Path) -> io::Result<Vec<u8>> {
    let mut f = File::open(path)?;

    let mut buf = vec![0; 4096];
    f.read_exact(&mut buf)?;

    Ok(buf)
}
