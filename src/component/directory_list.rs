//! Factory widget that displays a listing of the contents of a directory.

use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::{self, Debug};
use std::sync::{Arc, Mutex};

use anyhow::bail;
use educe::Educe;
use futures::prelude::*;
use glib::clone;
use glib::translate::{from_glib_full, IntoGlib};
use relm4::actions::{ActionGroupName, RelmAction, RelmActionGroup};
use relm4::factory::{DynamicIndex, FactoryComponent, FactorySender};
use relm4::gtk::{gdk, gio, glib, pango, prelude::*};
use relm4::prelude::*;
use relm4::view;
use tracing::*;

use super::app::AppMsg;
use super::new_folder_dialog::{NewFolderDialog, NewFolderDialogMsg};
use crate::ops;
use crate::util::{self, fmt_files_as_uris, BitsetExt, GFileInfoExt};

mod actions;

use actions::*;

/// The requested minimum width of the widget.
const WIDTH: i32 = 200;

/// The spacing between elements of a list item.
const SPACING: i32 = 2;

/// Button number identifying the right click button on a mouse.
const BUTTON_RIGHT_CLICK: u32 = 3;

#[derive(Debug)]
pub struct Directory {
    /// The sorted list model (with a selection) that is displayed in the list view.
    list_model: gtk::MultiSelection,

    new_folder_dialog: Option<Controller<NewFolderDialog>>,
}

impl Directory {
    /// Returns the listed directory.
    pub fn dir(&self) -> gio::File {
        self.directory_list().file().unwrap()
    }

    /// Returns the underlying directory list model.
    fn directory_list(&self) -> gtk::DirectoryList {
        self.list_model
            .model()
            .and_downcast::<gtk::SortListModel>()
            .unwrap()
            .model()
            .and_downcast()
            .unwrap()
    }

    /// Returns the file info for the files that are currently selected.
    ///
    /// This function does not perform any I/O.
    fn selected_file_info(&self) -> Vec<gio::FileInfo> {
        let selected_set = self.list_model.selection();
        selected_set
            .iter()
            .flat_map(|pos| {
                self.list_model
                    .item(pos)
                    .map(|item| item.downcast::<gio::FileInfo>().unwrap())
            })
            .collect()
    }
}

/// Used to communicate the file selection status to the parent widget.
#[derive(Educe)]
#[educe(Debug)]
pub enum Selection {
    /// A selection of at least one file.
    Files(FileSelection),

    /// No file is selected.
    None,
}

/// A selection of at least one file.
#[derive(Educe)]
#[educe(Debug)]
pub struct FileSelection {
    /// The shared parent of the selected files.
    #[educe(Debug(method = "util::fmt_file_as_uri"))]
    pub parent: gio::File,

    /// The selected files.
    #[educe(Debug(method = "util::fmt_files_as_uris"))]
    pub files: Vec<gio::File>,
}

#[derive(Debug)]
pub enum DirectoryMessage {
    OpenItemAtPosition(u32),

    /// Open the application launcher dialog for the given file.
    ChooseAndLaunchApp(gio::File),

    /// Send the files in the current selection to the trash.
    TrashSelection,

    /// Restore files in the current selection from the trash.
    RestoreSelectionFromTrash,

    ShowNewFolderDialog,
}

#[relm4::factory(pub)]
impl FactoryComponent for Directory {
    type ParentInput = AppMsg;
    type ParentWidget = panel::Paned;
    type Widgets = DirectoryWidgets;
    type Init = gio::File;
    type Input = DirectoryMessage;
    type Output = AppMsg;
    type CommandOutput = ();

    view! {
        root = gtk::Stack {
            set_width_request: WIDTH,

            add_child = &gtk::Spinner {
                set_halign: gtk::Align::Center,
                set_valign: gtk::Align::Center,
                set_spinning: true,
            } -> { set_name: "spinner" },

            add_child = &gtk::ScrolledWindow {
                set_hscrollbar_policy: gtk::PolicyType::Never,

                #[wrap(Some)]
                #[name = "list_view"]
                set_child = &gtk::ListView {
                    set_factory: Some(&factory),
                    set_model: Some(&self.list_model),

                    connect_activate[sender] => move |_, position| {
                        sender.input(DirectoryMessage::OpenItemAtPosition(position))
                    }
                },
            } -> { set_name: "listing" },
        }
    }

