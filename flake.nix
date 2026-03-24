rec {
  description = "Software soundboard";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/release-25.11";

    flake-parts.url = "github:hercules-ci/flake-parts";
    flake-parts.inputs.nixpkgs-lib.follows = "nixpkgs";

    naersk.url = "github:nix-community/naersk";
    naersk.inputs.nixpkgs.follows = "nixpkgs";

    rust-overlay.url = "github:oxalica/rust-overlay";
    rust-overlay.inputs.nixpkgs.follows = "nixpkgs";
  };

  outputs =
    { flake-parts, ... }@inputs:
    let
      name = "fahhh";
    in
    flake-parts.lib.mkFlake
      {
        inherit inputs;
        specialArgs = {
          root = ./.;
        };
      }
      (
        {
          self,
          inputs,
          root,
          lib,
          ...
        }:
        {
          systems = [
            "x86_64-linux"
            "aarch64-linux"
          ];
          perSystem =
            { pkgs, system, ... }:
            let
              rust = (inputs.rust-overlay.lib.mkRustBin { } pkgs).stable.latest.default.override {
                extensions = [
                  "rustfmt"
                  "clippy"
                  "rust-analyzer"
                  "rust-src"
                ];
              };

              rustc = rust;
              cargo = rust;

              naersk' = pkgs.callPackage inputs.naersk {
                inherit rustc cargo;
              };

              flake-root = pkgs.writeShellApplication {
                name = "flake-root";
                text = ''
                  current="$PWD"
                  while [[ "$current" != "/" ]]; do
                    if [[ -f "$current/flake.nix" ]]; then
                      echo "$current"
                      exit 0
                    fi
                    current="$(dirname "$current")"
                  done
                  echo "no flake.nix found" >&2
                  exit 1
                '';
              };

              env =
                let
                  llvmPackages = pkgs.llvmPackages;
                  libclang = llvmPackages.libclang;
                in
                {
                  LIBCLANG_PATH = "${libclang.lib}/lib";
                  BINDGEN_EXTRA_CLANG_ARGS = "-isystem ${pkgs.glibc.dev}/include";
                };

              nativeBuildInputs = with pkgs; [
                pkg-config
              ];

              buildInputs = with pkgs; [
                gtk4
                alsa-lib
                pipewire
              ];

              externalPackages = with pkgs; [
                nil
                nixfmt-rfc-style

                rustc
                cargo

                markdownlint-cli
                nodePackages.markdown-link-check
                marksman

                nodePackages.cspell

                mdbook
                nodePackages.prettier
                nodePackages.vscode-langservers-extracted
                nodePackages.prettier
                nodePackages.yaml-language-server
                taplo

                fd
                cachix

                release-plz
              ];

              scripts = {
                run = ''
                  cd "$(flake-root)"

                  dbus-run-session -- cargo run --bin ${name}
                '';
                format = ''
                  cd "$(flake-root)"

                  prettier --write .

                  # shellcheck disable=SC2046
                  nixfmt $(fd '.*.nix$' .)

                  cargo fmt --all
                  cargo clippy --fix --allow-dirty
                '';
                lint = ''
                  cd "$(flake-root)"

                  prettier --check .

                  cspell lint . --no-progress

                  # shellcheck disable=SC2046
                  nixfmt --check $(fd '.*.nix$' .)

                  markdownlint --ignore-path .markdownignore .
                  if [[ -z "''${NIX_BUILD_TOP:-}" ]]; then
                    # shellcheck disable=SC2046
                    markdown-link-check \
                      --config .markdown-link-check.json \
                      --quiet \
                      $(fd '.*.md' .)
                  fi

                  if [[ -z "''${NIX_BUILD_TOP:-}" ]]; then
                    taplo lint \
                      --schema "https://raw.githubusercontent.com/release-plz/release-plz/refs/tags/release-plz-v0.3.148/.schema/latest.json" \
                      .release-plz.toml
                  fi

                  if [[ -z "''${NIX_BUILD_TOP:-}" ]]; then
                    cargo clippy -- -D warnings
                    cargo test
                  fi
                '';
              };

              scriptPackages = builtins.map (
                { name, value }:
                pkgs.writeShellApplication {
                  name = "dev-${name}";
                  runtimeInputs = externalPackages ++ [ flake-root ];
                  text = value;
                }
              ) (lib.attrsToList scripts);
            in
            {
              packages =
                let
                  default = naersk'.buildPackage (
                    let
                      cargoToml = builtins.fromTOML (builtins.readFile (lib.path.append root "src/${name}/Cargo.toml"));
                    in
                    {
                      inherit buildInputs nativeBuildInputs;
                      src = root;
                      cargoBuildOptions =
                        prev:
                        prev
                        ++ [
                          "-p"
                          "${name}"
                        ];
                      name = cargoToml.package.name;
                      version = cargoToml.package.version;
                    }
                    // env
                  );

                  docs =
                    pkgs.runCommand "${name}-docs"
                      {
                        src = self;
                        nativeBuildInputs = [ pkgs.mdbook ];
                      }
                      ''
                        mdbook build -d "$out" "$src/docs"
                      '';
                in
                {
                  default = default;
                  ${name} = default;
                  docs = docs;
                };

              apps =
                let
                  package = self.packages.${system}.default;

                  app = {
                    type = "app";
                    program = lib.getExe package;
                    meta.description = description;
                  };
                in
                {
                  default = app;
                  ${name} = app;
                };

              devShells.default = pkgs.mkShell (
                {
                  inherit buildInputs nativeBuildInputs;
                  packages = externalPackages ++ scriptPackages;
                }
                // env
              );

              checks.default =
                pkgs.runCommand "${name}-checks-default"
                  {
                    src = self;
                    nativeBuildInputs = externalPackages ++ [ flake-root ];
                  }
                  ''
                    cd "$src"
                    ${scripts.lint}
                    touch "$out"
                  '';
            };
        }
      );

  nixConfig = {
    extra-substituters = [
      "https://haras.cachix.org"
    ];
    extra-trusted-public-keys = [
      "haras.cachix.org-1:/HIo1JYqOIH1Nwk1EGXhuPPvDW0WekxIbY5CiXUZbYw="
    ];
  };
}
