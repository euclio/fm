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
    }
}

pub enum FilePreviewMsg {}

#[relm4_macros::widget(pub)]
impl Widgets<FilePreviewModel, AppModel> for FilePreviewWidgets {
    view! {
        gtk::Box {
            append = &gtk::Label {
                set_label: "Foo Bar"
            }
        }
    }
}
