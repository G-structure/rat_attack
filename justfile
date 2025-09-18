test-ws-upgrade:
    #!/usr/bin/env bash
    set -e
    cargo build
    ./target/debug/ct-bridge &
    SERVER_PID=$!
    sleep 1
    cargo test --test ws_upgrade
    kill $SERVER_PID