    fn output_to_parent_input(output: Self::Output) -> Option<AppMsg> {
        Some(output)
    }

    fn init_model(dir: Self::Init, _index: &DynamicIndex, _sender: FactorySender<Self>) -> Self {
        debug_assert!(
            dir.query_file_type(gio::FileQueryInfoFlags::NONE, gio::Cancellable::NONE)
                == gio::FileType::Directory
        );

        let directory_list = gtk::DirectoryList::new(
            Some(
                &[
                    &**gio::FILE_ATTRIBUTE_STANDARD_NAME,
                    &**gio::FILE_ATTRIBUTE_STANDARD_DISPLAY_NAME,
                    &**gio::FILE_ATTRIBUTE_STANDARD_ICON,
                    &**gio::FILE_ATTRIBUTE_STANDARD_TYPE,
                    &**gio::FILE_ATTRIBUTE_STANDARD_CONTENT_TYPE,
                    &**gio::FILE_ATTRIBUTE_STANDARD_IS_SYMLINK,
                ]
                .join(","),
            ),
            Some(&dir),
        );

        let list_model = gtk::SortListModel::new(Some(directory_list.clone()), Some(file_sorter()));

        let list_model = gtk::MultiSelection::new(Some(list_model));

        Directory {
            list_model,

            // This can't be initialized here, since we need make the dialog transient for
            // something but we don't have a reference to a widget here.
            new_folder_dialog: None,
        }
    }

    fn init_widgets(
        &mut self,
        _index: &DynamicIndex,
        root: &Self::Root,
        _returned_widget: &gtk::Widget,
        sender: FactorySender<Self>,
    ) -> Self::Widgets {
        let factory = gtk::SignalListItemFactory::new();

        factory.connect_setup(clone!(
            @strong sender as sender,
            @weak self.list_model as selection,
        => move |_, item| {
            let item = item.downcast_ref::<gtk::ListItem>().unwrap();
            build_list_item_view(&selection, item, &sender);
        }));

        // Store the drop controllers we add by widget so that we can remove them on unbind.
        let controllers = Arc::new(Mutex::new(HashMap::new()));

        factory.connect_bind(clone!(
                @strong sender as sender,
                @strong controllers as controllers,
        => move |_, list_item| {
            let list_item = list_item.downcast_ref::<gtk::ListItem>().unwrap();
            let widget = list_item.child().unwrap();

            let info = list_item
                .item()
                .and_downcast::<gio::FileInfo>()
                .unwrap();

            if matches!(info.file_type(), gio::FileType::Directory) {
                let dir = info.file().unwrap();
                let target = new_drop_target_for_dir(dir, sender.clone());
                widget.add_controller(target.clone());
                controllers.lock().unwrap().insert(widget, target);
            }
        }));

        factory.connect_unbind(move |_, list_item| {
            let list_item = list_item.downcast_ref::<gtk::ListItem>().unwrap();
            let widget = list_item.child().unwrap();

            if let Some(controller) = controllers.lock().unwrap().remove(&widget) {
                widget.remove_controller(&controller);
            }
        });

        let sender_ = sender.clone();
        self.list_model
            .connect_selection_changed(move |selection, _, _| {
                send_new_selection(selection, &sender_);
            });
        let sender_ = sender.clone();
        self.list_model
            .connect_items_changed(move |selection, _, _, _| {
                send_new_selection(selection, &sender_);
            });

        let widgets = view_output!();

        let click_controller = gtk::GestureClick::builder()
            .button(BUTTON_RIGHT_CLICK)
            .build();
        let dir = self.dir();

        let menu = gtk::PopoverMenu::builder().has_arrow(false).build();
        menu.set_parent(&widgets.list_view);

        click_controller.connect_pressed(
            clone!(@strong dir, @weak widgets.list_view as list_view, @strong menu => move |_, _, x, y| {
                let model = populate_directory_menu_model();

                menu.set_menu_model(Some(&model));
                menu.set_pointing_to(Some(&gdk::Rectangle::new(x as i32, y as i32, 1, 1)));
                menu.popup();
            }),
        );
        register_directory_context_actions(widgets.list_view.upcast_ref(), sender.clone());
        widgets.list_view.add_controller(click_controller);

        self.directory_list()
            .bind_property("loading", &widgets.root, "visible-child-name")
            .transform_to(|_, loading| Some(if loading { "spinner" } else { "listing" }))
            .sync_create()
            .build();

        let drop_target = new_drop_target_for_dir(self.dir(), sender);
        widgets.list_view.add_controller(drop_target);

        self.new_folder_dialog = Some(
            NewFolderDialog::builder()
                .transient_for(&widgets.list_view)
                .launch(dir)
                .detach(),
        );

        widgets
    }

