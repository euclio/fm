//! Widget that lists commonly-visited places in the filesystem. Inspired by GTK 3's
//! [`PlacesSidebar`].
//!
//! Selecting a place entry notifies the parent widget to update the current "root" directory. New
//! directory selections will be based on the new root.
//!
//! [`PlacesSidebar`]: https://docs.gtk.org/gtk3/class.PlacesSidebar.html

use glib::clone;
use gtk::prelude::*;
use gtk::{gdk, gio, glib};
use relm4::{gtk, ComponentParts, ComponentSender, SimpleComponent};
use tracing::*;

use super::app::AppMsg;
use crate::filesystem;

mod place;

use place::PlaceObject;

#[derive(Debug)]
pub enum PlacesSidebarMsg {
    /// A new sidebar entry was selected.
    SelectionChanged(gio::File),

    /// The displayed places have changed.
    Update,
}

#[derive(Debug)]
pub struct PlacesSidebarModel {
    _volume_monitor: gio::VolumeMonitor,
    places_model: gtk::SingleSelection,
    mounts_model: gtk::SingleSelection,
}

impl PlacesSidebarModel {
    fn update_mounts(&mut self) {
        info!("updating mounts");

        let selected_file = self
            .mounts_model
            .selected_item()
            .map(|place| place.property::<gio::File>("file"));

        let store = self
            .mounts_model
            .model()
            .unwrap()
            .downcast::<gio::ListStore>()
            .unwrap();

        store.remove_all();

        let volume_monitor = gio::VolumeMonitor::get();

        for drive in volume_monitor.connected_drives() {
            for volume in drive.volumes() {
                if let Some(mount) = volume.get_mount() {
                    store.append(&PlaceObject::new(
                        &mount.name(),
                        &mount.default_location(),
                        &mount.symbolic_icon(),
                    ));
                }
            }
        }

        store.append(&PlaceObject::new(
            &glib::host_name(),
            &gio::File::for_uri("file:///"),
            gio::ThemedIcon::new("drive-harddisk-symbolic").upcast_ref(),
        ));

        for mount in volume_monitor.mounts() {
            if mount.is_shadowed() || mount.volume().is_some() {
                continue;
            }

            store.append(&PlaceObject::new(
                &mount.name(),
                &mount.default_location(),
                &mount.symbolic_icon(),
            ));
        }

        if let Some(file) = selected_file {
            let pos = store.iter::<PlaceObject>().position(|item| {
                if let Ok(item) = item {
                    item.property::<gio::File>("file") == file
                } else {
                    false
                }
            });

            if let Some(pos) = pos {
                self.mounts_model.set_selected(pos as u32);
            }
        }
    }
}

#[relm4::component(pub)]
impl SimpleComponent for PlacesSidebarModel {
    type Widgets = PlacesSidebarWidgets;
    type Init = gio::File;
    type Input = PlacesSidebarMsg;
    type Output = AppMsg;

    view! {
        gtk::ScrolledWindow {
            set_hscrollbar_policy: gtk::PolicyType::Never,
            set_vexpand: true,
            set_width_request: 150,

            gtk::Box {
                set_orientation: gtk::Orientation::Vertical,

                // Holds static places.
                #[name = "places"]
                gtk::ListView {
                    add_css_class: "navigation-sidebar",
                },

                gtk::Separator {},

                // Holds volumes, mounts, and drives, which may change.
                #[name = "mounts"]
                gtk::ListView {
                    add_css_class: "navigation-sidebar",
                },
            }
        }
    }

