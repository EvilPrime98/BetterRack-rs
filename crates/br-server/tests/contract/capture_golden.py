"""Record the reference (Bun) server's answers to every request in requests.txt as golden data.

    python capture_golden.py http://127.0.0.1:3100 golden/responses.json

JSON bodies are stored as JSON (volatile `generatedAt` and `createdAt` dropped), anything else as
sha256 + length, so page/thumbnail bytes are pinned without committing images. Run it only against
the shared test library (make_test_library.py), never against a real library.
"""
import hashlib
import json
import sys
import urllib.error
import urllib.request
from pathlib import Path

HEADERS = ("etag", "cache-control", "content-type")
VOLATILE = {"generatedAt", "createdAt"}


def scrub(v):
    if isinstance(v, dict):
        return {k: scrub(x) for k, x in v.items() if k not in VOLATILE}
    if isinstance(v, list):
        return [scrub(x) for x in v]
    return v


def fetch(base, method, path):
    req = urllib.request.Request(base + path, method=method)
    try:
        r = urllib.request.urlopen(req)
    except urllib.error.HTTPError as e:
        r = e
    body = r.read()
    entry = {"status": r.status, "headers": {h: r.headers[h] for h in HEADERS if r.headers.get(h)}}
    try:
        entry["json"] = scrub(json.loads(body))
    except ValueError:
        entry["sha256"] = hashlib.sha256(body).hexdigest()
        entry["length"] = len(body)
    return entry


def main():
    base, out = sys.argv[1].rstrip("/"), Path(sys.argv[2])
    reqs = Path(__file__).with_name("requests.txt").read_text(encoding="utf-8").splitlines()
    golden = {}
    for line in reqs:
        if not line.strip() or line.startswith("#"):
            continue
        method, path = line.split(" ", 1)
        golden[line] = fetch(base, method, path)
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(golden, indent=2, sort_keys=True, ensure_ascii=False) + "\n", encoding="utf-8")
    print(f"{len(golden)} responses -> {out}")


if __name__ == "__main__":
    main()
