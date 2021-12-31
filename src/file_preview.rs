//! Widget that displays file metadata and a small preview.

use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use chrono::{DateTime, Local};
use humansize::{file_size_opts::DECIMAL, FileSize};
use log::*;
use mime::Mime;
use relm4::gtk::prelude::*;
use relm4::{gtk, ComponentUpdate, Sender, Widgets};
use sourceview::prelude::*;
use sourceview5 as sourceview;

use super::{AppModel, AppMsg, Model};

/// The buffer size used to read the beginning of a file to predict its mime type and preview its
/// contents.
const PREVIEW_BUFFER_SIZE: usize = 4096;

/// The requested width of the text preview, in pixels.
const PREVIEW_WIDTH: i32 = 500;

/// The requested height of the text preview, in pixels.
const PREVIEW_HEIGHT: i32 = 400;

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
    mime: Mime,
    language: Option<sourceview::Language>,
    size: u64,
    created: SystemTime,
    modified: SystemTime,
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

                let path_str = path.to_string_lossy();

                // FIXME: gio::content_type_guess doesn't let you pass `None` for `data`, but we
                // should do this if we're unable to read the file. See gtk-rs/gir#1133.
                let (content_type, uncertain) = gio::content_type_guess(Some(&path_str), &contents);

                let language = sourceview::LanguageManager::default()
                    .expect("language manager is not available")
                    .guess_language(Some(&path_str), Some(&content_type));

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

                let (size, created, modified) = (|| -> io::Result<_> {
                    // Fall back to symlink_metadata to handle broken symlinks.
                    let metadata = path.metadata().or_else(|_| path.symlink_metadata())?;

                    let size = metadata.len();
                    let created = metadata.created()?;
                    let modified = metadata.modified()?;

                    Ok((size, created, modified))
                })()
                .unwrap_or_else(|e| {
                    info!("unable to read metadata: {}", e);
                    (0, SystemTime::UNIX_EPOCH, SystemTime::UNIX_EPOCH)
                });

                Some(FileInfo {
                    path,
                    language,
                    mime,
                    preview,
                    size,
                    created,
                    modified,
                })
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
            add_css_class: "file-preview-widget",
            set_baseline_position: gtk::BaselinePosition::Center,
            set_halign: gtk::Align::Center,
            set_orientation: gtk::Orientation::Vertical,
            set_valign: gtk::Align::Center,
            set_visible: watch! { model.file.is_some() },

            append = &gtk::Box {
                add_css_class: "file-preview",
                set_height_request: PREVIEW_HEIGHT,
                set_width_request: PREVIEW_WIDTH,
                append: image = &gtk::Image {
                    set_visible: false,
                    set_hexpand: true,
                    set_icon_size: gtk::IconSize::Large,
                },
                append: picture = &gtk::Picture {
                    add_css_class: "bordered",
                    set_visible: false,
                    set_can_shrink: true,
                    set_width_request: PREVIEW_WIDTH,
                },
                append: text_container = &gtk::ScrolledWindow {
                    add_css_class: "bordered",
                    set_visible: false,
                    set_overflow: gtk::Overflow::Hidden,

                    set_child: text = Some(&sourceview::View) {
                        add_css_class: "file-preview-source",
                        set_editable: false,
                        set_hexpand: true,
                        set_monospace: true,
                    }
                },
            },

            append = &gtk::Grid {
                add_css_class: "file-preview-info",
                attach(0, 0, 2, 1): file_name = &gtk::Label {
                    add_css_class: "file-name",
                    set_hexpand: true,
                    set_halign: gtk::Align::Start,
                },
                attach(0, 1, 2, 1): file_type = &gtk::Label {
                    add_css_class: iterate!(["file-type", "dim-label"]),
                    set_halign: gtk::Align::Start,
                },
                attach(0, 2, 2, 1) = &gtk::Label {
                    set_label: "Information",
                    add_css_class: "section-title",
                    set_halign: gtk::Align::Start,
                },
                attach(0, 3, 1, 1) = &gtk::Label {
                    set_label: "Created",
                    add_css_class: iterate!(["info-name", "dim-label"]),
                    set_halign: gtk::Align::Start,
                },
                attach(1, 3, 1, 1): created = &gtk::Label {
                    add_css_class: "info-value",
                    set_halign: gtk::Align::End,
                },
                attach(0, 4, 1, 1) = &gtk::Label {
                    set_label: "Modified",
                    add_css_class: iterate!(["info-name", "dim-label"]),
                    set_halign: gtk::Align::Start,
                },
                attach(1, 4, 1, 1): modified = &gtk::Label {
                    add_css_class: "info-value",
                    set_halign: gtk::Align::End,
                },
            }
        }
    }

    fn post_init(&self) {
        let buffer = text
            .buffer()
            .downcast::<sourceview::Buffer>()
            .expect("sourceview was not backed by sourceview buffer");

        if let Some(scheme) = &sourceview::StyleSchemeManager::new().scheme("oblivion") {
            buffer.set_style_scheme(Some(scheme));
        }
    }

    fn manual_view(&self) {
        let file = match &model.file {
            Some(file) => file,
            None => return,
        };

        self.file_name.set_text(
            &file
                .path
                .file_name()
                .expect("file must have a name")
                .to_string_lossy(),
        );
        self.file_type.set_text(&format!(
            "{} — {}",
            file.mime,
            file.size.file_size(DECIMAL).unwrap(),
        ));
        self.created.set_text(&format_system_time(file.created));
        self.modified.set_text(&format_system_time(file.modified));

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

                let buffer = self
                    .text
                    .buffer()
                    .downcast::<sourceview::Buffer>()
                    .expect("sourceview was not backed by sourceview buffer");

                buffer.set_language(file.language.as_ref());
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
    #[allow(clippy::match_like_matches_macro)]
    match (mime.type_().as_str(), mime.subtype().as_str()) {
        ("text", _) => true,
        ("application", "toml") => true,
        ("application", "x-shellscript") => true,
        _ => false,
    }
}

/// Formats a `SystemTime` as a human-readable date string.
fn format_system_time(time: SystemTime) -> String {
    let datetime: DateTime<Local> = time.into();
    datetime.format("%A, %B %-d, %Y at %-I:%M %p").to_string()
}
