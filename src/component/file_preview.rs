//! Widget that displays file metadata and a small preview.

use std::error::Error;
use std::io;

use futures::stream::{AbortHandle, Abortable, Aborted};
use futures::{future, prelude::*};
use glib::GString;
use gtk::{gdk, gio, glib};
use itertools::{Itertools, MinMaxResult};
use mime::Mime;
use relm4::gtk::prelude::*;
use relm4::prelude::*;
use sourceview::{prelude::*, Language};
use sourceview5 as sourceview;
use tracing::*;

use super::directory_list::FileSelection;
use crate::util::{self, pluralize};

mod pdf;

use pdf::{Pdf, PdfPageChange};

/// The buffer size used to read the beginning of a file to predict its mime type and preview its
/// contents.
const PREVIEW_BUFFER_SIZE: usize = 4096;

/// Date format used when a single file is selected.
const LONG_DATE_FORMAT: &str = "%A, %B %-d, %Y at %-I:%M %p";

/// Date format used when multiple files are selected.
const SHORT_DATE_FORMAT: &str = "%b %-d, %Y";

/// String displayed if some information was unable to be determined, such as the creation time.
const MISSING_INFO: &str = "—";

#[derive(Debug)]
enum FilePreview {
    /// Text to be displayed in a [`FilePreviewWidgets::text`].
    Text(String, Option<Language>),

    /// Image file, to be displayed in [`FilePreviewWidgets::picture`].
    Image(gio::File),

    /// Video preview.
    Video(gio::File),

    /// PDF document.
    Pdf(Pdf),

    /// Non-text, non-image file to be previewed as an icon in [`FilePreviewWidgets::image`].
    Icon(gdk::Paintable),

    /// An error occurred while loading the file.
    Error(Box<dyn Error>),
}

#[derive(Debug)]
pub struct FileInfo {
    file: gio::File,
    info: gio::FileInfo,
    mime: Mime,
    contents: Option<Vec<u8>>,
}

#[derive(Debug)]
pub struct FilePreviewModel {
    info: Vec<FileInfo>,
    preview: Option<FilePreview>,
    abort_preview: Option<AbortHandle>,
    file_name_text: String,
    file_type_text: String,
    created_text: String,
    modified_text: String,
}

impl FilePreviewModel {
    fn update_single_file_preview(
        &mut self,
        widgets: &mut FilePreviewWidgets,
        sender: ComponentSender<Self>,
    ) {
        assert!(self.info.len() == 1);
        let file = &self.info[0];

        self.file_name_text = file.info.display_name().to_string();

        self.file_type_text = format!(
            "{} — {}",
            file.mime,
            glib::format_size(file.info.size() as u64)
        );

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

        let preview = match (file.mime.type_(), file.mime.subtype()) {
            (mime::IMAGE, _) => {
                let gfile = file.file.clone();

                // Texture loading can be expensive and may block the UI thread.
                widgets.spinner.start();
                widgets.stack.set_visible_child(&widgets.spinner);
                sender.oneshot_command(async move {
                    let texture_result = gdk::Texture::from_file(&gfile);

                    FilePreviewCommand::TextureLoaded(gfile, texture_result)
                });

                FilePreview::Image(file.file.clone())
            }
            (mime::VIDEO, _) => {
                FilePreview::Video(file.file.clone())
            }
            (_, mime::PDF) => {
                // TODO: This should be async.
                match poppler::Document::from_gfile(&file.file, None, gio::Cancellable::NONE) {
                    Ok(document) => FilePreview::Pdf(Pdf::new(document)),
                    Err(e) => {
                        error!("error loading PDF: {}", e);

                        FilePreview::Error(Box::new(e))
                    }
                }
            }
            _ => match &file.contents {
                Some(contents) if !contents.contains(&b'\0') => {
                    let language = sourceview::LanguageManager::default()
                        .guess_language(file.file.path(), Some(&file.info.content_type().unwrap()));
                    FilePreview::Text(String::from_utf8_lossy(contents).into(), language)
                }
                _ => {
                    let icon_theme = gtk::IconTheme::for_display(&gdk::Display::default().unwrap());
                    FilePreview::Icon(util::icon_for_file(&icon_theme, 512, &file.info))
                }
            },
        };

        info!("new preview: {:?}", preview);

        self.preview = Some(preview);
    }

