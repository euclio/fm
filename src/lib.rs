use std::path::PathBuf;

use gtk::prelude::*;

use relm::{connect, Component, ContainerWidget, Relm, Update, Widget};
use relm_derive::Msg;

use crate::widgets::FilePanes;

mod widgets;

#[derive(Msg)]
pub enum Msg {
    Quit,
}

pub struct Model {
    selected_path: PathBuf,
}

pub struct Win {
    window: gtk::Window,
    _file_panes: Component<FilePanes>,
}

impl Update for Win {
    type Model = Model;
    type ModelParam = PathBuf;
    type Msg = Msg;

    fn model(_: &Relm<Self>, selected_path: PathBuf) -> Model {
        Model { selected_path }
    }

    fn update(&mut self, event: Msg) {
        match event {
            Msg::Quit => gtk::main_quit(),
        }
    }
}

impl Widget for Win {
    type Root = gtk::Window;

    fn root(&self) -> Self::Root {
        self.window.clone()
    }

    fn view(relm: &Relm<Self>, model: Self::Model) -> Self {
        let window = gtk::Window::new(gtk::WindowType::Toplevel);

        connect!(
            relm,
            window,
            connect_delete_event(_, _),
            return (Some(Msg::Quit), Inhibit(false))
        );

        let file_panes = window.add_widget::<FilePanes>(model.selected_path.clone());

        window.show_all();

        Win {
            window,
            _file_panes: file_panes,
        }
    }
}
