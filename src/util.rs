use std::borrow::Cow;
use std::ffi::OsString;
use std::path::PathBuf;

use glib::variant::{FromVariant, StaticVariantType, ToVariant, Variant};
use glib::VariantTy;
use relm4::gtk::glib;

/// Wrapper type that implements [`ToVariant`] and [`FromVariant`] for [`PathBuf`];
pub struct PathBufVariant(pub PathBuf);

impl ToVariant for PathBufVariant {
    #[cfg(unix)]
    fn to_variant(&self) -> Variant {
        use std::os::unix::ffi::OsStrExt;

        self.0.as_os_str().as_bytes().to_variant()
    }

    #[cfg(windows)]
    fn to_variant(&self) -> Variant {
        use std::os::windows::ffi::OsStrExt;

        self.0
            .as_os_str()
            .encode_wide()
            .collect::<Vec<_>>()
            .to_variant()
    }
}

impl StaticVariantType for PathBufVariant {
    #[cfg(unix)]
    fn static_variant_type() -> Cow<'static, VariantTy> {
        <&[u8]>::static_variant_type()
    }

    #[cfg(windows)]
    fn static_variant_type() -> Cow<'static, VariantTy> {
        <Vec<u16>>::static_variant_type()
    }
}

impl FromVariant for PathBufVariant {
    #[cfg(unix)]
    fn from_variant(variant: &Variant) -> Option<Self> {
        use std::os::unix::ffi::OsStringExt;

        let bytes = <_>::from_variant(variant)?;
        Some(Self(OsString::from_vec(bytes).into()))
    }

    #[cfg(windows)]
    fn from_variant(variant: &Variant) -> Option<Self> {
        use std::os::windows::ffi::OsStringExt;

        let wide = <_>::from_variant(variant)?;
        Some(Self(OsString::from_wide(wide).into()))
    }
}
