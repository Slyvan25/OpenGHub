{
  description = "OpenGHub, an open source Logitech G HUB for Linux";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs =
    { self, nixpkgs }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
      ];
      forAllSystems = f: nixpkgs.lib.genAttrs systems (system: f nixpkgs.legacyPackages.${system});
    in
    {
      packages = forAllSystems (pkgs: rec {
        openghub = pkgs.callPackage ./nix/package.nix { };
        default = openghub;
      });

      overlays.default = final: _prev: {
        openghub = final.callPackage ./nix/package.nix { };
      };

      # programs.openghub.enable = true; installs the app and its udev rule,
      # which gives the logged-in user access to Logitech hidraw nodes and
      # /dev/uinput (for the racing wheel force-feedback driver).
      nixosModules.default =
        {
          config,
          lib,
          pkgs,
          ...
        }:
        let
          cfg = config.programs.openghub;
        in
        {
          options.programs.openghub = {
            enable = lib.mkEnableOption "OpenGHub, an open source Logitech G HUB";
            package = lib.mkOption {
              type = lib.types.package;
              default = self.packages.${pkgs.stdenv.hostPlatform.system}.default;
              defaultText = lib.literalExpression "openghub.packages.\${system}.default";
              description = "The OpenGHub package to use.";
            };
          };

          config = lib.mkIf cfg.enable {
            environment.systemPackages = [ cfg.package ];
            services.udev.packages = [ cfg.package ];
            boot.kernelModules = [ "uinput" ];
          };
        };

      devShells = forAllSystems (pkgs: {
        default = pkgs.mkShell {
          inputsFrom = [ self.packages.${pkgs.stdenv.hostPlatform.system}.default ];
          packages = [ pkgs.cargo-tauri ];
        };
      });

      formatter = forAllSystems (pkgs: pkgs.nixfmt);
    };
}
