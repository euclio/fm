# fm

`fm` is a small, general-purpose file manager built using GTK and [Relm4].

The directories are visualized using [Miller columns], which enables quick
navigation throughout the hierarchy.

`fm` is in the early stages of development. Manipulating important files with it
risks data loss.

## Platform support

Development is currently focused on Linux, but bug reports for other platforms
are welcome.

The application is known to run successfully on MacOS. Other platforms are
untested, but if you can get the system dependencies to build, `fm` should work.

## Hacking

`fm` is a Rust project that utilizes [GTK 4][install-gtk],
[libpanel][install-libpanel], [GtkSourceView][install-gtksourceview], and
[libadwaita][install-libadwaita].

1. First, [install Rust and Cargo][install-rust].

2. Install system dependencies.

    #### Arch Linux

    ```sh
    $ pacman -Syu gtk4 libadwaita libpanel-git gtksourceview-git
    ```

3. Build and run the application.

    ```sh
    $ cargo run
    ```

## License

`fm` is licensed under the MIT license.

[Miller columns]: https://en.wikipedia.org/wiki/Miller_columns
[install-rust]: https://www.rust-lang.org/tools/install
[install-gtk]: https://www.gtk.org/docs/installations/
[install-gtksourceview]: https://wiki.gnome.org/Projects/GtkSourceView
[install-libadwaita]: https://gnome.pages.gitlab.gnome.org/libadwaita/
[install-libpanel]: https://gitlab.gnome.org/chergert/libpanel
[Relm4]: https://aaronerhardt.github.io/relm4-book/book/
