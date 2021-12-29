//! Widget that lists commonly-visited places in the filesystem. Inspired by GTK 3's
//! [`PlacesSidebar`].
//!
//! Selecting a place entry notifies the parent widget to update the current "root" directory. New
//! directory selections will be based on the new root.
//!
//! [`PlacesSidebar`]: https://docs.gtk.org/gtk3/class.PlacesSidebar.html

use std::path::{Path, PathBuf};

use gtk::glib;
use gtk::prelude::*;
use relm4::{gtk, send, ComponentUpdate, Model, Sender, Widgets};

use super::{AppModel, AppMsg};

mod place;

use place::PlaceObject;

pub enum PlacesSidebarMsg {
    SelectionChanged(PathBuf),
}

pub struct PlacesSidebarModel {
    selection_model: gtk::SingleSelection,
}

impl Model for PlacesSidebarModel {
    type Msg = PlacesSidebarMsg;
    type Widgets = PlacesSidebarWidgets;
    type Components = ();
}

impl ComponentUpdate<AppModel> for PlacesSidebarModel {
    fn init_model(parent_model: &AppModel) -> Self {
        let store = gio::ListStore::new(PlaceObject::static_type());

        let mut places = vec![PlaceObject::new("Home", &glib::home_dir(), "user-home")];

        let user_dirs = [
            (glib::UserDirectory::Documents, "folder-documents"),
            (glib::UserDirectory::Downloads, "folder-download"),
            (glib::UserDirectory::Music, "folder-music"),
            (glib::UserDirectory::Pictures, "folder-pictures"),
            (glib::UserDirectory::Videos, "folder-videos"),
        ];

        places.extend(user_dirs.iter().filter_map(|(kind, icon)| {
            let path = glib::user_special_dir(*kind);

            if !path.exists() {
                return None;
            }

            let name = path.file_name().unwrap_or_default().to_string_lossy();
            Some(PlaceObject::new(&name, &path, icon))
        }));

        // TODO: Trash

        places.push(PlaceObject::new(
            "Computer",
            Path::new("/"),
            "drive-harddisk",
        ));

        // If the root matches an existing place, set the selection to that place.
        let root_place_position = places.iter().position(|place| {
            let path = place
                .property("file")
                .unwrap()
                .get::<gio::File>()
                .unwrap()
                .path()
                .unwrap();
            path == parent_model.root
        });

        for place in &places {
            store.append(place);
        }

        let selection_model = gtk::SingleSelection::builder()
            .model(&store)
            .autoselect(false)
            .build();

        if let Some(pos) = root_place_position {
            selection_model.select_item(pos as u32, true);
        }

        PlacesSidebarModel { selection_model }
    }

    fn update(
        &mut self,
        msg: PlacesSidebarMsg,
        _components: &(),
        _sender: Sender<PlacesSidebarMsg>,
        parent_sender: Sender<AppMsg>,
    ) {
        match msg {
            PlacesSidebarMsg::SelectionChanged(path) => {
                send!(parent_sender, AppMsg::NewRoot(path));
            }
        }
    }
}

pub struct PlacesSidebarWidgets {
    root: gtk::ScrolledWindow,
}

impl Widgets<PlacesSidebarModel, AppModel> for PlacesSidebarWidgets {
    type Root = gtk::ScrolledWindow;

    fn init_view(
        model: &PlacesSidebarModel,
        _components: &(),
        sender: Sender<PlacesSidebarMsg>,
    ) -> Self {
        let root = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .build();

        let factory = gtk::SignalListItemFactory::new();
        factory.connect_setup(move |_, list_item| {
            let root = gtk::Box::builder()
                .orientation(gtk::Orientation::Horizontal)
                .spacing(5)
                .build();

            let image = gtk::Image::new();
            root.append(&image);

            let name_label = gtk::Label::new(None);
            root.append(&name_label);

            let list_item_expression = gtk::ConstantExpression::new(list_item);
            let place_expression = gtk::PropertyExpression::new(
                gtk::ListItem::static_type(),
                Some(&list_item_expression),
                "item",
            );

            let name_expression = gtk::PropertyExpression::new(
                PlaceObject::static_type(),
                Some(&place_expression),
                "name",
            );
            name_expression.bind(&name_label, "label", Some(&name_label));

            let icon_expression = gtk::PropertyExpression::new(
                PlaceObject::static_type(),
                Some(&place_expression),
                "icon",
            );
            icon_expression.bind(&image, "icon_name", Some(&image));

            list_item.set_child(Some(&root));
        });

        model
            .selection_model
            .connect_selection_changed(move |selection, _, _| {
                let selected_item = selection.selected_item().unwrap();
                let place = selected_item.downcast::<PlaceObject>().unwrap();
                let path = place
                    .property("file")
                    .unwrap()
                    .get::<gio::File>()
                    .unwrap()
                    .path()
                    .unwrap();

                send!(sender, PlacesSidebarMsg::SelectionChanged(path));
            });

        let list_view = gtk::ListView::builder()
            .factory(&factory)
            .model(&model.selection_model)
            .css_classes(vec![String::from("navigation-sidebar")])
            .build();

        root.set_child(Some(&list_view));

        PlacesSidebarWidgets { root }
    }

    fn view(&mut self, _model: &PlacesSidebarModel, _sender: Sender<PlacesSidebarMsg>) {}

    fn root_widget(&self) -> gtk::ScrolledWindow {
        self.root.clone()
    }
}
