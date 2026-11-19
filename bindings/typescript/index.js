// SPDX-License-Identifier: Apache-2.0
import init, { initSync, Validator as NativeValidator } from './pkg/clins_ts.js';
export { initSync };
export default init;
export class Validator {
  constructor(definitionsJson) { this.inner = new NativeValidator(definitionsJson); }
  validateJson(resourceJson, profile) { return this.inner.validateJson(resourceJson, profile); }
  validate(resourceJson, profile) { return JSON.parse(this.validateJson(resourceJson, profile)); }
  free() { this.inner.free(); }
}
