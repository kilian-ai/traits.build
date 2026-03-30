#!/usr/bin/env python3
"""
ACP Lifecycle Traits - Start, stop, and check status of ACP proxy.
"""

import os
import signal
import subprocess
import time
from typing import Optional

ACP_PROXY_PORT = 9315
PID_FILE = "/tmp/acp_proxy.pid"

AGENT_MAP = {
    "opencode": ("opencode", ["acp"]),
    "claude": ("claude-code-acp", []),
    "codex": ("codex-acp", []),
    "copilot": ("copilot", ["--acp"]),
}


def get_env_for_agent(agent: str) -> dict:
    """Get environment variables for the agent."""
    env = os.environ.copy()

    if agent == "opencode":
        key = os.environ.get("OPENAI_API_KEY")
        if key:
            env["OPENAI_API_KEY"] = key
            env["OCODE_API_KEY"] = key
    elif agent == "claude":
        key = os.environ.get("ANTHROPIC_API_KEY")
        if key:
            env["ANTHROPIC_API_KEY"] = key
    elif agent == "codex":
        key = os.environ.get("OPENAI_API_KEY")
        if key:
            env["OPENAI_API_KEY"] = key
    elif agent == "copilot":
        key = os.environ.get("GITHUB_TOKEN")
        if key:
            env["GITHUB_TOKEN"] = key

    return env


def is_proxy_running() -> bool:
    """Check if the ACP proxy is already running."""
    import socket

    for addr in [("127.0.0.1", socket.AF_INET), ("::1", socket.AF_INET6)]:
        try:
            sock = socket.socket(addr[1], socket.SOCK_STREAM)
            sock.settimeout(1)
            result = sock.connect_ex(("localhost", ACP_PROXY_PORT))
            sock.close()
            if result == 0:
                return True
        except Exception:
            pass
    return False


def stop_existing_proxy() -> bool:
    """Stop any existing ACP proxy process."""
    if os.path.exists(PID_FILE):
        try:
            with open(PID_FILE) as f:
                pid = int(f.read().strip())
            os.killpg(pid, signal.SIGTERM)
            os.remove(PID_FILE)
            time.sleep(0.5)
            return True
        except Exception:
            if os.path.exists(PID_FILE):
                os.remove(PID_FILE)
    return False


def start_proxy(agent: str) -> Optional[subprocess.Popen]:
    """Start the ACP proxy with the specified agent."""
    stop_existing_proxy()

    if agent not in AGENT_MAP:
        raise ValueError(f"Unknown agent: {agent}. Available: {list(AGENT_MAP.keys())}")

    cmd, args = AGENT_MAP[agent]
    full_cmd = [cmd] + args

    env = get_env_for_agent(agent)
    env["NO_COLOR"] = "1"

    proc = subprocess.Popen(
        ["acp-proxy", "--no-auth", "--port", str(ACP_PROXY_PORT)] + full_cmd,
        env=env,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        preexec_fn=os.setsid,
    )

    # Wait for proxy to start
    for _ in range(30):
        time.sleep(0.5)
        if is_proxy_running():
            return proc

    proc.kill()
    raise Exception("Failed to start ACP proxy")


def acp_start(agent: str = "opencode") -> str:
    """Start the ACP proxy for the specified agent."""
    try:
        proc = start_proxy(agent)
        if proc:
            with open(PID_FILE, "w") as f:
                f.write(str(proc.pid))
            return f"ACP proxy started for {agent}"
        else:
            return "ACP proxy already running"
    except Exception as e:
        return f"Failed to start ACP proxy: {e}"


def acp_stop() -> str:
    """Stop the ACP proxy."""
    if os.path.exists(PID_FILE):
        try:
            with open(PID_FILE) as f:
                pid = int(f.read().strip())
            os.killpg(pid, signal.SIGTERM)
            os.remove(PID_FILE)
            return "ACP proxy stopped"
        except Exception as e:
            return f"Failed to stop ACP proxy: {e}"
    else:
        return "No ACP proxy process found"


def acp_status() -> str:
    """Check the status of the ACP proxy."""
    if is_proxy_running():
        return "ACP proxy is running"
    else:
        return "ACP proxy is not running"
