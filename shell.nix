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
pkgs.callPackage (
  {
    stdenv,
    mkShell,
    rustup,
    rustPlatform,
  }:
  mkShell {
    strictDeps = true;
    nativeBuildInputs = with pkgs; [
      rustup
      rustPlatform.bindgenHook
      nodejs_24
    ];
    LD_LIBRARY_PATH = libPath;

    packages = [
      myPython
    ];

    PYO3_PYTHON="${myPython}/bin/python3";

    shellHook = ''
      export PATH="''${CARGO_HOME:-~/.cargo}/bin":"$PATH"

      export TMPDIR=/tmp

      export PYO3_PYTHON=${myPython}/bin/python3

      export PATH="$HOME/.local/bin:$PATH"
    '';
  }
) { }