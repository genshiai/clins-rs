import { beforeAll, describe, test, expect } from 'bun:test';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { initSync, Validator } from '../index.js';
const root = resolve(import.meta.dir, '../../..');
beforeAll(() => initSync({module: readFileSync(resolve(import.meta.dir, '../pkg/clins_ts_bg.wasm'))}));
describe('Rust WASM SDK', () => {
  test('R4, invalid and incomplete', () => {
    const v = new Validator();
    try {
      expect(v.validate('{"resourceType":"Patient","active":true}').status).toBe('valid');
      expect(v.validate('{"resourceType":"Patient","active":"yes"}').status).toBe('invalid');
      expect(v.validate('{"resourceType":"Patient"}', 'urn:missing').status).toBe('incomplete');
      expect(() => v.validate('{')).toThrow();
    } finally { v.free(); }
  });
  test('CLINS bundle and mutations', () => {
    const v = new Validator(readFileSync(`${root}/data/definitions.json`, 'utf8'));
    try {
      for (const name of ['patient','conditions','allergies','laboratory','referral','discharge','patient_summary','medications']) {
        const resource = JSON.parse(readFileSync(`${root}/crates/core/tests/fixtures/${name}.json`, 'utf8'));
        expect(v.validate(JSON.stringify(resource)).status).toBe('valid');
        if (name === 'patient') resource.gender = 'invalid';
        else if (['referral','discharge','patient_summary'].includes(name)) delete resource.entry[0].resource.section;
        else if (name === 'medications') resource.entry[1].resource.status = 'invalid';
        else if (name === 'conditions') delete resource.entry[1].resource.onsetDateTime;
        else delete resource.entry[1].resource.code;
        expect(v.validate(JSON.stringify(resource)).status).toBe('invalid');
      }
    } finally { v.free(); }
  }, 120_000);
});