    fn update_multiple_file_preview(&mut self) {
        let files = &self.info;

        self.file_name_text = format!("{} items", files.len());

        self.file_type_text = format!(
            "{} — {}",
            format_item_types(files),
            glib::format_size(files.iter().map(|file| file.info.size() as u64).sum())
        );

        self.created_text =
            format_datetime_range(files.iter().flat_map(|f| f.info.creation_date_time()));
        self.modified_text =
            format_datetime_range(files.iter().flat_map(|f| f.info.modification_date_time()));

        let icon_theme = gtk::IconTheme::for_display(&gdk::Display::default().unwrap());

        let icon_paintable = icon_theme
            .lookup_icon(
                "view-paged-symbolic",
                &[],
                128,
                1,
                gtk::TextDirection::Ltr,
                gtk::IconLookupFlags::empty(),
            )
            .upcast::<gdk::Paintable>();

        self.preview = Some(FilePreview::Icon(icon_paintable));
    }
}

#[derive(Debug)]
pub enum FilePreviewCommand {
    /// A texture has finished loading.
    TextureLoaded(gio::File, Result<gdk::Texture, glib::Error>),
}

#[relm4::component(pub)]
impl Component for FilePreviewModel {
    type Widgets = FilePreviewWidgets;
    type Init = ();
    type Input = FilePreviewMsg;
    type Output = ();
    type CommandOutput = FilePreviewCommand;

