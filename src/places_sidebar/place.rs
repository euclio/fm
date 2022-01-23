use std::path::Path;

use glib::Object;
use relm4::gtk::{gio, glib};

glib::wrapper! {
    /// GObject representing an entry in the places sidebar.
    pub struct PlaceObject(ObjectSubclass<imp::PlaceObject>);
}

impl PlaceObject {
    pub fn new(name: &str, file: &Path, icon: &str) -> Self {
        Object::new(&[
            ("name", &name),
            ("file", &gio::File::for_path(file)),
            ("icon", &icon),
        ])
        .expect("unable to create PlaceObject")
    }
}

mod imp {
    use std::cell::RefCell;
    use std::path::PathBuf;

    use gtk::gio::{self, prelude::*};
    use gtk::glib::{self, ParamFlags, ParamSpec, ParamSpecObject, ParamSpecString, Value};
    use gtk::subclass::prelude::*;
    use once_cell::sync::Lazy;
    use relm4::gtk;

    pub struct PlaceObject {
        name: RefCell<String>,
        file: RefCell<gio::File>,
        icon: RefCell<String>,
    }

    impl Default for PlaceObject {
        fn default() -> Self {
            PlaceObject {
                name: Default::default(),
                file: RefCell::new(gio::File::for_path(PathBuf::from("/"))),
                icon: Default::default(),
            }
        }
    }

    impl ObjectImpl for PlaceObject {
        fn properties() -> &'static [ParamSpec] {
            static PROPERTIES: Lazy<Vec<ParamSpec>> = Lazy::new(|| {
                vec![
                    ParamSpecString::new("name", "name", "name", None, ParamFlags::READWRITE),
                    ParamSpecObject::new(
                        "file",
                        "file",
                        "file",
                        gio::File::static_type(),
                        ParamFlags::READWRITE,
                    ),
                    ParamSpecString::new("icon", "icon", "icon", None, ParamFlags::READWRITE),
                ]
            });
            PROPERTIES.as_ref()
        }

        fn property(&self, _obj: &Self::Type, _id: usize, pspec: &ParamSpec) -> Value {
            match pspec.name() {
                "name" => self.name.borrow().to_value(),
                "file" => self.file.borrow().to_value(),
                "icon" => self.icon.borrow().to_value(),
                name => panic!("unknown property name: {}", name),
            }
        }

        fn set_property(&self, _obj: &Self::Type, _id: usize, value: &Value, pspec: &ParamSpec) {
            match pspec.name() {
                "name" => {
                    self.name.replace(value.get().unwrap());
                }
                "file" => {
                    self.file.replace(value.get().unwrap());
                }
                "icon" => {
                    self.icon.replace(value.get().unwrap());
                }
                name => panic!("unknown property name: {}", name),
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PlaceObject {
        const NAME: &'static str = "PlaceObject";
        type Type = super::PlaceObject;
        type ParentType = glib::Object;
    }
}
