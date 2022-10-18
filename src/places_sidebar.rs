//! Widget that lists commonly-visited places in the filesystem. Inspired by GTK 3's
//! [`PlacesSidebar`].
//!
//! Selecting a place entry notifies the parent widget to update the current "root" directory. New
//! directory selections will be based on the new root.
//!
//! [`PlacesSidebar`]: https://docs.gtk.org/gtk3/class.PlacesSidebar.html

use std::path::{Path, PathBuf};

use glib::clone;
use gtk::prelude::*;
use gtk::{gio, glib};
use relm4::{gtk, ComponentParts, ComponentSender, SimpleComponent};

use crate::AppMsg;

mod place;

use place::PlaceObject;

#[derive(Debug)]
pub enum PlacesSidebarMsg {
    SelectionChanged(PathBuf),
}

#[derive(Debug)]
pub struct PlacesSidebarModel {
    selection_model: gtk::SingleSelection,
}

#[relm4::component(pub)]
impl SimpleComponent for PlacesSidebarModel {
    type Widgets = PlacesSidebarWidgets;
    type Init = PathBuf;
    type Input = PlacesSidebarMsg;
    type Output = AppMsg;

    view! {
        gtk::ScrolledWindow {
            set_hscrollbar_policy: gtk::PolicyType::Never,
            set_vexpand: true,
            set_width_request: 150,
        }
    }

    fn init(
        root_dir: PathBuf,
        root: &Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
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
            let path = glib::user_special_dir(*kind)?;

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
            let path = place.property::<gio::File>("file").path().unwrap();
            path == root_dir
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

        let model = PlacesSidebarModel { selection_model };
        let widgets = view_output!();

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

        model.selection_model.connect_selection_changed(
            clone!(@strong sender => move |selection, _, _| {
                let selected_item = selection.selected_item().unwrap();
                let place = selected_item.downcast::<PlaceObject>().unwrap();
                let path = place.property::<gio::File>("file").path().unwrap();

                sender.input(PlacesSidebarMsg::SelectionChanged(path));
            }),
        );

        let list_view = gtk::ListView::builder()
            .factory(&factory)
            .model(&model.selection_model)
            .css_classes(vec![String::from("navigation-sidebar")])
            .build();

        root.set_child(Some(&list_view));

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: PlacesSidebarMsg, sender: ComponentSender<PlacesSidebarModel>) {
        match msg {
            PlacesSidebarMsg::SelectionChanged(path) => {
                sender.output(AppMsg::NewRoot(gio::File::for_path(path)));
            }
        }
    }
}
