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
        linuxPkgs = with pkgs; [
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
        ];
        onLinux = system == "x86_64-linux";
      in
      {
        devShells.default =
          with pkgs;
          mkShell {
            nativeBuildInputs = [
              pkg-config
              rustToolchain
              wgsl-analyzer
            ]
            ++ (if onLinux then linuxPkgs else [ ]);

            LD_LIBRARY_PATH =
              if onLinux then
                # Critical: tell the dynamic linker where to find libraries
                pkgs.lib.makeLibraryPath [
                  pkgs.vulkan-loader
                  pkgs.alsa-lib
                  pkgs.jack2
                  pkgs.wayland
                  pkgs.libxkbcommon
                  pkgs.xorg.libX11
                  pkgs.xorg.libXcursor
                  pkgs.xorg.libXi
                  pkgs.xorg.libXrandr
                ]
              else
                null;

            # Tell winit where to find libwayland-client.so specifically
            WINIT_WAYLAND_LIBNAME = if onLinux then "${pkgs.wayland}/lib/libwayland-client.so" else null;
          };
      }
    );
}
