# Implementation plan

Current schedule, integration milestones and external prerequisites (Japanese):
[使いどころと開発予定](roadmap.ja.md).

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

The source, CLI/HTTP and SDK adapters are implemented and tested. The subsequent
engine pass resolves the five known CLINS discriminator warnings, implements local
FHIRPath functions/context, and shares an identical engine corpus across all adapters.
Versioned artifacts and independent synthetic reference comparison are verified before
publication. Comprehensive certification and national service connection tests require
evidence beyond successful sample acceptance; those claims are not made by a release.
See `coverage.md`, `remaining-work.md` and `verification.md` for the precise boundaries.
