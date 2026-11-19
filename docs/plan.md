# Implementation plan

1. Extract an independent in-memory Rust Core with original public types and retain MIT provenance.
2. Integrate a current JSON-based Rust FHIRPath evaluator and pinned R4/JP Core/CLINS definitions.
3. Isolate checksum-verified package preparation, CLI and HTTP adapters.
4. Build Python native, TypeScript WASM and Kotlin/JVM bindings over that same Core.
5. Run official/synthetic positive and negative cases; verify bindings, browser execution,
   malformed input, incomplete outcomes and artifact contents.
6. Ship an explicitly experimental alpha with source, usable artifacts and reproducible CI.
7. Advance toward conformance with an explicit remaining-rule inventory; publish a stable release
   only after the coverage claims are supported. National service connection testing requires
   the participating institution's real connection environment.

The independently usable alpha and complete conformance certification are distinct milestones.
The estimate for a complete independent validator and mature multi-platform SDK distribution was
10–20 person-weeks; that was not a promise that exhaustive validation could be proven in one turn.
See `coverage.md` for concrete limits rather than treating successful fixture tests as completion
of item 7. The source, SDK artifact and CI verification record is maintained in `verification.md`.
