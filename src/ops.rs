//! Filesystem operations.
//!
//! This module contains functions that abstract filesystem operations at a higher level than
//! raw gio.

use std::sync::atomic::{AtomicU64, Ordering};

use futures::prelude::*;
use gtk::{gio, glib, prelude::*};
use relm4::{gtk, Sender};
use tracing::*;

use crate::{AppMsg, Transfer};

static ID: AtomicU64 = AtomicU64::new(0);

/// File transfer progress update.
#[derive(Debug)]
pub struct Progress {
    /// Uniquely identifies the ongoing operation.
    pub id: u64,

    pub current: i64,
    pub total: i64,
}

impl Progress {
    /// Returns true if this is the final update that will be sent for this operation.
    pub fn is_complete(&self) -> bool {
        self.current == self.total
    }
}

/// Move a file to a destination.
pub async fn move_(file: gio::File, destination: gio::File, sender: Sender<AppMsg>) {
    info!("moving {} to {}", file.uri(), destination.uri());

    let (file_display_name, destination_display_name) = futures::join!(
        file.query_info_future(
            gio::FILE_ATTRIBUTE_STANDARD_DISPLAY_NAME,
            gio::FileQueryInfoFlags::NONE,
            glib::PRIORITY_DEFAULT,
        )
        .map_ok(|info| info.display_name()),
        destination
            .parent()
            .unwrap()
            .query_info_future(
                gio::FILE_ATTRIBUTE_STANDARD_DISPLAY_NAME,
                gio::FileQueryInfoFlags::NONE,
                glib::PRIORITY_DEFAULT
            )
            .map_ok(|info| info.display_name()),
    );

    let id = ID.fetch_add(1, Ordering::SeqCst);
    let description = format!(
        "Moving '{}' to '{}'",
        file_display_name.unwrap_or_else(|_| "file".into()),
        destination_display_name.unwrap_or_else(|_| "destination".into()),
    );

    sender
        .send(AppMsg::Transfer(Transfer::New { id, description }))
        .unwrap();

    let (res, mut progress) = file.move_future(
        &destination,
        gio::FileCopyFlags::NONE,
        glib::source::PRIORITY_DEFAULT,
    );

    let sender_ = sender.clone();
    relm4::spawn_local(async move {
        while let Some((current, total)) = progress.next().await {
            let _ = sender_.send(AppMsg::Transfer(Transfer::Progress(Progress {
                id,
                current,
                total,
            })));
        }
    });

    if let Err(err) = res.await {
        let _ = sender.send(AppMsg::Error(Box::new(err)));
    }
}