    fn update_with_view(
        &mut self,
        widgets: &mut Self::Widgets,
        msg: Self::Input,
        sender: FactorySender<Self>,
    ) {
        match msg {
            DirectoryMessage::OpenItemAtPosition(pos) => {
                let file_info = self
                    .list_model
                    .item(pos)
                    .and_downcast::<gio::FileInfo>()
                    .unwrap();

                debug!(
                    "opening item at position {}: {}",
                    pos,
                    file_info.display_name()
                );

                open_application_for_file(&file_info.file().unwrap(), &sender);
            }
            DirectoryMessage::ChooseAndLaunchApp(file) => {
                let dialog = gtk::AppChooserDialog::new(
                    widgets.root.toplevel_window().as_ref(),
                    gtk::DialogFlags::MODAL,
                    &file,
                );

                dialog.connect_response(clone!(@strong file => move |this, response| {
                    if let gtk::ResponseType::Ok = response {
                        if let Some(app_info) = this.app_info() {
                            let _ = app_info.launch(&[file.clone()], gio::AppLaunchContext::NONE);
                        }
                    }

                    this.hide();
                }));

                dialog.show();
            }
            DirectoryMessage::TrashSelection => {
                let selected_file_info = self.selected_file_info();

                info!("trashing files: {:?}", fmt_file_info(&selected_file_info));

                let sender = sender.clone();
                relm4::spawn_local(async move {
                    let results = future::join_all(selected_file_info.iter().map(|f| {
                        f.file()
                            .unwrap()
                            .trash_future(glib::source::PRIORITY_DEFAULT)
                            .map(move |res| (res, f))
                    }))
                    .await;

                    let trashed_files = results
                        .into_iter()
                        .flat_map(|(result, info)| match result {
                            Ok(_) => Some(info),
                            Err(e) => {
                                sender.output(AppMsg::Error(Box::new(e)));
                                None
                            }
                        })
                        .collect::<Vec<_>>();

                    if !trashed_files.is_empty() {
                        sender.output(AppMsg::Toast(match &trashed_files[..] {
                            [info] => format!("'{}' moved to trash", info.display_name()),
                            _ => format!("{} files moved to trash", trashed_files.len()),
                        }));
                    }
                });
            }
            DirectoryMessage::RestoreSelectionFromTrash => {
                let selected_file_info = self.selected_file_info();

                info!("restoring files: {:?}", fmt_file_info(&selected_file_info));

                let sender = sender.clone();
                relm4::spawn_local(async move {
                    future::join_all(selected_file_info.iter().map(|info| async {
                        let file = info.file().unwrap();

                        let info = file
                            .query_info_future(
                                gio::FILE_ATTRIBUTE_TRASH_ORIG_PATH,
                                gio::FileQueryInfoFlags::empty(),
                                glib::source::PRIORITY_DEFAULT,
                            )
                            .await;

                        let info = match info {
                            Ok(info) => info,
                            Err(err) => {
                                sender.output(AppMsg::Error(Box::new(err)));
                                return;
                            }
                        };

                        let original_path = info
                            .attribute_byte_string(gio::FILE_ATTRIBUTE_TRASH_ORIG_PATH)
                            .unwrap();
                        let original_path = gio::File::for_parse_name(&original_path);

                        ops::move_(file, original_path, sender.output_sender().clone()).await;
                    }))
                    .await;
                });
            }
            DirectoryMessage::ShowNewFolderDialog => self
                .new_folder_dialog
                .as_ref()
                .unwrap()
                .emit(NewFolderDialogMsg::Show),
        }

        self.update_view(widgets, sender);
    }
}

