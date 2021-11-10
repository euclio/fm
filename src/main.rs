use std::io;
use std::path::PathBuf;

use argh::FromArgs;
use relm::Widget;

use fm::Win;

/// File manager.
#[derive(FromArgs)]
struct Args {
    /// directory to open.
    #[argh(positional)]
    dir: PathBuf,
}

fn main() -> Result<(), io::Error> {
    env_logger::init();
    let args: Args = argh::from_env();
    Win::run(args.dir.canonicalize()?).unwrap();
    Ok(())
}
