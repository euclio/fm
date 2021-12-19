//! Factory widget that displays a listing of the contents of a directory.

use std::path::{Path, PathBuf};
use std::rc::Rc;

use libpanel::prelude::*;
use relm4::factory::{DynamicIndex, FactoryPrototype, FactoryVecDeque};
use relm4::gtk::{glib, pango};
use relm4::{gtk, Sender};

use super::AppMsg;

/// The requested minimum width of the widget.
const WIDTH: i32 = 200;

/// The spacing between elements of a list item.
const SPACING: i32 = 2;

#[derive(Debug)]
pub struct Directory {
    store: gtk::DirectoryList,
}

impl Directory {
    pub fn new(dir: &Path) -> Self {
        assert!(dir.is_dir());

        Directory {
            store: gtk::DirectoryList::new(
                Some("standard::name,standard::display-name,standard::icon,standard::file-type"),
                Some(&gio::File::for_path(dir)),
            ),
        }
    }

    /// Returns the listed directory.
    pub fn dir(&self) -> PathBuf {
        self.store.file().and_then(|f| f.path()).unwrap()
    }
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

        let selection_model = gtk::SingleSelection::builder()
            .model(&self.store)
            .autoselect(false)
            .build();
        selection_model.connect_selection_changed(move |selection, _, _| {
            if let Some(item) = selection.selected_item() {
                let file_info = item.downcast::<gio::FileInfo>().unwrap();

                let directory_list = selection.model().downcast::<gtk::DirectoryList>().unwrap();
                let dir = directory_list.file().and_then(|f| f.path()).unwrap();

                sender
                    .send(AppMsg::NewSelection(dir.join(file_info.name())))
                    .unwrap();
            }
        });

        let list_view = gtk::ListView::builder()
            .factory(&factory)
            .model(&selection_model)
            .build();
        scroller.set_child(Some(&list_view));

        FactoryWidgets { root: scroller }
    }

    fn position(&self, _index: &Rc<DynamicIndex>) {}

    fn update(&self, _index: &Rc<DynamicIndex>, _widgets: &FactoryWidgets) {}

    fn get_root(widgets: &FactoryWidgets) -> &gtk::ScrolledWindow {
        &widgets.root
    }
}
