{
  description = "Rig development environment";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs?ref=nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      flake-utils,
      rust-overlay,
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };
        rustToolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
      in
      {
        devShells.default =
          with pkgs;
          mkShell {
            nativeBuildInputs = [
              pkg-config

              # graphics
              vulkan-loader
              vulkan-headers
              vulkan-validation-layers
              wayland
              wayland-protocols
              wayland-scanner
              libxkbcommon

              # audio
              alsa-lib
              jack2

              rustToolchain
              wgsl-analyzer
            ]
            ++ (with xorg; [
              libX11
              libXcursor
              libXrandr
              libXi
            ]);

            # Critical: tell the dynamic linker where to find libraries
            LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath [
              pkgs.vulkan-loader
              pkgs.alsa-lib
              pkgs.jack2
              pkgs.wayland
              pkgs.libxkbcommon
              pkgs.xorg.libX11
              pkgs.xorg.libXcursor
              pkgs.xorg.libXi
              pkgs.xorg.libXrandr
            ];

            # Tell winit where to find libwayland-client.so specifically
            WINIT_WAYLAND_LIBNAME = "${pkgs.wayland}/lib/libwayland-client.so";
          };
      }
    );
}
