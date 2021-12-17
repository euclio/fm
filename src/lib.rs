use std::path::{Component, Path, PathBuf};

use log::*;
use relm4::factory::FactoryVec;
use relm4::gtk::prelude::*;
use relm4::{gtk, AppUpdate, Model, Sender, Widgets};

mod directory_list;

use directory_list::Directory;

pub struct AppModel {
    directories: FactoryVec<Directory>,
}

impl AppModel {
    pub fn new(root: &Path) -> AppModel {
        let mut model = AppModel {
            directories: FactoryVec::new(),
        };

        model.directories.push(Directory::new(&root));

        model
    }

    /// Returns the deepest directory that is listed (the rightmost listing).
    pub fn last_dir(&self) -> PathBuf {
        self.directories
            .as_slice()
            .last()
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
    type Components = ();
}

impl AppUpdate for AppModel {
    fn update(&mut self, msg: AppMsg, _components: &(), _sender: Sender<AppMsg>) -> bool {
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
                            self.directories.pop();
                            last_dir.pop();
                        }
                        Component::Normal(name) => {
                            let component_path = last_dir.join(name);
                            if component_path.is_dir() {
                                self.directories.push(Directory::new(&component_path));
                                last_dir = component_path;
                            }
                        }
                        _ => todo!(),
                    }
                }
            }
        }

        true
    }
}

#[relm4_macros::widget(pub)]
impl Widgets<AppModel, ()> for AppWidgets {
    view! {
        gtk::ApplicationWindow {
            set_title: Some("fm"),
            set_child = Some(&libpanel::Paned) {
                factory!(model.directories),
            }
        }
    }
}
