use std::path::PathBuf;

use gtk::prelude::*;
use log::*;
use relm::{Update, Widget};
use relm_derive::{widget, Msg};

use crate::widgets::FilePanes;

mod widgets;

#[derive(Msg)]
pub enum Msg {
    NewLocation(PathBuf),
    Quit,
}

pub struct Model {
    selected_path: PathBuf,
}

#[widget]
impl Widget for Win {
    fn model(selected_path: PathBuf) -> Model {
        Model { selected_path }
    }

    fn update(&mut self, event: Msg) {
        match event {
            Msg::NewLocation(location) => {
                self.model.selected_path = location;
                self.components
                    .file_panes
                    .emit(<FilePanes as Update>::Msg::NewRoot(
                        self.model.selected_path.clone(),
                    ));
            }
            Msg::Quit => gtk::main_quit(),
        }
    }

    view! {
        gtk::Window {
            gtk::Paned {
                gtk::PlacesSidebar {
                    open_location(_, loc, _) => {
                        info!("new sidebar location clicked: {}", loc.uri());

                        match loc.path() {
                            Some(path) => Some(Msg::NewLocation(path)),
                            None => {
                                error!("no path for location, ignoring");
                                None
                            }
                        }
                    }
                },
                gtk::ScrolledWindow {
                    #[name="file_panes"]
                    FilePanes(self.model.selected_path.clone()),
                },
            },
            delete_event(_, _) => (Msg::Quit, Inhibit(false)),
        }
    }
}
