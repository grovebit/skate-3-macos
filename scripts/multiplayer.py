#!/usr/bin/env python3
"""Launch a local multiplayer lobby using native development builds."""
import argparse
from contextlib import ExitStack
import os
from pathlib import Path
import secrets
import shlex
import signal
import socket
import subprocess
import sys
import tempfile
import time


REPO = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(REPO))
from tools.asset_pipeline.versions import installed

# The development build play.sh uses; its rpath finds Bevy's libraries.
GAME = REPO / os.environ.get("CARGO_TARGET_DIR", "target") / "debug/skate3rust"


def stop_players(children):
    # Each player owns a process group: the crash supervisor and the game it
    # starts. Stopping only the supervisor would leave the game running. macOS
    # reports EPERM for a group whose processes all exited but are unreaped.
    for child in children:
        try:
            os.killpg(child.pid, signal.SIGTERM)
        except (ProcessLookupError, PermissionError):
            pass
    deadline = time.monotonic() + 5
    for child in children:
        try:
            child.wait(timeout=max(0, deadline - time.monotonic()))
        except subprocess.TimeoutExpired:
            pass
        try:
            os.killpg(child.pid, signal.SIGKILL)
        except (ProcessLookupError, PermissionError):
            pass
        child.wait()


def asset_root(explicit, repo):
    if explicit:
        return Path(explicit).resolve()
    installation = installed(repo / "data")
    if installation is None:
        raise ValueError("No converted game data; run ./play.sh first")
    return installation[0] / "assets"


def player_command(index, players, assets, map_path, ports, session, two_controllers):
    command = [str(GAME), "--assets", str(assets),
               "--map", str(map_path), "--net-session", str(session)]
    if index == 0:
        command += ["--net-host", f"127.0.0.1:{ports[0]}"]
    else:
        command += ["--net-local", f"127.0.0.1:{ports[index]}",
                    f"127.0.0.1:{ports[0]}", "--spawn-offset", str(index * 2),
                    "--appearance", "test-uninstalled-outfit"]
    command += ["--player-title", f"Multiplayer {chr(65 + index)} - {players} player test"]
    if two_controllers and index < 2:
        command += ["--controller", str(index)]
    return command


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--players", type=int, choices=range(2, 11), default=2)
    parser.add_argument("--assets", default=os.environ.get("SKATE3_ASSETS"))
    parser.add_argument("--map", type=Path, help="Defaults to the installation's University map")
    parser.add_argument("--two-controllers", action="store_true")
    parser.add_argument("--dry-run", action="store_true", help="Validate paths and print commands without building or launching")
    args = parser.parse_args(argv)
    try:
        assets = asset_root(args.assets, REPO)
        if not (assets / "private/game.json").is_file():
            raise ValueError("Prepared game assets are missing; run ./play.sh first")
        map_path = (args.map or assets.parent / "maps/University.skate").resolve()
        if map_path.suffix != ".skate" or not map_path.is_file():
            raise ValueError(f"Expected an existing .skate map: {map_path}")
        if not args.dry_run:
            subprocess.run(["cargo", "build", "--locked", "-p", "skate-game",
                            "--bin", "skate3rust"], cwd=REPO, check=True)
        # Reserve distinct loopback ports together; release just before launch.
        with ExitStack() as reservations:
            ports = []
            for _ in range(args.players):
                sock = reservations.enter_context(socket.socket(socket.AF_INET, socket.SOCK_DGRAM))
                sock.bind(("127.0.0.1", 0))
                ports.append(sock.getsockname()[1])
            session = secrets.randbelow(2**63 - 1) + 1
            commands = [player_command(i, args.players, assets, map_path, ports,
                                       session, args.two_controllers) for i in range(args.players)]
        if args.dry_run:
            for command in commands:
                print(shlex.join(command))
            return 0
        log_root = REPO / "logs/multiplayer"
        log_root.mkdir(parents=True, exist_ok=True)
        logs = Path(tempfile.mkdtemp(prefix="session-", dir=log_root))
        env = dict(os.environ, SKATE3_MODS=str(REPO / "mods"))
        children = []
        previous_handlers = {sig: signal.signal(sig, signal.default_int_handler)
                             for sig in (signal.SIGTERM, signal.SIGHUP)}
        try:
            for index, command in enumerate(commands):
                label = chr(97 + index)
                with (logs / f"player-{label}.out.log").open("wb") as out, \
                        (logs / f"player-{label}.err.log").open("wb") as err:
                    children.append(subprocess.Popen(command, cwd=REPO, env=env,
                                                     stdout=out, stderr=err,
                                                     start_new_session=True))
            print(f"Started {args.players} players; player A hosts. Logs: {logs}", flush=True)
            print("Press Ctrl+C here to stop all players. Steam is not required.", flush=True)
            return int(any([child.wait() != 0 for child in children]))
        finally:
            # Finish reaping even if the terminal sends another shutdown signal.
            previous_handlers[signal.SIGINT] = signal.getsignal(signal.SIGINT)
            for sig in previous_handlers:
                signal.signal(sig, signal.SIG_IGN)
            try:
                stop_players(children)
            finally:
                for sig, handler in previous_handlers.items():
                    signal.signal(sig, handler)
    except KeyboardInterrupt:
        return 130
    except (OSError, ValueError, KeyError, TypeError, subprocess.CalledProcessError) as error:
        parser.exit(1, f"Multiplayer launch failed: {error}\n")


if __name__ == "__main__":
    raise SystemExit(main())
