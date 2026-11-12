{
  description = "OpenAction controls for the Umbriel compositor";

  inputs = {
    nix-opendeck.url = "github:missmoode/nix-opendeck";
    nixpkgs.follows = "nix-opendeck/nixpkgs";
  };

  outputs =
    { nixpkgs, nix-opendeck, ... }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
      ];

      cargoManifest = fromTOML (builtins.readFile ./Cargo.toml);

      binaryName = cargoManifest.package.name;
      pluginId = cargoManifest.package.metadata.umbriel-controls.plugin_id;

      formatterFor =
        pkgs:
        pkgs.nixfmt-tree.override {
          runtimeInputs = with pkgs; [
            (mdformat.withPlugins (ps: [
              ps.mdformat-gfm
              ps.mdformat-gfm-alerts
            ]))
            yamlfmt
            prettier
            rustfmt
            shfmt
          ];

          settings.formatter = {
            markdown = {
              command = "mdformat";
              options = [
                "--wrap"
                "88"
                "--compact-tables"
              ];
              includes = [ "*.md" ];
              excludes = [ "CHANGELOG.md" ];
            };

            yaml = {
              command = "yamlfmt";
              includes = [
                "*.yaml"
                "*.yml"
              ];
            };

            web = {
              command = "prettier";
              options = [ "--write" ];
              includes = [
                "*.html"
                "*.js"
                "*.css"
                "*.json"
              ];
              excludes = [
                "assets/propertyInspector/sdpi-components.js"
                "assets/manifest.json"
              ];
            };

            rust = {
              command = "rustfmt";
              options = [
                "--edition"
                "2024"
              ];
              includes = [ "*.rs" ];
            };

            shell = {
              command = "shfmt";
              options = [
                "-w"
                "-i"
                "2"
              ];
              includes = [ "*.sh" ];
            };
          };
        };
    in
    {
      packages = nixpkgs.lib.genAttrs systems (
        system:
        let
          pkgs = import nixpkgs { inherit system; };

          mkUmbrielControls = pkgs.lib.makeOverridable (
            {
              buildFeatures ? [ ],
              defaultPalette ? null,
            }:
            nix-opendeck.lib.mkRustOpenDeckPlugin pkgs (finalAttrs: {
              pname = "umbriel-controls";
              version = cargoManifest.package.version;

              src = ./.;

              inherit pluginId binaryName buildFeatures;

              binaryInstallPath = "${finalAttrs.binaryName}-${pkgs.stdenv.hostPlatform.rust.rustcTarget}";

              cargoLock.lockFile = ./Cargo.lock;

              licenseFiles = [
                "LICENSE"
                "crates/umbriel-ipc/LICENSE"
              ];

              nativeBuildInputs = pkgs.lib.optionals (defaultPalette != null) [ pkgs.makeWrapper ];
              postInstall = pkgs.lib.optionalString (defaultPalette != null) ''
                wrapProgram "$out/${pluginId}/${finalAttrs.binaryInstallPath}" \
                  --set UMBRIEL_CONTROLS_DEFAULT_PALETTE ${pkgs.lib.escapeShellArg defaultPalette}
              '';

              meta = {
                description = "Stream Deck controls for the Umbriel compositor";
                license = pkgs.lib.licenses.gpl3Plus;
                platforms = pkgs.lib.platforms.linux;
              };
            })
          ) { };
        in
        {
          default = mkUmbrielControls;
          dev = mkUmbrielControls.override { buildFeatures = [ "diagnostics" ]; };
          noctalia-template = pkgs.writeText "umbriel-controls-noctalia.json.tmpl" (
            builtins.readFile ./noctaliatemplate/umbriel-controls.json.tmpl
          );
        }
      );

      formatter = nixpkgs.lib.genAttrs systems (
        system:
        let
          pkgs = import nixpkgs { inherit system; };
        in
        formatterFor pkgs
      );

      devShells = nixpkgs.lib.genAttrs systems (
        system:
        let
          pkgs = import nixpkgs { inherit system; };
        in
        {
          default = pkgs.mkShell {
            packages = with pkgs; [
              nixd
              nixfmt
              cargo
              rustc
              rustfmt
              clippy
              rust-analyzer
              jq
              zip
              (formatterFor pkgs)
            ];
          };
        }
      );
    };
}
