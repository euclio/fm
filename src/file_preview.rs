use std::path::PathBuf;

use log::*;
use mime::Mime;
use relm4::gtk::prelude::*;
use relm4::{gtk, ComponentUpdate, Sender, Widgets};

use super::{AppModel, AppMsg, Model};

#[derive(Debug)]
enum FilePreview {
    Image(PathBuf),
    Icon(gio::Icon),
}

#[derive(Debug)]
struct FileInfo {
    path: PathBuf,
    preview: FilePreview,
}

#[derive(Debug)]
pub struct FilePreviewModel {
    file: Option<FileInfo>,
}

impl Model for FilePreviewModel {
    type Msg = FilePreviewMsg;
    type Widgets = FilePreviewWidgets;
    type Components = ();
}

impl ComponentUpdate<AppModel> for FilePreviewModel {
    fn init_model(_parent_model: &AppModel) -> Self {
        FilePreviewModel { file: None }
    }

    fn update(
        &mut self,
        msg: FilePreviewMsg,
        _components: &(),
        _sender: Sender<FilePreviewMsg>,
        _parent_sender: Sender<AppMsg>,
    ) {
        info!("received message: {:?}", msg);

        self.file = match msg {
            FilePreviewMsg::NewSelection(path) if path.is_dir() => None,
            FilePreviewMsg::NewSelection(path) => {
                let (content_type, uncertain) =
                    gio::content_type_guess(Some(&path.to_string_lossy()), &[]);

                let mime = gio::content_type_get_mime_type(&content_type)
                    .expect("unable to determine mime type")
                    .parse::<Mime>()
                    .expect("could not parse guessed mime type");

                info!("identified file as {}, uncertain: {}", mime, uncertain);

                let preview = match (mime.type_(), mime.subtype()) {
                    (mime::IMAGE, _) => FilePreview::Image(path.clone()),
                    _ => FilePreview::Icon(gio::content_type_get_icon(&content_type)),
                };

                Some(FileInfo { path, preview })
            }
        }
    }
}

#[derive(Debug)]
pub enum FilePreviewMsg {
    NewSelection(PathBuf),
}

#[relm4_macros::widget(pub)]
impl Widgets<FilePreviewModel, AppModel> for FilePreviewWidgets {
    view! {
        gtk::Box {
            set_baseline_position: gtk::BaselinePosition::Center,
            set_orientation: gtk::Orientation::Vertical,
            set_valign: gtk::Align::Center,
            set_visible: watch! { model.file.is_some() },
            append: image = &gtk::Image {
                set_icon_size: gtk::IconSize::Large,
            },
            append = &gtk::ScrolledWindow {
                set_visible: false,

                set_child = Some(&gtk::TextView) {
                    set_editable: false,
                }
            },
            append = &gtk::Label {
                set_label?: watch! {
                    &model.file.as_ref()
                        .and_then(|file| file.path.file_name())
                        .map(|name| name.to_string_lossy())
                },
            }
        }
    }

    fn manual_view(&self) {
        let file = match &model.file {
            Some(file) => file,
            None => {
                self.image.set_visible(false);
                return;
            }
        };

        self.image.set_visible(true);

        match &file.preview {
            FilePreview::Image(path) => {
                self.image.set_file(Some(&path.to_string_lossy()));
            }
            FilePreview::Icon(icon) => {
                self.image.set_gicon(Some(icon));
            }
        }
    }
}
