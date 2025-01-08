{
  module = { mkMizeRustModule, ... }: mkMizeRustModule {
    modName = "dev";
    src = ./.;

  };
}
