use glib::Object;
use relm4::gtk::{gdk, glib};

glib::wrapper! {
    /// A [`gdk::Paintable`] implementation that allows placing an additional, smaller paintable in
    /// the bottom-left corner (also known as an emblem). Used to display small symlink arrows.
    pub struct EmblemedPaintable(ObjectSubclass<imp::EmblemedPaintable>)
        @implements gdk::Paintable;
}

impl EmblemedPaintable {
    pub fn new(icon: &gdk::Paintable, emblem: &gdk::Paintable) -> Self {
        Object::new(&[("icon", &icon), ("emblem", &emblem)])
            .expect("unable to created EmblemedPaintable")
    }
}

mod imp {
    use std::cell::RefCell;

    use gdk::subclass::prelude::*;
    use glib::{ParamFlags, ParamSpec, ParamSpecObject, Value};
    use gtk::{graphene, prelude::*};
    use once_cell::sync::Lazy;
    use relm4::gtk::{self, gdk, glib};

    #[derive(Debug, Default)]
    pub struct EmblemedPaintable {
        icon: RefCell<Option<gdk::Paintable>>,
        emblem: RefCell<Option<gdk::Paintable>>,
    }

    impl ObjectImpl for EmblemedPaintable {
        fn properties() -> &'static [ParamSpec] {
            static PROPERTIES: Lazy<Vec<ParamSpec>> = Lazy::new(|| {
                vec![
                    ParamSpecObject::new(
                        "icon",
                        "icon",
                        "icon",
                        gdk::Paintable::static_type(),
                        ParamFlags::READWRITE,
                    ),
                    ParamSpecObject::new(
                        "emblem",
                        "emblem",
                        "emblem",
                        gdk::Paintable::static_type(),
                        ParamFlags::READWRITE,
                    ),
                ]
            });
            PROPERTIES.as_ref()
        }

        fn property(&self, _obj: &Self::Type, _id: usize, pspec: &ParamSpec) -> Value {
            match pspec.name() {
                "icon" => self.icon.borrow().to_value(),
                "emblem" => self.emblem.borrow().to_value(),
                name => panic!("unknown property name: {}", name),
            }
        }

        fn set_property(&self, _obj: &Self::Type, _id: usize, value: &Value, pspec: &ParamSpec) {
            match pspec.name() {
                "icon" => {
                    self.icon.replace(value.get().unwrap());
                }
                "emblem" => {
                    self.emblem.replace(value.get().unwrap());
                }
                name => panic!("unknown property name: {}", name),
            }
        }
    }

    impl PaintableImpl for EmblemedPaintable {
        fn snapshot(&self, _obj: &Self::Type, snapshot: &gdk::Snapshot, width: f64, height: f64) {
            self.icon
                .borrow()
                .as_ref()
                .unwrap()
                .snapshot(snapshot, width, height);

            let gtk_snapshot = snapshot.downcast_ref::<gtk::Snapshot>().unwrap();
            gtk_snapshot.save();
            gtk_snapshot.translate(&graphene::Point::new(0.0, 0.5 * height as f32));

            self.emblem
                .borrow()
                .as_ref()
                .unwrap()
                .snapshot(snapshot, 0.5 * width, 0.5 * height);
            gtk_snapshot.restore();
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for EmblemedPaintable {
        const NAME: &'static str = "EmblemedPaintable";
        type Type = super::EmblemedPaintable;
        type ParentType = glib::Object;
        type Interfaces = (gdk::Paintable,);
    }
}
