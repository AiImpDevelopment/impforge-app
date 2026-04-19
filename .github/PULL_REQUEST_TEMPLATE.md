<!-- SPDX-License-Identifier: MIT -->
## Summary

What does this PR change and why?

## Type of Change

- [ ] Bug fix (non-breaking)
- [ ] New feature (non-breaking)
- [ ] Breaking change
- [ ] Documentation only
- [ ] Refactor / internal cleanup
- [ ] Build / CI change

## Linked Issue

Closes #(issue number)

## Tests

Describe how you verified this change. Reference test names if applicable.

- [ ] Added new tests covering the change
- [ ] Updated existing tests
- [ ] All `cargo test --workspace` pass locally

## Crown-Jewel Checklist (non-negotiable)

- [ ] `cargo check --all-targets --workspace` — 0 errors, 0 warnings
- [ ] `cargo clippy --all-targets --workspace -- -D warnings` — 0 violations
- [ ] `cargo test --workspace` — all tests pass
- [ ] `pnpm check` — 0 errors, 0 warnings
- [ ] No new `.unwrap()` in production code
- [ ] No new `#[allow(*)]` attributes
- [ ] Every new source file has a `SPDX-License-Identifier: MIT` header
- [ ] No Pro-source identifiers introduced (Dim-9: `./scripts/cj-dim9-no-pro-leak.sh`)
- [ ] Pre-commit hook ran without `--no-verify`

## Screenshots / Recordings (UI changes only)

If this PR touches Svelte components or routes, attach before/after screenshots or a short clip.

## Additional Notes

Anything reviewers should know — design decisions, follow-ups, dependencies, breaking-change migration steps.
