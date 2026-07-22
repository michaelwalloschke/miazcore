#!/usr/bin/env python3
"""One-time build-12340 protocol probe used to generate the pdump fixture.

This is deliberately infrastructure tooling, not the Learning Client. It uses
only Python's standard library and implements the packet facts recorded in the
Wayfinder protocol trace. It never prints credentials, SRP material, or session
keys.
"""

from __future__ import annotations

import hashlib
import hmac
import os
import socket
import struct
import sys
from dataclasses import dataclass
from pathlib import Path


AUTH_HOST = "127.0.0.1"
AUTH_PORT = 3724
BUILD = 12340
CHARACTER_NAME = "Miaztest"

SMSG_AUTH_CHALLENGE = 0x1EC
CMSG_AUTH_SESSION = 0x1ED
SMSG_AUTH_RESPONSE = 0x1EE
CMSG_CHAR_CREATE = 0x036
SMSG_CHAR_CREATE = 0x03A
CMSG_CHAR_ENUM = 0x037
SMSG_CHAR_ENUM = 0x03B

CLIENT_DIRECTION_KEY = bytes.fromhex("c2b3723cc6aed9b5343c53ee2f4367ce")
SERVER_DIRECTION_KEY = bytes.fromhex("cc98ae04e897eaca12ddc09342915357")


def fail(message: str) -> "None":
    raise RuntimeError(message)


def recv_exact(sock: socket.socket, size: int) -> bytes:
    data = bytearray()
    while len(data) < size:
        chunk = sock.recv(size - len(data))
        if not chunk:
            fail(f"connection closed with {size - len(data)} bytes outstanding")
        data.extend(chunk)
    return bytes(data)


def sha1(*parts: bytes) -> bytes:
    digest = hashlib.sha1()
    for part in parts:
        digest.update(part)
    return digest.digest()


def cstring(buffer: bytes, offset: int) -> tuple[str, int]:
    end = buffer.index(0, offset)
    return buffer[offset:end].decode("ascii"), end + 1


class Rc4:
    def __init__(self, key: bytes) -> None:
        state = list(range(256))
        j = 0
        for i in range(256):
            j = (j + state[i] + key[i % len(key)]) & 0xFF
            state[i], state[j] = state[j], state[i]
        self.state = state
        self.i = 0
        self.j = 0
        self.crypt(bytes(1024))

    def crypt(self, data: bytes) -> bytes:
        output = bytearray(len(data))
        for index, value in enumerate(data):
            self.i = (self.i + 1) & 0xFF
            self.j = (self.j + self.state[self.i]) & 0xFF
            self.state[self.i], self.state[self.j] = self.state[self.j], self.state[self.i]
            key_byte = self.state[(self.state[self.i] + self.state[self.j]) & 0xFF]
            output[index] = value ^ key_byte
        return bytes(output)


@dataclass
class Session:
    account: str
    session_key: bytes
    realm_id: int
    realm_address: str


def wow_interleave(shared_secret: int) -> bytes:
    secret = shared_secret.to_bytes(32, "little")
    lead = 0
    while lead < len(secret) and secret[lead] == 0:
        lead += 1
    if lead % 2:
        lead += 1
    secret = secret[lead:]
    even = sha1(secret[0::2])
    odd = sha1(secret[1::2])
    return b"".join(bytes((a, b)) for a, b in zip(even, odd))


