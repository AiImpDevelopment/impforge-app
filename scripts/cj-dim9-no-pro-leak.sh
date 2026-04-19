#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Crown-Jewel Guardian Dim-9: Block any Pro-source identifiers leaking into MIT codebase.
set -e

FORBIDDEN_PATTERNS=(
  'forge_pro_mesh'
  'forge_remote::Hub'
  'forge_remote::hub::'
  'forge_broker'
  'forge_autonomy::tcmm'
  'forge_industry'
  'forge_creator'
  'forge_quest'
  'forge_flow'
  'forge_github'
  'forge_advanced'
  'forge_sandbox'
  'duckdb_analytics'
  'crypto_shield::FHE'
  'intent_hub::clustering'
  'model_orchestrator_v2'
  'global_pipeline_wiring'
  'BUSL-1.1'
  'Elastic-2.0'
  'License: Elastic-2.0'
  'PRO_DYNAMICS'
)

# Files exempt from Dim-9 scan (the guardian itself defines the forbidden list,
# and docs may legitimately compare MIT vs Pro).
SELF_PATH='scripts/cj-dim9-no-pro-leak.sh'

VIOLATIONS=()
for pattern in "${FORBIDDEN_PATTERNS[@]}"; do
  matches=$(git diff --cached --name-only --diff-filter=AM \
    | grep -vF "$SELF_PATH" \
    | xargs -r grep -lF "$pattern" 2>/dev/null \
    | grep -vF "$SELF_PATH" || true)
  if [[ -n "$matches" ]]; then
    VIOLATIONS+=("$pattern -> $matches")
  fi
done

if [ ${#VIOLATIONS[@]} -gt 0 ]; then
  echo "ERROR: Crown-Jewel Dim-9 violation — Pro-source identifiers in MIT codebase:"
  printf '  - %s\n' "${VIOLATIONS[@]}"
  echo ""
  echo "Per REGEL 000-MIT-REWRITE: MIT must be a fresh rewrite, never feature-flagged Pro source."
  echo "If this is intentional (e.g., docs comparing MIT vs Pro), prefix the line with '# CJ-DIM9-EXEMPT'."
  exit 1
fi
