export { default, initSync, type InitInput, type InitOutput, type SyncInitInput } from './pkg/clins_ts.js';
export type Status = 'valid' | 'invalid' | 'incomplete';
export interface Issue { severity: 'fatal' | 'error' | 'warning' | 'information'; code: string; diagnostics: string; expression?: string[] }
export interface Report { status: Status; fhir_version: '4.0.1'; engine: string; structural_checks_passed: boolean; deferred_checks: number; definitions: {url: string; version: string; conversion_warnings: string[]}[]; packages: {name: string; version: string; file: string; sha256: string; source: string}[]; outcome: {resourceType: 'OperationOutcome'; issue: Issue[]} }
export declare class Validator {
  constructor(definitionsJson?: string);
  validateJson(resourceJson: string, profile?: string): string;
  validate(resourceJson: string, profile?: string): Report;
  free(): void;
}
