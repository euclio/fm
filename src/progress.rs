use gtk::prelude::*;
use relm4::gtk;
use relm4::prelude::*;

use crate::ops::Progress;
use crate::AppMsg;

#[derive(Debug)]
pub struct TransferProgress {
    pub id: u64,
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
    type Init = Progress;
    type Input = TransferProgressMsg;
    type Output = ();
    type ParentInput = AppMsg;
    type ParentWidget = gtk::Box;

    view! {
        gtk::Box {
            gtk::ProgressBar {
                #[watch]
                set_fraction: self.current as f64 / self.total as f64,
            }
        }
    }

    fn init_model(progress: Self::Init, _: &DynamicIndex, _: FactorySender<Self>) -> Self {
        TransferProgress {
            id: progress.id,
            current: progress.current,
            total: progress.total,
        }
    }
}
