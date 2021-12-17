//! Widget that displays file metadata and a small preview.

use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};

use log::*;
use mime::Mime;
use relm4::gtk::prelude::*;
use relm4::{gtk, ComponentUpdate, Sender, Widgets};

use super::{AppModel, AppMsg, Model};

/// The buffer size used to read the beginning of a file to predict its mime type and preview its
/// contents.
const PREVIEW_BUFFER_SIZE: usize = 4096;

#[derive(Debug)]
enum FilePreview {
    /// Plain text to be displayed in a [`FilePreviewWidgets::text`].
    Text(String),

    /// Image file, to be displayed in [`FilePreviewWidgets::picture`].
    Image(gio::File),

    /// Non-text, non-image file to be displayed in [`FilePreviewWidgets::image`].
    Icon(gio::Icon),
}

#[derive(Debug)]
struct FileInfo {
    path: PathBuf,
    preview: FilePreview,
}

#[derive(Debug)]
pub struct FilePreviewModel {
    file: Option<FileInfo>,
}

impl Model for FilePreviewModel {
    type Msg = FilePreviewMsg;
    type Widgets = FilePreviewWidgets;
    type Components = ();
}

impl ComponentUpdate<AppModel> for FilePreviewModel {
    fn init_model(_parent_model: &AppModel) -> Self {
        FilePreviewModel { file: None }
    }

    fn update(
        &mut self,
        msg: FilePreviewMsg,
        _components: &(),
        _sender: Sender<FilePreviewMsg>,
        _parent_sender: Sender<AppMsg>,
    ) {
        info!("received message: {:?}", msg);

        self.file = match msg {
            FilePreviewMsg::NewSelection(path) if path.is_dir() => None,
            FilePreviewMsg::NewSelection(path) => {
                // TODO: make async?
                let contents = read_start_of_file(&path).unwrap_or_default();

                // FIXME: gio::content_type_guess doesn't let you pass `None` for `data`, but we
                // should do this if we're unable to read the file. See gtk-rs/gir#1133.
                let (content_type, uncertain) =
                    gio::content_type_guess(Some(&path.to_string_lossy()), &contents);

                let mime = gio::content_type_get_mime_type(&content_type)
                    .expect("unable to determine mime type")
                    .parse::<Mime>()
                    .expect("could not parse guessed mime type");

                info!("identified file as {}, uncertain: {}", mime, uncertain);

                let preview = match (mime.type_(), mime.subtype()) {
                    (mime::IMAGE, _) => FilePreview::Image(gio::File::for_path(&path)),
                    _ if is_plain_text(&mime) => {
                        FilePreview::Text(String::from_utf8_lossy(&contents).into())
                    }
                    _ => FilePreview::Icon(gio::content_type_get_icon(&content_type)),
                };

                Some(FileInfo { path, preview })
            }
        }
    }
}

#[derive(Debug)]
pub enum FilePreviewMsg {
    NewSelection(PathBuf),
}

#[relm4_macros::widget(pub)]
impl Widgets<FilePreviewModel, AppModel> for FilePreviewWidgets {
    view! {
        gtk::Box {
            set_baseline_position: gtk::BaselinePosition::Center,
            set_orientation: gtk::Orientation::Vertical,
            set_valign: gtk::Align::Center,
            set_visible: watch! { model.file.is_some() },
            append: image = &gtk::Image {
                set_visible: false,
                set_icon_size: gtk::IconSize::Large,
            },
            append: picture = &gtk::Picture {
                set_visible: false,
            },
            append: text_container = &gtk::ScrolledWindow {
                set_visible: false,

                set_child: text = Some(&gtk::TextView) {
                    set_editable: false,
                }
            },
            append = &gtk::Label {
                set_label?: watch! {
                    &model.file.as_ref()
                        .and_then(|file| file.path.file_name())
                        .map(|name| name.to_string_lossy())
                },
            }
        }
    }

    fn manual_view(&self) {
        let file = match &model.file {
            Some(file) => file,
            None => return,
        };

        self.picture.set_visible(false);
        self.image.set_visible(false);
        self.text_container.set_visible(false);

        match &file.preview {
            FilePreview::Image(file) => {
                self.picture.set_file(Some(file));
                self.picture.set_visible(true);
            }
            FilePreview::Icon(icon) => {
                self.image.set_gicon(Some(icon));
                self.image.set_visible(true);
            }
            FilePreview::Text(text) => {
                self.text.buffer().set_text(text);
                self.text_container.set_visible(true);
            }
        }
    }
}

fn read_start_of_file(path: &Path) -> io::Result<Vec<u8>> {
    use std::io::Read;

    let mut f = File::open(path)?;

    let mut buf = vec![0; PREVIEW_BUFFER_SIZE];
    let n = f.read(&mut buf)?;
    buf.truncate(n);

    Ok(buf)
}

/// Returns `true` for mime types that are "reasonably" readable as plain text.
///
/// The definition of "reasonably" is intentionally left vague...
fn is_plain_text(mime: &Mime) -> bool {
    match (mime.type_(), mime.subtype()) {
        (mime::TEXT, _) => true,
        _ => false,
    }
}
