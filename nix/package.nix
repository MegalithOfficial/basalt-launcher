{
  lib,
  rustPlatform,
  importNpmLock,
  nodejs,
  pkg-config,
  wrapGAppsHook3,
  makeDesktopItem,
  copyDesktopItems,
  glib-networking,
  gtk3,
  libayatana-appindicator,
  librsvg,
  openssl,
  webkitgtk_4_1,
  buildChannel ? "release",
}:

let
  source = lib.cleanSourceWith {
    src = ../.;
    filter = path: type:
      let name = baseNameOf path;
      in !(builtins.elem name [ ".git" "dist" "node_modules" "target" ]);
  };
  manifest = builtins.fromTOML (builtins.readFile ../src-tauri/Cargo.toml);
  desktopItem = makeDesktopItem {
    name = "basalt-launcher";
    desktopName = "Basalt Launcher";
    comment = "A polished Minecraft launcher";
    exec = "basalt-launcher";
    icon = "basalt-launcher";
    categories = [ "Game" ];
  };
in
rustPlatform.buildRustPackage {
  pname = if buildChannel == "dev" then "basalt-launcher-dev" else "basalt-launcher";
  inherit (manifest.package) version;
  src = source;

  cargoRoot = "src-tauri";
  buildAndTestSubdir = "src-tauri";
  cargoLock.lockFile = ../src-tauri/Cargo.lock;

  npmDeps = importNpmLock { npmRoot = source; };

  nativeBuildInputs = [
    nodejs
    pkg-config
    wrapGAppsHook3
    copyDesktopItems
    importNpmLock.npmConfigHook
  ];

  buildInputs = [
    glib-networking
    gtk3
    libayatana-appindicator
    librsvg
    openssl
    webkitgtk_4_1
  ];

  BASALT_BUILD_CHANNEL = buildChannel;

  preBuild = ''
    npm run build:frontend
  '';

  doCheck = false;

  postInstall = ''
    install -Dm644 ../src-tauri/icons/128x128.png \
      "$out/share/icons/hicolor/128x128/apps/basalt-launcher.png"
  '';

  desktopItems = [ desktopItem ];

  meta = {
    description = "A polished Minecraft launcher with practical instance and content management";
    homepage = "https://github.com/MegalithOfficial/basalt-launcher";
    license = lib.licenses.gpl3Only;
    mainProgram = "basalt-launcher";
    platforms = lib.platforms.linux;
  };
}
