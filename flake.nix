{
  description = "A build flake for fm a small filemanager built with GTK4";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs?ref=nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
  };

  outputs = { self, nixpkgs, rust-overlay }:
    let
      # Systems supported
      allSystems = [
        "x86_64-linux" # 64-bit Intel/AMD Linux
        "aarch64-linux" # 64-bit ARM Linux
        "x86_64-darwin" # 64-bit Intel macOS
        "aarch64-darwin" # 64-bit ARM macOS
      ];

      # Helper to provide system-specific attributes
      forAllSystems = f: nixpkgs.lib.genAttrs allSystems (system: f {
        pkgs = import nixpkgs { inherit system; };
      });
    in
    {
      packages = forAllSystems ({ pkgs }: {
        default = pkgs.rustPlatform.buildRustPackage {
          name = "fm";
          src = ./.;

          buildInputs = [
            pkgs.gtk4
            pkgs.libadwaita
            pkgs.libpanel
            pkgs.gtksourceview5
            pkgs.poppler
          ];
          nativeBuildInputs = [ pkgs.pkg-config ];

          cargoLock = {
            lockFile = ./Cargo.lock;
          };
        };
      });
    };
}
