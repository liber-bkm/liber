{
  description = "liber - a small CLI bookmark manager";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
      in
      {
        packages.default = pkgs.buildGoModule {
          pname = "liber";
          version = "0.7.1";
          src = ./.;

          vendorHash = null;

          ldflags = [ "-X main.Version=0.7.1" ];
          buildInputs = [ pkgs.fzf pkgs.single-file-cli pkgs.monolith];


          meta = with pkgs.lib; {
            description = "A small CLI bookmark manager (html + markdown + archive)";
            homepage = "https://example.invalid/liber";
            license = licenses.mit;
            mainProgram = "liber";
          };
        };

        devShells.default = pkgs.mkShell {
          buildInputs = [ pkgs.go pkgs.gopls pkgs.fzf pkgs.monolith pkgs.single-file-cli];
        };

        apps.default = flake-utils.lib.mkApp { drv = self.packages.${system}.default; };
      });
}
