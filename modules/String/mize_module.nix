{ craneLib
, mkSelString
, mkMizeRustModule
, ...
}:
{

module = mkMizeRustModule {
  src = ./.;
  modName = "String";
  test = "hiiiiiiiiiiiiiii";
};

}
