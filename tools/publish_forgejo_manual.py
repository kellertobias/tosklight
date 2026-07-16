#!/usr/bin/env python3
"""Idempotently attach a generated manual artifact to a Forgejo hosted release."""

from __future__ import annotations

import argparse
import json
import mimetypes
import os
import sys
import urllib.error
import urllib.parse
import urllib.request
import uuid
from pathlib import Path


class Forgejo:
    def __init__(self, base: str, repository: str, token: str):
        self.api = f"{base.rstrip('/')}/api/v1/repos/{repository}"
        self.token = token

    def request(self, method: str, path: str, data: bytes | None = None, content_type: str = "application/json"):
        request = urllib.request.Request(
            f"{self.api}{path}",
            data=data,
            method=method,
            headers={
                "Authorization": f"token {self.token}",
                "Accept": "application/json",
                "Content-Type": content_type,
                "User-Agent": "tosklight-manual-release/1",
            },
        )
        try:
            with urllib.request.urlopen(request, timeout=60) as response:
                body = response.read()
                return response.status, json.loads(body) if body else None
        except urllib.error.HTTPError as error:
            body = error.read().decode("utf-8", errors="replace")
            if error.code == 404:
                return 404, None
            raise RuntimeError(f"Forgejo API {method} {path} returned {error.code}: {body[:500]}") from error

    def release_for_tag(self, tag: str):
        _, release = self.request("GET", f"/releases/tags/{urllib.parse.quote(tag, safe='')}")
        if release:
            return release
        payload = json.dumps({
            "tag_name": tag,
            "name": f"ToskLight {tag}",
            "body": "Release artifacts for this version. The operator manual is generated from the same Markdown shipped in application Help.",
            "draft": False,
            "prerelease": "-" in tag,
        }).encode()
        _, release = self.request("POST", "/releases", payload)
        return release

    def upload(self, release: dict, source: Path, asset_name: str) -> None:
        for asset in release.get("assets", []):
            if asset.get("name") == asset_name:
                self.request("DELETE", f"/releases/{release['id']}/assets/{asset['id']}")
        boundary = f"----ToskLight{uuid.uuid4().hex}"
        file_type = mimetypes.guess_type(asset_name)[0] or "application/octet-stream"
        data = source.read_bytes()
        body = b"".join([
            f"--{boundary}\r\n".encode(),
            f'Content-Disposition: form-data; name="attachment"; filename="{asset_name}"\r\n'.encode(),
            f"Content-Type: {file_type}\r\n\r\n".encode(),
            data,
            f"\r\n--{boundary}--\r\n".encode(),
        ])
        self.request(
            "POST",
            f"/releases/{release['id']}/assets?name={urllib.parse.quote(asset_name)}",
            body,
            f"multipart/form-data; boundary={boundary}",
        )


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--tag", required=True)
    parser.add_argument("--file", type=Path, required=True)
    parser.add_argument("--asset-name")
    args = parser.parse_args()
    base = os.environ.get("FORGEJO_SERVER_URL", "")
    repository = os.environ.get("FORGEJO_REPOSITORY", "")
    token = os.environ.get("FORGEJO_TOKEN", "")
    if not all((base, repository, token)):
        print("FORGEJO_SERVER_URL, FORGEJO_REPOSITORY, and FORGEJO_TOKEN are required", file=sys.stderr)
        return 2
    if not args.tag.startswith("v") or not args.file.is_file():
        print("a v-prefixed tag and existing manual artifact are required", file=sys.stderr)
        return 2
    name = args.asset_name or f"tosklight-manual-{args.tag}{args.file.suffix}"
    if Path(name).name != name or not name:
        print("asset name must be a plain filename", file=sys.stderr)
        return 2
    try:
        api = Forgejo(base, repository, token)
        release = api.release_for_tag(args.tag)
        api.upload(release, args.file, name)
    except Exception as error:
        print(f"manual release upload failed: {error}", file=sys.stderr)
        return 1
    print(f"Attached {name} to Forgejo release {args.tag}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
