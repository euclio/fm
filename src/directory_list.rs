//! Factory widget that displays a listing of the contents of a directory.

use std::cell::RefCell;

use anyhow::bail;
use educe::Educe;
use glib::translate::{from_glib_full, IntoGlib};
use glib::{clone, closure, Object};
use log::*;
use relm4::actions::{ActionGroupName, RelmAction, RelmActionGroup};
use relm4::factory::{DynamicIndex, FactoryComponent, FactorySender};
use relm4::gtk::{gdk, gio, glib, pango, prelude::*};
use relm4::{gtk, panel};

use crate::util;
use crate::AppMsg;

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
    /// The underlying directory list.
    directory_list: gtk::DirectoryList,

    /// The sorted list model (with a selection) that is displayed in the list view.
    list_model: gtk::SingleSelection,
}

impl Directory {
    /// Returns the listed directory.
    pub fn dir(&self) -> gio::File {
        self.directory_list.file().unwrap()
    }
}

/// Used to communicate the file selection status to the parent widget.
#[derive(Educe)]
#[educe(Debug)]
pub enum Selection {
    /// A single-file selection.
    File(#[educe(Debug(method = "util::fmt_file_as_uri"))] gio::File),

    /// No file is selected.
    None,
}

pub struct DirectoryWidgets;

impl FactoryComponent for Directory {
    type ParentInput = AppMsg;
    type ParentWidget = panel::Paned;
    type Widgets = DirectoryWidgets;
    type Init = gio::File;
    type Input = ();
    type Output = AppMsg;
    type Root = gtk::ScrolledWindow;
    type CommandOutput = ();

    fn output_to_parent_input(output: Self::Output) -> Option<AppMsg> {
        Some(output)
    }

    fn init_root(&self) -> Self::Root {
        relm4::view! {
            root = gtk::ScrolledWindow {
                set_width_request: WIDTH,
                set_hscrollbar_policy: gtk::PolicyType::Never,
            }
        }
        root
    }

