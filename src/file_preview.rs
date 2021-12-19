use std::path::PathBuf;

use relm4::gtk::prelude::*;
use relm4::{gtk, ComponentUpdate, Sender, Widgets};

use super::{AppModel, AppMsg, Model};

pub struct FilePreviewModel {
    path: Option<PathBuf>,
    name: Option<String>,
    icon: Option<gio::Icon>,
}

impl Model for FilePreviewModel {
    type Msg = FilePreviewMsg;
    type Widgets = FilePreviewWidgets;
    type Components = ();
}

impl ComponentUpdate<AppModel> for FilePreviewModel {
    fn init_model(_parent_model: &AppModel) -> Self {
        FilePreviewModel {
            name: None,
            path: None,
            icon: None,
        }
    }

    fn update(
        &mut self,
        msg: FilePreviewMsg,
        _components: &(),
        _sender: Sender<FilePreviewMsg>,
        _parent_sender: Sender<AppMsg>,
    ) {
        match msg {
            FilePreviewMsg::NewSelection(path) if path.is_dir() => {
                self.path = None;
                self.name = None;
                self.icon = None;
            }
            FilePreviewMsg::NewSelection(path) => {
                self.name = path.file_name().map(|name| name.to_string_lossy().to_string());

                let (content_type, uncertain) = gio::content_type_guess(
                    Some(&path.to_string_lossy()),
                    &[],
                );
                let mime = gio::content_type_get_mime_type(&content_type).unwrap();

                self.icon = Some(gio::content_type_get_icon(&content_type));

                self.path = Some(path);
            }
        }
    }
}

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
            append = &gtk::Image {
                set_visible: watch! { model.name.is_some() },
                set_file: watch! { model.path.as_ref().map(|p| p.to_string_lossy()).as_deref() },
                set_gicon: watch! { model.icon.as_ref() },
                set_icon_size: gtk::IconSize::Large,
            },
            append = &gtk::ScrolledWindow {
                set_visible: false,

                set_child = Some(&gtk::TextView) {
                    set_editable: false,
                }
            },
            append = &gtk::Label {
                set_label: watch! { model.name.as_deref().unwrap_or("") },
            }
        }
    }
}
