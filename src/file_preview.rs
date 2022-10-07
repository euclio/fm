//! Widget that displays file metadata and a small preview.

use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use chrono::{DateTime, Local};
use gtk::{gdk, gio, glib};
use log::*;
use mime::Mime;
use relm4::gtk::prelude::*;
use relm4::{adw, gtk, ComponentParts, ComponentSender, SimpleComponent};
use sourceview::prelude::*;
use sourceview5 as sourceview;

use crate::util;

/// The buffer size used to read the beginning of a file to predict its mime type and preview its
/// contents.
const PREVIEW_BUFFER_SIZE: usize = 4096;

#[derive(Debug)]
enum FilePreview {
    /// Plain text to be displayed in a [`FilePreviewWidgets::text`].
    Text(String),

    /// Image file, to be displayed in [`FilePreviewWidgets::picture`].
    Image(gio::File),

    /// Non-text, non-image file to be previewed as an icon in [`FilePreviewWidgets::image`].
    Icon(gdk::Paintable),
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

#[relm4::component(pub)]
impl SimpleComponent for FilePreviewModel {
    type Widgets = FilePreviewWidgets;
    type Init = ();
    type Input = FilePreviewMsg;
    type Output = ();

    view! {
        adw::Clamp {
            gtk::Box {
                add_css_class: "file-preview-widget",
                set_baseline_position: gtk::BaselinePosition::Center,
                set_orientation: gtk::Orientation::Vertical,
                set_valign: gtk::Align::Center,
                #[watch]
                set_visible: model.file.is_some(),

                gtk::Box {
                    add_css_class: "file-preview",

                    #[name = "image"]
                    gtk::Image {
                        set_visible: false,
                        set_hexpand: true,
                        set_icon_size: gtk::IconSize::Large,
                    },

                    #[name = "picture"]
                    gtk::Picture {
                        add_css_class: "bordered",
                        set_visible: false,
                        set_hexpand: true,
                    },

                    #[name = "text_container"]
                    gtk::ScrolledWindow {
                        add_css_class: "bordered",
                        set_hexpand: true,
                        set_propagate_natural_height: true,
                        set_visible: false,
                        set_overflow: gtk::Overflow::Hidden,

                        #[name = "text"]
                        sourceview::View {
                            add_css_class: "file-preview-source",
                            set_cursor_visible: false,
                            set_editable: false,
                            set_monospace: true,
                        }
                    },
                },

                gtk::Grid {
                    add_css_class: "file-preview-info",
                    attach[0, 0, 2, 1]: file_name = &gtk::Label {
                        add_css_class: "file-name",
                        set_hexpand: true,
                        set_halign: gtk::Align::Start,
                    },
                    attach[0, 1, 2, 1]: file_type = &gtk::Label {
                        #[iterate]
                        add_css_class: ["file-type", "dim-label"],
                        set_halign: gtk::Align::Start,
                    },
                    attach[0, 2, 2, 1] = &gtk::Label {
                        set_label: "Information",
                        add_css_class: "section-title",
                        set_halign: gtk::Align::Start,
                    },
                    attach[0, 3, 1, 1] = &gtk::Label {
                        set_label: "Created",
                        #[iterate]
                        add_css_class: ["info-name", "dim-label"],
                        set_halign: gtk::Align::Start,
                    },
                    attach[1, 3, 1, 1]: created = &gtk::Label {
                        add_css_class: "info-value",
                        set_halign: gtk::Align::End,
                    },
                    attach[0, 4, 1, 1] = &gtk::Label {
                        set_label: "Modified",
                        #[iterate]
                        add_css_class: ["info-name", "dim-label"],
                        set_halign: gtk::Align::Start,
                    },
                    attach[1, 4, 1, 1]: modified = &gtk::Label {
                        add_css_class: "info-value",
                        set_halign: gtk::Align::End,
                    },
                }
            }
        }
    }

