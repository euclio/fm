//! Factory widget that displays a listing of the contents of a directory.

use std::path::{Path, PathBuf};

use glib::clone;
use libpanel::prelude::*;
use log::*;
use relm4::actions::RelmAction;
use relm4::factory::{DynamicIndex, FactoryPrototype, FactoryVecDeque};
use relm4::gtk::{gdk, glib, pango, prelude::*};
use relm4::{gtk, send, Sender};

use super::{AppMsg, OpenDefaultAction, TrashFileAction};

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
    pub fn new(dir: &Path) -> Self {
        assert!(dir.is_dir());

        let directory_list = gtk::DirectoryList::new(
            Some(
                &[
                    "standard::name",
                    "standard::display-name",
                    "standard::icon",
                    "standard::file-type",
                    "standard::content-type",
                ]
                .join(","),
            ),
            Some(&gio::File::for_path(dir)),
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

    /// Returns the listed directory.
    pub fn dir(&self) -> PathBuf {
        self.directory_list.file().and_then(|f| f.path()).unwrap()
    }
}

/// Used to communicate the file selection status to the parent widget.
#[derive(Debug)]
pub enum Selection {
    /// A single-file selection.
    File(PathBuf),

    /// No file is selected.
    None,
}

#[derive(Debug)]
pub struct FactoryWidgets {
    root: gtk::ScrolledWindow,
}

impl FactoryPrototype for Directory {
    type Factory = FactoryVecDeque<Self>;
    type Widgets = FactoryWidgets;
    type Root = gtk::ScrolledWindow;
    type View = libpanel::Paned;
    type Msg = AppMsg;

    fn generate(&self, _index: &DynamicIndex, sender: Sender<AppMsg>) -> FactoryWidgets {
        let factory = gtk::SignalListItemFactory::new();

        let dir = self.dir();
        factory.connect_setup(
            clone!(@weak self.list_model as selection => move |_, list_item| {
                build_list_item_view(&dir, &selection, list_item);
            }),
        );

        let sender_ = sender.clone();
        self.list_model
            .connect_selection_changed(move |selection, _, _| {
                send_new_selection(selection, &sender_);
            });

        let list_view = gtk::ListView::builder()
            .factory(&factory)
            .model(&self.list_model)
            .build();

        let dir = self.dir();
        let sender_ = sender;
        list_view.connect_activate(
            clone!(@weak self.list_model as list_model => move |_, position| {
                if let Some(item) = list_model.upcast_ref::<gio::ListModel>().item(position) {
                    let info = item.downcast_ref::<gio::FileInfo>().unwrap();
                    let path = dir.join(info.name());
                    open_application_for_path(&path, &sender_);
                }
            }),
        );

        let scroller = gtk::ScrolledWindow::builder()
            .width_request(WIDTH)
            .child(&list_view)
            .build();

        FactoryWidgets { root: scroller }
    }

    fn position(&self, _index: &DynamicIndex) {}

    fn update(&self, _index: &DynamicIndex, _widgets: &FactoryWidgets) {}

    fn get_root(widgets: &FactoryWidgets) -> &gtk::ScrolledWindow {
        &widgets.root
    }
}

/// Construct the view for an uninitialized list item, and set it as the item's child.
///
/// This view displays an icon, the name of the file, and an arrow indicating if the item is a file
/// or directory.
fn build_list_item_view(dir: &Path, selection: &gtk::SingleSelection, list_item: &gtk::ListItem) {
    let root = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .hexpand(true)
        .spacing(SPACING)
        .build();

    let list_item_expression = gtk::ConstantExpression::new(list_item);
    let file_info_expression = gtk::PropertyExpression::new(
        gtk::ListItem::static_type(),
        Some(&list_item_expression),
        "item",
    );

    fn value_to_file_info(value: &glib::Value) -> Option<gio::FileInfo> {
        value
            .get::<Option<glib::Object>>()
            .unwrap()
            .map(|obj| obj.downcast::<gio::FileInfo>().unwrap())
    }

    let icon_image = gtk::Image::new();
    root.append(&icon_image);
    let icon_expression = gtk::ClosureExpression::new(
        |args| {
            value_to_file_info(&args[1])
                .and_then(|file_info| file_info.icon())
                .unwrap_or_else(|| gio::Icon::for_string("text-x-generic").unwrap())
        },
        &[file_info_expression.clone().upcast()],
    );
    icon_expression.bind(&icon_image, "gicon", Some(&icon_image));

    let file_name_label = gtk::Label::builder()
        .ellipsize(pango::EllipsizeMode::Middle)
        .build();
    root.append(&file_name_label);
    let display_name_expression = gtk::ClosureExpression::new(
        |args| {
            value_to_file_info(&args[1])
                .map(|file_info| file_info.display_name().to_string())
                .unwrap_or_default()
        },
        &[file_info_expression.clone().upcast()],
    );
    display_name_expression.bind(&file_name_label, "label", Some(&file_name_label));

    let directory_icon = gtk::Image::builder()
        .halign(gtk::Align::End)
        .hexpand(true)
        .build();
    root.append(&directory_icon);
    let directory_icon_expression = gtk::ClosureExpression::new(
        |args| {
            value_to_file_info(&args[1])
                .and_then(|file_info| match file_info.file_type() {
                    gio::FileType::Directory => {
                        Some(gio::Icon::for_string("go-next-symbolic").unwrap())
                    }
                    _ => None,
                })
                // FIXME: Remove this unwrap when gtk/gtk-rs-core#419 is released.
                .unwrap_or_else(|| gio::Icon::for_string("go-next-symbolic").unwrap())
        },
        &[file_info_expression.upcast()],
    );
    directory_icon_expression.bind(&directory_icon, "gicon", Some(&directory_icon));

    let menu = gtk::PopoverMenu::from_model(None::<&gio::MenuModel>);
    menu.set_parent(&root);
    menu.set_has_arrow(false);

    let click_controller = gtk::GestureClick::builder()
        .button(BUTTON_RIGHT_CLICK)
        .build();
    let dir = dir.to_owned();
    click_controller.connect_released(
        clone!(@weak selection, @weak list_item, @weak menu => move |_, _, x, y| {
            let target = gdk::Rectangle { x: x as i32, y: y as i32, height: 1, width: 1 };
            handle_right_click(&dir, &selection, &list_item, menu, target);
        }),
    );
    root.add_controller(&click_controller);

    list_item.set_child(Some(&root));
}

/// Notifies the main component of the path of a new selection.
fn send_new_selection(selection: &gtk::SingleSelection, sender: &Sender<AppMsg>) {
    let selection = if let Some(item) = selection.selected_item() {
        let file_info = item.downcast::<gio::FileInfo>().unwrap();

        let directory_list = selection
            .model()
            .downcast::<gtk::SortListModel>()
            .unwrap()
            .model()
            .unwrap()
            .downcast::<gtk::DirectoryList>()
            .unwrap();
        let dir = directory_list.file().and_then(|f| f.path()).unwrap();

        Selection::File(dir.join(file_info.name()))
    } else {
        Selection::None
    };

    send!(sender, AppMsg::NewSelection(selection));
}

/// Handles the right click operation on an individual list item.
fn handle_right_click(
    dir: &Path,
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
        menu.set_pointing_to(&target);
        menu.popup();
    }
}

/// Constructs a new menu model for the given file info. Used to dynamically populate the menu on
/// right click.
fn populate_menu_model(file_info: &gio::FileInfo, dir: &Path) -> gio::Menu {
    let uri = format!("file://{}", dir.join(file_info.name()).to_string_lossy());

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
        &RelmAction::<TrashFileAction>::to_menu_item_with_target_value("Move to Trash", &uri),
    );

    menu_model
}

/// Opens the default application for the given path.
fn open_application_for_path(path: &Path, sender: &Sender<AppMsg>) {
    info!("opening {:?} in external application", path);

    if let Err(e) = gio::AppInfo::launch_default_for_uri(
        &format!("file://{}", path.display()),
        None::<&gio::AppLaunchContext>,
    ) {
        send!(sender, AppMsg::Error(e.into()));
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