    view! {
        adw::Clamp {
            gtk::Box {
                add_css_class: "file-preview-widget",
                set_orientation: gtk::Orientation::Vertical,
                set_vexpand: true,
                #[watch]
                set_visible: !model.info.is_empty(),

                #[name = "stack"]
                gtk::Stack {
                    add_css_class: "file-preview",
                    set_vhomogeneous: false,

                    #[name = "spinner"]
                    gtk::Spinner {
                        set_halign: gtk::Align::Center,
                        set_valign: gtk::Align::Center,
                    },

                    #[name = "icon"]
                    adw::Clamp {
                        set_maximum_size: 256,

                        #[name = "icon_picture"]
                        gtk::Picture {
                            set_hexpand: true,
                        },
                    },

                    #[name = "picture"]
                    gtk::Picture {
                        add_css_class: "bordered",
                        set_halign: gtk::Align::Center,
                        set_hexpand: true,
                        set_valign: gtk::Align::Center,
                        set_vexpand: true,
                    },

                    #[name = "text_container"]
                    gtk::ScrolledWindow {
                        add_css_class: "bordered",
                        set_hexpand: true,
                        set_propagate_natural_height: true,
                        set_overflow: gtk::Overflow::Hidden,
                        set_valign: gtk::Align::Center,

                        #[name = "text"]
                        sourceview::View {
                            add_css_class: "file-preview-source",
                            set_cursor_visible: false,
                            set_editable: false,
                            set_monospace: true,
                            set_valign: gtk::Align::Center,
                        }
                    },

                    #[name = "video"]
                    gtk::Video {

                    },

                    #[name = "pdf_container"]
                    gtk::AspectFrame {

                        set_obey_child: false,

                        #[wrap(Some)]
                        set_child = &gtk::Overlay {

                            #[name = "pdf"]
                            gtk::DrawingArea {
                                add_css_class: "bordered",
                                set_hexpand: true,
                                set_vexpand: true,
                            },

                            #[name = "pdf_previous_button"]
                            add_overlay = &gtk::Button {
                                set_icon_name: "go-previous-symbolic",
                                add_css_class: "osd",
                                set_margin_start: 5,
                                set_halign: gtk::Align::Start,
                                set_valign: gtk::Align::Center,
                                connect_clicked =>
                                    FilePreviewMsg::ChangePdfPage(PdfPageChange::Previous),
                            },

                            #[name = "pdf_next_button"]
                            add_overlay = &gtk::Button {
                                set_icon_name: "go-next-symbolic",
                                add_css_class: "osd",
                                set_margin_end: 5,
                                set_halign: gtk::Align::End,
                                set_valign: gtk::Align::Center,
                                connect_clicked =>
                                    FilePreviewMsg::ChangePdfPage(PdfPageChange::Next),
                            },
                        }
                    },

                    #[name = "error"]
                    adw::StatusPage {
                        set_icon_name: Some("dialog-warning-symbolic"),
                        set_title: "Cannot Display Preview",
                        set_description: Some(""),
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
            abort_preview: None,
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

    fn update_with_view(
        &mut self,
        widgets: &mut Self::Widgets,
        msg: FilePreviewMsg,
        sender: ComponentSender<Self>,
        _: &Self::Root,
    ) {
        info!("received message: {:?}", msg);

        match msg {
            FilePreviewMsg::Hide => {
                self.info = vec![];
                self.update_view(widgets, sender);
                return;
            }
            FilePreviewMsg::NewSelection(selection) => {
                let (abort_handle, abort_registration) = AbortHandle::new_pair();

                if let Some(handle) = self.abort_preview.replace(abort_handle) {
                    handle.abort();
                }

                let query_info_fut =
                    Abortable::new(query_selection_info(selection), abort_registration);

                widgets.spinner.start();
                widgets.stack.set_visible_child(&widgets.spinner);

                let sender = sender.clone();
                relm4::spawn_local(async move {
                    match query_info_fut.await {
                        Err(Aborted) => (),
                        Ok(info) => sender.input(FilePreviewMsg::FileInfoLoaded(info)),
                    }
                });
            }
            FilePreviewMsg::FileInfoLoaded(Err(e)) => {
                self.abort_preview.take();

                error!("error while loading preview: {}", e);

                self.preview = Some(FilePreview::Error(Box::new(e)));
            }
            FilePreviewMsg::FileInfoLoaded(Ok(info)) => {
                self.abort_preview.take();

                info!("new file info: {:?}", info);

                self.info = info;

                match self.info.len() {
                    0 => (),
                    1 => self.update_single_file_preview(widgets, sender.clone()),
                    _ => self.update_multiple_file_preview(),
                }
            }
            FilePreviewMsg::ChangePdfPage(change) => {
                if let Some(FilePreview::Pdf(pdf)) = &mut self.preview {
                    pdf.update_page(change);

                    if let Some(page) = pdf.current_page() {
                        let (w, h) = page.size();
                        widgets.pdf_container.set_ratio((w / h) as f32);
                    }
                }
            }
        };

        self.update_view(widgets, sender);
    }

    fn update_cmd_with_view(
        &mut self,
        widgets: &mut Self::Widgets,
        message: Self::CommandOutput,
        _: ComponentSender<Self>,
        _: &Self::Root,
    ) {
        if let FilePreviewCommand::TextureLoaded(file, Ok(texture)) = message {
            if matches!(&self.preview, Some(FilePreview::Image(f)) if *f == file) {
                widgets.picture.set_paintable(Some(&texture));
                widgets.stack.set_visible_child(&widgets.picture);
            }
        }
    }

    fn pre_view(&self, widgets: &mut Self::Widgets) {
        info!("preview: {:?}", self.preview);

        match &self.preview {
            Some(FilePreview::Image(_)) => (),
            Some(FilePreview::Icon(paintable)) => {
                widgets.icon_picture.set_paintable(Some(paintable));
                widgets.stack.set_visible_child(&widgets.icon);
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
            Some(FilePreview::Video(file)) => {
                widgets.video.set_file(Some(file));
                widgets.stack.set_visible_child(&widgets.video);
            }
            Some(FilePreview::Pdf(pdf)) => {
                if let Some(page) = pdf.current_page() {
                    widgets
                        .pdf_previous_button
                        .set_visible(pdf.has_previous_page());
                    widgets.pdf_next_button.set_visible(pdf.has_next_page());

                    let (w, h) = page.size();
                    widgets.pdf_container.set_ratio((w / h) as f32);

                    widgets.pdf.set_draw_func(move |_, ctx, w, h| {
                        ctx.set_source_rgb(1.0, 1.0, 1.0);
                        ctx.paint().unwrap();

                        let (page_w, page_h) = page.size();

                        ctx.identity_matrix();

                        ctx.scale(f64::from(w) / page_w, f64::from(h) / page_h);

                        page.render(ctx);
                    });
                }

                widgets.stack.set_visible_child(&widgets.pdf_container);
            }
            Some(FilePreview::Error(e)) => {
                widgets.error.set_description(Some(&e.to_string()));
                widgets.stack.set_visible_child(&widgets.error);
            }
            None => (),
        }
    }
}

#[derive(Debug)]
pub enum FilePreviewMsg {
    /// Update the preview to show the contents of a new file.
    NewSelection(FileSelection),

    /// Queried file information is now available.
    FileInfoLoaded(Result<Vec<FileInfo>, glib::Error>),

    /// Change PDF page.
    ChangePdfPage(PdfPageChange),

    /// Empty the contents of the preview.
    Hide,
}

/// Query the relevant file info for the selection. The info will be returned in the same order as
/// the files in the selection.
async fn query_selection_info(selection: FileSelection) -> Result<Vec<FileInfo>, glib::Error> {
    // Fast path: if the only selected file is a directory, it will be hidden.
    if selection.files.len() == 1 {
        let info = selection.files[0]
            .query_info_future(
                gio::FILE_ATTRIBUTE_STANDARD_TYPE,
                gio::FileQueryInfoFlags::NONE,
                glib::PRIORITY_DEFAULT,
            )
            .await;

        if let Ok(info) = info {
            if info.file_type() == gio::FileType::Directory {
                return Ok(vec![]);
            }
        }
    }

    let attributes = [
        &**gio::FILE_ATTRIBUTE_STANDARD_CONTENT_TYPE,
        &**gio::FILE_ATTRIBUTE_STANDARD_DISPLAY_NAME,
        &**gio::FILE_ATTRIBUTE_STANDARD_ICON,
        &**gio::FILE_ATTRIBUTE_STANDARD_TYPE,
        &**gio::FILE_ATTRIBUTE_STANDARD_IS_SYMLINK,
        &**gio::FILE_ATTRIBUTE_STANDARD_SIZE,
        &**gio::FILE_ATTRIBUTE_TIME_CREATED,
        &**gio::FILE_ATTRIBUTE_TIME_MODIFIED,
    ]
    .join(",");

    let is_single_file = selection.files.len() == 1;
    let selection_info = future::join_all(selection.files.into_iter().map(|file| async {
        let info = file
            .query_info_future(
                &attributes,
                gio::FileQueryInfoFlags::NONE,
                glib::PRIORITY_DEFAULT,
            )
            .await;

        if let Err(e) = &info {
            warn!("unable to query file info: {}", e);
        }

        match info {
            Ok(info) => {
                let content_type = info
                    .content_type()
                    .unwrap_or_else(|| GString::from("application/octet-stream"));
                let mime = gio::content_type_get_mime_type(&content_type)
                    .expect("unable to determine mime type")
                    .parse::<Mime>()
                    .expect("could not parse guessed mime type");

                // Binary data will not be previewed.
                let contents = if is_single_file && is_plain_text(&mime) {
                    Some(read_start_of_file(&file).await.unwrap_or_default())
                } else {
                    None
                };

                Ok(FileInfo {
                    file,
                    info,
                    mime,
                    contents,
                })
            }
            Err(e) => {
                warn!("unable to query file info: {}", e);
                Err(e)
            }
        }
    }));

    selection_info.await.into_iter().collect()
}

/// Return at most a single I/O buffer's worth of a file's contents from the beginning.
async fn read_start_of_file(file: &gio::File) -> Result<Vec<u8>, io::Error> {
    let mut contents = Vec::with_capacity(PREVIEW_BUFFER_SIZE);

    let reader = file
        .read_future(glib::PRIORITY_DEFAULT)
        .await
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?
        .into_async_buf_read(PREVIEW_BUFFER_SIZE);

    let n = reader
        .take(PREVIEW_BUFFER_SIZE as u64)
        .read_to_end(&mut contents)
        .await?;
    contents.truncate(n);

    Ok(contents)
}

/// Returns `true` for mime types that are "reasonably" readable as plain text.
///
/// The definition of "reasonably" is intentionally left vague...
#[rustfmt::skip]
fn is_plain_text(mime: &Mime) -> bool {
    matches!(
        (mime.type_().as_str(), mime.subtype().as_str()),
            | ("text", _)
            | ("application", "javascript")
            | ("application", "json")
            | ("application", "toml")
            | ("application", "x-shellscript")
            | ("application", "xml")
    )
}

/// Produces a description of the types of a group of files.
fn format_item_types(files: &[FileInfo]) -> String {
    let mut documents = 0;
    let mut folders = 0;

    for file in files.iter() {
        if file.info.file_type() == gio::FileType::Directory {
            folders += 1;
        } else {
            documents += 1;
        }
    }

    match (documents, folders) {
        (0, _) => format!("{} folder{}", folders, pluralize!(folders)),
        (_, 0) => format!("{} document{}", documents, pluralize!(documents)),
        (_, _) => format!(
            "{} document{}, {} folder{}",
            documents,
            pluralize!(documents),
            folders,
            pluralize!(folders)
        ),
    }
}

/// Formats a [`GDateTime`](glib::DateTime) as a human-readable date string.
fn format_datetime(dt: &glib::DateTime) -> String {
    dt.format(LONG_DATE_FORMAT).unwrap().into()
}

/// Formats an iterator of [`GDateTime](glib::DateTime) objects as a range between the earliest and
/// latest times.
fn format_datetime_range(dts: impl Iterator<Item = glib::DateTime>) -> String {
    let (min, max) = match dts.minmax() {
        MinMaxResult::NoElements => return MISSING_INFO.to_string(),
        MinMaxResult::OneElement(e) => (e.clone(), e),
        MinMaxResult::MinMax(min, max) => (min, max),
    };

    if min.ymd() == max.ymd() {
        min.format(SHORT_DATE_FORMAT).unwrap().to_string()
    } else {
        format!(
            "{} — {}",
            min.format(SHORT_DATE_FORMAT).unwrap(),
            max.format(SHORT_DATE_FORMAT).unwrap()
        )
    }
}
