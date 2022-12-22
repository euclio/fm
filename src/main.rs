use std::fs;
use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use log::*;
use relm4::{gtk, RelmApp};

use fm::AppModel;

/// A paned file manager with automatic preview.
#[derive(Parser, Debug)]
#[clap(about, version, author)]
struct Args {
    /// The file or directory to open.
    #[clap(default_value = ".")]
    file: PathBuf,
}

fn main() -> Result<()> {
    env_logger::init();

    let args = Args::parse();
    info!("running with arguments: {:?}", args);

    // Call `gtk::init` manually because we instantiate GTK types in the app model.
    gtk::init().unwrap();

    relm4::set_global_css(include_str!("styles.css"));
    let app = RelmApp::new("io.github.fm");
    app.run::<AppModel>(fs::canonicalize(args.file)?);

    info!("main loop exited");

    Ok(())
}
