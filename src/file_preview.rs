//! Widget that displays file metadata and a small preview.

use std::io::{self, prelude::*};

use glib::GString;
use gtk::{gdk, gio, glib};
use itertools::{Itertools, MinMaxResult};
use log::*;
use mime::Mime;
use relm4::gtk::prelude::*;
use relm4::{adw, gtk, ComponentParts, ComponentSender, SimpleComponent};
use sourceview::{prelude::*, Language};
use sourceview5 as sourceview;

use crate::directory_list::FileSelection;
use crate::util;

/// The buffer size used to read the beginning of a file to predict its mime type and preview its
/// contents.
const PREVIEW_BUFFER_SIZE: usize = 4096;

/// String displayed if some information was unable to be determined, such as the creation time.
const MISSING_INFO: &str = "—";

#[derive(Debug)]
enum FilePreview {
    /// Text to be displayed in a [`FilePreviewWidgets::text`].
    Text(String, Option<Language>),

    /// Image file, to be displayed in [`FilePreviewWidgets::picture`].
    Image(gio::File),

    /// Non-text, non-image file to be previewed as an icon in [`FilePreviewWidgets::image`].
    Icon(gdk::Paintable),
}

#[derive(Debug)]
struct FileInfo {
    file: gio::File,
    info: gio::FileInfo,
    contents: Vec<u8>,
}

#[derive(Debug)]
pub struct FilePreviewModel {
    info: Vec<FileInfo>,
    preview: Option<FilePreview>,
    file_name_text: String,
    file_type_text: String,
    created_text: String,
    modified_text: String,
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
                set_visible: !model.info.is_empty(),

