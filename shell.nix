{ pkgs ? import <nixpkgs> { config = {}; overlays = []; } }:
let 
  libPath = with pkgs; lib.makeLibraryPath [
    wayland
    libxkbcommon
    vulkan-loader
  ];
  myPython = pkgs.python3.withPackages (python-pkgs: with python-pkgs; [
    debugpy
    numpy
    quaternion
    uv
  ]);
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

  packages = [
    myPython
  ];

  PYO3_PYTHON="${myPython}/bin/python3";

  shellHook = ''
    export TMPDIR=/tmp
    export PYO3_PYTHON=${myPython}/bin/python3
    export PATH="$HOME/.local/bin:$PATH"
  '';
}