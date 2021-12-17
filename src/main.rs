use std::env;

use relm4::gtk;
use relm4::RelmApp;

use fm4::AppModel;

fn main() {
    env_logger::init();

    // Call `gtk::init` manually because we instantiate GTK types in our model.
    gtk::init().unwrap();

    let model = AppModel::new(&env::current_dir().unwrap());
    let app = RelmApp::new(model);
    app.run();
}
