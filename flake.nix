{
  description = "Development environment for Neko Fans Server";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs = { self, nixpkgs }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];

      forAllSystems = f:
        nixpkgs.lib.genAttrs systems (system:
          f (import nixpkgs { inherit system; }));
    in
    {
      packages = forAllSystems (pkgs:
        let
          inherit (pkgs.stdenv.hostPlatform) system;

          python = pkgs.python313.withPackages (ps: [
            ps.fastapi
            ps.uvicorn
          ]);

          defaultEnv = ''
            export REDIS_URL="''${REDIS_URL:-redis://127.0.0.1:''${REDIS_PORT:-6379}}"
            export GALLERY_DL_WORKER_URL="''${GALLERY_DL_WORKER_URL:-http://127.0.0.1:''${WORKER_PORT:-8090}/query}"
            export GALLERY_DL_CACHE_TTL_SECONDS="''${GALLERY_DL_CACHE_TTL_SECONDS:-900}"
            export RUST_LOG="''${RUST_LOG:-info,neko_server=trace}"
          '';
        in
        {
          neko-server = pkgs.writeShellApplication {
            name = "neko-server";
            runtimeInputs = with pkgs; [
              cargo
              pkg-config
              rustc
              stdenv.cc
            ];
            text = ''
              ${defaultEnv}

              cargo run --bin neko_server -- --port "''${SERVER_PORT:-8080}"
            '';
          };

          neko-worker = pkgs.writeShellApplication {
            name = "neko-worker";
            runtimeInputs = [ python pkgs.gallery-dl ];
            text = ''
              exec uvicorn worker:app \
                --app-dir gallery-dl-worker \
                --host "''${HOST:-127.0.0.1}" \
                --port "''${WORKER_PORT:-8090}"
            '';
          };

          neko-redis = pkgs.writeShellApplication {
            name = "neko-redis";
            runtimeInputs = [ pkgs.redis ];
            text = ''
              mkdir -p .nix/redis
              exec redis-server \
                --save "" \
                --appendonly no \
                --dir .nix/redis \
                --port "''${REDIS_PORT:-6379}"
            '';
          };

          neko-dev = pkgs.writeShellApplication {
            name = "neko-dev";
            runtimeInputs = with pkgs; [
              coreutils
              redis
            ];
            text = ''
              set -euo pipefail

              export REDIS_PORT="''${REDIS_PORT:-6379}"
              export WORKER_PORT="''${WORKER_PORT:-8090}"
              export SERVER_PORT="''${SERVER_PORT:-''${PORT:-8080}}"
              ${defaultEnv}

              cleanup() {
                trap - EXIT INT TERM
                for pid in "''${server_pid:-}" "''${worker_pid:-}" "''${redis_pid:-}"; do
                  if [ -n "$pid" ]; then
                    kill "$pid" 2>/dev/null || true
                  fi
                done
              }
              trap cleanup EXIT INT TERM

              echo "Starting Redis on :$REDIS_PORT"
              ${self.packages.${system}.neko-redis}/bin/neko-redis &
              redis_pid=$!

              redis_ready=0
              for _ in $(seq 1 50); do
                if redis-cli -p "$REDIS_PORT" ping >/dev/null 2>&1; then
                  redis_ready=1
                  break
                fi
                sleep 0.1
              done

              if [ "$redis_ready" -ne 1 ]; then
                echo "Redis did not start on :$REDIS_PORT" >&2
                exit 1
              fi

              echo "Starting gallery-dl worker on :$WORKER_PORT"
              ${self.packages.${system}.neko-worker}/bin/neko-worker &
              worker_pid=$!

              echo "Starting Rust API on :$SERVER_PORT"
              ${self.packages.${system}.neko-server}/bin/neko-server &
              server_pid=$!

              echo "Neko dev stack is running. Press Ctrl-C to stop."
              wait -n "$redis_pid" "$worker_pid" "$server_pid"
            '';
          };
        });

      apps = forAllSystems (pkgs:
        let
          inherit (pkgs.stdenv.hostPlatform) system;
          mkApp = name: description: {
            type = "app";
            program = "${self.packages.${system}.${name}}/bin/${name}";
            meta.description = description;
          };
        in
        {
          default = mkApp "neko-dev" "Start the full local development stack";
          neko-dev = mkApp "neko-dev" "Start Redis, the gallery-dl worker, and the Rust API";
          neko-server = mkApp "neko-server" "Start the Rust API on port 8080";
          neko-worker = mkApp "neko-worker" "Start the gallery-dl worker on port 8090";
          neko-redis = mkApp "neko-redis" "Start local Redis on port 6379";
        });

      devShells = forAllSystems (pkgs:
        let
          inherit (pkgs.stdenv.hostPlatform) system;

          python = pkgs.python313.withPackages (ps: [
            ps.fastapi
            ps.uvicorn
          ]);
        in
        {
          default = pkgs.mkShell {
            packages = with pkgs; [
              cargo
              cargo-watch
              clippy
              docker-compose
              gallery-dl
              pkg-config
              python
              redis
              rust-analyzer
              rustc
              rustfmt
              self.packages.${system}.neko-dev
              self.packages.${system}.neko-server
              self.packages.${system}.neko-worker
              self.packages.${system}.neko-redis
            ];

            REDIS_URL = "redis://127.0.0.1:6379";
            GALLERY_DL_WORKER_URL = "http://127.0.0.1:8090/query";
            GALLERY_DL_CACHE_TTL_SECONDS = "900";
            RUST_LOG = "info,neko_server=trace";
            RUST_SRC_PATH = "${pkgs.rustPlatform.rustLibSrc}";

            shellHook = ''
              echo "Neko Server dev shell"
              echo "  neko-dev     start Redis, worker, and API together"
              echo "  neko-redis   start local Redis"
              echo "  neko-worker  start gallery-dl worker"
              echo "  neko-server  start Rust API on :8080"
            '';
          };
        });
    };
}
