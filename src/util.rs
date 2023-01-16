//! Utility functions.

use std::{
    fmt::{self, Debug},
    iter::{self, Chain, Once},
};

use relm4::gtk::{self, gdk, gio, glib, prelude::*};

mod emblemed_paintable;

use emblemed_paintable::EmblemedPaintable;

/// Extension functions for [`Result`]s containing [`GError`](glib::Error)s.
pub trait GResultExt {
    /// Filter out [`gio::IOErrorEnum::FailedHandled`] errors, since these indicate that the error
    /// was already handled.
    fn filter_handled(self) -> Self;
}

impl GResultExt for Result<(), glib::Error> {
    fn filter_handled(self) -> Self {
        if let Some(gio::IOErrorEnum::FailedHandled) = self
            .as_ref()
            .err()
            .and_then(|e| e.kind::<gio::IOErrorEnum>())
        {
            Ok(())
        } else {
            self
        }
    }
}

/// Returns a [`gdk::Paintable`] that should be used for file icons for files.
///
/// This will usually correspond to [`gio::FileInfo::gicon`], but for symlinks an additional
/// symlink emblem will be added to the bottom left. For this to work correctly, the file info must
/// have been queried with the `standard::is-symlink` attribute.
pub fn icon_for_file(
    theme: &gtk::IconTheme,
    size: i32,
    file_info: &gio::FileInfo,
) -> gdk::Paintable {
    let icon = file_info
        .icon()
        .unwrap_or_else(|| gio::Icon::for_string("text-x-generic").unwrap());

    let icon_paintable = theme
        .lookup_by_gicon(
            &icon,
            size,
            1,
            gtk::TextDirection::Ltr,
            gtk::IconLookupFlags::empty(),
        )
        .upcast::<gdk::Paintable>();

    if file_info.is_symlink() && theme.has_icon("emblem-symbolic-link") {
        let emblem = theme
            .lookup_icon(
                "emblem-symbolic-link",
                &[],
                size,
                1,
                gtk::TextDirection::Ltr,
                gtk::IconLookupFlags::empty(),
            )
            .upcast::<gdk::Paintable>();

        EmblemedPaintable::new(&icon_paintable, &emblem).upcast()
    } else {
        icon_paintable
    }
}

/// Format a [`GFile`](gio::File) as its URI for nicer [`Debug`] output.
pub fn fmt_file_as_uri(file: &gio::File, f: &mut fmt::Formatter) -> fmt::Result {
    f.write_str(&file.uri())
}

/// Format a slice of [`GFile`](gio::File)s as URIs for nicer [`Debug`] output.
pub fn fmt_files_as_uris(files: &[gio::File], f: &mut fmt::Formatter) -> fmt::Result {
    f.debug_list()
        .entries(files.iter().map(GFileDebug))
        .finish()
}

struct GFileDebug<'a>(&'a gio::File);

impl Debug for GFileDebug<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt_file_as_uri(self.0, f)
    }
}

/// Extension methods for [`gtk::Bitset`].
pub trait BitsetExt {
    /// Iterate directly over the bitset.
    fn iter(&self) -> BitsetIter;
}

pub struct BitsetIter<'a>(Option<Chain<Once<u32>, gtk::BitsetIter<'a>>>);

impl Iterator for BitsetIter<'_> {
    type Item = u32;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.as_mut().and_then(|it| it.next())
    }
}

impl BitsetExt for gtk::Bitset {
    fn iter(&self) -> BitsetIter {
        match gtk::BitsetIter::init_first(self) {
            Some((iter, first_value)) => BitsetIter(Some(iter::once(first_value).chain(iter))),
            None => BitsetIter(None),
        }
    }
}

/// Extension methods for `[gio::FileInfo]`.
pub trait GFileInfoExt {
    /// [`gio::FileInfo`]s managed by a [`gtk::DirectoryList`] have the "standard::file" attribute
    /// set to the [`gio::File`] that they refer to. This extension method handles the extraction
    /// of the object.
    fn file(&self) -> Option<gio::File>;
}

impl GFileInfoExt for gio::FileInfo {
    fn file(&self) -> Option<gio::File> {
        let file = self
            .attribute_object("standard::file")?
            .downcast::<gio::File>()
            .unwrap();
        Some(file)
    }
}

/// Returns "s" if the provided expression is not equal to 1, otherwise the empty string.
macro_rules! pluralize {
    ($e:expr) => {
        if $e != 1 {
            "s"
        } else {
            ""
        }
    };
}
pub(crate) use pluralize;
