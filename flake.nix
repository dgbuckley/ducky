{
  description = "A Nix-flake-based Rust development environment";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/release-22.11";
    rust-overlay.url = "github:oxalica/rust-overlay";
    naersk = {
      url = "github:nix-community/naersk";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = {
    self,
    nixpkgs,
    rust-overlay,
    naersk,
  }: let
    inherit (nixpkgs) lib;
    genSystems =
      lib.genAttrs
      [
        "x86_64-linux"
        "aarch64-linux"
      ];

    pkgsFor = genSystems (system:
      import nixpkgs {
        inherit system;
        overlays = [
          (import rust-overlay)
          (self: super: {
            rustToolchain = let
              rust = super.rust-bin;
            in
              rust.stable.latest.default;
          })
        ];
      });
  in {
    packages = genSystems (system: let
      pkgs = pkgsFor.${system};
    in rec {
      default = naersk.lib.${system}.buildPackage {
        pname = "ducky";
        root = ./.;
        buildInputs = [pkgs.openssl pkgs.pkg-config];
      };
      ducky = default;
    });

    overlays.default = _: prev: {
      ducky = self.packages.${prev.system}.default;
    };

    devShells = genSystems (system: let
      pkgs = pkgsFor.${system};
    in {
      default = pkgs.mkShell {
        packages = with pkgs.pkgs; [
          rustToolchain
          openssl
          pkg-config
          cargo-deny
          cargo-edit
          cargo-watch
          cargo-release
          rust-analyzer
        ];

        shellHook = ''
          ${pkgs.rustToolchain}/bin/cargo --version
        '';
      };
    });
  };
}
