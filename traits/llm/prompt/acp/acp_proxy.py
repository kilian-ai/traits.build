#!/usr/bin/env python3
"""
ACP Proxy Trait - Routes prompts to ACP agents via chrome-acp proxy.
Supports: opencode, claude, codex, copilot
"""

import asyncio
import json
import os
import subprocess
import signal
import time
from typing import Any, Optional


ACP_PROXY_URL = "ws://localhost:9315/ws"
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
            with open(PID_FILE, "w") as f:
                f.write(str(proc.pid))
            return proc

    proc.kill()
    raise Exception("Failed to start ACP proxy")


class AcpClient:
    def __init__(self):
        self.ws = None
        self.session_id = None
        self.response_parts = []
        self.msg_id = 1

    async def connect(self):
        import websockets

        self.ws = await websockets.connect(ACP_PROXY_URL)
        await self.ws.send(json.dumps({"type": "connect"}))

        while True:
            msg = await self.recv()
            if msg.get("type") == "status":
                if not msg.get("payload", {}).get("connected"):
                    raise Exception("Failed to connect to agent")
                break
            elif msg.get("type") == "error":
                raise Exception(
                    f"Connection error: {msg.get('payload', {}).get('message')}"
                )

    async def send(self, msg_type: str, payload: Any = None):
        msg = {"type": msg_type}
        if payload is not None:
            msg["payload"] = payload
        await self.ws.send(json.dumps(msg))

    async def recv(self) -> dict:
        data = await self.ws.recv()
        return json.loads(data)

    async def new_session(self, cwd: str = None):
        payload = {}
        if cwd:
            payload["cwd"] = cwd

        await self.send("new_session", payload)

        while True:
            msg = await self.recv()
            if msg.get("type") == "session_created":
                self.session_id = msg.get("payload", {}).get("sessionId")
                return self.session_id
            elif msg.get("type") == "error":
                raise Exception(
                    f"Session error: {msg.get('payload', {}).get('message')}"
                )

    async def prompt(self, text: str):
        await self.send("prompt", {"content": [{"type": "text", "text": text}]})

        while True:
            msg = await self.recv()

            if msg.get("type") == "session_update":
                payload = msg.get("payload", {})
                update = payload.get("update", {})
                session_update = update.get("sessionUpdate", "")

                if session_update == "agent_message_chunk":
                    content = update.get("content", {})
                    if content.get("type") == "text":
                        self.response_parts.append(content.get("text", ""))
                elif session_update == "agent_message":
                    for item in update.get("content", []):
                        if item.get("type") == "text":
                            self.response_parts.append(item.get("text", ""))

            elif msg.get("type") == "prompt_complete":
                return

            elif msg.get("type") == "error":
                raise Exception(
                    f"Prompt error: {msg.get('payload', {}).get('message')}"
                )

    async def close(self):
        if self.ws:
            await self.ws.close()


async def run_acp(prompt: str, cwd: str = None) -> str:
    """Run ACP prompt via chrome-acp proxy."""
    client = AcpClient()

    try:
        await client.connect()
    except Exception as e:
        return f"Error connecting to ACP proxy: {e}"

    try:
        await client.new_session(cwd=cwd)
    except Exception as e:
        return f"Error creating session: {e}"

    try:
        await client.prompt(prompt)
    except Exception as e:
        return f"Error sending prompt: {e}"

    await client.close()

    return "".join(client.response_parts) or "[No response from agent]"


def acp_proxy(
    prompt: str, agent: str = "opencode", cwd: str = ".", auto_approve: bool = False
) -> str:
    """
    Route a prompt to an ACP agent via chrome-acp proxy.

    Automatically starts the ACP proxy if not running.

    Args:
        prompt: The user prompt to send
        agent: Agent to use (opencode, claude, codex, copilot)
        cwd: Working directory
        auto_approve: Not used (proxy handles permissions)

    Returns:
        Agent response text
    """
    cwd = os.path.abspath(cwd) if cwd else os.getcwd()

    if not is_proxy_running():
        try:
            start_proxy(agent)
        except Exception as e:
            return f"Error starting ACP proxy: {e}"

    return asyncio.run(run_acp(prompt, cwd))


if __name__ == "__main__":
    import sys

    agent = sys.argv[1] if len(sys.argv) > 1 else "opencode"
    result = acp_proxy("Use shell tool to run pwd", agent)
    print(result)