/// Construct the view for an uninitialized list item, and set it as the item's child.
///
/// This view displays an icon, the name of the file, and an arrow indicating if the item is a file
/// or directory.
fn build_list_item_view(
    selection: &gtk::MultiSelection,
    list_item: &gtk::ListItem,
    sender: &FactorySender<Directory>,
) {
    view! {
        #[name = "root"]
        gtk::Box {
            set_orientation: gtk::Orientation::Horizontal,
            set_hexpand: true,
            set_spacing: SPACING,

            #[name = "icon"]
            gtk::Image {},

            #[name = "file_name"]
            gtk::Label {
                set_ellipsize: pango::EllipsizeMode::Middle,
            },

            #[name = "directory_icon"]
            gtk::Image {
                set_halign: gtk::Align::End,
                set_hexpand: true,
            },

            #[name = "menu"]
            gtk::PopoverMenu::from_model(gio::MenuModel::NONE) {
                set_has_arrow: false,
            },

            #[name = "rename_popover"]
            gtk::Popover {
                gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,
                    set_spacing: 12,

                    #[name = "entry"]
                    gtk::Entry {},

                    gtk::Button {
                        set_label: "Rename",
                        add_css_class: "suggested-action",
                        connect_clicked[entry] => move |_| {
                            entry.emit_activate();
                        }
                    }
                }
            },
        }
    }

    list_item
        .bind_property("item", &icon, "paintable")
        .transform_to(|_, item: Option<gio::FileInfo>| {
            item.map(|info| {
                // FIXME: How inefficient is it to query this every time?
                let icon_theme = gtk::IconTheme::for_display(&gdk::Display::default().unwrap());

                util::icon_for_file(&icon_theme, 16, &info)
            })
        })
        .build();

    list_item
        .bind_property("item", &file_name, "label")
        .transform_to(|_, item: Option<gio::FileInfo>| item.map(|info| info.display_name()))
        .build();

    list_item
        .bind_property("item", &directory_icon, "gicon")
        .transform_to(|_, item: Option<gio::FileInfo>| {
            item.and_then(|info| match info.file_type() {
                gio::FileType::Directory => {
                    Some(gio::Icon::for_string("go-next-symbolic").unwrap())
                }
                _ => None,
            })
        })
        .build();

    let click_controller = gtk::GestureClick::builder()
        .button(BUTTON_RIGHT_CLICK)
        .build();
    click_controller.connect_pressed(
        clone!(@weak selection, @weak list_item, @weak menu => move |_, _, x, y| {
            // If the clicked item isn't part of the selection, select it.
            let position = list_item.position();

            if !list_item.is_selected() {
                selection.select_item(position, true);
            }

            let item = list_item.item().unwrap();
            let info = item.downcast_ref::<gio::FileInfo>().unwrap();

            let model = populate_entry_menu_model(info);

            menu.set_menu_model(Some(&model));
            menu.set_pointing_to(Some(&gdk::Rectangle::new(x as i32, y as i32, 1, 1)));
            menu.popup();
        }),
    );
    root.add_controller(click_controller);

    let drag_source_controller = gtk::DragSource::builder()
        .actions(gdk::DragAction::MOVE)
        .build();

    // TODO: The documentation seems pretty adamant that you need to listen to `drag-end` if you're
    // supporting `DragAction::MOVE`, but everything seems to work as expected if you don't, at
    // least with Nautilus...
    list_item
        .bind_property("item", &drag_source_controller, "content")
        .transform_to(|_, item: Option<gio::FileInfo>| {
            item.map(|item| {
                let file_info = item.downcast_ref::<gio::FileInfo>().unwrap();
                let file = file_info.file().unwrap();

                // Dip into FFI here since the Rust bindings don't currently provide a way to
                // construct the content provider from a GFile.
                let content_provider: gdk::ContentProvider = unsafe {
                    from_glib_full(gdk::ffi::gdk_content_provider_new_typed(
                        gio::File::static_type().into_glib(),
                        file,
                    ))
                };

                content_provider
            })
        })
        .build();
    root.add_controller(drag_source_controller);

    register_entry_context_actions(root.upcast_ref(), &rename_popover, sender.clone());

    list_item.set_child(Some(&root));
}

