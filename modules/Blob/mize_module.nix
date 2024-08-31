{ craneLib
, toolchain_version
, ...
}:
craneLib.buildPackage {
  src = ./.;
  selector_string = builtins.toJSON {
    inherit toolchain_version;
    name = "Blob";
  };
}
