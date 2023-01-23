//! Actions for the directory entry right-click menu.

use relm4::actions::ActionName;

relm4::new_action_group!(pub DirectoryListRightClickActionGroup, "directory-list");

pub struct OpenDefaultAction;

impl ActionName for OpenDefaultAction {
    type Group = DirectoryListRightClickActionGroup;
    type Target = String;
    type State = ();

    const NAME: &'static str = "open-default";
}

pub struct OpenChooserAction;

impl ActionName for OpenChooserAction {
    type Group = DirectoryListRightClickActionGroup;
    type Target = String;
    type State = ();

    const NAME: &'static str = "open-chooser";
}

pub struct NewFolderAction;

impl ActionName for NewFolderAction {
    type Group = DirectoryListRightClickActionGroup;
    type Target = ();
    type State = ();

    const NAME: &'static str = "new-folder";
}

pub struct RenameAction;

impl ActionName for RenameAction {
    type Group = DirectoryListRightClickActionGroup;
    type Target = String;
    type State = ();

    const NAME: &'static str = "rename";
}

pub struct TrashSelectionAction;

impl ActionName for TrashSelectionAction {
    type Group = DirectoryListRightClickActionGroup;
    type Target = ();
    type State = ();

    const NAME: &'static str = "trash-selection";
}

pub struct RestoreSelectionFromTrashAction;

impl ActionName for RestoreSelectionFromTrashAction {
    type Group = DirectoryListRightClickActionGroup;
    type Target = ();
    type State = ();

    const NAME: &'static str = "restore-selection-from-trash";
}
