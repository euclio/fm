//! Factory widget that displays a listing of the contents of a directory.

use std::path::{Path, PathBuf};
use std::rc::Rc;

use gdk::keys::constants::{Left as LEFT, Right as RIGHT};
use glib::clone;
use libpanel::prelude::*;
use log::*;
use relm4::factory::{DynamicIndex, FactoryPrototype, FactoryVecDeque};
use relm4::gtk::{self, gdk, glib, pango, prelude::*};
use relm4::{send, Sender};

use super::AppMsg;

/// The requested minimum width of the widget.
const WIDTH: i32 = 200;

/// The spacing between elements of a list item.
const SPACING: i32 = 2;

#[derive(Debug)]
pub struct Directory {
    pub model: gtk::SingleSelection,
}

impl Directory {
    pub fn new(dir: &Path) -> Self {
        assert!(dir.is_dir());

        let model = gtk::DirectoryList::new(
            Some("standard::name,standard::display-name,standard::icon,standard::file-type"),
            Some(&gio::File::for_path(dir)),
        );

        let file_sorter = gtk::CustomSorter::new(move |a, b| {
            let a = a.downcast_ref::<gio::FileInfo>().unwrap();
            let b = b.downcast_ref::<gio::FileInfo>().unwrap();

            a.display_name()
                .to_lowercase()
                .cmp(&b.display_name().to_lowercase())
                .into()
        });
        let model = gtk::SortListModel::new(Some(&model), Some(&file_sorter));

        let model = gtk::SingleSelection::builder()
            .model(&model)
            .autoselect(false)
            .build();

        Directory { model }
    }

    /// Returns the listed directory.
    pub fn dir(&self) -> PathBuf {
        // FIXME: This is nasty, and also duplicated with the code below.
        self.model
            .model()
            .downcast::<gtk::SortListModel>()
            .unwrap()
            .model()
            .unwrap()
            .downcast::<gtk::DirectoryList>()
            .unwrap()
            .file()
            .and_then(|f| f.path())
            .unwrap()
    }
}

#[derive(Debug)]
pub struct FactoryWidgets {
    root: gtk::ScrolledWindow,
    list_view: gtk::ListView,
}

impl FactoryPrototype for Directory {
    type Factory = FactoryVecDeque<Self>;
    type Widgets = FactoryWidgets;
    type Root = gtk::ScrolledWindow;
    type View = libpanel::Paned;
    type Msg = AppMsg;

    fn generate(&self, _index: &Rc<DynamicIndex>, sender: Sender<AppMsg>) -> FactoryWidgets {
        let scroller = gtk::ScrolledWindow::builder().width_request(WIDTH).build();

        let factory = gtk::SignalListItemFactory::new();
        factory.connect_setup(move |_, list_item| {
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

            list_item.set_child(Some(&root));
        });
        let model_sender = sender.clone();

        self.model
            .connect_selection_changed(move |selection, _, _| {
                if let Some(item) = selection.selected_item() {
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

                    send!(
                        model_sender,
                        AppMsg::NewSelection {
                            selection: dir.join(file_info.name()),
                            src_dir: dir,
                        }
                    );
                }
            });

        let list_view = gtk::ListView::builder()
            .factory(&factory)
            .model(&self.model)
            .build();

        let event_controller = gtk::EventControllerKey::new();
        let dir = self.dir();
        let controller_sender = sender.clone();
        event_controller.connect_key_pressed(clone!(@weak self.model as model => @default-panic, move |_, key, _, _| {
            if !matches!(key, LEFT | RIGHT) {
                return gtk::Inhibit(false);
            }

            if let Some(item) = model.selected_item() {
                let file_info = item.downcast::<gio::FileInfo>().unwrap();
                let path = dir.join(file_info.name());

                match key {
                    LEFT => {
                        model.unselect_all();
                        send!(controller_sender, AppMsg::PrevDir(path));
                    }
                    RIGHT if path.is_dir() => {
                        send!(controller_sender, AppMsg::NextDir(path));
                    }
                    _ => (),
                }
            }

            gtk::Inhibit(false)
        }));
        list_view.add_controller(&event_controller);

        scroller.set_child(Some(&list_view));

        FactoryWidgets {
            root: scroller,
            list_view,
        }
    }

    fn position(&self, _index: &Rc<DynamicIndex>) {}

    fn update(&self, index: &Rc<DynamicIndex>, widgets: &FactoryWidgets) {
        info!("updating directory list {:?}", &index);
        widgets.list_view.grab_focus();
    }

    fn get_root(widgets: &FactoryWidgets) -> &gtk::ScrolledWindow {
        &widgets.root
    }
}
