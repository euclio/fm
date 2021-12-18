use std::path::PathBuf;

use relm4::gtk::prelude::*;
use relm4::{gtk, ComponentUpdate, Sender, Widgets};

use super::{AppModel, AppMsg, Model};

pub struct FilePreviewModel {
    file: Option<PathBuf>,
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
        parent_sender: Sender<AppMsg>,
    ) {
        match msg {
            FilePreviewMsg::NewSelection(path) => {
                self.file = Some(path);
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
            append = &gtk::Label {
                set_label: watch! {
                    &model.file.as_ref()
                        .and_then(|f| f.file_name())
                        .map(|name| name.to_string_lossy())
                        .unwrap_or_else(Default::default)
                }
            }
        }
    }
}
