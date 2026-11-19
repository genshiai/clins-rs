import { test, expect } from '@playwright/test';
import type { Validator } from '../index.js';
declare global { interface Window { ready: boolean; validatorFactory: (definitions?: string) => Validator } }
test('browser executes shared engine cases including local FHIRPath functions', async ({page}) => {
  await page.goto('/');
  await page.waitForFunction(() => window.ready);
  const mismatches = await page.evaluate(async () => {
    const definitions = await fetch('/fixtures/engine-definitions.json').then(r => r.text());
    const cases = await fetch('/fixtures/engine-cases.tsv').then(r => r.text());
    const v = window.validatorFactory(definitions);
    try {
      return cases.trimEnd().split('\n').flatMap(line => {
        const [name, expected, profile, resource] = line.split('\t');
        const report = v.validate(resource, profile);
        return report.status === expected ? [] : [{name, expected, report}];
      });
    } finally { v.free(); }
  });
  expect(mismatches).toEqual([]);
});
test('browser executes R4 and CLINS through the same Rust WASM', async ({page}) => {
  const errors: string[] = [];
  page.on('pageerror', error => errors.push(error.message));
  await page.goto('/');
  await page.waitForFunction(() => window.ready);
  const statuses = await page.evaluate(async () => {
    const r4 = window.validatorFactory();
    const out = [r4.validate('{"resourceType":"Patient","active":true}').status, r4.validate('{"resourceType":"Patient","active":"yes"}').status];
    r4.free();
    const definitions = await fetch('/definitions.json').then(r => r.text());
    const clins = window.validatorFactory(definitions);
    try {
      for (const name of ['patient','conditions','allergies','laboratory','referral','discharge','patient_summary','medications']) {
        out.push(clins.validate(await fetch(`/fixtures/${name}.json`).then(r => r.text())).status);
      }
    } finally { clins.free(); }
    return out;
  });
  expect(statuses).toEqual(['valid','invalid',...Array(8).fill('valid')]);
  expect(errors).toEqual([]);
});
