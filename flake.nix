{
  description = "RustTavern native Nix package";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-26.05";

  outputs =
    {
      self,
      nixpkgs,
    }:
    let
      supportedSystems = [
        "x86_64-linux"
        "aarch64-linux"
      ];
      forAllSystems = nixpkgs.lib.genAttrs supportedSystems;
    in
    {
      packages = forAllSystems (
        system:
        let
          pkgs = import nixpkgs { inherit system; };
          packageFor =
            gitBranch:
            pkgs.callPackage ./nix/package.nix {
              src = self;
              inherit gitBranch;
              gitRevision = self.shortRev or self.dirtyShortRev or "";
            };
          rusttavern = packageFor "main";
          canary = packageFor "dev";
        in
        {
          inherit canary rusttavern;
          default = rusttavern;
        }
      );

      apps = forAllSystems (
        system:
        let
          package = self.packages.${system}.default;
        in
        {
          default = {
            type = "app";
            program = "${package}/bin/rusttavern";
            meta = {
              description = package.meta.description;
            };
          };
        }
      );

      checks = forAllSystems (system: {
        package = self.packages.${system}.default;
      });
    };
}
