{ pkgs, runVM, source, src }:

let
  build = name: buildCommands: pkgs.rustPlatform.buildRustPackage {
    pname = "console-${name}-test-binaries";
    version = "0.1.0";
    src = source;
    cargoLock.lockFile = src + "/Cargo.lock";
    CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS = "-C target-cpu=x86-64";
    CARGO_PROFILE_DEV_DEBUG = "0";
    CARGO_PROFILE_TEST_DEBUG = "0";
    RUSTC_BOOTSTRAP = "console_core_path_rasterizer,console_operating_system_linux";
    doCheck = false;
    auditable = false;
    buildPhase = ''
      runHook preBuild
      set -o pipefail
      mkdir -p "$out/bin"
      build_tests() {
        local suite="$1"
        shift
        cargo test --offline --locked --no-run --message-format=json "$@" \
          | ${pkgs.python3}/bin/python3 ${./test-executables.py} \
          > test-executables
        test -s test-executables
        mkdir "$out/$suite"
        while IFS= read -r executable; do
          install -m755 "$executable" "$out/$suite/"
        done < test-executables
      }
      ${buildCommands}
      runHook postBuild
    '';
    installPhase = "runHook preInstall; runHook postInstall";
  };
  nightLight = build "night-light" ''
    build_tests rust -p console-night-light
    cargo build --offline --locked -p console-night-light --bin console-warm
    install -m755 target/debug/console-warm "$out/bin/"
  '';
  rasterizer = build "rasterizer" ''
    build_tests default -p console-core-path-rasterizer --lib
    build_tests std -p console-core-path-rasterizer --lib --no-default-features --features std
    build_tests no-std-simd -p console-core-path-rasterizer --lib --no-default-features --features no-std-float,simd
    cargo check --offline --locked -p console-core-path-rasterizer --no-default-features --features no-std-float
  '';
  compute = build "gpu-compute" ''
    cargo build --offline --locked --release -p console-kernel --features hosted-compute --example gpu-checksums
    cargo build --offline --locked --release -p console-operating-system-linux --example gpu-image-mask
    install -m755 target/release/examples/{gpu-checksums,gpu-image-mask} "$out/bin/"
  '';
  worker = pkgs.runCommandCC "console-vulkan-compute-worker" {
    buildInputs = [ pkgs.vulkan-headers pkgs.vulkan-loader ];
  } ''
    mkdir -p "$out/bin"
    cc -std=c17 -O2 -Wall -Wextra -Werror \
      ${source}/crates/console-operating-system-linux/compute/vulkan.c -lvulkan -o "$out/bin/vulkan"
  '';
  gpuCheck = name: script: runVM {
    inherit name;
    command = [ (pkgs.writeShellScript "console-${name}-check" ''
      set -eu
      cd "$STATE_DIRECTORY"
      compile_shader() {
        ${pkgs.glslang}/bin/glslangValidator --target-env vulkan1.0 -V "$1.comp" -o "$1.spv"
        ${pkgs.spirv-tools}/bin/spirv-val --target-env vulkan1.0 "$1.spv"
      }
      ${script}
    '') ];
    module = {
      hardware.graphics.enable = true;
      systemd.services.console-test.environment.VK_DRIVER_FILES = "/run/opengl-driver/share/vulkan/icd.d/lvp_icd.x86_64.json";
    };
  };
in
{
  night-light = runVM {
    name = "night-light";
    command = [ (pkgs.writeShellScript "console-night-light-check" ''
      set -eu
      for test in ${nightLight}/rust/*; do
        "$test"
      done
      exec ${pkgs.python3}/bin/python3 ${source}/nix/night-light-wayland.py ${nightLight}/bin/console-warm
    '') ];
  };
  rasterizer = runVM {
    name = "rasterizer";
    command = [ (pkgs.writeShellScript "console-rasterizer-check" ''
      set -eu
      for test in ${rasterizer}/{default,std,no-std-simd}/*; do
        echo "Running $test"
        "$test"
      done
    '') ];
  };
  kernel-gpu-checksums = gpuCheck "kernel-gpu-checksums" ''
    for shader in checksums wrapping-sum; do
      ${compute}/bin/gpu-checksums --emit-shader "$shader" > "$shader.comp"
      compile_shader "$shader"
    done
    exec ${compute}/bin/gpu-checksums ${worker}/bin/vulkan checksums.spv
  '';
  system-gpu-compute = gpuCheck "system-gpu-compute" ''
    for shader in mask blur threshold; do
      ${compute}/bin/gpu-image-mask --emit-shader "$shader" > "$shader.comp"
      compile_shader "$shader"
    done
    ${compute}/bin/gpu-image-mask --require ${worker}/bin/vulkan blur.spv threshold.spv
    exec ${compute}/bin/gpu-image-mask --require ${worker}/bin/vulkan mask.spv
  '';
}
