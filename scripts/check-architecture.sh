#!/usr/bin/env bash
set -euo pipefail

core_manifest="crates/loremesh-core/Cargo.toml"
for forbidden in ratatui crossterm rusqlite tokio reqwest sqlx graphify; do
  if rg -n "^${forbidden}[[:space:]]*=" "$core_manifest"; then
    echo "forbidden core dependency: ${forbidden}" >&2
    exit 1
  fi
done

if rg -n --glob '*.rs' --glob '!**/tests/**' --glob '!**/*test*' '(unwrap|expect|panic|todo|unimplemented)!\s*\(' crates/*/src; then
  echo "forbidden panic-style macro in production source" >&2
  exit 1
fi

echo "architecture checks passed"
