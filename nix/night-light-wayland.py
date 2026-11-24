"""Exercise console-warm against a compositor advertising Hyprland CTM."""

import os
import socket
import struct
import subprocess
import sys
import tempfile
from pathlib import Path


def words(*values):
    return struct.pack("=" + "I" * len(values), *values)


def unpack(payload):
    return struct.unpack("=" + "I" * (len(payload) // 4), payload)


def event(stream, obj, opcode, payload):
    stream.sendall(words(obj, ((len(payload) + 8) << 16) | opcode) + payload)


def receive(stream):
    header = stream.read(8)
    assert len(header) == 8, "client disconnected before the expected request"
    obj, tag = unpack(header)
    return obj, tag & 0xFFFF, stream.read((tag >> 16) - 8)


def announce(stream, registry, name, interface, version):
    text = interface.encode("ascii") + b"\0"
    padded = text.ljust((len(text) + 3) // 4 * 4, b"\0")
    event(stream, registry, 0, words(name, len(text)) + padded + words(version))


def bind(stream, registry, name):
    obj, opcode, payload = receive(stream)
    assert (obj, opcode) == (registry, 0)
    args = unpack(payload)
    assert args[0] == name
    return args[-1]


def sync(stream, server):
    obj, opcode, payload = receive(stream)
    assert (obj, opcode) == (1, 0)
    (callback,) = unpack(payload)
    event(server, callback, 0, words(0))
    event(server, 1, 1, words(callback))


def matrix_batch(stream, manager, outputs):
    for output in outputs:
        obj, opcode, payload = receive(stream)
        assert (obj, opcode) == (manager, 0)
        target, red, rg, rb, gr, green, gb, br, bg, blue = unpack(payload)
        assert target == output
        assert (rg, rb, gr, gb, br, bg) == (0,) * 6
        assert red == 128 and 80 < green < 95 and 45 < blue < 63
    assert receive(stream) == (manager, 1, b"")


def exercise(binary, home, blocked):
    server, client = socket.socketpair()
    with server, client, server.makefile("rb") as stream:
        server.settimeout(5)
        env = dict(os.environ, HOME=str(home), WAYLAND_SOCKET=str(client.fileno()))
        args = [binary, "daemon", "--gamma", "50"]
        with subprocess.Popen(args, env=env, pass_fds=[client.fileno()], stderr=subprocess.PIPE) as daemon:
            client.close()
            try:
                obj, opcode, payload = receive(stream)
                assert (obj, opcode) == (1, 1)
                (registry,) = unpack(payload)
                announce(server, registry, 40, "hyprland_ctm_control_manager_v1", 2)
                announce(server, registry, 41, "wl_output", 3)
                sync(stream, server)
                manager = bind(stream, registry, 40)
                first = bind(stream, registry, 41)
                if blocked:
                    event(server, manager, 0, b"")
                sync(stream, server)
                if blocked:
                    _, stderr = daemon.communicate(timeout=5)
                    assert daemon.returncode != 0 and b"another client controls" in stderr
                    return
                matrix_batch(stream, manager, [first])
                sync(stream, server)

                announce(server, registry, 42, "wl_output", 3)
                second = bind(stream, registry, 42)
                matrix_batch(stream, manager, [first, second])
                event(server, registry, 1, words(41))
                assert receive(stream) == (first, 0, b"")
                matrix_batch(stream, manager, [second])
            finally:
                if daemon.poll() is None:
                    daemon.terminate()
                    daemon.wait(timeout=5)


with tempfile.TemporaryDirectory() as scratch:
    binary = sys.argv[1]
    home = Path(scratch)
    setting = home / ".config/console/warm"
    setting.parent.mkdir(parents=True)
    setting.write_text("ordinary\n", encoding="utf-8")
    subprocess.run([binary, "daemon"], check=True, env=dict(os.environ, HOME=scratch))
    invalid = subprocess.run(
        [binary, "daemon", "--temperature", "bad"],
        capture_output=True,
        env=dict(os.environ, HOME=scratch),
    )
    assert invalid.returncode != 0 and b"--temperature expects" in invalid.stderr
    setting.write_text("always\n", encoding="utf-8")
    exercise(binary, home, blocked=False)
    exercise(binary, home, blocked=True)
