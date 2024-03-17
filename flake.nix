{
  inputs = {
    crane = {
      url = "github:ipetkov/crane";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    flake-parts.url = "github:hercules-ci/flake-parts";
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    systems.url = "github:nix-systems/default";
    # Dev tools
    treefmt-nix.url = "github:numtide/treefmt-nix";
  };

  outputs = inputs:
    inputs.flake-parts.lib.mkFlake { inherit inputs; } {
      systems = import inputs.systems;
      imports = [
        inputs.treefmt-nix.flakeModule
      ];
      perSystem = { config, self', pkgs, lib, system, ... }:
        let
          overlays = [ (import inputs.rust-overlay) ];
          cargoToml = builtins.fromTOML (builtins.readFile ./Cargo.toml);
          pkgs = import inputs.nixpkgs {
            inherit system overlays;
          };
        in
        rec {
          # Rust package
          packages.default = pkgs.rustPlatform.buildRustPackage {
            inherit (cargoToml.package) name version;
            src = ./.;
            cargoLock.lockFile = ./Cargo.lock;
          };

          # Rust dev environment
          devShells.default = pkgs.mkShell rec {
            inputsFrom = [
              config.treefmt.build.devShell
            ];
            shellHook = ''
              # For rust-analyzer 'hover' tooltips to work.
              export RUST_SRC_PATH=${pkgs.rustPlatform.rustLibSrc}
              nu
            '';
            buildInputs = with pkgs; [
              pkg-config
              python3
              rust-bin.stable.latest.default
              sccache
              fontconfig
              libxkbcommon
              libGL

              # WINIT_UNIX_BACKEND=x11
              xorg.libXcursor
              xorg.libXrandr
              xorg.libXi
              xorg.libX11
              vulkan-headers
              vulkan-loader
              wayland
              wayland-protocols
            ];
            nativeBuildInputs = with pkgs; [
              pkg-config
            ];
            LD_LIBRARY_PATH = "${lib.makeLibraryPath buildInputs}";
            RUST_BACKTRACE = 1;
          };


          treefmt.config = {
            projectRootFile = "flake.nix";
            programs = {
              nixpkgs-fmt.enable = true;
              rustfmt.enable = true;
            };
          };
        };
    };
}