    fn init(_: (), root: &Self::Root, _sender: ComponentSender<Self>) -> ComponentParts<Self> {
        let model = FilePreviewModel { file: None };

        let widgets = view_output!();

        let buffer = widgets
            .text
            .buffer()
            .downcast::<sourceview::Buffer>()
            .expect("sourceview was not backed by sourceview buffer");

        if let Some(scheme) = &sourceview::StyleSchemeManager::new().scheme("oblivion") {
            buffer.set_style_scheme(Some(scheme));
        }

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: FilePreviewMsg, _sender: ComponentSender<Self>) {
        info!("received message: {:?}", msg);

        self.file = match msg {
            FilePreviewMsg::Hide => None,
            FilePreviewMsg::NewSelection(path) if path.is_dir() => None,
            FilePreviewMsg::NewSelection(path) => {
                // TODO: make async?
                let file_info = match gio::File::for_path(&path).query_info(
                    &[
                        *gio::FILE_ATTRIBUTE_STANDARD_CONTENT_TYPE,
                        *gio::FILE_ATTRIBUTE_STANDARD_ICON,
                        *gio::FILE_ATTRIBUTE_STANDARD_IS_SYMLINK,
                    ]
                    .join(","),
                    gio::FileQueryInfoFlags::NONE,
                    gio::Cancellable::NONE,
                ) {
                    Ok(info) => info,
                    Err(e) => {
                        warn!("unable to query file info: {}", e);
                        return;
                    }
                };

                let content_type = file_info.content_type().unwrap();

                let contents = if path.is_file() {
                    read_start_of_file(&path).unwrap_or_default()
                } else {
                    Vec::default()
                };

                let language = sourceview::LanguageManager::default()
                    .guess_language(Some(&path), Some(&content_type));

                let mime = gio::content_type_get_mime_type(&content_type)
                    .expect("unable to determine mime type")
                    .parse::<Mime>()
                    .expect("could not parse guessed mime type");

                info!("identified file as {}", mime);

                let preview = match (mime.type_(), mime.subtype()) {
                    (mime::IMAGE, _) => FilePreview::Image(gio::File::for_path(&path)),
                    _ if is_plain_text(&mime) => {
                        FilePreview::Text(String::from_utf8_lossy(&contents).into())
                    }
                    _ => {
                        let icon_theme =
                            gtk::IconTheme::for_display(&gdk::Display::default().unwrap());
                        FilePreview::Icon(util::icon_for_file(&icon_theme, 128, &file_info))
                    }
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

    fn pre_view(&self, widgets: &mut Self::Widgets) {
        if let Some(file) = &self.file {
            widgets.file_name.set_text(
                &file
                    .path
                    .file_name()
                    .expect("file must have a name")
                    .to_string_lossy(),
            );
            widgets.file_type.set_text(&format!(
                "{} — {}",
                file.mime,
                glib::format_size(file.size),
            ));
            widgets.created.set_text(&format_system_time(file.created));
            widgets
                .modified
                .set_text(&format_system_time(file.modified));

            widgets.picture.set_visible(false);
            widgets.image.set_visible(false);
            widgets.text_container.set_visible(false);

            match &file.preview {
                FilePreview::Image(file) => {
                    widgets.picture.set_file(Some(file));
                    widgets.picture.set_visible(true);
                }
                FilePreview::Icon(paintable) => {
                    widgets.image.set_paintable(Some(paintable));
                    widgets.image.set_visible(true);
                }
                FilePreview::Text(text) => {
                    widgets.text.buffer().set_text(text);
                    widgets.text_container.set_visible(true);

                    let buffer = widgets
                        .text
                        .buffer()
                        .downcast::<sourceview::Buffer>()
                        .expect("sourceview was not backed by sourceview buffer");

                    buffer.set_language(file.language.as_ref());
                }
            }
        }
    }
}

#[derive(Debug)]
pub enum FilePreviewMsg {
    /// Update the preview to show the contents of a new file.
    NewSelection(PathBuf),

    /// Empty the contents of the preview.
    Hide,
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
        ("application", "javascript") => true,
        ("application", "json") => true,
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
