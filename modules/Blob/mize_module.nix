{ craneLib
, ...
}:
craneLib.buildPackage {
  src = ./.;
}
