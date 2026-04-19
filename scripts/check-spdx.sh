#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Verify every staged .rs / .ts / .svelte / .js file has SPDX MIT header.
set -e

MISSING=()
while IFS= read -r file; do
  if [[ -z "$file" ]]; then continue; fi
  if [[ "$file" =~ \.(rs|ts|js|svelte|toml|css|html)$ ]]; then
    if ! head -5 "$file" | grep -qF "SPDX-License-Identifier: MIT"; then
      if [[ "$file" =~ \.json$ ]]; then continue; fi
      MISSING+=("$file")
    fi
  fi
done < <(git diff --cached --name-only --diff-filter=AM)

if [ ${#MISSING[@]} -gt 0 ]; then
  echo "ERROR: Files missing SPDX MIT header:"
  printf '  - %s\n' "${MISSING[@]}"
  echo ""
  echo "Add this comment as the first line:"
  echo "  // SPDX-License-Identifier: MIT   (Rust/TS/JS)"
  echo "  /* SPDX-License-Identifier: MIT */ (CSS)"
  echo "  <!-- SPDX-License-Identifier: MIT --> (HTML/Svelte/MD)"
  echo "  # SPDX-License-Identifier: MIT     (TOML/shell)"
  exit 1
fi
