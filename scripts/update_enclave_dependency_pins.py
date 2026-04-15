#!/usr/bin/env python3
"""Refresh moving enclave dependency pins in enclave/Dockerfile.

The updater intentionally opens PRs instead of deploying automatically. Docker
tags such as latest are resolved to immutable digests, while GitHub/PyPI
dependencies are resolved to concrete versions or commits.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import sys
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
DOCKERFILE = ROOT / "enclave" / "Dockerfile"


class UpdateError(RuntimeError):
    pass


def request_json(url: str, headers: dict[str, str] | None = None) -> object:
    request = urllib.request.Request(url, headers=headers or {})
    try:
        with urllib.request.urlopen(request, timeout=30) as response:
            return json.loads(response.read().decode("utf-8"))
    except urllib.error.HTTPError as exc:
        detail = exc.read().decode("utf-8", "replace")
        raise UpdateError(f"GET {url} failed: HTTP {exc.code}: {detail[:300]}") from exc
    except urllib.error.URLError as exc:
        raise UpdateError(f"GET {url} failed: {exc.reason}") from exc


def request_headers(url: str, headers: dict[str, str] | None = None) -> dict[str, str]:
    request = urllib.request.Request(url, headers=headers or {}, method="HEAD")
    try:
        with urllib.request.urlopen(request, timeout=30) as response:
            return dict(response.headers.items())
    except urllib.error.HTTPError as exc:
        detail = exc.read().decode("utf-8", "replace")
        raise UpdateError(f"HEAD {url} failed: HTTP {exc.code}: {detail[:300]}") from exc
    except urllib.error.URLError as exc:
        raise UpdateError(f"HEAD {url} failed: {exc.reason}") from exc


def github_headers() -> dict[str, str]:
    headers = {
        "Accept": "application/vnd.github+json",
        "X-GitHub-Api-Version": "2022-11-28",
        "User-Agent": "lightfriend-enclave-dependency-updater",
    }
    token = os.environ.get("GITHUB_TOKEN")
    if token:
        headers["Authorization"] = f"Bearer {token}"
    return headers


def dockerhub_token(repository: str) -> str:
    query = urllib.parse.urlencode({"service": "registry.docker.io", "scope": f"repository:{repository}:pull"})
    data = request_json(f"https://auth.docker.io/token?{query}")
    if not isinstance(data, dict) or not isinstance(data.get("token"), str):
        raise UpdateError(f"Docker Hub token response for {repository} did not include token")
    return data["token"]


def registry_bearer_token(www_authenticate: str) -> str:
    match = re.match(r'Bearer\s+(.*)$', www_authenticate, re.IGNORECASE)
    if not match:
        raise UpdateError(f"Unsupported registry auth challenge: {www_authenticate}")

    params: dict[str, str] = {}
    for key, value in re.findall(r'(\w+)="([^"]*)"', match.group(1)):
        params[key] = value

    realm = params.pop("realm", None)
    if not realm:
        raise UpdateError(f"Registry auth challenge did not include realm: {www_authenticate}")

    token_url = f"{realm}?{urllib.parse.urlencode(params)}"
    data = request_json(token_url)
    if not isinstance(data, dict) or not isinstance(data.get("token"), str):
        raise UpdateError(f"Registry token response for {token_url} did not include token")
    return data["token"]


def parse_image(image: str) -> tuple[str, str, str]:
    if "/" not in image:
        registry = "registry-1.docker.io"
        path_tag = f"library/{image}"
    else:
        first, rest = image.split("/", 1)
        if "." in first or ":" in first or first == "localhost":
            registry = first
            path_tag = rest
        else:
            registry = "registry-1.docker.io"
            path_tag = image

    if ":" in path_tag.rsplit("/", 1)[-1]:
        path, tag = path_tag.rsplit(":", 1)
    else:
        path, tag = path_tag, "latest"
    return registry, path, tag


def resolve_image_digest(image: str) -> str:
    registry, path, tag = parse_image(image)
    headers = {
        "Accept": "application/vnd.docker.distribution.manifest.list.v2+json, "
        "application/vnd.oci.image.index.v1+json, "
        "application/vnd.docker.distribution.manifest.v2+json, "
        "application/vnd.oci.image.manifest.v1+json",
        "User-Agent": "lightfriend-enclave-dependency-updater",
    }
    if registry == "registry-1.docker.io":
        headers["Authorization"] = f"Bearer {dockerhub_token(path)}"

    url = f"https://{registry}/v2/{path}/manifests/{tag}"
    try:
        response_headers = request_headers(url, headers)
    except UpdateError as exc:
        if "HTTP 401" not in str(exc):
            raise

        challenge_request = urllib.request.Request(url, headers=headers, method="HEAD")
        try:
            urllib.request.urlopen(challenge_request, timeout=30).close()
        except urllib.error.HTTPError as auth_exc:
            challenge = auth_exc.headers.get("WWW-Authenticate")
            if auth_exc.code != 401 or not challenge:
                raise exc from auth_exc
            headers["Authorization"] = f"Bearer {registry_bearer_token(challenge)}"
            response_headers = request_headers(url, headers)
        except urllib.error.URLError as auth_exc:
            raise exc from auth_exc

    digest = response_headers.get("Docker-Content-Digest") or response_headers.get("docker-content-digest")
    if not digest or not digest.startswith("sha256:"):
        raise UpdateError(f"Registry response for {image} did not include Docker-Content-Digest")

    display_registry = "" if registry == "registry-1.docker.io" else f"{registry}/"
    return f"{display_registry}{path}@{digest}"


def latest_github_release(owner: str, repo: str) -> str:
    data = request_json(f"https://api.github.com/repos/{owner}/{repo}/releases/latest", github_headers())
    if not isinstance(data, dict) or not isinstance(data.get("tag_name"), str):
        raise UpdateError(f"GitHub latest release response for {owner}/{repo} did not include tag_name")
    return data["tag_name"]


def github_branch_commit(owner: str, repo: str, branch: str) -> str:
    data = request_json(f"https://api.github.com/repos/{owner}/{repo}/commits/{branch}", github_headers())
    if not isinstance(data, dict) or not isinstance(data.get("sha"), str):
        raise UpdateError(f"GitHub commit response for {owner}/{repo}@{branch} did not include sha")
    return data["sha"]


def latest_pypi_version(package: str) -> str:
    data = request_json(f"https://pypi.org/pypi/{package}/json")
    if not isinstance(data, dict):
        raise UpdateError(f"PyPI response for {package} was not an object")
    info = data.get("info")
    if not isinstance(info, dict) or not isinstance(info.get("version"), str):
        raise UpdateError(f"PyPI response for {package} did not include info.version")
    return info["version"]


def replace_arg(contents: str, name: str, value: str) -> tuple[str, bool]:
    pattern = re.compile(rf"^ARG {re.escape(name)}=.*$", re.MULTILINE)
    replacement = f"ARG {name}={value}"
    if not pattern.search(contents):
        raise UpdateError(f"Could not find ARG {name}=... in {DOCKERFILE}")
    updated = pattern.sub(replacement, contents, count=1)
    return updated, updated != contents


def current_arg(contents: str, name: str) -> str:
    match = re.search(rf"^ARG {re.escape(name)}=(.*)$", contents, re.MULTILINE)
    if not match:
        raise UpdateError(f"Could not find ARG {name}=... in {DOCKERFILE}")
    return match.group(1).strip()


def resolve_updates() -> dict[str, str]:
    return {
        "VEHICLE_COMMAND_COMMIT": github_branch_commit("teslamotors", "vehicle-command", "main"),
        "TUWUNEL_IMAGE": resolve_image_digest("jevolk/tuwunel:latest"),
        "MAUTRIX_WHATSAPP_IMAGE": resolve_image_digest("dock.mau.dev/mautrix/whatsapp:latest"),
        "MAUTRIX_SIGNAL_IMAGE": resolve_image_digest("dock.mau.dev/mautrix/signal:latest"),
        "CLOUDFLARED_VERSION": latest_github_release("cloudflare", "cloudflared"),
        "MAUTRIX_TELEGRAM_VERSION": latest_pypi_version("mautrix-telegram"),
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true", help="exit non-zero when Dockerfile pins are stale")
    args = parser.parse_args()

    contents = DOCKERFILE.read_text()
    updates = resolve_updates()
    changed = False

    for name, value in updates.items():
        old = current_arg(contents, name)
        contents, did_change = replace_arg(contents, name, value)
        changed = changed or did_change
        if old != value:
            print(f"{name}: {old} -> {value}")
        else:
            print(f"{name}: already {value}")

    if args.check:
        if changed:
            print("Dependency pins are stale; run this script without --check to update.", file=sys.stderr)
            return 1
        return 0

    if changed:
        DOCKERFILE.write_text(contents)
        print(f"Updated {DOCKERFILE.relative_to(ROOT)}")
    else:
        print("No dependency pin changes.")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except UpdateError as exc:
        print(f"FATAL: {exc}", file=sys.stderr)
        raise SystemExit(2)
