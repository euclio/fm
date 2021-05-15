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

fn main() {
    let args: Args = argh::from_env();
    Win::run(args.dir).unwrap();
}
