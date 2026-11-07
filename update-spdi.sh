#!/usr/bin/env bash
set -euo pipefail

url="https://sdpi-components.dev/releases/v4/sdpi-components.js"
destination="assets/propertyInspector/sdpi-components.js"

mkdir -p "$(dirname "$destination")"

curl --fail --location --retry 3 \
  --output "${destination}.tmp" \
  "$url"

mv "${destination}.tmp" "$destination"

echo "Updated SDPI Components:"
sha256sum "$destination"

git add "$destination"
