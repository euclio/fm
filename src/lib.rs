//! Small, general purpose file manager built using GTK.
//!
//! Generally, each top-level module corresponds to a different Relm4 component.

#![warn(clippy::dbg_macro)]
#![warn(clippy::print_stderr)]
#![warn(clippy::print_stdout)]
#![warn(clippy::todo)]

use std::path::{Component, Path, PathBuf};

use gtk::{gio, prelude::*};
use log::*;
use relm4::factory::FactoryVecDeque;
use relm4::{gtk, send, AppUpdate, Model, RelmComponent, Sender, Widgets};
use relm4_components::ParentWindow;

mod alert;
mod config;
mod directory_list;
mod file_preview;
mod places_sidebar;
mod util;

use crate::alert::{AlertModel, AlertMsg};
use crate::config::State;
use crate::directory_list::{Directory, Selection};
use crate::file_preview::{FilePreviewModel, FilePreviewMsg};
use crate::places_sidebar::PlacesSidebarModel;

#[derive(Debug)]
pub struct AppModel {
    root: PathBuf,
    directories: FactoryVecDeque<Directory>,

    /// Whether the directory panes scroll window should update its scroll position to the upper
    /// bound on the next view update.
    update_directory_scroll_position: bool,

    /// Open the app chooser to open the given path on the next view update.
    open_app_for_path: Option<PathBuf>,

    state: State,
}

impl AppModel {
    pub fn new(root: &Path) -> AppModel {
        let root = if !root.is_dir() {
            root.parent().unwrap_or(root)
        } else {
            root
        };

        let state = State::read()
            .map_err(|e| {
                warn!("unable to read application state: {}", e);
                e
            })
            .unwrap_or_default();

        info!("starting with application state: {:?}", state);

        let mut model = AppModel {
            root: root.to_owned(),
            directories: FactoryVecDeque::new(),
            update_directory_scroll_position: false,
            open_app_for_path: None,
            state,
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

    /// Trigger the application chooser to pick an application to open the given file path.
    ChooseAndLaunchApp(PathBuf),
}

impl Model for AppModel {
    type Msg = AppMsg;
    type Widgets = AppWidgets;
    type Components = AppComponents;
}

impl AppUpdate for AppModel {
    fn update(&mut self, msg: AppMsg, components: &AppComponents, _sender: Sender<AppMsg>) -> bool {
        info!("received message: {:?}", msg);

        self.open_app_for_path = None;
        self.update_directory_scroll_position = false;

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

                self.update_directory_scroll_position = true;
            }
            AppMsg::NewSelection(Selection::None) => {
                let file_preview = &components.file_preview;
                send!(file_preview, FilePreviewMsg::Hide);

                self.update_directory_scroll_position = true;
            }
            AppMsg::NewRoot(new_root) => {
                info!("new root: {:?}", new_root);

                self.directories.clear();

                self.root = new_root;
                self.directories.push_back(Directory::new(&self.root));

                let file_preview = &components.file_preview;
                send!(file_preview, FilePreviewMsg::Hide);

                self.update_directory_scroll_position = true;
            }
            AppMsg::ChooseAndLaunchApp(path) => self.open_app_for_path = Some(path),
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
            set_default_size: args!(model.state.width, model.state.height),
            set_title: Some("fm"),
            set_child = Some(&gtk::Paned) {
                set_start_child: components.places_sidebar.root_widget(),
                set_end_child: directory_panes_scroller = &gtk::ScrolledWindow {
                    set_child = Some(&panel::Paned) {
                        factory!(model.directories),
                        append: components.file_preview.root_widget(),
                    },
                },
                set_resize_end_child: true,
                set_resize_start_child: false,
                set_shrink_end_child: false,
                set_shrink_start_child: false,
            },

            connect_close_request => move |this| {
                let (width, height) = this.default_size();
                let is_maximized = this.is_maximized();

                let new_state = State {
                    width,
                    height,
                    is_maximized,
                };

                if let Err(e) = new_state.write() {
                    warn!("unable to write application state: {}", e);
                }

                gtk::Inhibit(false)
            }
        }
    }

    fn post_init() {
        // TODO: There's sometimes a delay in updating the adjustment upper bound when a new pane
        // is added, causing this code to not trigger at the right time. Needs more investigation.
        directory_panes_scroller
            .hadjustment()
            .connect_notify(Some("upper"), |this, _| {
                set_adjustment_to_upper_bound(this);
            });
    }

    fn post_view(&mut self) {
        if model.state.is_maximized {
            self.main_window.maximize();
        }

        if model.update_directory_scroll_position {
            // Although this function is already called whenever the hadjustment changes, we also
            // sometimes want to scroll when the adjustment doesn't change.
            //
            // Consider the user selecting a new directory entry on a partially obscured panel. The
            // adjustment won't change, because the total number of panels is the same. However,
            // we still want to scroll over to it because it's new information that the user wants
            // to see.
            set_adjustment_to_upper_bound(&self.directory_panes_scroller.hadjustment());
        }

        if let Some(path) = &model.open_app_for_path {
            choose_and_launch_app_for_path(&self.main_window, path);
        }
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

/// Updates the value of an adjustment to its upper bound.
///
/// This is used to keep new directories and file information visible inside the directory panes
/// scroll window as user interacts with the application.
fn set_adjustment_to_upper_bound(adjustment: &gtk::Adjustment) {
    adjustment.set_value(adjustment.upper());
}
