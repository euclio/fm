use std::fs;
use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use log::*;
use relm4::{gtk, RelmApp};
use tracing_tree::HierarchicalLayer;
use tracing_subscriber::{registry::Registry, prelude::*};
use tracing_log::LogTracer;

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
    let subscriber = Registry::default().with(HierarchicalLayer::new(2));
    tracing::subscriber::set_global_default(subscriber).unwrap();

    LogTracer::init()?;

    let args = Args::parse();
    info!("running with arguments: {:?}", args);

    // Call `gtk::init` manually because we instantiate GTK types in the app model.
    gtk::init().unwrap();

    relm4::set_global_css(include_bytes!("styles.css"));
    let app = RelmApp::<AppModel>::new("io.github.fm");
    app.run(fs::canonicalize(args.file)?);

    info!("main loop exited");

    Ok(())
}
