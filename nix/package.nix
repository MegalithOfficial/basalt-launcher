{
  lib,
  rustPlatform,
  importNpmLock,
  nodejs,
  pkg-config,
  wrapGAppsHook4,
  glib-networking,
  cacert,
  npmHooks,
  cargo-tauri,
  openssl,
  stdenv,
  webkitgtk_4_1,
  gsettings-desktop-schemas,
  addDriverRunpath,
  libGL,
  libx11,
  libxcursor,
  libxext,
  libxrandr,
  libxxf86vm,
  flite,
  alsa-lib,
  libjack2,
  libpulseaudio,
  pipewire,
  udev,
  wayland,
  buildChannel ? "release",
}:

let
  source = lib.cleanSourceWith {
    src = ../.;
    filter =
      path: type:
      let
        name = baseNameOf path;
      in
      !(builtins.elem name [
        ".git"
        "dist"
        "node_modules"
        "target"
      ]);
  };
  manifest = fromTOML (builtins.readFile ../src-tauri/Cargo.toml);

  # similar to https://github.com/NixOS/nixpkgs/blob/2fcb964de67fcf60b43471c55d5d99e61a9ccb5a/pkgs/by-name/mo/modrinth-app/package.nix#L54
  runtimeDependencies = lib.optionalString stdenv.hostPlatform.isLinux (
    lib.makeLibraryPath [
      addDriverRunpath.driverLink

      # glfw
      libGL
      libx11
      libxcursor
      libxext
      libxrandr
      libxxf86vm
      wayland

      # lwjgl
      (lib.getLib stdenv.cc.cc)

      # narrator support
      flite

      # openal
      alsa-lib
      libjack2
      libpulseaudio
      pipewire

      # oshi
      udev
    ]
  );
in
rustPlatform.buildRustPackage {
  pname = if buildChannel == "dev" then "basalt-launcher-dev" else "basalt-launcher";
  inherit (manifest.package) version;
  src = source;

  postPatch = ''
    substituteInPlace src-tauri/tauri.conf.json \
      --replace 'bun run build:frontend' 'npm run build:frontend'
  '';

  cargoRoot = "src-tauri";
  buildAndTestSubdir = "src-tauri";
  cargoLock.lockFile = ../src-tauri/Cargo.lock;

  npmDeps = importNpmLock { npmRoot = source; };

  nativeBuildInputs = [
    cacert
    cargo-tauri.hook
    nodejs
    pkg-config
    importNpmLock.npmConfigHook
    npmHooks.npmInstallHook
    wrapGAppsHook4
  ];

  buildInputs = [
    openssl
    webkitgtk_4_1
    glib-networking
    gsettings-desktop-schemas
  ];

  BASALT_BUILD_CHANNEL = buildChannel;
  BASALT_DISTRIBUTION = "nix";

  doCheck = false;

  preFixup = lib.optionalString stdenv.hostPlatform.isLinux ''
    gappsWrapperArgs+=(
      --set LD_LIBRARY_PATH ${runtimeDependencies}
      --set __NV_DISABLE_EXPLICIT_SYNC 1
    )
  '';

  meta = {
    description = "A polished Minecraft launcher with practical instance and content management";
    homepage = "https://github.com/MegalithOfficial/basalt-launcher";
    license = lib.licenses.gpl3Only;
    mainProgram = "basalt-launcher";
    platforms = lib.platforms.linux;
  };
}
