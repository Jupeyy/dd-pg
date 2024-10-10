{
  inputs = {
    utils.url = "github:numtide/flake-utils";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, fenix, nixpkgs, utils }:
    utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };
      in
      {
        devShells.default = pkgs.mkShell {
          nativeBuildInputs =
            [
              fenix.packages.${system}.complete.toolchain
            ];
          buildInputs = with pkgs; [
            fontconfig
            libxkbcommon
            glfw-wayland
            cmake
            autoreconfHook
            wayland
            alsa-lib
            alsa-utils
            wasm-pack
            wasm-bindgen-cli
            openssl
            pkg-config

            vulkan-loader
            vulkan-tools
            vulkan-headers
          ];

          LD_LIBRARY_PATH = "${pkgs.lib.makeLibraryPath [
            pkgs.wayland
            pkgs.libxkbcommon
            pkgs.fontconfig
            pkgs.alsa-lib
            pkgs.vulkan-loader
          ]}";
        };
      });
}
