{
  description = "Wastebin is a pastebin";

  # Nixpkgs / NixOS version to use.
  # inputs.nixpkgs.url = "nixpkgs/nixos-21.11";

  outputs = { self, nixpkgs }:
    let

      # to work with older version of flakes
      lastModifiedDate = self.lastModifiedDate or self.lastModified or "19700101";

      # Generate a user-friendly version number.
      version = builtins.substring 0 8 lastModifiedDate;

      # System types to support.
      supportedSystems = [ "x86_64-linux" "x86_64-darwin" "aarch64-linux" "aarch64-darwin" ];

      # Helper function to generate an attrset '{ x86_64-linux = f "x86_64-linux"; ... }'.
      forAllSystems = nixpkgs.lib.genAttrs supportedSystems;

      # Nixpkgs instantiated for supported system types.
      nixpkgsFor = forAllSystems (system: import nixpkgs { inherit system; });

    in
    {

      # Provide some binary packages for selected system types.
      packages = forAllSystems (system:
        let
          pkgs = nixpkgsFor.${system};
        in
        {
          wastebin = with pkgs; rustPlatform.buildRustPackage rec {
            pname = "wastebin";
            version = "2.4.3";

            src = ./.;

            cargoHash = "sha256-/3nIvDueiU6WelTlgzNYWGhToDUtf3BfUCbWkJhWAaw=";

            nativeBuildInputs = [ pkg-config ];

            buildInputs = [ sqlite zstd ];

            env.ZSTD_SYS_USE_PKG_CONFIG = true;

            meta = with lib; {
              description = "Wastebin is a pastebin";
              homepage = "https://github.com/matze/wastebin";
              changelog = "https://github.com/matze/wastebin/blob/${src.rev}/CHANGELOG.md";
              license = licenses.mit;
              maintainers = with maintainers; [ pinpox ];
              mainProgram = "wastebin";
            };
          };
        });

      defaultPackage = forAllSystems (system: self.packages.${system}.wastebin);
    };
}

