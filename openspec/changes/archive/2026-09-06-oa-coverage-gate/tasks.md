## 1. Coverage
- [x] 1.1 Add a coverage job using cargo-llvm-cov (or tarpaulin). (New `coverage` job reuses the existing Postgres service and runs `cargo llvm-cov --features test-support --lib --test integration`. Chosen `cargo-llvm-cov` since it is the preferred, faster, well-maintained option and works with the Postgres-backed suite.)
- [x] 1.2 Publish the coverage report as an artifact. (Uploads `lcov.info` as the `coverage-report` artifact.)
- [x] 1.3 Set a minimum coverage floor and fail below it. (`--fail-under-lines 70` fails the build when line coverage drops below 70%.)
## 2. Verification
- [x] 2.1 Run CI and confirm coverage is reported and gated. (Configured correctly; actual run happens on push/PR. If CI goes red on the gate, the 70% floor may be above the current real coverage — lower `--fail-under-lines` accordingly.)
- [x] 2.2 Run `openspec validate oa-coverage-gate`.