    fn init_model(dir: Self::Init, _index: &DynamicIndex, _sender: FactorySender<Self>) -> Self {
        debug_assert!(
            dir.query_file_type(gio::FileQueryInfoFlags::NONE, gio::Cancellable::NONE)
                == gio::FileType::Directory
        );

        let directory_list = gtk::DirectoryList::new(
            Some(
                &[
                    "standard::name",
                    "standard::display-name",
                    "standard::icon",
                    "standard::file-type",
                    "standard::content-type",
                    "standard::is-symlink",
                ]
                .join(","),
            ),
            Some(&dir),
        );

        let list_model = gtk::SortListModel::new(Some(&directory_list), Some(&file_sorter()));

        let list_model = gtk::SingleSelection::builder()
            .model(&list_model)
            .autoselect(false)
            .build();

        Directory {
            directory_list,
            list_model,
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

        let dir = self.dir();
        factory.connect_setup(clone!(
            @strong dir,
            @strong sender as sender,
            @weak self.list_model as selection,
        => move |_, item| {
            let item = item.downcast_ref::<gtk::ListItem>().unwrap();
            build_list_item_view(dir.clone(), &selection, item, &sender);
        }));

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

        let list_view = gtk::ListView::builder()
            .factory(&factory)
            .model(&self.list_model)
            .build();

        let dir = self.dir();
        list_view.connect_activate(clone!(
            @strong dir,
            @strong sender as sender,
            @weak self.list_model as list_model,
        => move |_, position| {
            if let Some(item) = list_model.upcast_ref::<gio::ListModel>().item(position) {
                let info = item.downcast_ref::<gio::FileInfo>().unwrap();
                let file = dir.child(info.name());
                open_application_for_file(&file, &sender);
            }
        }));

        let drop_target = new_drop_target_for_dir(dir, sender);
        list_view.add_controller(&drop_target);

        let stack = gtk::Stack::builder().vhomogeneous(false).build();

        let spinner_page = stack.add_child(
            &gtk::Spinner::builder()
                .halign(gtk::Align::Center)
                .valign(gtk::Align::Center)
                .spinning(true)
                .build(),
        );
        spinner_page.set_name("spinner");

        let listing_page = stack.add_child(&list_view);
        listing_page.set_name("listing");

        self.directory_list
            .property_expression("loading")
            .chain_closure::<String>(closure!(|_: Option<Object>, loading: bool| {
                if loading {
                    String::from("spinner")
                } else {
                    String::from("listing")
                }
            }))
            .bind(&stack, "visible-child-name", gtk::Widget::NONE);

        root.set_child(Some(&stack));

        DirectoryWidgets
    }
}

/// Construct the view for an uninitialized list item, and set it as the item's child.
///
/// This view displays an icon, the name of the file, and an arrow indicating if the item is a file
/// or directory.
fn build_list_item_view(
    dir: gio::File,
    selection: &gtk::SingleSelection,
    list_item: &gtk::ListItem,
    sender: &FactorySender<Directory>,
) {
    let root = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .hexpand(true)
        .spacing(SPACING)
        .build();

    let file_info_expression = list_item.property_expression("item");

    let icon_image = gtk::Image::new();
    root.append(&icon_image);
    file_info_expression
        .chain_closure::<gdk::Paintable>(closure!(|_: Option<Object>, item: Option<Object>| {
            item.map(|item| {
                let file_info = item.downcast::<gio::FileInfo>().unwrap();

                // FIXME: How inefficient is it to query this every time?
                let icon_theme = gtk::IconTheme::for_display(&gdk::Display::default().unwrap());

                util::icon_for_file(&icon_theme, 16, &file_info)
            })
        }))
        .bind(&icon_image, "paintable", gtk::Widget::NONE);

    let file_name_label = gtk::Label::builder()
        .ellipsize(pango::EllipsizeMode::Middle)
        .build();
    root.append(&file_name_label);
    file_info_expression
        .chain_closure::<glib::GString>(closure!(|_: Option<Object>, item: Option<Object>| {
            item.map(|item| {
                let file_info = item.downcast::<gio::FileInfo>().unwrap();
                file_info.display_name()
            })
        }))
        .bind(&file_name_label, "label", gtk::Widget::NONE);

    let directory_icon = gtk::Image::builder()
        .halign(gtk::Align::End)
        .hexpand(true)
        .build();
    root.append(&directory_icon);
    file_info_expression
        .chain_closure::<gio::Icon>(closure!(|_: Option<Object>, item: Option<Object>| {
            item.and_then(|item| {
                let file_info = item.downcast::<gio::FileInfo>().unwrap();
                match file_info.file_type() {
                    gio::FileType::Directory => {
                        Some(gio::Icon::for_string("go-next-symbolic").unwrap())
                    }
                    _ => None,
                }
            })
        }))
        .bind(&directory_icon, "gicon", gtk::Widget::NONE);

    let menu = gtk::PopoverMenu::from_model(None::<&gio::MenuModel>);
    menu.set_parent(&root);
    menu.set_has_arrow(false);

    let click_controller = gtk::GestureClick::builder()
        .button(BUTTON_RIGHT_CLICK)
        .build();
    click_controller.connect_pressed(
        clone!(@strong dir, @weak selection, @weak list_item, @weak menu => move |_, _, x, y| {
            let target = gdk::Rectangle::new(x as i32, y as i32, 1, 1);
            handle_right_click(&dir, &selection, &list_item, menu, target);
        }),
    );
    root.add_controller(&click_controller);

    let drag_source_controller = gtk::DragSource::builder()
        .actions(gdk::DragAction::MOVE)
        .build();

    // TODO: The documentation seems pretty adamant that you need to listen to `drag-end` if you're
    // supporting `DragAction::MOVE`, but everything seems to work as expected if you don't, at
    // least with Nautilus...

    file_info_expression
        .chain_closure::<gdk::ContentProvider>(closure!(
            |_: Option<Object>, item: Option<Object>| {
                item.map(|item| {
                    let file_info = item.downcast_ref::<gio::FileInfo>().unwrap();
                    let file = dir.child(file_info.name());

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
            }
        ))
        .bind(&drag_source_controller, "content", gtk::Widget::NONE);

    root.add_controller(&drag_source_controller);

    let rename_popover = build_rename_popover(root.upcast_ref());
    register_context_actions(root.upcast_ref(), &rename_popover, sender.clone());

    list_item.set_child(Some(&root));
}

/// Construct the popover that is displayed when renaming an item.
fn build_rename_popover(parent: &gtk::Widget) -> gtk::Popover {
    let popover = gtk::Popover::new();

    let root = gtk::Box::new(gtk::Orientation::Horizontal, 12);

    let entry = gtk::Entry::new();
    root.append(&entry);

    let button = gtk::Button::builder()
        .label("Rename")
        .css_classes(vec![String::from("suggested-action")])
        .build();
    button.connect_clicked(clone!(@weak entry => move |_| {
        entry.emit_activate();
    }));

    root.append(&button);

    popover.set_child(Some(&root));
    popover.set_parent(parent);

    popover
}

/// Register right-click context menu actions and handlers.
fn register_context_actions(
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
            sender.output(AppMsg::ChooseAndLaunchApp(file));
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
                    &gio::FILE_ATTRIBUTE_STANDARD_EDIT_NAME,
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

    group.add_action(&RelmAction::<TrashFileAction>::new_with_target_value(
        move |_, uri: String| {
            let file = gio::File::for_uri(&uri);
            let parent = file.parent().expect("listed file must have a parent");
            if let Ok(()) = file.trash(None::<&gio::Cancellable>) {
                sender.output(AppMsg::NewSelection(Selection::File(parent)));
            }
        },
    ));

    let actions = group.into_action_group();
    list_item_view.insert_action_group(
        <DirectoryListRightClickActionGroup as ActionGroupName>::NAME,
        Some(&actions),
    );
}

/// Builds a new drop target that represents the current directory.
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
        let file = value.get::<gio::File>().unwrap();

        info!("dropping {}", file.uri());

        let destination = dir.child(file.basename().unwrap());
        let res = file.move_(&destination, gio::FileCopyFlags::NONE, gio::Cancellable::NONE, None);

        if let Err(err) = res {
            sender.output(AppMsg::Error(err.into()));
            return false;
        }

        true
    }));

