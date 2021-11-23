use std::path::{self, PathBuf};

use gtk::prelude::*;
use log::*;
use relm::{connect, Component, ContainerWidget, Relm, Widget};
use relm_derive::{widget, Msg};

use super::{DirectoryList, FilePreview};

/// Directory listing entry that will be displayed in the file preview.
#[derive(Debug, Clone)]
pub struct Selection {
    /// The selected item.
    selection: PathBuf,
}

pub struct Model {
    /// The root directory that the file panes are relative to.
    root: PathBuf,

    /// The directory listed in the rightmost pane.
    current_dir: PathBuf,

    /// Selected file. If `None`, the file panes will only contain a listing of the root directory.
    selection: Option<Selection>,

    /// Child widgets containing directory listings for each component of the selection.
    panes: Vec<Component<DirectoryList>>,

    relm: Relm<FilePanes>,
}

#[derive(Msg, Clone)]
pub enum Msg {
    NewRoot(PathBuf),
    NewEntrySelected(PathBuf),
}

/// Multi-pane widget that contains hierarchical directory listings, from left to right.
#[widget]
impl Widget for FilePanes {
    fn model(relm: &Relm<Self>, selected_dir: PathBuf) -> Model {
        assert!(selected_dir.is_absolute());

        Model {
            relm: relm.clone(),
            root: selected_dir.clone(),
            current_dir: selected_dir.clone(),
            selection: None,
            panes: vec![],
        }
    }

    fn init_view(&mut self) {
        self.push_directory(self.model.root.clone());
    }

    fn update(&mut self, event: Msg) {
        match event {
            Msg::NewRoot(root) => {
                info!("new file root: {}", root.display());

                while let Some(pane) = self.model.panes.pop() {
                    self.root().remove_widget(pane);
                }

                self.model.root = root;

                self.init_view();
            }
            Msg::NewEntrySelected(entry) => self.handle_new_selection(entry),
        }
    }

    fn handle_new_selection(&mut self, new_entry: PathBuf) {
        info!(
            "new selection: {:?}, current dir: {:?}",
            new_entry, self.model.current_dir
        );

        // Remove panes for uncommon parent directories.
        let diff = pathdiff::diff_paths(&new_entry, &self.model.current_dir)
            .expect("selection must be relative to the current directory");

        info!("selection diff: {:?}", diff);

        for component in diff.components() {
            match component {
                path::Component::ParentDir => self.pop_directory(),
                path::Component::Normal(name) => {
                    let component_path = self.model.current_dir.join(name);
                    if component_path.is_dir() {
                        self.push_directory(component_path);
                    }
                }
                _ => unreachable!("unexpected component: {:?}", component),
            }
        }

        self.model.selection = Some(Selection {
            selection: new_entry,
        });

        self.components
            .preview
            .emit(super::file_preview::Msg::NewSelection(
                self.model.selection.clone(),
            ));
    }

    fn push_directory(&mut self, dir: PathBuf) {
        use dazzle::traits::MultiPanedExt;

        assert!(dir.is_dir());

        self.model.current_dir = dir;

        let new_directory_list = self
            .root()
            .add_widget::<DirectoryList>(self.model.current_dir.clone());

        let n_children = self.root().n_children() as i32;
        self.root()
            .set_child_index(&self.widgets.preview, n_children);

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
        self.model.current_dir.pop();
        let directory_list = self.model.panes.pop().expect("no directories in the stack");
        self.root().remove_widget(directory_list);
    }

    view! {
        dazzle::MultiPaned {
            halign: gtk::Align::Start, // FIXME: This causes the resize grips to stop working.
            orientation: gtk::Orientation::Horizontal,

            #[name="preview"]
            FilePreview(),
        }
    }
}
