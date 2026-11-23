{ lib, configuration }:
(lib.evalModules {
  modules = [
    {
      options.programs.calculator.enable = lib.mkEnableOption "the native Console Calculator";
    }
    configuration
  ];
}).config.programs
