use std::path::PathBuf;

use gtk::prelude::*;
use relm::{Component, ContainerWidget, Widget};
use relm_derive::widget;

use super::DirectoryList;

pub struct Model {
    selected_dir: PathBuf,
    panes: Vec<Component<DirectoryList>>,
}

#[widget]
impl Widget for FilePanes {
    fn model(selected_dir: PathBuf) -> Model {
        Model {
            selected_dir,
            panes: vec![],
        }
    }

    fn init_view(&mut self) {
        let pane = self
            .root()
            .add_widget::<DirectoryList>(self.model.selected_dir.clone());
        self.model.panes.push(pane);
    }

    fn update(&mut self, _: ()) {}

    view! {
        dazzle::MultiPaned {
            orientation: gtk::Orientation::Horizontal,
        }
    }
}
