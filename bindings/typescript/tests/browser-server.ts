import { resolve } from 'node:path';
const sdk = resolve(import.meta.dir, '..');
const root = resolve(sdk, '../..');
const fixtures = new Set(['patient','conditions','allergies','laboratory','referral','discharge','patient_summary','medications']);
Bun.serve({hostname: '127.0.0.1', port: 8097, fetch(request) {
  const path = new URL(request.url).pathname;
  if (path === '/') return new Response(`<!doctype html><meta charset="utf-8"><title>clins-ts test</title><script type="module">import init,{Validator} from '/index.js'; await init(); window.validatorFactory = definitions => new Validator(definitions); window.ready = true;</script>`, {headers:{'content-type':'text/html'}});
  if (path === '/definitions.json') return new Response(Bun.file(`${root}/data/definitions.json`));
  if (path.startsWith('/fixtures/')) {
    const name = path.slice('/fixtures/'.length).replace(/\.json$/, '');
    if (fixtures.has(name)) return new Response(Bun.file(`${root}/crates/core/tests/fixtures/${name}.json`));
  }
  if (['/index.js','/pkg/clins_ts.js','/pkg/clins_ts_bg.wasm'].includes(path)) return new Response(Bun.file(`${sdk}${path}`));
  return new Response('not found', {status:404});
}});