                #[name = "stack"]
                gtk::Stack {
                    add_css_class: "file-preview",
                    set_vhomogeneous: false,

                    #[name = "image"]
                    gtk::Image {
                        set_hexpand: true,
                        set_icon_size: gtk::IconSize::Large,
                    },

                    #[name = "picture"]
                    gtk::Picture {
                        add_css_class: "bordered",
                        set_hexpand: true,
                    },

                    #[name = "text_container"]
                    gtk::ScrolledWindow {
                        add_css_class: "bordered",
                        set_hexpand: true,
                        set_propagate_natural_height: true,
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
                        #[watch]
                        set_text: &model.file_name_text,

                        add_css_class: "file-name",
                        set_hexpand: true,
                        set_halign: gtk::Align::Start,
                    },
                    attach[0, 1, 2, 1]: file_type = &gtk::Label {
                        #[watch]
                        set_text: &model.file_type_text,

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
                        #[watch]
                        set_text: &model.created_text,
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
                        #[watch]
                        set_text: &model.modified_text,
                        add_css_class: "info-value",
                        set_halign: gtk::Align::End,
                    },
                }
            }
        }
    }

    fn init(_: (), root: &Self::Root, _sender: ComponentSender<Self>) -> ComponentParts<Self> {
        let model = FilePreviewModel {
            info: vec![],
            created_text: String::new(),
            file_name_text: String::new(),
            file_type_text: String::new(),
            modified_text: String::new(),
            preview: None,
        };

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

        let selection = match msg {
            FilePreviewMsg::Hide => {
                self.info = vec![];
                return;
            }
            // If the only selected file is a directory, then the preview will be hidden.
            FilePreviewMsg::NewSelection(selection) if is_single_directory(&selection) => {
                self.info = vec![];
                return;
            }
            FilePreviewMsg::NewSelection(selection) => selection,
        };

        let info = selection
            .files
            .into_iter()
            .map(|file| {
                // TODO: make async?
                let file_info = file.query_info(
                    &[
                        *gio::FILE_ATTRIBUTE_STANDARD_CONTENT_TYPE,
                        *gio::FILE_ATTRIBUTE_STANDARD_DISPLAY_NAME,
                        *gio::FILE_ATTRIBUTE_STANDARD_ICON,
                        *gio::FILE_ATTRIBUTE_STANDARD_IS_SYMLINK,
                        *gio::FILE_ATTRIBUTE_STANDARD_SIZE,
                        *gio::FILE_ATTRIBUTE_TIME_CREATED,
                        *gio::FILE_ATTRIBUTE_TIME_MODIFIED,
                    ]
                    .join(","),
                    gio::FileQueryInfoFlags::NONE,
                    gio::Cancellable::NONE,
                )?;

                let contents = if file
                    .query_file_type(gio::FileQueryInfoFlags::NONE, gio::Cancellable::NONE)
                    == gio::FileType::Regular
                {
                    read_start_of_file(&file).unwrap_or_default()
                } else {
                    vec![]
                };

                Ok(FileInfo {
                    file,
                    info: file_info,
                    contents,
                })
            })
            .collect::<Result<_, glib::Error>>();

        if let Err(e) = &info {
            warn!("unable to query file info: {}", e);
        }

        self.info = info.unwrap_or_default();
        if self.info.is_empty() {
            return;
        }

        match &self.info[..] {
            [] => (),
            [file] => {
                self.file_name_text = file.info.display_name().to_string();

                let content_type = file
                    .info
                    .content_type()
                    .unwrap_or_else(|| GString::from("application/octet-stream"));

                let mime = gio::content_type_get_mime_type(&content_type)
                    .expect("unable to determine mime type")
                    .parse::<Mime>()
                    .expect("could not parse guessed mime type");

                info!("identified {} as {}", file.info.display_name(), mime);

                self.file_type_text =
                    format!("{} — {}", mime, glib::format_size(file.info.size() as u64),);

                self.created_text = file
                    .info
                    .creation_date_time()
                    .as_ref()
                    .map_or(String::from(MISSING_INFO), format_datetime);

                self.modified_text = file
                    .info
                    .modification_date_time()
                    .as_ref()
                    .map_or(String::from(MISSING_INFO), format_datetime);

                let preview = match (mime.type_(), mime.subtype()) {
                    (mime::IMAGE, _) => FilePreview::Image(file.file.clone()),
                    _ if is_plain_text(&mime) && !file.contents.contains(&b'\0') => {
                        let language = sourceview::LanguageManager::default()
                            .guess_language(file.file.path(), Some(&content_type));
                        FilePreview::Text(String::from_utf8_lossy(&file.contents).into(), language)
                    }
                    _ => {
                        let icon_theme =
                            gtk::IconTheme::for_display(&gdk::Display::default().unwrap());
                        FilePreview::Icon(util::icon_for_file(&icon_theme, 128, &file.info))
                    }
                };

                info!("new preview: {:?}", preview);

                self.preview = Some(preview);
            }
            files => {
                self.file_name_text = format!("{} Documents", files.len());

                self.file_type_text =
                    glib::format_size(files.iter().map(|file| file.info.size() as u64).sum())
                        .to_string();
            }
        }
    }

    fn pre_view(&self, widgets: &mut Self::Widgets) {
        info!("preview: {:?}", self.preview);

        match &self.preview {
            Some(FilePreview::Image(file)) => {
                widgets.picture.set_file(Some(file));
                widgets.stack.set_visible_child(&widgets.picture);
            }
            Some(FilePreview::Icon(paintable)) => {
                widgets.image.set_paintable(Some(paintable));
                widgets.stack.set_visible_child(&widgets.image);
            }
            Some(FilePreview::Text(text, language)) => {
                widgets.text.buffer().set_text(text);

                let buffer = widgets
                    .text
                    .buffer()
                    .downcast::<sourceview::Buffer>()
                    .expect("sourceview was not backed by sourceview buffer");

                buffer.set_language(language.as_ref());

                widgets.stack.set_visible_child(&widgets.text_container);
            }
            None => (),
        }
    }
}

#[derive(Debug)]
pub enum FilePreviewMsg {
    /// Update the preview to show the contents of a new file.
    NewSelection(FileSelection),

    /// Empty the contents of the preview.
    Hide,
}

fn is_single_directory(selection: &FileSelection) -> bool {
    selection.files.len() == 1
        && selection.files[0].query_file_type(gio::FileQueryInfoFlags::NONE, gio::Cancellable::NONE)
            == gio::FileType::Directory
}

fn read_start_of_file(file: &gio::File) -> Result<Vec<u8>, io::Error> {
    let mut contents = Vec::with_capacity(PREVIEW_BUFFER_SIZE);

    let reader = file.read(gio::Cancellable::NONE).unwrap().into_read();

    let n = reader.take(PREVIEW_BUFFER_SIZE as u64).read_to_end(&mut contents)?;
    contents.truncate(n);

    Ok(contents)
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

/// Formats a [`GDateTime`](glib::DateTime) as a human-readable date string.
fn format_datetime(dt: &glib::DateTime) -> String {
    dt.format("%A, %B %-d, %Y at %-I:%M %p").unwrap().into()
}

fn format_datetime_range(dts: impl Iterator<Item = glib::DateTime>) -> String {
    let (min, max) = match dts.minmax() {
        MinMaxResult::NoElements => panic!("iterator cannot be empty"),
        MinMaxResult::OneElement(e) => (e.clone(), e),
        MinMaxResult::MinMax(min, max) => (min, max),
    };

    return String::new();
}
