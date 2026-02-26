## 📋 Summary

<!--
Explain WHAT this PR does and WHY.
Focus on the motivation and impact rather than implementation details.
-->

Implements a comprehensive request/response validation middleware for MoFA that provides schema-based validation, rate limiting, input sanitization, and detailed error reporting. This middleware helps improve security, data integrity, and provides better error messages to clients.

## 🔗 Related Issues

Closes #356

---

## 🧠 Context

<!--
Why is this change needed?
What problem does it solve?
Any relevant background or design decisions.
-->

The MoFA framework lacked a middleware system for validating incoming requests and responses. This led to potential security and data integrity issues. The implementation adds:

- Schema-based validation for JSON requests
- Rate limiting per client/agent/user
- Input sanitization to prevent XSS and SQL injection attacks
- Custom validation rules per endpoint
- Comprehensive error reporting

---

## 🛠️ Changes

<!--
High-level list of changes.
Avoid low-level diffs — reviewers can see those.
-->

- Added new `validation` module in `mofa-foundation` crate
- Created schema validator with support for required fields, min/max length, patterns, email, URL, UUID validation
- Implemented rate limiter with configurable key types (IP, Client ID, Agent ID, User ID, API Key)
- Added input sanitizer for XSS prevention, SQL injection protection, and path traversal prevention
- Created validation middleware that integrates all features
- Added comprehensive test suite with 19 tests

---

## 🧪 How you Tested

<!--
Provide clear steps for reviewers to validate the change.
Include commands, endpoints, or scenarios.
-->

1. Ran unit tests: `cargo test -p mofa-foundation validation`
2. All 19 validation tests pass
3. Schema validation tested (required field, min/max length, nested fields, patterns)
4. Rate limiter tested (basic limiting, different clients, status checking)
5. Sanitizer tested (HTML escaping, SQL injection prevention, path traversal)

---

## 📸 Screenshots / Logs (if applicable)

<!-- CLI output, logs, or UI screenshots -->

N/A

---

## ⚠️ Breaking Changes

- [x] No breaking changes
 change (describe below- [ ] Breaking)

If breaking:

---

## 🧹 Checklist

### Code Quality
- [x] Code follows Rust idioms and project conventions
- [ ] `cargo fmt` run
- [ ] `cargo clippy` passes without warnings

### Testing
- [x] Tests added/updated
- [x] `cargo test -p mofa-foundation validation` passes locally without any error

### Documentation
- [x] Public APIs documented
- [ ] README / docs updated (if needed)

### PR Hygiene
- [x] PR is small and focused (one logical change)
- [x] Branch is up to date with `main`
- [x] No unrelated commits
- [x] Commit messages explain **why**, not only **what**

---

## 🚀 Deployment Notes (if applicable)

<!-- migrations, config changes, env vars, rollout steps -->

No migrations or config changes required. The middleware is optional and disabled by default - users can enable it by creating a `ValidationMiddleware` instance and calling `validate_request()` in their request handlers.

---

## 🧩 Additional Notes for Reviewers

<!-- Anything reviewers should pay attention to -->

The implementation follows the existing MoFA architecture patterns and is placed in the foundation layer as specified in CLAUDE.md.
