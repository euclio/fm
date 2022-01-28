use relm4::gtk::{self, gdk, gio, prelude::*};

mod emblemed_paintable;

use emblemed_paintable::EmblemedPaintable;

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
