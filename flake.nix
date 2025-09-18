{
  description = "Notes";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs = { self, nixpkgs }:
  let
    systems = [ "x86_64-linux" "aarch64-darwin" ];
  in
  {
    # Three shells per system: ai, docs, web
    devShells = nixpkgs.lib.genAttrs systems (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          config.allowUnfree = true;
        };

        loadDotenv = ''
          if [ -f .env ]; then
            set -a
            # shellcheck disable=SC1091
            . ./.env
            set +a
          fi
        '';

        npmAITools = ''
          export NPM_CONFIG_PREFIX="$PWD/.npm-global"
          export PATH="$NPM_CONFIG_PREFIX/bin:$PATH"
          if ! command -v codex >/dev/null 2>&1; then
            echo "Installing OpenAI Codex CLI..."
            npm install -g @openai/codex
          fi
          if ! command -v claude >/dev/null 2>&1; then
            echo "Installing Anthropic Claude Code CLI..."
            npm install -g @anthropic-ai/claude-code
          fi
          if ! command -v wrangler >/dev/null 2>&1; then
            echo "Installing Wrangler CLI..."
            npm install -g wrangler@latest
          fi
          if ! comand -v opencode-ai >/dev/null 2>&1; then
            echo "Installing opencode"
            npm install -g opencode-ai
          fi 
        '';

        mdbookSetup = import ./nix/mdbook { inherit pkgs system; };
      in
      {
        ai = pkgs.mkShell {
          packages = with pkgs; [
            git
            curl
            nodejs_20
            nodePackages.npm
            just
            gh
          ];
          shellHook = ''
            ${loadDotenv}
            ${npmAITools}
            echo "[ai] node=$(node -v) npm=$(npm -v)"
            echo "[ai] codex=$(command -v codex || echo missing)  claude=$(command -v claude || echo missing)  wrangler=$(command -v wrangler || echo missing)"
          '';
        };

        docs = pkgs.mkShell {
          buildInputs = mdbookSetup.buildInputs ++ [ pkgs.just pkgs.nodejs_20 pkgs.nodePackages.npm ];
          shellHook = ''
            ${loadDotenv}
            ${npmAITools}
            ${mdbookSetup.shellHook}
          '';
        };

        web = pkgs.mkShell {
          packages = with pkgs; [
            git
            nodejs_20
            nodePackages.npm
            nodePackages.pnpm
            just
          ];
          shellHook = ''
            ${loadDotenv}
            ${npmAITools}
            echo "[web] node=$(node -v) pnpm=$(pnpm -v)"
            cd site 2>/dev/null || true
          '';
        };
      });

    # Apps: mdBook + Starlight
    apps = nixpkgs.lib.genAttrs systems (system:
      let
        pkgs = import nixpkgs { inherit system; config.allowUnfree = true; };
        mdbookSetup = import ./nix/mdbook { inherit pkgs system; };
      in mdbookSetup.apps // {
        starlight-dev = {
          type = "app";
          program = pkgs.writeShellScript "starlight-dev" ''
            cd site
            exec ${pkgs.nodePackages.pnpm}/bin/pnpm dev
          '';
        };
        starlight-build = {
          type = "app";
          program = pkgs.writeShellScript "starlight-build" ''
            cd site
            exec ${pkgs.nodePackages.pnpm}/bin/pnpm build
          '';
        };
      });
  };
}
