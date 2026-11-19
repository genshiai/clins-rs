# clins-ts

ESM TypeScript declarations and browser/Bun/Node-compatible WebAssembly. One Rust validator.

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.130 --locked
./scripts/build-wasm.sh
cd bindings/typescript
bun pm pack --destination ../../dist
```

Browser:

```ts
import init, { Validator } from 'clins-ts';
await init();
const v = new Validator(definitionsJson); // omit bundle for embedded R4
try { console.log(v.validate(resourceJson).status); } finally { v.free(); }
```

Bun or Node ESM:

```ts
import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { initSync, Validator } from 'clins-ts';
const require = createRequire(import.meta.url);
initSync({module: readFileSync(require.resolve('clins-ts/wasm'))});
const v = new Validator();
console.log(v.validate('{"resourceType":"Patient"}').status);
v.free();
```

`validateJson` returns report JSON, `validate` returns the typed report. Pass JSON strings;
parsing resource decimals into JS Numbers before calling the SDK can lose precision.
The definition bundle is separate public reference data. For large documents use a Web Worker
to keep the page responsive. No network request is made by validation.
This alpha's runtime verification matrix is in `docs/verification.md`; Node compatibility
of the ESM API is distinct from whether Node was included in a particular test run.
