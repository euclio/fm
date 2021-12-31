use std::path::{Component, Path, PathBuf};
use std::rc::Rc;

use log::*;
use relm4::factory::{DynamicIndex, FactoryVecDeque};
use relm4::gtk::prelude::*;
use relm4::{gtk, send, AppUpdate, Model, RelmComponent, Sender, Widgets};

mod directory_list;
mod file_preview;
mod places_sidebar;

use directory_list::Directory;
use file_preview::{FilePreviewModel, FilePreviewMsg};
use places_sidebar::PlacesSidebarModel;

#[derive(Debug)]
pub struct AppModel {
    root: PathBuf,
    directories: FactoryVecDeque<Directory>,
}

impl AppModel {
    pub fn new(root: &Path) -> AppModel {
        let root = if !root.is_dir() {
            root.parent().unwrap_or(root)
        } else {
            root
        };

        let mut model = AppModel {
            root: root.to_owned(),
            directories: FactoryVecDeque::new(),
        };

        model.directories.push_back(Directory::new(root));

        model
    }

    /// Returns the deepest directory that is listed (the rightmost listing).
    pub fn last_dir(&self) -> PathBuf {
        self.directories
            .get(self.directories.len() - 1)
            .expect("there must be at least one directory listed")
            .dir()
    }
}

#[derive(Debug)]
pub enum AppMsg {
    /// The file root has changed. Existing directory trees are now invalid and must be popped off
    /// the stack.
    NewRoot(PathBuf),

    /// A new selection was made within the existing directory listings. This can result in a
    /// number of possible changes:
    ///
    /// - If the new selection is higher in the directory tree than the old selection, the lower
    /// listings must be removed.
    /// - If the new selection is a directory, a new directory listing is pushed onto the listing
    /// stack.
    /// - If the new selection is a file, the preview must be updated.
    NewSelection {
        /// The selected directory.
        selection: PathBuf,

        /// The directory that the selection originated from.
        src_dir: PathBuf,
    },

    /// Select the first entry in the next directory over from the current one. This is used when
    /// pressing the right arrow key on a directory in a directory list, because we don't know the
    /// path of the first entry in the next directory list. Pressing the left arrow key simply uses
    /// [`NewSelection`].
    NextDir(PathBuf),

    PrevDir(PathBuf),
}

impl Model for AppModel {
    type Msg = AppMsg;
    type Widgets = AppWidgets;
    type Components = AppComponents;
}

impl AppUpdate for AppModel {
    fn update(&mut self, msg: AppMsg, components: &AppComponents, _sender: Sender<AppMsg>) -> bool {
        match msg {
            AppMsg::NewSelection { selection, src_dir } => {
                let mut last_dir = self.last_dir();

                // if self.root.starts_with(&path) && path != self.root {
                //     // Tried to navigate above the root, ignore it.
                //     return true;
                // }

                let diff = pathdiff::diff_paths(&selection, &last_dir)
                    .expect("new selection must be relative to the listed directories");

                info!(
                    "new selection: {:?}, last dir: {:?}, diff: {:?}",
                    selection, last_dir, diff
                );

                for component in diff.components() {
                    match component {
                        Component::ParentDir => {
                            self.directories.pop_back();
                            last_dir.pop();
                        }
                        Component::Normal(name) => {
                            let component_path = last_dir.join(name);
                            if component_path.is_dir() {
                                self.directories.push_back(Directory::new(&component_path));
                                last_dir = component_path;
                            }
                        }
                        _ => unreachable!("unexpected path component: {:?}", component),
                    }
                }

                let file_preview = &components.file_preview;
                send!(file_preview, FilePreviewMsg::NewSelection(selection));
            }
            AppMsg::NewRoot(new_root) => {
                info!("new root: {:?}", new_root);

                self.directories.clear();

                self.root = new_root;
                self.directories.push_back(Directory::new(&self.root));
            }
            AppMsg::NextDir(_selected_dir) => {
                if let Some(next_dir_widget) = self.directories.get_mut(self.directories.len() - 1)
                {
                    // Select the first entry in the listing.
                    next_dir_widget.model.set_selected(0);
                }
            }
            AppMsg::PrevDir(dir) => {
                // Fake a mutation so that the previous widget takes the focus.
                let _foo = self.directories.get_mut(self.directories.len() - 1);

                // Fake a mutation so that the previous widget takes the focus.
                let _bar = self.directories.get_mut(self.directories.len() - 2);
            }
        }

        true
    }
}

#[derive(relm4_macros::Components)]
pub struct AppComponents {
    file_preview: RelmComponent<FilePreviewModel, AppModel>,
    places_sidebar: RelmComponent<PlacesSidebarModel, AppModel>,
}

#[relm4_macros::widget(pub)]
impl Widgets<AppModel, ()> for AppWidgets {
    view! {
        gtk::ApplicationWindow {
            set_title: Some("fm"),
            set_child = Some(&gtk::Paned) {
                set_start_child: components.places_sidebar.root_widget(),
                set_end_child = &libpanel::Paned {
                    factory!(model.directories),
                    append: components.file_preview.root_widget(),
                },
                set_resize_end_child: true,
                set_resize_start_child: false,
                set_shrink_end_child: false,
                set_shrink_start_child: false,
            }
        }
    }
}
