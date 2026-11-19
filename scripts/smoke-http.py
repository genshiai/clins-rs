#!/usr/bin/env python3
"""Exercise the real loopback server with public synthetic fixtures only."""
import json
import signal
import subprocess
import sys
from pathlib import Path
from urllib.error import HTTPError
from urllib.request import Request, ProxyHandler, build_opener
ROOT = Path(__file__).resolve().parent.parent
binary = Path(sys.argv[1] if len(sys.argv)>1 else ROOT/'target/release/clins').resolve()
process = subprocess.Popen([str(binary),'--packages',str(ROOT/'clins-packages.lock.json'),'serve','--bind','127.0.0.1:0'], stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, text=True)
try:
    line = process.stderr.readline().strip()
    assert line.startswith('Offline Rust FHIR validation on http://127.0.0.1:'), line
    base = line.rsplit(' ',1)[-1]
    opener = build_opener(ProxyHandler({}))
    def call(path, body=None):
        request = Request(base+path, data=body, headers={'Content-Type':'application/fhir+json'})
        with opener.open(request, timeout=30) as response:
            assert response.headers['Cache-Control'] == 'no-store'
            return json.load(response)
    capability = call('/capabilities')
    assert capability['engine'].startswith('clins-core/')
    for name in ['patient','conditions','allergies','laboratory','referral','discharge','patient_summary','medications']:
        payload = (ROOT/f'crates/core/tests/fixtures/{name}.json').read_bytes()
        report = call('/validate', payload)
        assert report['status']=='valid', (name, report['outcome'])
    outcome = call('/$validate',b'{"resourceType":"Patient","meta":{"profile":["urn:missing"]}}')
    assert outcome['resourceType']=='OperationOutcome'
    assert any(issue['severity']=='error' for issue in outcome['issue'])
    try:
        call('/validate', b'{"resourceType":"Patient","active":true,"active":false}')
        raise AssertionError('duplicate properties were accepted')
    except HTTPError as error:
        assert error.code==400
    print(json.dumps({'engine':capability['engine'],'synthetic_documents':8,'incomplete_operation':'error','ambiguous_json':'rejected'}))
finally:
    if process.poll() is None:
        process.send_signal(signal.SIGINT)
        try: process.wait(timeout=10)
        except subprocess.TimeoutExpired:
            process.kill(); process.wait()
