#!/usr/bin/env python3
"""Publish the server module with environment values from its application profile."""

from __future__ import annotations

import os
import shlex
import subprocess
import sys
import tomllib
from pathlib import Path
from urllib.parse import urlsplit


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_PROFILE = "example"
DEFAULT_MODULE_NAME = "kommunikation"
MODULE_ENV_KEYS = (
    "JMAP_DISPATCH_ENABLED",
    "OIDC_ISSUER_URL",
    "SYSTEM_JMAP_USERNAME",
    "SYSTEM_JMAP_PASSWORD",
    "MESSAGE_ID_DOMAIN",
    "MAIL_UNSUBSCRIBE_BASE_URL",
)


def required_table(profile: dict, key: str) -> dict:
    value = profile.get(key)
    if not isinstance(value, dict):
        raise ValueError(f"Profile section [{key}] is required")
    return value


def required_string(section: dict, key: str, *, allow_empty: bool = False) -> str:
    value = section.get(key)
    if not isinstance(value, str):
        raise ValueError(f"Profile value {key!r} must be a string")
    if not allow_empty and not value.strip():
        raise ValueError(f"Profile value {key!r} must not be empty")
    return value


def environment_flag(section: dict, key: str) -> str:
    value = section.get(key)
    if isinstance(value, bool):
        return str(value).lower()
    if isinstance(value, str) and value.strip().lower() in {"true", "false"}:
        return value.strip().lower()
    raise ValueError(f"Profile value {key!r} must be a TOML boolean or 'true'/'false'")


def profile_environment(profile: dict) -> tuple[str, dict[str, str]]:
    oauth = required_table(profile, "oauth")
    server = required_table(profile, "server")
    spacetimedb = required_table(profile, "spacetimedb")
    mail = required_table(profile, "mail")

    issuer_url = required_string(oauth, "issuer_url")
    frontend_base_url = required_string(server, "frontend_base_url")
    frontend_host = urlsplit(frontend_base_url).hostname
    if not frontend_host:
        raise ValueError("[server].frontend_base_url must contain a hostname")

    spacetimedb_uri = required_string(spacetimedb, "uri")
    module_name = required_string(spacetimedb, "module_name")
    values = {
        "JMAP_DISPATCH_ENABLED": environment_flag(mail, "jmap_dispatch_enabled"),
        "OIDC_ISSUER_URL": issuer_url,
        "SYSTEM_JMAP_USERNAME": required_string(
            mail, "system_jmap_username", allow_empty=True
        ),
        "SYSTEM_JMAP_PASSWORD": required_string(
            mail, "system_jmap_password", allow_empty=True
        ),
        "MESSAGE_ID_DOMAIN": frontend_host,
        "MAIL_UNSUBSCRIBE_BASE_URL": (
            f"{spacetimedb_uri.rstrip('/')}/v1/database/{module_name}"
            "/route/mailing-list/unsubscribe"
        ),
    }
    return module_name, values


def _server_description(arguments: list[str]) -> str:
    for index, argument in enumerate(arguments):
        if argument in {"-s", "--server"}:
            return arguments[index + 1] if index + 1 < len(arguments) else "(missing value)"
        if argument.startswith("--server="):
            return argument.partition("=")[2]
    return "the local default server"


def main() -> int:
    arguments = sys.argv[1:]
    dry_run = "--dry-run" in arguments
    if dry_run:
        arguments.remove("--dry-run")
    if any(
        argument in {"-p", "--module-path"} or argument.startswith("--module-path=")
        for argument in arguments
    ):
        print("The module path is selected by this wrapper.", file=sys.stderr)
        return 2

    profile_name = os.environ.get("APP_PROFILE", DEFAULT_PROFILE).strip() or DEFAULT_PROFILE
    if Path(profile_name).name != profile_name:
        print("APP_PROFILE must be a profile directory name.", file=sys.stderr)
        return 2

    profile_path = ROOT / "profiles" / profile_name / "profile.toml"
    try:
        with profile_path.open("rb") as profile_file:
            profile = tomllib.load(profile_file)
        module_name, profile_values = profile_environment(profile)
    except (OSError, tomllib.TOMLDecodeError, ValueError) as error:
        print(f"Failed to load publish profile {profile_path}: {error}", file=sys.stderr)
        return 2

    env = os.environ.copy()
    env["APP_PROFILE"] = profile_name
    env.update(profile_values)
    command = [
        "spacetime",
        "publish",
        "--module-path",
        str(ROOT / "server"),
        module_name,
        *arguments,
    ]

    print(
        f"Publishing profile {profile_name!r} as module {module_name!r} "
        f"to {_server_description(arguments)}"
    )
    print(f"Module environment keys: {', '.join(MODULE_ENV_KEYS)}")
    if dry_run:
        print(shlex.join(command))
        return 0
    return subprocess.run(command, cwd=ROOT, env=env, check=False).returncode


if __name__ == "__main__":
    raise SystemExit(main())
