use std::path::{Component, Path, PathBuf};

use glib::clone;
use gtk::{gio, glib, prelude::*};
use log::*;
use relm4::actions::{RelmAction, RelmActionGroup};
use relm4::factory::FactoryVecDeque;
use relm4::{gtk, send, AppUpdate, Model, RelmComponent, Sender, Widgets};
use relm4_components::ParentWindow;

mod alert;
mod directory_list;
mod file_preview;
mod places_sidebar;

use crate::alert::{AlertModel, AlertMsg};
use crate::directory_list::{
    Directory, DirectoryListRightClickActionGroup, OpenChooserAction, OpenDefaultAction, Selection,
    TrashFileAction,
};
use crate::file_preview::{FilePreviewModel, FilePreviewMsg};
use crate::places_sidebar::PlacesSidebarModel;

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
    /// Display an arbitrary error in an alert dialog.
    Error(Box<dyn std::error::Error>),

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
    NewSelection(Selection),
}

impl Model for AppModel {
    type Msg = AppMsg;
    type Widgets = AppWidgets;
    type Components = AppComponents;
}

impl AppUpdate for AppModel {
    fn update(&mut self, msg: AppMsg, components: &AppComponents, _sender: Sender<AppMsg>) -> bool {
        info!("received message: {:?}", msg);

        match msg {
            AppMsg::Error(err) => {
                let error_alert = &components.error_alert;
                send!(
                    error_alert,
                    AlertMsg::Show {
                        text: err.to_string()
                    }
                );
            }
            AppMsg::NewSelection(Selection::File(path)) => {
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
                send!(file_preview, FilePreviewMsg::NewSelection(path));
            }
            AppMsg::NewSelection(Selection::None) => {
                let file_preview = &components.file_preview;
                send!(file_preview, FilePreviewMsg::Hide);
            }
            AppMsg::NewRoot(new_root) => {
                info!("new root: {:?}", new_root);

                self.directories.clear();

                self.root = new_root;
                self.directories.push_back(Directory::new(&self.root));

                let file_preview = &components.file_preview;
                send!(file_preview, FilePreviewMsg::Hide);
            }
        }

        true
    }
}

#[derive(relm4_macros::Components)]
pub struct AppComponents {
    error_alert: RelmComponent<AlertModel, AppModel>,
    file_preview: RelmComponent<FilePreviewModel, AppModel>,
    places_sidebar: RelmComponent<PlacesSidebarModel, AppModel>,
}

#[relm4_macros::widget(pub)]
impl Widgets<AppModel, ()> for AppWidgets {
    view! {
        main_window = gtk::ApplicationWindow {
            set_title: Some("fm"),
            set_child = Some(&gtk::Paned) {
                set_start_child: components.places_sidebar.root_widget(),
                set_end_child = &panel::Paned {
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

    fn pre_view(&mut self) {
        // Handle the right-click actions from the directory entries.
        let group = RelmActionGroup::<DirectoryListRightClickActionGroup>::new();
        group.add_action(RelmAction::<OpenDefaultAction>::new_with_target_value(
            move |_, path: PathBuf| {
                let uri = gio::File::for_path(path).uri();
                let _ = gio::AppInfo::launch_default_for_uri(&uri, None::<&gio::AppLaunchContext>);
            },
        ));

        group.add_action(RelmAction::<OpenChooserAction>::new_with_target_value(
            clone!(@weak self.main_window as window =>
                move |_, path: PathBuf| {
                    choose_and_launch_app_for_path(&window, &path);
                }
            ),
        ));
        group.add_action(RelmAction::<TrashFileAction>::new_with_target_value(
            move |_, path: PathBuf| {
                let file = gio::File::for_path(&path);
                let _ = file.trash(None::<&gio::Cancellable>);
            },
        ));

        let actions = group.into_action_group();
        self.main_window
            .insert_action_group("dir-entry", Some(&actions));
    }
}

impl ParentWindow for AppWidgets {
    fn parent_window(&self) -> Option<gtk::Window> {
        Some(self.main_window.clone().upcast::<gtk::Window>())
    }
}

/// Creates a new [`gtk::AppChooserDialog`], shows it, and launches the selected application, if
/// any.
///
/// Ideally this would be done in a child component, but we're limited by GTK here. For a typical
/// dialog (see the [`alert`] module), we construct the dialog once and then modify its properties
/// to respond to model updates while it's hidden for efficiency. In this case,
/// `AppChooserDialog`'s `gfile` property is read-only, so we can't update the dialog after it's
/// been created, Furthermore, even if we work around this by creating a new dialog manually in the
/// view update, in a child component we don't have access to the parent widgets during the view
/// update. This prevents us from setting the transient parent for the dialog and triggers a GTK
/// warning. It's much easier to just handle everything in the `App` widget.
fn choose_and_launch_app_for_path(parent: &gtk::ApplicationWindow, path: &Path) {
    let file = gio::File::for_path(path);

    let dialog = gtk::AppChooserDialog::new(Some(parent), gtk::DialogFlags::MODAL, &file);

    dialog.connect_response(move |this, response| {
        if let gtk::ResponseType::Ok = response {
            if let Some(app_info) = this.app_info() {
                let _ = app_info.launch(&[file.clone()], None::<&gio::AppLaunchContext>);
            }
        }

        this.hide();
    });

    dialog.show();
}