    drop_target
}

/// Notifies the main component of the path of a new selection.
fn send_new_selection(selection: &gtk::SingleSelection, sender: &FactorySender<Directory>) {
    let selection = if let Some(item) = selection.selected_item() {
        let file_info = item.downcast::<gio::FileInfo>().unwrap();

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

        Selection::File(dir.child(file_info.name()))
    } else {
        Selection::None
    };

    sender.output(AppMsg::NewSelection(selection));
}

/// Handles the right click operation on an individual list item.
fn handle_right_click(
    dir: &gio::File,
    selection: &gtk::SingleSelection,
    list_item: &gtk::ListItem,
    menu: gtk::PopoverMenu,
    target: gdk::Rectangle,
) {
    // If the right-clicked item isn't part of the selection, select it.
    let position = list_item.position();

    if !list_item.is_selected() {
        selection.set_selected(position);
    }

    if let Some(item) = list_item.item() {
        let info = item.downcast_ref::<gio::FileInfo>().unwrap();

        let menu_model = populate_menu_model(info, dir);

        menu.set_menu_model(Some(&menu_model));
        menu.set_pointing_to(Some(&target));
        menu.popup();
    }
}

/// Constructs a new menu model for the given file info. Used to dynamically populate the menu on
/// right click.
fn populate_menu_model(file_info: &gio::FileInfo, dir: &gio::File) -> gio::Menu {
    let file = dir.child(file_info.name());
    let uri = file.uri().to_string();

    let menu_model = gio::Menu::new();

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

        menu_model.append_item(&menu_item);
    }

    menu_model.append_item(
        &RelmAction::<OpenChooserAction>::to_menu_item_with_target_value("Open with...", &uri),
    );

    menu_model.append_item(
        &RelmAction::<TrashFileAction>::to_menu_item_with_target_value("Move to Trash", &uri),
    );

    menu_model.append_item(&RelmAction::<RenameAction>::to_menu_item_with_target_value(
        "Rename...",
        &uri,
    ));

    menu_model.freeze();

    menu_model
}

/// Opens the default application for the given file.
fn open_application_for_file(file: &gio::File, sender: &FactorySender<Directory>) {
    info!("opening {} in external application", file.uri());

    if let Err(e) =
        gio::AppInfo::launch_default_for_uri(file.uri().as_str(), None::<&gio::AppLaunchContext>)
    {
        sender.output(AppMsg::Error(e.into()));
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
