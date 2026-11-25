{
  description = "haj - beautiful, cross-distro package manager and tui";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs =
    { self, nixpkgs }:
    let
      inherit (nixpkgs) lib;
      systems = [
        "x86_64-linux"
        "aarch64-linux"
      ];
      forAllSystems = fn: lib.genAttrs systems (system: fn nixpkgs.legacyPackages.${system});
      cargoToml = builtins.fromTOML (builtins.readFile ./Cargo.toml);

      mkHaj =
        pkgs:
        {
          withPacman ? false,
        }:
        pkgs.rustPlatform.buildRustPackage {
          pname = cargoToml.package.name;
          inherit (cargoToml.package) version;

          src = lib.cleanSource ./.;
          cargoLock.lockFile = ./Cargo.lock;

          buildNoDefaultFeatures = !withPacman;

          nativeBuildInputs = with pkgs; [
            pkg-config
            installShellFiles
          ];

          buildInputs =
            with pkgs;
            [ openssl ] ++ lib.optionals withPacman [ pacman ];

          # Integration tests invoke host /usr/bin/pacman; run offline unit tests in sandbox.
          cargoTestFlags = [
            "--bin"
            "haj"
          ];

          postInstall = ''
            installShellCompletion --cmd haj \
              --bash src/completions/bash.sh \
              --fish src/completions/fish.sh \
              --zsh  src/completions/zsh.sh
          '';

          meta = {
            description = cargoToml.package.description;
            homepage = cargoToml.package.repository;
            license = lib.licenses.gpl3Plus;
            mainProgram = "haj";
            platforms = lib.platforms.linux;
          };
        };
    in
    {
      packages = forAllSystems (pkgs: rec {
        haj = mkHaj pkgs { };
        haj-alpm = mkHaj pkgs { withPacman = true; };
        default = haj;
      });

      checks = forAllSystems (pkgs: {
        inherit (self.packages.${pkgs.stdenv.hostPlatform.system}) haj haj-alpm;
      });

      apps = forAllSystems (pkgs: {
        default = {
          type = "app";
          program = lib.getExe self.packages.${pkgs.stdenv.hostPlatform.system}.default;
        };
      });
    };
}
