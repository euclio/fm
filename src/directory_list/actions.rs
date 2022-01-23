//! Actions for the directory entry right-click menu.

use relm4::actions::ActionName;

use crate::util::PathBufVariant;

relm4::new_action_group!(pub DirectoryListRightClickActionGroup, "dir-entry");

pub struct OpenDefaultAction;

impl ActionName for OpenDefaultAction {
    type Group = DirectoryListRightClickActionGroup;
    type Target = PathBufVariant;
    type State = ();

    fn name() -> &'static str {
        "open-default"
    }
}

pub struct OpenChooserAction;

impl ActionName for OpenChooserAction {
    type Group = DirectoryListRightClickActionGroup;
    type Target = PathBufVariant;
    type State = ();

    fn name() -> &'static str {
        "open-chooser"
    }
}

pub struct TrashFileAction;

impl ActionName for TrashFileAction {
    type Group = DirectoryListRightClickActionGroup;
    type Target = PathBufVariant;
    type State = ();

    fn name() -> &'static str {
        "trash-file"
    }
}
