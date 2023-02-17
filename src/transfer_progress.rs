use relm4::gtk;
use relm4::panel::prelude::OrientableExt;
use relm4::prelude::*;

use crate::ops::Progress;
use crate::AppMsg;

#[derive(Debug)]
pub struct NewTransfer {
    pub id: u64,
    pub description: String,
}

#[derive(Debug)]
pub struct TransferProgress {
    pub id: u64,

    description: String,
    current: i64,
    total: i64,
}

#[derive(Debug)]
pub enum TransferProgressMsg {
    Update(Progress),
}

#[relm4::factory(pub)]
impl FactoryComponent for TransferProgress {
    type CommandOutput = ();
    type Init = NewTransfer;
    type Input = TransferProgressMsg;
    type Output = ();
    type ParentInput = AppMsg;
    type ParentWidget = gtk::ListBox;

    view! {
        gtk::Box {
            set_orientation: gtk::Orientation::Vertical,

            gtk::Label {
                set_text: &self.description,
            },
            gtk::ProgressBar {
                #[watch]
                set_fraction: self.current as f64 / self.total as f64,
            },
        }
    }

    fn init_model(new_transfer: Self::Init, _: &DynamicIndex, _: FactorySender<Self>) -> Self {
        TransferProgress {
            id: new_transfer.id,
            description: new_transfer.description,
            current: 0,
            total: 1,
        }
    }

    fn update(&mut self, msg: Self::Input, _: FactorySender<Self>) {
        match msg {
            TransferProgressMsg::Update(Progress { current, total, .. }) => {
                self.current = current;
                self.total = total;
            }
        }
    }
}
