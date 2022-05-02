//! Actions for the directory entry right-click menu.

use std::path::PathBuf;

use relm4::actions::ActionName;

relm4::new_action_group!(pub DirectoryListRightClickActionGroup, "directory-list");

pub struct OpenDefaultAction;

impl ActionName for OpenDefaultAction {
    type Group = DirectoryListRightClickActionGroup;
    type Target = PathBuf;
    type State = ();

    const NAME: &'static str = "open-default";
}

pub struct OpenChooserAction;

impl ActionName for OpenChooserAction {
    type Group = DirectoryListRightClickActionGroup;
    type Target = PathBuf;
    type State = ();

    const NAME: &'static str = "open-chooser";
}

pub struct RenameAction;

impl ActionName for RenameAction {
    type Group = DirectoryListRightClickActionGroup;
    type Target = PathBuf;
    type State = ();

    const NAME: &'static str = "rename";
}

pub struct TrashFileAction;

impl ActionName for TrashFileAction {
    type Group = DirectoryListRightClickActionGroup;
    type Target = PathBuf;
    type State = ();

    const NAME: &'static str = "trash-file";
}