    fn init(
        root_dir: gio::File,
        root: &Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let volume_monitor = gio::VolumeMonitor::get();

        volume_monitor.connect_volume_added(clone!(@strong sender => move |_, _| {
            sender.input(PlacesSidebarMsg::Update);
        }));
        volume_monitor.connect_volume_changed(clone!(@strong sender => move |_, _| {
            sender.input(PlacesSidebarMsg::Update);
        }));
        volume_monitor.connect_volume_removed(clone!(@strong sender => move |_, _| {
            sender.input(PlacesSidebarMsg::Update);
        }));
        volume_monitor.connect_mount_added(clone!(@strong sender => move |_, _| {
            sender.input(PlacesSidebarMsg::Update);
        }));
        volume_monitor.connect_mount_changed(clone!(@strong sender => move |_, _| {
            sender.input(PlacesSidebarMsg::Update);
        }));
        volume_monitor.connect_mount_removed(clone!(@strong sender => move |_, _| {
            sender.input(PlacesSidebarMsg::Update);
        }));
        volume_monitor.connect_drive_connected(clone!(@strong sender => move |_, _| {
            sender.input(PlacesSidebarMsg::Update);
        }));
        volume_monitor.connect_drive_changed(clone!(@strong sender => move |_, _| {
            sender.input(PlacesSidebarMsg::Update);
        }));
        volume_monitor.connect_drive_disconnected(clone!(@strong sender => move |_, _| {
            sender.input(PlacesSidebarMsg::Update);
        }));

        let mut store = gio::ListStore::new(PlaceObject::static_type());

        store.append(&PlaceObject::new(
            "Recent",
            &gio::File::for_uri("recent:///"),
            gio::ThemedIcon::new("document-open-recent-symbolic").upcast_ref(),
        ));

        store.append(&PlaceObject::new(
            "Home",
            &gio::File::for_path(glib::home_dir()),
            gio::ThemedIcon::new("user-home-symbolic").upcast_ref(),
        ));

        let user_dirs = [
            (glib::UserDirectory::Desktop, "user-desktop-symbolic"),
            (glib::UserDirectory::Documents, "folder-documents-symbolic"),
            (glib::UserDirectory::Downloads, "folder-download-symbolic"),
            (glib::UserDirectory::Music, "folder-music-symbolic"),
            (glib::UserDirectory::Pictures, "folder-pictures-symbolic"),
            (glib::UserDirectory::Videos, "folder-videos-symbolic"),
        ];

        store.extend(user_dirs.iter().filter_map(|(kind, icon)| {
            let path = glib::user_special_dir(*kind)?;

            if !path.exists() || path == glib::home_dir() {
                return None;
            }

            let file = gio::File::for_path(&path);
            let name = file
                .query_info(
                    gio::FILE_ATTRIBUTE_STANDARD_DISPLAY_NAME,
                    gio::FileQueryInfoFlags::NONE,
                    gio::Cancellable::NONE,
                )
                .ok()?;
            Some(PlaceObject::new(
                name.display_name().as_str(),
                &file,
                gio::ThemedIcon::new(icon).upcast_ref(),
            ))
        }));

        store.append(&PlaceObject::new(
            "Trash",
            &gio::File::for_uri("trash:///"),
            gio::ThemedIcon::new("user-trash-symbolic").upcast_ref(),
        ));

        let mount_store = gio::ListStore::new(PlaceObject::static_type());

        let mut model = PlacesSidebarModel {
            places_model: gtk::SingleSelection::builder()
                .model(&store)
                .autoselect(false)
                .build(),
            mounts_model: gtk::SingleSelection::builder()
                .model(&mount_store)
                .autoselect(false)
                .build(),
            _volume_monitor: volume_monitor,
        };

        // If the root matches an existing place, set the selection to that place.
        let root_place_position = model
            .places_model
            .model()
            .unwrap()
            .iter::<PlaceObject>()
            .position(|place| {
                if let Ok(place) = place {
                    place.property::<gio::File>("file").uri() == root_dir.uri()
                } else {
                    false
                }
            });
        if let Some(pos) = root_place_position {
            model.places_model.set_selected(pos as u32);
        }

        model.update_mounts();

        let widgets = view_output!();

        let factory = gtk::SignalListItemFactory::new();
        let sender_ = sender.clone();
        factory.connect_setup(move |_, item| {
            let item = item.downcast_ref::<gtk::ListItem>().unwrap();

            let root = gtk::Box::builder()
                .orientation(gtk::Orientation::Horizontal)
                .spacing(5)
                .build();

            let image = gtk::Image::new();
            root.append(&image);

            let name_label = gtk::Label::new(None);
            root.append(&name_label);

            let list_item_expression = gtk::ConstantExpression::new(item);
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
            icon_expression.bind(&image, "gicon", Some(&image));

            let drop_target = gtk::DropTarget::builder()
                .actions(gdk::DragAction::MOVE)
                .preload(true)
                .build();

            drop_target.set_types(&[gio::File::static_type()]);

            let sender_ = sender_.clone();
            drop_target.connect_drop(clone!(@strong item => move |_, value, _, _| {
                let place = item.item().and_downcast::<PlaceObject>().unwrap();
                let destination = place.property::<gio::File>("file");

                filesystem::handle_drop(value, &destination, sender_.output_sender().clone());

                true
            }));

            root.add_controller(drop_target);

            item.set_child(Some(&root));
        });

        model.places_model.connect_selection_changed(
            clone!(@strong sender, @weak model.mounts_model as mounts => move |selection, _, _| {
                if let Some(selected_item) = selection.selected_item() {
                    mounts.set_selected(gtk::INVALID_LIST_POSITION);

                    let place = selected_item.downcast::<PlaceObject>().unwrap();
                    let file = place.property::<gio::File>("file");

                    sender.input(PlacesSidebarMsg::SelectionChanged(file));
                }
            }),
        );
        model.mounts_model.connect_selection_changed(
            clone!(@strong sender, @weak model.places_model as places => move |selection, _, _| {
                if let Some(selected_item) = selection.selected_item() {
                    places.set_selected(gtk::INVALID_LIST_POSITION);

                    let place = selected_item.downcast::<PlaceObject>().unwrap();
                    let file = place.property::<gio::File>("file");

                    sender.input(PlacesSidebarMsg::SelectionChanged(file));
                }
            }),
        );

        widgets.places.set_factory(Some(&factory));
        widgets.places.set_model(Some(&model.places_model));

        widgets.mounts.set_factory(Some(&factory));
        widgets.mounts.set_model(Some(&model.mounts_model));

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: PlacesSidebarMsg, sender: ComponentSender<PlacesSidebarModel>) {
        match msg {
            PlacesSidebarMsg::SelectionChanged(file) => {
                sender.output(AppMsg::NewRoot(file)).unwrap();
            }
            PlacesSidebarMsg::Update => self.update_mounts(),
        }
    }
}
