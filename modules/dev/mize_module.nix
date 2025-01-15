{
  module = { pkgs, mkMizeRustModule, mkMizeRustShell, ... }: mkMizeRustModule {
    modName = "dev";
    src = ./.;

    devShell = mkMizeRustShell {
      nativeBuildInputs = with pkgs; [
        pkg-config
      ];

      buildInputs = with pkgs; [
        openssl
      ];
    };
  };

}
