use gtk::{gio, glib, prelude::*};
use log::*;
use relm4::{gtk, Sender};

use crate::AppMsg;

/// Move a dropped file into the destination directory.
pub fn handle_drop(value: &glib::Value, destination: &gio::File, error_sender: &Sender<AppMsg>) {
    let file = value.get::<gio::File>().unwrap();

    let destination_file = destination.child(file.basename().unwrap());

    if destination_file.equal(&file) {
        return;
    }

    info!("moving {} to {}", file.uri(), destination_file.uri());
    let res = file.move_(
        &destination_file,
        gio::FileCopyFlags::NONE,
        gio::Cancellable::NONE,
        None,
    );

    if let Err(err) = res {
        error_sender.emit(AppMsg::Error(Box::new(err)));
    }
}
