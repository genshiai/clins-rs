import {test} from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {fileURLToPath} from 'node:url';
import {initSync, Validator} from '../index.js';
const root = fileURLToPath(new URL('../../../', import.meta.url));
initSync({module:readFileSync(new URL('../pkg/clins_ts_bg.wasm', import.meta.url))});
test('Node ESM uses the same WASM and preserves the JSON boundary', () => {
  const r4 = new Validator();
  try {
    assert.equal(r4.validate('{"resourceType":"Patient","active":true}').status, 'valid');
    assert.equal(r4.validate('{"resourceType":"Patient","active":"yes"}').status, 'invalid');
    assert.equal(r4.validate('{"resourceType":"Patient"}', 'urn:missing').status, 'incomplete');
    assert.equal(r4.validate('{"resourceType":"Patient","telecom":[{"rank":0}]}').status, 'invalid');
    assert.throws(() => r4.validate('{"resourceType":"Patient","active":true,"active":false}'));
  } finally { r4.free(); }
  const clins = new Validator(readFileSync(`${root}/data/definitions.json`, 'utf8'));
  try {
    for (const name of ['patient','conditions','allergies','laboratory','referral','discharge','patient_summary','medications']) {
      assert.equal(clins.validate(readFileSync(`${root}/crates/core/tests/fixtures/${name}.json`, 'utf8')).status, 'valid');
    }
  } finally { clins.free(); }
});
