import json
import unittest
from pathlib import Path
from clins_py import Validator
ROOT = Path(__file__).resolve().parents[3]

class ValidatorTest(unittest.TestCase):
    def test_r4_and_errors(self):
        validator = Validator()
        self.assertEqual(validator.validate('{"resourceType":"Patient","active":true}')['status'], 'valid')
        self.assertEqual(validator.validate('{"resourceType":"Patient","active":"yes"}')['status'], 'invalid')
        self.assertEqual(validator.validate('{"resourceType":"Patient"}', 'urn:missing')['status'], 'incomplete')
        self.assertEqual(validator.validate('{"resourceType":"Patient","telecom":[{"rank":0}]}')['status'], 'invalid')
        with self.assertRaises(ValueError): validator.validate('{')
        with self.assertRaises(ValueError): validator.validate('{"resourceType":"Patient","active":true,"active":false}')

    def test_clins_positive_and_negative(self):
        validator = Validator((ROOT / 'data/definitions.json').read_text())
        for name in ['patient', 'conditions', 'allergies', 'laboratory', 'referral', 'discharge', 'patient_summary', 'medications']:
            with self.subTest(name=name):
                resource = json.loads((ROOT / f'crates/core/tests/fixtures/{name}.json').read_text())
                self.assertEqual(validator.validate(json.dumps(resource))['status'], 'valid')
                if name == 'patient': resource['gender'] = 'invalid'
                elif name in ['referral', 'discharge', 'patient_summary']: del resource['entry'][0]['resource']['section']
                elif name == 'medications': resource['entry'][1]['resource']['status'] = 'invalid'
                elif name == 'conditions': del resource['entry'][1]['resource']['onsetDateTime']
                else: del resource['entry'][1]['resource']['code']
                self.assertEqual(validator.validate(json.dumps(resource))['status'], 'invalid')

if __name__ == '__main__': unittest.main()
