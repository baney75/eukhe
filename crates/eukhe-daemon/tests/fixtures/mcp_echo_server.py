#!/usr/bin/env python3
"""Minimal stdio MCP server used by the daemon MCP e2e tests.

Speaks newline-delimited JSON-RPC 2.0 over stdin/stdout (the MCP stdio
framing) with the three requests the generic kernel registry needs:
`initialize`, `tools/list`, and `tools/call`. Pure stdlib: the settings
entry points any Python 3 interpreter at this script, no venv required.

`echo` is declared read-only (`readOnlyHint: true`). With `--stamp <path>`
the server also lists `stamp`, which declares nothing (an effect of
unknown kind) and appends its message as one line to `<path>`, so a test
can count how often it ran.
"""

import json
import sys

STAMP_PATH = sys.argv[sys.argv.index("--stamp") + 1] if "--stamp" in sys.argv else None

ECHO_TOOL_SCHEMA = {
    "type": "object",
    "properties": {
        "message": {"type": "string", "description": "Text echoed back."},
    },
    "required": ["message"],
}


def initialize_result(requested_version):
    return {
        "protocolVersion": requested_version or "2024-11-05",
        "capabilities": {"tools": {"listChanged": False}},
        "serverInfo": {"name": "fixture-echo", "version": "1.0.0"},
    }


def echo_result(arguments):
    message = arguments.get("message", "")
    return {
        "content": [{"type": "text", "text": str(message)}],
        "isError": False,
    }


def stamp_result(arguments):
    message = str(arguments.get("message", ""))
    with open(STAMP_PATH, "a", encoding="utf-8") as stamps:
        stamps.write(message + "\n")
    return {
        "content": [{"type": "text", "text": f"stamped {message}"}],
        "isError": False,
    }


def tools():
    listed = [
        {
            "name": "echo",
            "description": "Echoes the message argument back.",
            "inputSchema": ECHO_TOOL_SCHEMA,
            "annotations": {"readOnlyHint": True},
        }
    ]
    if STAMP_PATH is not None:
        listed.append(
            {
                "name": "stamp",
                "description": "Appends the message argument to the stamp file.",
                "inputSchema": ECHO_TOOL_SCHEMA,
            }
        )
    return listed


def respond(request_id, result):
    sys.stdout.write(
        json.dumps({"jsonrpc": "2.0", "id": request_id, "result": result}) + "\n"
    )
    sys.stdout.flush()


def respond_error(request_id, code, message):
    sys.stdout.write(
        json.dumps(
            {
                "jsonrpc": "2.0",
                "id": request_id,
                "error": {"code": code, "message": message},
            }
        )
        + "\n"
    )
    sys.stdout.flush()


def main():
    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue
        try:
            message = json.loads(line)
        except ValueError:
            continue
        request_id = message.get("id")
        if request_id is None:
            # Notifications (e.g. `notifications/initialized`) need no reply.
            continue
        method = message.get("method")
        params = message.get("params") or {}
        if method == "initialize":
            respond(request_id, initialize_result(params.get("protocolVersion")))
        elif method == "tools/list":
            respond(request_id, {"tools": tools()})
        elif method == "tools/call":
            arguments = params.get("arguments") or {}
            if params.get("name") == "stamp" and STAMP_PATH is not None:
                respond(request_id, stamp_result(arguments))
            else:
                respond(request_id, echo_result(arguments))
        else:
            respond_error(request_id, -32601, f"Method not found: {method}")


if __name__ == "__main__":
    main()
