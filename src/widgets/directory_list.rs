use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use log::*;

use gtk::prelude::*;
use relm::{connect, Relm, Update, Widget};
use relm_derive::Msg;

#[repr(i32)]
enum Column {
    Name,
    Icon,
    IsDir,
}

pub struct Directory {
    relm: Relm<DirectoryList>,
    dir: PathBuf,
}

#[derive(Msg, Clone)]
pub enum Msg {
    /// Fired when the selection in the tree view has changed. This event will then trigger
    /// [`Self::NewEntrySelected`] that contains information from the selection.
    SelectionChanged,

    /// Fired when the selection in the tree view has changed. Contains the path of the entry that
    /// is now selected.
    NewEntrySelected(PathBuf),
}

/// Displays the contents of a directory using a [`gtk::TreeView`].
pub struct DirectoryList {
    root: gtk::ScrolledWindow,
    model: Directory,
}

impl Widget for DirectoryList {
    type Root = gtk::ScrolledWindow;

    fn root(&self) -> Self::Root {
        self.root.clone()
    }

    fn view(relm: &Relm<Self>, model: Self::Model) -> Self {
        let root = gtk::ScrolledWindow::new(gtk::NONE_ADJUSTMENT, gtk::NONE_ADJUSTMENT);

        root.set_size_request(150, -1);

        let view = gtk::TreeView::new();
        view.set_headers_visible(false);

        let file_column = gtk::TreeViewColumn::new();

        let icon_cell = gtk::CellRendererPixbuf::new();
        file_column.pack_start(&icon_cell, false);
        file_column.add_attribute(&icon_cell, "gicon", Column::Icon as _);

        let filename_cell = gtk::CellRendererText::new();
        filename_cell
            .set_property("ellipsize", &pango::EllipsizeMode::End)
            .unwrap();
        file_column.pack_start(&filename_cell, true);
        file_column.add_attribute(&filename_cell, "text", Column::Name as _);

        let dir_expand_cell = gtk::CellRendererPixbuf::new();
        file_column.pack_end(&dir_expand_cell, false);
        file_column.add_attribute(&dir_expand_cell, "gicon", Column::IsDir as _);

        view.append_column(&file_column);

        let store = list_store_for(&model.dir).unwrap();
        view.set_model(Some(&store));

        root.add(&view);

        root.show_all();

        connect!(relm, view, connect_cursor_changed(_), Msg::SelectionChanged);

        DirectoryList { root, model }
    }
}

fn list_store_for(dir: &Path) -> io::Result<gtk::ListStore> {
    let model = gtk::ListStore::new(&[
        String::static_type(),
        gio::Icon::static_type(),
        gio::Icon::static_type(),
    ]);

    for entry in fs::read_dir(dir)?.filter_map(|x| x.ok()) {
        if let Ok(name) = entry.file_name().into_string() {
            let is_dir = entry.file_type().unwrap().is_dir();

            let mime_type = if !is_dir {
                mime_guess::from_path(&name).first_or_text_plain()
            } else {
                "inode/directory".parse().unwrap()
            };

            let icon = gio::content_type_get_icon(mime_type.as_ref());

            let dir_icon = if is_dir {
                Some(gio::Icon::for_string("go-next-symbolic").unwrap())
            } else {
                None
            };

            model.insert_with_values(
                None,
                &[
                    (Column::Name as _, &name),
                    (Column::Icon as _, &icon),
                    (Column::IsDir as _, &dir_icon),
                ],
            );
        }
    }

    model.set_sort_column_id(
        gtk::SortColumn::Index(Column::Name as _),
        gtk::SortType::Ascending,
    );

    Ok(model)
}

impl Update for DirectoryList {
    type Model = Directory;
    type ModelParam = PathBuf;
    type Msg = Msg;

    fn model(relm: &Relm<Self>, dir: PathBuf) -> Directory {
        assert!(dir.is_dir());
        Directory {
            relm: relm.clone(),
            dir,
        }
    }

    fn update(&mut self, event: Msg) {
        match event {
            Msg::SelectionChanged => {
                let child = self.root.child().unwrap();
                let tree_view = child.downcast_ref::<gtk::TreeView>().unwrap();
                let selection = tree_view.selection();

                if let Some((model, iter)) = selection.selected() {
                    let name = model
                        .value(&iter, Column::Name as _)
                        .get::<String>()
                        .unwrap();

                    let selected_entry = self.model.dir.join(name);
                    info!("selected {}", selected_entry.display());
                    self.model
                        .relm
                        .stream()
                        .emit(Msg::NewEntrySelected(selected_entry));
                }
            }
            Msg::NewEntrySelected(_) => (),
        }
    }
}
