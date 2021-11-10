use std::path::{self, PathBuf};

use gtk::prelude::*;
use log::*;
use relm::{connect, Component, ContainerWidget, Relm, Widget};
use relm_derive::{widget, Msg};

use super::DirectoryList;

pub struct Model {
    selected_dir: PathBuf,
    panes: Vec<Component<DirectoryList>>,
    relm: Relm<FilePanes>,
}

#[derive(Msg, Clone)]
pub enum Msg {
    NewEntrySelected(PathBuf),
}

/// Multi-pane widget that contains hierarchical directory listings, from left to right.
#[widget]
impl Widget for FilePanes {
    fn model(relm: &Relm<Self>, selected_dir: PathBuf) -> Model {
        assert!(selected_dir.is_absolute());

        Model {
            relm: relm.clone(),
            selected_dir,
            panes: vec![],
        }
    }

    fn init_view(&mut self) {
        self.push_directory(self.model.selected_dir.clone());
    }

    fn update(&mut self, event: Msg) {
        match event {
            Msg::NewEntrySelected(entry) => self.handle_new_selection(entry),
        }
    }

    fn handle_new_selection(&mut self, new_entry: PathBuf) {
        let diff = pathdiff::diff_paths(&new_entry, &self.model.selected_dir);
        info!(
            "current directory: {}, new_directory: {}, diff: {:?}",
            self.model.selected_dir.display(),
            new_entry.display(),
            diff.as_ref().map(|diff| diff.display())
        );

        match diff {
            Some(relative_path) => {
                for component in relative_path.components() {
                    match component {
                        path::Component::Normal(name) => {
                            let component = self.model.selected_dir.join(name);
                            if component.is_dir() {
                                self.model.selected_dir = component;
                                self.push_directory(self.model.selected_dir.clone());
                            }
                        }
                        path::Component::ParentDir => {
                            self.pop_directory();
                            self.model.selected_dir.pop();
                        }
                        _ => todo!(),
                    }
                }
            }
            None => todo!(),
        }
    }

    fn push_directory(&mut self, dir: PathBuf) {
        use dazzle::traits::MultiPanedExt;

        assert!(dir.is_dir());

        let new_directory_list = self.root().add_widget::<DirectoryList>(dir);

        info!(
            "added new directory pane ({} pane(s) total)",
            self.root().n_children()
        );

        use super::directory_list::Msg::*;

        connect!(
            new_directory_list@NewEntrySelected(ref entry),
            self.model.relm,
            Msg::NewEntrySelected(entry.clone())
        );

        self.model.panes.push(new_directory_list);
    }

    fn pop_directory(&mut self) {
        let directory_list = self.model.panes.pop().expect("no directories in the stack");
        self.root().remove_widget(directory_list);
    }

    view! {
        dazzle::MultiPaned {
            halign: gtk::Align::Start, // FIXME: This causes the resize grips to stop working.
            orientation: gtk::Orientation::Horizontal,
        }
    }
}