/// Register right-click context menu actions and handlers.
fn register_entry_context_actions(
    list_item_view: &gtk::Widget,
    rename_popover: &gtk::Popover,
    sender: FactorySender<Directory>,
) {
    let group = RelmActionGroup::<DirectoryListRightClickActionGroup>::new();

    group.add_action(&RelmAction::<OpenDefaultAction>::new_with_target_value(
        move |_, uri: String| {
            let _ = gio::AppInfo::launch_default_for_uri(&uri, None::<&gio::AppLaunchContext>);
        },
    ));

    group.add_action(&RelmAction::<OpenChooserAction>::new_with_target_value(
        clone!(@strong sender => move |_, uri: String| {
            let file = gio::File::for_uri(&uri);
            sender.input(DirectoryMessage::ChooseAndLaunchApp(file));
        }),
    ));

    // This is a bit nasty: we create a new handler each time that the action is activated so that
    // we don't rely on the view alone to provide the file path, instead relying on the action
    // parameter. We have to disconnect the old handler each time because registering a new handler
    // is additive.
    let previous_handler_id = RefCell::new(None);
    group.add_action(&RelmAction::<RenameAction>::new_with_target_value(
        clone!(@weak rename_popover, @strong sender => move |_, uri: String| {
            let root = rename_popover.child().unwrap().downcast::<gtk::Box>().unwrap();
            let entry = root.first_child().unwrap().downcast::<gtk::Entry>().unwrap();

            if let Some(id) = previous_handler_id.borrow_mut().take() {
                glib::signal_handler_disconnect(&entry, id);
            }

            let file = gio::File::for_uri(&uri);
            if let Ok(edit_name) = file
                .query_info(
                    gio::FILE_ATTRIBUTE_STANDARD_EDIT_NAME,
                    gio::FileQueryInfoFlags::NONE,
                    gio::Cancellable::NONE,
                )
                .map(|info| info.edit_name())
            {
                entry.set_text(&edit_name);
            }

            let signal_handler_id = entry.connect_activate(clone!(
                    @weak rename_popover,
                    @strong file,
                    @strong sender => move |this| {
                        let new_name = this.text();
                        info!("renaming {} to {}", file.uri(), new_name);


                        let res = (|| -> anyhow::Result<()> {
                            if new_name.is_empty() {
                                bail!("File name cannot be empty.");
                            }

                            file.set_display_name(&new_name, gio::Cancellable::NONE)?;

                            Ok(())
                        })();

                        if let Err(err) = res {
                            sender.output(AppMsg::Error(err.into()));
                        }

                        rename_popover.popdown();
            }));

            *previous_handler_id.borrow_mut() = Some(signal_handler_id);

            rename_popover.popup();
        }),
    ));

    let sender_ = sender.clone();
    group.add_action(&RelmAction::<TrashSelectionAction>::new_stateless(
        move |_| sender_.input(DirectoryMessage::TrashSelection),
    ));

    group.add_action(
        &RelmAction::<RestoreSelectionFromTrashAction>::new_stateless(move |_| {
            sender.input(DirectoryMessage::RestoreSelectionFromTrash)
        }),
    );

    let actions = group.into_action_group();
    list_item_view.insert_action_group(
        <DirectoryListRightClickActionGroup as ActionGroupName>::NAME,
        Some(&actions),
    );
}

fn register_directory_context_actions(
    directory_list_view: &gtk::Widget,
    sender: FactorySender<Directory>,
) {
    let group = RelmActionGroup::<DirectoryListRightClickActionGroup>::new();

    group.add_action(&RelmAction::<NewFolderAction>::new_stateless(move |_| {
        sender.input(DirectoryMessage::ShowNewFolderDialog)
    }));

    directory_list_view.insert_action_group(
        <DirectoryListRightClickActionGroup as ActionGroupName>::NAME,
        Some(&group.into_action_group()),
    );
}

/// Builds a new drop target that copies files to the given directory.
///
/// The drop target accepts [`gio::File`]s and rejects files that are already in the same
/// directory.
fn new_drop_target_for_dir(dir: gio::File, sender: FactorySender<Directory>) -> gtk::DropTarget {
    let drop_target = gtk::DropTarget::builder()
        .actions(gdk::DragAction::MOVE)
        .preload(true)
        .build();

    drop_target.set_types(&[gio::File::static_type()]);

    drop_target.connect_value_notify(clone!(@strong dir => move |this| {
        if let Some(value) = this.value() {
            let file = value.get::<gio::File>().unwrap();

            info!("attempting to drop file {}", file.uri());

            if file.parent().as_ref() == Some(&dir) {
                info!("rejecting drop; file is already in directory");
                this.reject();
            }
        }
    }));

    drop_target.connect_drop(clone!(@strong dir => move |_, value, _, _| {
        ops::handle_drop(value, &dir, sender.output_sender().clone());

        true
    }));

    drop_target
}

