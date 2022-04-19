# fm

`fm` is a small, general-purpose file manager built using GTK and [Relm4].

![Screenshot](https://user-images.githubusercontent.com/1372438/164090003-20bca431-e2ef-475a-86d4-df64d10e1989.png)

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

    Note that libpanel is alpha software and may not be packaged for your
    system. In that case, you can build it from source, install it, and then
    build `fm` with the `PKG_CONFIG_PATH` environment variable set to
    `PKG_CONFIG_PATH="/path/to/libpanel/lib/pkgconfig:$PKG_CONFIG_PATH"`.

    #### Arch Linux

    ```sh
    $ pacman -Syu gtk4 libadwaita libpanel-git gtksourceview5
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
