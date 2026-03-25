{ pkgs ? import <nixpkgs> { config = {}; overlays = []; } }:
let 
  overrides = (builtins.fromTOML (builtins.readFile ./rust-toolchain.toml));
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
    RUSTC_VERSION = overrides.toolchain.channel;
    LD_LIBRARY_PATH = libPath;

    packages = [
      myPython
    ];

    PYO3_PYTHON="${myPython}/bin/python3";

    shellHook = ''
      export PATH="''${CARGO_HOME:-~/.cargo}/bin":"$PATH"
      export PATH="''${RUSTUP_HOME:-~/.rustup}/toolchains/$RUSTC_VERSION-${stdenv.hostPlatform.rust.rustcTarget}/bin":"$PATH"

      export TMPDIR=/tmp

      export PYO3_PYTHON=${myPython}/bin/python3

      export PATH="$HOME/.local/bin:$PATH"
    '';
  }
) { }