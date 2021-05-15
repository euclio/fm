use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use gtk::prelude::*;
use relm::{Relm, Update, Widget};

const NAME_COLUMN: u8 = 0;
const ICON_COLUMN: u8 = 1;
const IS_DIR_COLUMN: u8 = 2;

pub struct Directory {
    dir: PathBuf,
}

pub struct DirectoryList {
    root: gtk::ScrolledWindow,
}

impl Widget for DirectoryList {
    type Root = gtk::ScrolledWindow;

    fn root(&self) -> Self::Root {
        self.root.clone()
    }

    fn view(_: &Relm<Self>, model: Self::Model) -> Self {
        let root = gtk::ScrolledWindow::new(gtk::NONE_ADJUSTMENT, gtk::NONE_ADJUSTMENT);

        root.set_size_request(150, -1);

        let view = gtk::TreeView::new();

        let file_column = gtk::TreeViewColumn::new();

        let icon_cell = gtk::CellRendererPixbuf::new();
        file_column.pack_start(&icon_cell, false);
        file_column.add_attribute(&icon_cell, "gicon", ICON_COLUMN.into());

        let filename_cell = gtk::CellRendererText::new();
        filename_cell
            .set_property("ellipsize", &pango::EllipsizeMode::End)
            .unwrap();
        file_column.pack_start(&filename_cell, true);
        file_column.add_attribute(&filename_cell, "text", NAME_COLUMN.into());

        let dir_expand_cell = gtk::CellRendererPixbuf::new();
        file_column.pack_end(&dir_expand_cell, false);
        file_column.add_attribute(&dir_expand_cell, "gicon", IS_DIR_COLUMN.into());

        view.append_column(&file_column);

        let store = list_store_for(&model.dir).unwrap();
        view.set_model(Some(&store));

        root.add(&view);

        root.show_all();

        DirectoryList { root }
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
                Some(gio::Icon::new_for_string("go-next-symbolic").unwrap())
            } else {
                None
            };

            model.insert_with_values(
                None,
                &[NAME_COLUMN.into(), ICON_COLUMN.into(), IS_DIR_COLUMN.into()],
                &[&name, &icon, &dir_icon],
            );
        }
    }

    model.set_sort_column_id(
        gtk::SortColumn::Index(NAME_COLUMN.into()),
        gtk::SortType::Ascending,
    );

    Ok(model)
}

impl Update for DirectoryList {
    type Model = Directory;
    type ModelParam = PathBuf;
    type Msg = ();

    fn model(_: &Relm<Self>, dir: PathBuf) -> Directory {
        Directory { dir }
    }

    fn update(&mut self, _: ()) {}
}
