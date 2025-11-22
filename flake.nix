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
              openssl
            ];

            buildInputs = [
              vulkan-loader
              vulkan-headers
              vulkan-validation-layers

              libxkbcommon
              rustToolchain
              wgsl-analyzer
            ]
            ++ (with xorg; [
              libX11
              libXcursor
              libXrandr
              libXi
            ]);

            LD_LIBRARY_PATH = "${vulkan-loader}/lib";
          };
      }
    );
}
