{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    rust-overlay.inputs.nixpkgs.follows = "nixpkgs";
  };

  outputs = { self, rust-overlay, nixpkgs }:
    let
      system = "x86_64-linux";
      overlays = [ (import rust-overlay) ];
      pkgs = import nixpkgs {
        inherit overlays system;
      };
    in
    {
      devShells.${system}.default = pkgs.mkShell {
        buildInputs = [
          # (pkgs.rust-bin.selectLatestNightlyWith (toolchain: toolchain.default.override {
          #   targets = [ "thumbv6m-none-eabi" ];
          #   extensions = [ "rust-src" ];
          # }))
          (pkgs.rust-bin.stable.latest.default.override {
            extensions = [ "rust-src" ];
            targets = [ "thumbv6m-none-eabi" ];
          })
          pkgs.rust-analyzer
          pkgs.flip-link
          pkgs.probe-run
          pkgs.elf2uf2-rs
          pkgs.rustfmt
        ];
      };
    };
}
