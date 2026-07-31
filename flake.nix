{
  description = "Basalt Launcher";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs = { self, nixpkgs, ... }:
    let
      supportedSystems = [ "x86_64-linux" "aarch64-linux" ];
      forAllSystems = nixpkgs.lib.genAttrs supportedSystems;
    in {
      packages = forAllSystems (system:
        let
          pkgs = import nixpkgs { inherit system; };
          basalt-launcher = pkgs.callPackage ./nix/package.nix { };
          basalt-launcher-dev = pkgs.callPackage ./nix/package.nix {
            buildChannel = "dev";
          };
        in {
          inherit basalt-launcher basalt-launcher-dev;
          default = basalt-launcher;
        });

      apps = forAllSystems (system:
        let
          mkApp = package: {
            type = "app";
            program = "${nixpkgs.lib.getExe package}";
          };
        in {
          default = mkApp self.packages.${system}.default;
          dev = mkApp self.packages.${system}.basalt-launcher-dev;
        });

      checks = forAllSystems (system: {
        inherit (self.packages.${system}) basalt-launcher;
      });
    };
}
