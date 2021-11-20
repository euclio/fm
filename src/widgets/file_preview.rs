use relm::Widget;
use relm_derive::widget;

pub struct Model {}

#[widget]
impl Widget for FilePreview {
    fn model() -> Model {
        Model {}
    }

    fn update(&mut self, _event: ()) {}

    view! {
        gtk::Box {}
    }
}
