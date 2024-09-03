{ craneLib
, mkSelString
, mkMizeRustModule
, ...
}:
mkMizeRustModule {
  src = ./.;
  modName = "String";
  test = "hiiiiiiiiiiiiiii";
}
