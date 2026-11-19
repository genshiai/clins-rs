"""Offline FHIR validation. JSON strings preserve decimal precision."""
import json
from ._clins import Validator as _Validator, __version__

class Validator:
    def __init__(self, definitions_json: str | None = None):
        self._inner = _Validator(definitions_json)

    def validate_json(self, resource_json: str, profile: str | None = None) -> str:
        return self._inner.validate_json(resource_json, profile)

    def validate(self, resource_json: str, profile: str | None = None) -> dict:
        return json.loads(self.validate_json(resource_json, profile))

__all__ = ["Validator", "__version__"]