def login(account: str, password: str) -> Session:
    account = account.upper()
    password = password.upper()
    account_bytes = account.encode("ascii")
    challenge_tail = b"".join(
        (
            struct.pack("<I", 0x576F57),
            struct.pack("<BBBH", 3, 3, 5, BUILD),
            struct.pack("<I", 0x783836),  # x86
            struct.pack("<I", 0x4F5358),  # OSX
            struct.pack("<I", 0x656E5553),  # enUS
            struct.pack("<i", 0),
            socket.inet_aton("127.0.0.1"),
            struct.pack("B", len(account_bytes)),
            account_bytes,
        )
    )
    challenge = b"\x00\x08" + struct.pack("<H", len(challenge_tail)) + challenge_tail

    with socket.create_connection((AUTH_HOST, AUTH_PORT), timeout=10) as auth:
        auth.settimeout(10)
        auth.sendall(challenge)
        header = recv_exact(auth, 3)
        if header[0] != 0 or header[2] != 0:
            fail(f"login challenge rejected with result 0x{header[2]:02x}")
        server_public = recv_exact(auth, 32)
        generator_length = recv_exact(auth, 1)[0]
        generator_bytes = recv_exact(auth, generator_length)
        prime_length = recv_exact(auth, 1)[0]
        prime_bytes = recv_exact(auth, prime_length)
        salt = recv_exact(auth, 32)
        recv_exact(auth, 16)  # CRC salt; executable proof is disabled on this realm.
        security_flags = recv_exact(auth, 1)[0]
        if security_flags:
            fail(f"unsupported account security flags 0x{security_flags:02x}")

        generator = int.from_bytes(generator_bytes, "little")
        prime = int.from_bytes(prime_bytes, "little")
        server_public_int = int.from_bytes(server_public, "little")
        if not server_public_int or server_public_int % prime == 0:
            fail("invalid SRP6 server public key")

        private = int.from_bytes(os.urandom(32), "little")
        client_public_int = pow(generator, private, prime)
        client_public = client_public_int.to_bytes(32, "little")
        x = int.from_bytes(sha1(salt, sha1(account_bytes, b":", password.encode("ascii"))), "little")
        scrambling = int.from_bytes(sha1(client_public, server_public), "little")
        base = (server_public_int - 3 * pow(generator, x, prime)) % prime
        shared_secret = pow(base, private + scrambling * x, prime)
        session_key = wow_interleave(shared_secret)
        xor_hash = bytes(a ^ b for a, b in zip(sha1(prime_bytes), sha1(generator_bytes)))
        client_proof = sha1(
            xor_hash,
            sha1(account_bytes),
            salt,
            client_public,
            server_public,
            session_key,
        )
        expected_server_proof = sha1(client_public, client_proof, session_key)
        auth.sendall(
            b"\x01"
            + client_public
            + client_proof
            + bytes(20)  # controlled realm permits a zero CRC proof
            + b"\x00"  # telemetry key count
            + b"\x00"  # no security-token value
        )
        proof_header = recv_exact(auth, 2)
        if proof_header != b"\x01\x00":
            fail(f"login proof rejected with result 0x{proof_header[1]:02x}")
        server_proof = recv_exact(auth, 20)
        recv_exact(auth, 10)  # account flags, hardware survey id, trailing unknown
        if not hmac.compare_digest(server_proof, expected_server_proof):
            fail("login server proof mismatch")

        auth.sendall(b"\x10" + bytes(4))
        realm_header = recv_exact(auth, 3)
        if realm_header[0] != 0x10:
            fail(f"expected realm-list opcode, got 0x{realm_header[0]:02x}")
        realm_payload = recv_exact(auth, struct.unpack("<H", realm_header[1:])[0])

    offset = 4
    realm_count = struct.unpack_from("<H", realm_payload, offset)[0]
    offset += 2
    for _ in range(realm_count):
        _realm_type, locked, flags = struct.unpack_from("BBB", realm_payload, offset)
        offset += 3
        name, offset = cstring(realm_payload, offset)
        address, offset = cstring(realm_payload, offset)
        _population = struct.unpack_from("<f", realm_payload, offset)[0]
        offset += 4
        _characters, _timezone, realm_id = struct.unpack_from("BBB", realm_payload, offset)
        offset += 3
        build = None
        if flags & 0x04:
            _major, _minor, _patch, build = struct.unpack_from("BBBH", realm_payload, offset)
            offset += 5
        if name == "Miazcore Reference Realm":
            if locked:
                fail("Reference Realm is locked")
            if build not in (None, BUILD):
                fail(f"Reference Realm advertises unexpected build {build}")
            return Session(account, session_key, realm_id, address)
    fail("Miazcore Reference Realm was not present in the authenticated realm list")


