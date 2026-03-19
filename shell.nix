{ pkgs ? import <nixpkgs> { config = {}; overlays = []; } }:
let 
  libPath = with pkgs; lib.makeLibraryPath [
    wayland
    libxkbcommon
    vulkan-loader
  ];
in
pkgs.mkShell {
  strictDeps = true;
  nativeBuildInputs = with pkgs; [
    cargo
    rustc
    rustfmt
    rust-analyzer
    pkg-config
  ];
  LD_LIBRARY_PATH = libPath;
  RUST_SRC_PATH = "${pkgs.rust.packages.stable.rustPlatform.rustLibSrc}";

  packages = with pkgs; [
    (python3.withPackages (python-pkgs: with python-pkgs; [
      debugpy
      numpy
      quaternion
    ]))
  ];

  shellHook = ''
    export TMPDIR=/tmp
  '';
}