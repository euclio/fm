use std::path::{Component, Path, PathBuf};

use log::*;
use relm4::factory::FactoryVecDeque;
use relm4::gtk::prelude::*;
use relm4::{gtk, send, AppUpdate, Model, RelmComponent, Sender, Widgets};

mod directory_list;
mod file_preview;

use directory_list::Directory;
use file_preview::{FilePreviewModel, FilePreviewMsg};

#[derive(Debug)]
pub struct AppModel {
    directories: FactoryVecDeque<Directory>,
}

impl AppModel {
    pub fn new(root: &Path) -> AppModel {
        let mut model = AppModel {
            directories: FactoryVecDeque::new(),
        };

        model.directories.push_back(Directory::new(&root));

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
    NewSelection(PathBuf),
}

impl Model for AppModel {
    type Msg = AppMsg;
    type Widgets = AppWidgets;
    type Components = AppComponents;
}

impl AppUpdate for AppModel {
    fn update(&mut self, msg: AppMsg, components: &AppComponents, _sender: Sender<AppMsg>) -> bool {
        match msg {
            AppMsg::NewSelection(path) => {
                let mut last_dir = self.last_dir();

                let diff = pathdiff::diff_paths(&path, &last_dir)
                    .expect("new selection must be relative to the listed directories");

                info!(
                    "new selection: {:?}, last dir: {:?}, diff: {:?}",
                    path, last_dir, diff
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
                send!(file_preview, FilePreviewMsg::NewSelection(path.clone()));
            }
        }

        true
    }
}

#[derive(relm4_macros::Components)]
pub struct AppComponents {
    file_preview: RelmComponent<FilePreviewModel, AppModel>,
}

#[relm4_macros::widget(pub)]
impl Widgets<AppModel, ()> for AppWidgets {
    view! {
        gtk::ApplicationWindow {
            set_title: Some("fm"),
            set_child = Some(&libpanel::Paned) {
                append = &libpanel::Paned {
                    factory!(model.directories),
                    append: components.file_preview.root_widget(),
                },
            }
        }
    }
}