class WorldConnection:
    def __init__(self, sock: socket.socket, session_key: bytes) -> None:
        self.sock = sock
        client_key = hmac.new(CLIENT_DIRECTION_KEY, session_key, hashlib.sha1).digest()
        server_key = hmac.new(SERVER_DIRECTION_KEY, session_key, hashlib.sha1).digest()
        self.encryptor = Rc4(client_key)
        self.decryptor = Rc4(server_key)
        self.encrypted = False

    def receive(self) -> tuple[int, bytes]:
        first = recv_exact(self.sock, 1)
        if self.encrypted:
            first = self.decryptor.crypt(first)
        large = bool(first[0] & 0x80)
        rest = recv_exact(self.sock, 4 if large else 3)
        if self.encrypted:
            rest = self.decryptor.crypt(rest)
        header = first + rest
        if large:
            size = ((header[0] & 0x7F) << 16) | (header[1] << 8) | header[2]
            opcode = struct.unpack_from("<H", header, 3)[0]
        else:
            size = struct.unpack(">H", header[:2])[0]
            opcode = struct.unpack_from("<H", header, 2)[0]
        return opcode, recv_exact(self.sock, size - 2)

    def send(self, opcode: int, payload: bytes = b"") -> None:
        header = struct.pack(">H", len(payload) + 4) + struct.pack("<I", opcode)
        if self.encrypted:
            header = self.encryptor.crypt(header)
        self.sock.sendall(header + payload)


def verify_world_character(session: Session, create: bool) -> None:
    host, port_text = session.realm_address.rsplit(":", 1)
    with socket.create_connection((host, int(port_text)), timeout=10) as sock:
        sock.settimeout(20)
        world = WorldConnection(sock, session.session_key)
        opcode, challenge = world.receive()
        if opcode != SMSG_AUTH_CHALLENGE or len(challenge) != 40:
            fail(f"unexpected world challenge opcode/size: 0x{opcode:03x}/{len(challenge)}")
        server_seed = struct.unpack_from("<I", challenge, 4)[0]
        client_seed = int.from_bytes(os.urandom(4), "little")
        account_bytes = session.account.encode("ascii")
        world_proof = sha1(
            account_bytes,
            struct.pack("<I", 0),
            struct.pack("<I", client_seed),
            struct.pack("<I", server_seed),
            session.session_key,
        )
        auth_session = b"".join(
            (
                struct.pack("<II", BUILD, 0),
                account_bytes + b"\x00",
                struct.pack("<IIII", 0, client_seed, 0, 0),
                struct.pack("<I", session.realm_id),
                struct.pack("<Q", 0),
                world_proof,
                bytes(4),
            )
        )
        world.send(CMSG_AUTH_SESSION, auth_session)
        world.encrypted = True
        while True:
            opcode, payload = world.receive()
            if opcode == SMSG_AUTH_RESPONSE:
                if not payload or payload[0] != 0x0C:
                    fail(f"world authentication rejected with result {payload[0] if payload else 'empty'}")
                break

        if create:
            character_payload = CHARACTER_NAME.encode("ascii") + b"\x00" + bytes(
                (1, 1, 0, 0, 0, 0, 0, 0, 0)
            )
            world.send(CMSG_CHAR_CREATE, character_payload)
            while True:
                opcode, payload = world.receive()
                if opcode == SMSG_CHAR_CREATE:
                    if payload != b"\x2f":
                        fail(f"character creation failed with response {payload.hex()}")
                    print("protocol smoke: server generated Miaztest (response=0x2f)")
                    break

        world.send(CMSG_CHAR_ENUM)
        while True:
            opcode, payload = world.receive()
            if opcode == SMSG_CHAR_ENUM:
                if not payload or payload[0] != 1:
                    fail(f"expected exactly one fixture character, got {payload[0] if payload else 'empty'}")
                if CHARACTER_NAME.encode("ascii") + b"\x00" not in payload:
                    fail("character enumeration did not contain Miaztest")
                print("protocol smoke: authenticated world session enumerated exactly one Miaztest")
                return


def main() -> int:
    args = set(sys.argv[1:])
    if args - {"--create"}:
        fail("usage: bootstrap_character.py [--create]")
    base = Path(__file__).resolve().parent.parent
    account = (base / "secrets/fixture-account").read_text(encoding="ascii").strip()
    password = (base / "secrets/fixture-password").read_text(encoding="ascii").strip()
    if not (account and password):
        fail("fixture credential files are empty")
    session = login(account, password)
    print(f"protocol smoke: authenticated build {BUILD}; realm {session.realm_id} at {session.realm_address}")
    verify_world_character(session, create="--create" in args)
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, ValueError, RuntimeError, struct.error) as error:
        print(f"protocol smoke failed: {error}", file=sys.stderr)
        raise SystemExit(1)
