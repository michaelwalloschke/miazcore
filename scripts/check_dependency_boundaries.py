#!/usr/bin/env python3
"""Assert the Learning Client's direct dependency contract from Cargo metadata."""

from __future__ import annotations

import json
import sys


def fail(message: str) -> None:
    raise SystemExit(f"dependency boundary failed: {message}")


metadata = json.load(sys.stdin)
workspace_ids = set(metadata["workspace_members"])
workspace_packages = {
    package["name"]: package
    for package in metadata["packages"]
    if package["id"] in workspace_ids
}

expected_packages = {
    "client_protocol",
    "client_session",
    "client_bevy",
    "learning_client",
}
if set(workspace_packages) != expected_packages:
    fail(f"workspace packages are {sorted(workspace_packages)}")

dependencies = {
    package_name: {dependency["name"] for dependency in package["dependencies"]}
    for package_name, package in workspace_packages.items()
}

expected_protocol_dependencies = {
    "crypto-bigint",
    "hmac",
    "sha1",
    "sha2",
    "subtle",
    "zeroize",
}
if dependencies["client_protocol"] != expected_protocol_dependencies:
    fail(
        "client_protocol dependencies are "
        f"{sorted(dependencies['client_protocol'])}; expected approved crypto and fixture set"
    )
if "client_protocol" not in dependencies["client_session"]:
    fail("client_session must depend on client_protocol")
if dependencies["client_session"] != {"client_protocol", "getrandom", "zeroize"}:
    fail(
        "client_session dependencies are "
        f"{sorted(dependencies['client_session'])}; expected protocol, entropy, and zeroization"
    )
if "bevy" in dependencies["client_session"]:
    fail("client_session must remain engine-independent")
if not {"bevy", "client_session"}.issubset(dependencies["client_bevy"]):
    fail("client_bevy must depend on Bevy and client_session")
if "client_protocol" in dependencies["client_bevy"]:
    fail("client_bevy must not bypass client_session")
if not {"bevy", "client_bevy", "client_session"}.issubset(
    dependencies["learning_client"]
):
    fail("learning_client must compose Bevy, client_bevy, and client_session")

direct_bevy_users = {
    package_name
    for package_name, package_dependencies in dependencies.items()
    if "bevy" in package_dependencies
}
if direct_bevy_users != {"client_bevy", "learning_client"}:
    fail(f"unexpected direct Bevy users: {sorted(direct_bevy_users)}")

required_exact_versions = {
    ("client_protocol", "crypto-bigint"): "=0.6.1",
    ("client_protocol", "hmac"): "=0.12.1",
    ("client_protocol", "sha1"): "=0.10.6",
    ("client_protocol", "sha2"): "=0.10.9",
    ("client_protocol", "subtle"): "=2.6.1",
    ("client_protocol", "zeroize"): "=1.8.2",
    ("client_bevy", "bevy"): "=0.19.0",
    ("learning_client", "bevy"): "=0.19.0",
    ("client_session", "getrandom"): "=0.3.4",
    ("client_session", "zeroize"): "=1.8.2",
    ("client_bevy", "blake3"): "=1.8.5",
    ("learning_client", "blake3"): "=1.8.5",
}
for (package_name, dependency_name), expected_requirement in required_exact_versions.items():
    dependency = next(
        (
            dependency
            for dependency in workspace_packages[package_name]["dependencies"]
            if dependency["name"] == dependency_name
        ),
        None,
    )
    if dependency is None:
        fail(f"{package_name} is missing {dependency_name}")
    if dependency["req"] != expected_requirement:
        fail(
            f"{package_name} requires {dependency_name} {dependency['req']}; "
            f"expected {expected_requirement}"
        )

print("dependency boundary: four packages, one-way graph, approved crypto, and exact pins passed")
