//! Small, general purpose file manager built using GTK.
//!
//! Generally, each top-level module corresponds to a different Relm4 component.

#![warn(clippy::dbg_macro)]
#![warn(clippy::print_stderr)]
#![warn(clippy::print_stdout)]
#![warn(clippy::todo)]

use std::convert::identity;
use std::path::{self, PathBuf};

use glib::clone;
use gtk::{gio, glib, prelude::*};
use log::*;
use relm4::factory::FactoryVecDeque;
use relm4::prelude::*;

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
    /// The directory listed by the leftmost column.
    root: PathBuf,

    /// The directory listings. This factory acts as a stack, where new directories are pushed and
    /// popped relative to the root as the user clicks on new directory entries.
    directories: FactoryVecDeque<Directory>,

    error_alert: Controller<AlertModel>,
    file_preview: Controller<FilePreviewModel>,
    _places_sidebar: Controller<PlacesSidebarModel>,

    /// Whether the directory panes scroll window should update its scroll position to the upper
    /// bound on the next view update.
    update_directory_scroll_position: bool,

    /// Open the app chooser to open the given file on the next view update.
    open_app_for_file: Option<gio::File>,

    state: State,
}

impl AppModel {
    /// Returns the deepest directory that is listed (the rightmost listing).
    pub fn last_dir(&self) -> PathBuf {
        self.directories
            .back()
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

    /// Trigger the application chooser to pick an application to open the given file.
    ChooseAndLaunchApp(gio::File),
}

#[relm4::component(pub)]
impl SimpleComponent for AppModel {
    type Widgets = AppWidgets;
    type Init = PathBuf;
    type Input = AppMsg;
    type Output = ();

    view! {
        #[name = "main_window"]
        adw::Window {
            set_default_size: (state.width, state.height),
            set_title: Some("fm"),

            gtk::Box {
                set_orientation: gtk::Orientation::Vertical,

                adw::HeaderBar {},

                adw::Flap {
                    #[wrap(Some)]
                    set_flap = &gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        append: places_sidebar.widget(),
                    },

                    #[wrap(Some)]
                    set_separator = &gtk::Separator {},

                    #[wrap(Some)]
                    set_content = &gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,

                        #[name = "directory_panes_scroller"]
                        gtk::ScrolledWindow {
                            set_hexpand: true,
                            set_vexpand: true,

                            #[name = "directory_panes"]
                            panel::Paned {
                                append: file_preview.widget(),
                            },
                        },
                    },
                },
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

    fn init(
        dir: PathBuf,
        root: &Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let dir = if !dir.is_dir() {
            dir.parent().unwrap_or(&dir)
        } else {
            &dir
        };

        let state = State::read()
            .map_err(|e| {
                warn!("unable to read application state: {}", e);
                e
            })
            .unwrap_or_default();

        info!("starting with application state: {:?}", state);

        let file_preview = FilePreviewModel::builder().launch(()).detach();

        let places_sidebar = PlacesSidebarModel::builder()
            .launch(dir.to_path_buf())
            .forward(sender.input_sender(), identity);

        let widgets = view_output!();

        let mut model = AppModel {
            root: dir.to_owned(),
            directories: FactoryVecDeque::new(
                widgets.directory_panes.clone(),
                sender.input_sender(),
            ),
            error_alert: AlertModel::builder()
                .transient_for(widgets.main_window.clone())
                .launch(())
                .detach(),
            file_preview,
            _places_sidebar: places_sidebar,
            update_directory_scroll_position: false,
            open_app_for_file: None,
            state,
        };

        model.directories.guard().push_back(dir.to_path_buf());

        // TODO: There's sometimes a delay in updating the adjustment upper bound when a new pane
        // is added, causing this code to not trigger at the right time. Needs more investigation.
        widgets
            .directory_panes_scroller
            .hadjustment()
            .connect_notify(Some("upper"), |this, _| {
                set_adjustment_to_upper_bound(this);
            });

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, _sender: ComponentSender<Self>) {
        info!("received message: {:?}", msg);

        self.open_app_for_file = None;
        self.update_directory_scroll_position = false;

        match msg {
            AppMsg::Error(err) => {
                self.error_alert.emit(AlertMsg::Show {
                    text: err.to_string(),
                });
            }
            AppMsg::NewSelection(Selection::File(path)) => {
                let file = gio::File::for_path(&path);

                let mut last_dir = self.last_dir();

                let diff = pathdiff::diff_paths(&path, &last_dir)
                    .expect("new selection must be relative to the listed directories");

                info!(
                    "new selection: {:?}, last dir: {:?}, diff: {:?}",
                    path, last_dir, diff
                );

                let mut directories = self.directories.guard();

                for component in diff.components() {
                    match component {
                        path::Component::ParentDir => {
                            directories.pop_back();
                            last_dir.pop();
                        }
                        path::Component::Normal(name) => {
                            let component_path = last_dir.join(name);
                            if component_path.is_dir() {
                                directories.push_back(component_path.clone());
                                last_dir = component_path;
                            }
                        }
                        _ => unreachable!("unexpected path component: {:?}", component),
                    }
                }

                self.file_preview.emit(FilePreviewMsg::NewSelection(file));

                self.update_directory_scroll_position = true;
            }
            AppMsg::NewSelection(Selection::None) => {
                self.file_preview.emit(FilePreviewMsg::Hide);

                self.update_directory_scroll_position = true;
            }
            AppMsg::NewRoot(new_root) => {
                info!("new root: {:?}", new_root);

                let mut directories = self.directories.guard();

                directories.clear();

                self.root = new_root;
                directories.push_back(self.root.clone());

                self.file_preview.emit(FilePreviewMsg::Hide);

                self.update_directory_scroll_position = true;
            }
            AppMsg::ChooseAndLaunchApp(file) => self.open_app_for_file = Some(file),
        }
    }

    fn post_view(&self, widgets: &mut Self::Widgets) {
        if self.state.is_maximized {
            widgets.main_window.maximize();
        }

        if self.update_directory_scroll_position {
            // Although this function is already called whenever the hadjustment changes, we also
            // sometimes want to scroll when the adjustment doesn't change.
            //
            // Consider the user selecting a new directory entry on a partially obscured panel. The
            // adjustment won't change, because the total number of panels is the same. However,
            // we still want to scroll over to it because it's new information that the user wants
            // to see.
            set_adjustment_to_upper_bound(&widgets.directory_panes_scroller.hadjustment());
        }

        if let Some(file) = &self.open_app_for_file {
            choose_and_launch_app_for_file(widgets.main_window.clone(), file);
        }
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
fn choose_and_launch_app_for_file(parent: impl IsA<gtk::Window>, file: &gio::File) {
    let dialog = gtk::AppChooserDialog::new(Some(&parent), gtk::DialogFlags::MODAL, file);

    dialog.connect_response(clone!(@strong file => move |this, response| {
        if let gtk::ResponseType::Ok = response {
            if let Some(app_info) = this.app_info() {
                let _ = app_info.launch(&[file.clone()], None::<&gio::AppLaunchContext>);
            }
        }

        this.hide();
    }));

    dialog.show();
}

/// Updates the value of an adjustment to its upper bound.
///
/// This is used to keep new directories and file information visible inside the directory panes
/// scroll window as user interacts with the application.
fn set_adjustment_to_upper_bound(adjustment: &gtk::Adjustment) {
    adjustment.set_value(adjustment.upper());
}
