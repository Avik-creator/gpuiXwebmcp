#!/usr/bin/env python3
"""Print protocol JSON from the debugger WebSocket.

Sets Origin to the unpacked extension id so the allowlist accepts this CLI.
"""

from __future__ import annotations

import base64
import hashlib
import json
import os
import socket
import struct
import sys
import uuid

HOST = "127.0.0.1"
PORT = 17321
ORIGIN = "chrome-extension://ffaihbpimepkgggjclheahfddigmmfeg"


def _mask(payload: bytes) -> tuple[bytes, bytes]:
    key = os.urandom(4)
    masked = bytes(b ^ key[i % 4] for i, b in enumerate(payload))
    return key, masked


def send_text(sock: socket.socket, text: str) -> None:
    payload = text.encode("utf-8")
    key, masked = _mask(payload)
    header = bytearray([0x81])
    length = len(payload)
    if length < 126:
        header.append(0x80 | length)
    elif length < 65536:
        header.append(0x80 | 126)
        header.extend(struct.pack("!H", length))
    else:
        header.append(0x80 | 127)
        header.extend(struct.pack("!Q", length))
    sock.sendall(bytes(header) + key + masked)


def recv_exact(sock: socket.socket, size: int) -> bytes:
    chunks = bytearray()
    while len(chunks) < size:
        piece = sock.recv(size - len(chunks))
        if not piece:
            raise ConnectionError("socket closed")
        chunks.extend(piece)
    return bytes(chunks)


def recv_message(sock: socket.socket) -> str | None:
    header = recv_exact(sock, 2)
    opcode = header[0] & 0x0F
    masked = header[1] & 0x80
    length = header[1] & 0x7F
    if length == 126:
        length = struct.unpack("!H", recv_exact(sock, 2))[0]
    elif length == 127:
        length = struct.unpack("!Q", recv_exact(sock, 8))[0]
    mask_key = recv_exact(sock, 4) if masked else b""
    payload = recv_exact(sock, length)
    if masked:
        payload = bytes(b ^ mask_key[i % 4] for i, b in enumerate(payload))
    if opcode == 0x8:
        return None
    if opcode == 0x9:
        # ping -> pong
        key, masked_payload = _mask(payload)
        sock.sendall(bytes([0x8A, 0x80 | len(payload)]) + key + masked_payload)
        return recv_message(sock)
    if opcode != 0x1:
        return recv_message(sock)
    return payload.decode("utf-8")


def handshake(sock: socket.socket) -> None:
    key = base64.b64encode(uuid.uuid4().bytes).decode("ascii")
    request = (
        "GET / HTTP/1.1\r\n"
        f"Host: {HOST}:{PORT}\r\n"
        "Upgrade: websocket\r\n"
        "Connection: Upgrade\r\n"
        f"Sec-WebSocket-Key: {key}\r\n"
        "Sec-WebSocket-Version: 13\r\n"
        f"Origin: {ORIGIN}\r\n"
        "\r\n"
    )
    sock.sendall(request.encode("ascii"))
    response = b""
    while b"\r\n\r\n" not in response:
        piece = sock.recv(4096)
        if not piece:
            raise ConnectionError("no handshake response")
        response += piece
    status = response.split(b"\r\n", 1)[0]
    if b"101" not in status:
        raise ConnectionError(status.decode("ascii", "replace"))
    accept = base64.b64encode(
        hashlib.sha1((key + "258EAFA5-E914-47DA-95CA-C5AB0DC85B11").encode("ascii")).digest()
    )
    if accept not in response:
        raise ConnectionError("bad Sec-WebSocket-Accept")


def main() -> int:
    sock = socket.create_connection((HOST, PORT), timeout=5)
    sock.settimeout(None)
    handshake(sock)
    print(f"connected to ws://{HOST}:{PORT}", file=sys.stderr)
    try:
        while True:
            message = recv_message(sock)
            if message is None:
                break
            try:
                parsed = json.loads(message)
                print(json.dumps(parsed, indent=2))
            except json.JSONDecodeError:
                print(message)
            sys.stdout.flush()
    except KeyboardInterrupt:
        return 0
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
