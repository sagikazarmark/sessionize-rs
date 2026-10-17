{ pkgs, ... }:

{
  packages = with pkgs; [
    just

    cargo-audit
    cargo-deny
    cargo-hack
    cargo-release
    cargo-watch

    # native-tls feature
    openssl
    pkg-config
  ];

  languages = {
    rust = {
      enable = true;
      # channel = "stable";
      # targets = [ "wasm32-unknown-unknown" ];

      lld.enable = true;
    };
  };
}