/// Notifies the main component of the path of a new selection.
fn send_new_selection(selection: &gtk::MultiSelection, sender: &FactorySender<Directory>) {
    let selected_set = selection.selection();

    let selection = if selected_set.is_empty() {
        Selection::None
    } else {
        let directory_list = selection
            .model()
            .unwrap()
            .downcast::<gtk::SortListModel>()
            .unwrap()
            .model()
            .unwrap()
            .downcast::<gtk::DirectoryList>()
            .unwrap();
        let dir = directory_list.file().unwrap();

        let files = selected_set
            .iter()
            .flat_map(|pos| {
                selection
                    .item(pos)
                    .map(|item| item.downcast::<gio::FileInfo>().unwrap().file().unwrap())
            })
            .collect();

        Selection::Files(FileSelection { parent: dir, files })
    };

    sender.output(AppMsg::NewSelection(selection));
}

/// Constructs a new menu model for a directory entry's right-click context menu.
fn populate_entry_menu_model(file_info: &gio::FileInfo) -> gio::Menu {
    let file = file_info.file().unwrap();
    let uri = file.uri().to_string();

    let menu_model = gio::Menu::new();

    let open_section = gio::Menu::new();

    menu_model.append_section(None, &open_section);

    if let Some(app_info) =
        gio::AppInfo::default_for_type(&file_info.content_type().unwrap(), false)
    {
        let menu_item = RelmAction::<OpenDefaultAction>::to_menu_item_with_target_value(
            &format!("Open with {}", app_info.display_name()),
            &uri,
        );

        if let Some(icon) = &app_info.icon() {
            menu_item.set_icon(icon);
        }

        open_section.append_item(&menu_item);
    }

    open_section.append_item(
        &RelmAction::<OpenChooserAction>::to_menu_item_with_target_value("Open with...", &uri),
    );

    let modify_section = gio::Menu::new();

    menu_model.append_section(None, &modify_section);

    modify_section.append_item(&RelmAction::<RenameAction>::to_menu_item_with_target_value(
        "Rename...",
        &uri,
    ));

    if !file.has_uri_scheme("trash") {
        modify_section.append_item(&RelmAction::<TrashSelectionAction>::to_menu_item(
            "Move to Trash",
        ));
    } else {
        modify_section.append_item(
            &RelmAction::<RestoreSelectionFromTrashAction>::to_menu_item("Restore from Trash"),
        );
    }

    menu_model.freeze();

    menu_model
}

/// Constructs a new menu model for a directory's right-click context menu.
fn populate_directory_menu_model() -> gio::Menu {
    let model = gio::Menu::new();

    let open_section = gio::Menu::new();

    model.append_section(None, &open_section);

    open_section.append_item(&RelmAction::<NewFolderAction>::to_menu_item(
        "New Folder...",
    ));

    model.freeze();
    model
}

/// Opens the default application for the given file.
fn open_application_for_file(file: &gio::File, sender: &FactorySender<Directory>) {
    info!("opening {} in external application", file.uri());

    if let Err(e) =
        gio::AppInfo::launch_default_for_uri(file.uri().as_str(), None::<&gio::AppLaunchContext>)
    {
        sender.output(AppMsg::Error(Box::new(e)));
    }
}

/// Constructs a new sorter used to sort directory entries.
fn file_sorter() -> gtk::Sorter {
    gtk::CustomSorter::new(move |a, b| {
        let a = a.downcast_ref::<gio::FileInfo>().unwrap();
        let b = b.downcast_ref::<gio::FileInfo>().unwrap();

        a.display_name()
            .to_lowercase()
            .cmp(&b.display_name().to_lowercase())
            .into()
    })
    .upcast()
}

/// Returns a formattable object for a list of [`gio::FileInfo`] objects. Used to log the return
/// value of [`Directory::selected_file_info`].
fn fmt_file_info(info: &[gio::FileInfo]) -> impl Debug + '_ {
    struct Formatter<'a>(&'a [gio::FileInfo]);

    impl Debug for Formatter<'_> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            let files = self.0.iter().map(|i| i.file().unwrap()).collect::<Vec<_>>();
            fmt_files_as_uris(&files, f)
        }
    }

    Formatter(info)
}
