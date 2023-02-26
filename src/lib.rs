//! Small, general purpose file manager built using GTK.
//!
//! Generally, each top-level module corresponds to a different Relm4 component.

#![warn(clippy::dbg_macro)]
#![warn(clippy::print_stderr)]
#![warn(clippy::print_stdout)]
#![warn(clippy::todo)]

use std::convert::identity;
use std::path::{self, PathBuf};

use gtk::{gio, glib, prelude::*};
use ops::Progress;
use relm4::actions::{RelmAction, RelmActionGroup};
use relm4::factory::FactoryVecDeque;
use relm4::{prelude::*, MessageBroker};
use tracing::*;

mod alert;
mod config;
mod directory_list;
mod file_preview;
mod filesystem;
mod mount;
mod new_folder_dialog;
mod ops;
mod places_sidebar;
mod progress;
mod util;

use crate::alert::{AlertModel, AlertMsg};
use crate::config::State;
use crate::directory_list::{Directory, Selection};
use crate::file_preview::{FilePreviewModel, FilePreviewMsg};
use crate::mount::{Mount, MountMsg};
use crate::ops::Progress;
use crate::places_sidebar::PlacesSidebarModel;
use crate::progress::{TransferProgress, TransferProgressMsg};

static ERROR_BROKER: MessageBroker<AlertModel> = MessageBroker::new();

#[derive(Debug)]
pub struct AppModel {
    /// The directory listed by the leftmost column.
    root: gio::File,

    /// The directory listings. This factory acts as a stack, where new directories are pushed and
    /// popped relative to the root as the user clicks on new directory entries.
    directories: FactoryVecDeque<Directory>,

    /// Displays the progress of ongoing file operations.
    progress: FactoryVecDeque<TransferProgress>,

    error_alert: Controller<AlertModel>,
    file_preview: Controller<FilePreviewModel>,
    mount: Controller<Mount>,
    _places_sidebar: Controller<PlacesSidebarModel>,

    /// Whether the directory panes scroll window should update its scroll position to the upper
    /// bound on the next view update.
    update_directory_scroll_position: bool,

    state: State,
}

impl AppModel {
    /// Returns the deepest directory that is listed (the rightmost listing).
    pub fn last_dir(&self) -> gio::File {
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
    NewRoot(gio::File),

    /// A new selection was made within the existing directory listings. This can result in a
    /// number of possible changes:
    ///
    /// - If the new selection is higher in the directory tree than the old selection, the lower
    /// listings must be removed.
    /// - If the new selection is a directory, a new directory listing is pushed onto the listing
    /// stack.
    /// - If the new selection is a file, the preview must be updated.
    NewSelection(Selection),

    /// Update the file transfer progress indicator.
    Progress(Progress),

    /// Display a toast.
    Toast(String),

    /// Display the about window.
    About,

    /// Launch a dialog to mount a new mountable.
    Mount,
}

#[relm4::component(pub)]
impl Component for AppModel {
    type Widgets = AppWidgets;
    type Init = PathBuf;
    type Input = AppMsg;
    type Output = ();
    type CommandOutput = ();

    view! {
        #[name = "main_window"]
        adw::Window {
            set_default_size: (state.width, state.height),
            set_title: Some("fm"),

            #[name = "toast_overlay"]
            adw::ToastOverlay {
                gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,

                    adw::HeaderBar {
                        pack_end = &gtk::MenuButton {
                            set_icon_name: "open-menu-symbolic",
                            set_menu_model: Some(&primary_menu),
                        },

                        pack_end = &gtk::MenuButton {
                            #[wrap(Some)]
                            set_child = &gtk::Spinner {
                                start: (),
                            },

                            #[wrap(Some)]
                            set_popover = &gtk::Popover {
                                #[name = "transfer_progress"]
                                gtk::Box {},
                            }
                        },
                    },

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

    menu! {
        primary_menu: {
            section! {
                "Connect to server..." => MountAction,
            },
            section! {
                "About" => AboutAction,
            },
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

        let dir = gio::File::for_path(dir);

        let state = State::read()
            .map_err(|e| {
                warn!("unable to read application state: {}", e);
                e
            })
            .unwrap_or_default();

        info!("starting with application state: {:?}", state);

        let file_preview = FilePreviewModel::builder().launch(()).detach();

        let places_sidebar = PlacesSidebarModel::builder()
            .launch(dir.clone())
            .forward(sender.input_sender(), identity);

        let widgets = view_output!();

        let mut model = AppModel {
            root: dir.clone(),
            directories: FactoryVecDeque::new(
                widgets.directory_panes.clone(),
                sender.input_sender(),
            ),
            progress: FactoryVecDeque::new(
                widgets.transfer_progress.clone(),
                sender.input_sender(),
            ),
            mount: Mount::builder()
                .transient_for(&widgets.main_window)
                .launch(())
                .forward(sender.input_sender(), identity),
            error_alert: AlertModel::builder()
                .transient_for(widgets.main_window.clone())
                .launch_with_broker((), &ERROR_BROKER)
                .detach(),
            file_preview,
            _places_sidebar: places_sidebar,
            update_directory_scroll_position: false,
            state,
        };

        model.directories.guard().push_back(dir);

        let group = RelmActionGroup::<WindowActionGroup>::new();

        let sender_ = sender.clone();
        let about_action: RelmAction<AboutAction> = RelmAction::new_stateless(move |_| {
            sender_.input(AppMsg::About);
        });
        group.add_action(&about_action);

        let mount_action: RelmAction<MountAction> = RelmAction::new_stateless(move |_| {
            sender.input(AppMsg::Mount);
        });
        group.add_action(&mount_action);

        widgets
            .main_window
            .insert_action_group("win", Some(&group.into_action_group()));

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

    fn update_with_view(
        &mut self,
        widgets: &mut Self::Widgets,
        msg: Self::Input,
        _sender: ComponentSender<Self>,
        _: &Self::Root,
    ) {
        self.update_directory_scroll_position = false;

        match msg {
            AppMsg::Error(err) => {
                self.error_alert.emit(AlertMsg::Show {
                    text: err.to_string(),
                });
            }
            AppMsg::NewSelection(Selection::Files(selection)) => {
                let mut last_dir = self.last_dir();

                let file = if selection.files.len() == 1 {
                    selection.files.first().unwrap()
                } else {
                    &selection.parent
                };

                let file_path = match glib::Uri::split(&file.uri(), glib::UriFlags::NONE) {
                    Ok((_, _, _, _, path, _, _)) => PathBuf::from(&path),
                    Err(e) => {
                        warn!("unable to parse URI: {}", e);
                        return;
                    }
                };

                let last_dir_path = glib::Uri::split(&last_dir.uri(), glib::UriFlags::NONE)
                    .map(|(_, _, _, _, path, _, _)| path)
                    .expect("last visited directory must be a valid URI");

                let diff = pathdiff::diff_paths(file_path, &last_dir_path)
                    .expect("new selection must be relative to the listed directories");

                info!(
                    "new selection: {:?}, last dir: {}, diff: {}",
                    selection,
                    last_dir.uri(),
                    diff.display()
                );

                let mut directories = self.directories.guard();

                for component in diff.components() {
                    match component {
                        path::Component::ParentDir => {
                            directories.pop_back();
                            last_dir = last_dir.parent().unwrap();
                        }
                        path::Component::Normal(name) => {
                            let component_file = last_dir.child(name);
                            if component_file.query_file_type(
                                gio::FileQueryInfoFlags::NONE,
                                gio::Cancellable::NONE,
                            ) == gio::FileType::Directory
                            {
                                directories.push_back(component_file.clone());
                                last_dir = component_file;
                            }
                        }
                        _ => unreachable!("unexpected path component: {:?}", component),
                    }
                }

                self.file_preview
                    .emit(FilePreviewMsg::NewSelection(selection));

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
            AppMsg::Progress(progress) => {
                let idx = self
                    .progress
                    .iter()
                    .position(|child| child.id == progress.id);

                if let Some(idx) = idx {
                    self.progress
                        .send(idx, TransferProgressMsg::Update(progress));
                }
            }
            AppMsg::Toast(message) => {
                widgets.toast_overlay.add_toast(&adw::Toast::new(&message));
            }
            AppMsg::About => {
                gtk::AboutDialog::builder()
                    .authors(
                        env!("CARGO_PKG_AUTHORS")
                            .split(':')
                            .map(String::from)
                            .collect(),
                    )
                    .comments(env!("CARGO_PKG_DESCRIPTION"))
                    .copyright("© 2021 Andy Russell")
                    .license_type(gtk::License::MitX11)
                    .logo_icon_name("folder-symbolic")
                    .program_name(env!("CARGO_PKG_NAME"))
                    .version(env!("CARGO_PKG_VERSION"))
                    .website(env!("CARGO_PKG_HOMEPAGE"))
                    .build()
                    .show();
            }
            AppMsg::Mount => self.mount.emit(MountMsg::Mount),
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
    }
}

relm4::new_action_group!(WindowActionGroup, "win");
relm4::new_stateless_action!(AboutAction, WindowActionGroup, "about");
relm4::new_stateless_action!(MountAction, WindowActionGroup, "mount");

/// Updates the value of an adjustment to its upper bound.
///
/// This is used to keep new directories and file information visible inside the directory panes
/// scroll window as user interacts with the application.
fn set_adjustment_to_upper_bound(adjustment: &gtk::Adjustment) {
    adjustment.set_value(adjustment.upper());
}
