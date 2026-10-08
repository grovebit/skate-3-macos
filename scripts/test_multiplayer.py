import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

from multiplayer import asset_root, player_command, stop_players


class MultiplayerLauncherTests(unittest.TestCase):
    @unittest.skipUnless(sys.platform == "darwin", "macOS process group lifecycle")
    def test_shutdown_reaps_all_player_groups(self):
        children = []
        try:
            for _ in range(2):
                children.append(subprocess.Popen(
                    [sys.executable, "-c", "import time; time.sleep(60)"],
                    start_new_session=True))
            stop_players(children)
            for child in children:
                self.assertIsNotNone(child.poll())
                with self.assertRaises(ProcessLookupError):
                    os.killpg(child.pid, 0)
        finally:
            stop_players(children)

    def test_shutdown_tolerates_players_that_already_exited(self):
        child = subprocess.Popen([sys.executable, "-c", "pass"], start_new_session=True)
        os.waitid(os.P_PID, child.pid, os.WEXITED | os.WNOWAIT)  # exited, not yet reaped
        stop_players([child])
        self.assertEqual(child.returncode, 0)

    def test_players_share_session_and_host_but_have_distinct_ports(self):
        ports = list(range(40000, 40010))
        for index in range(10):
            command = player_command(index, 10, Path("/data with spaces/assets"),
                                     Path("/maps/University.skate"), ports, 123, True)
            self.assertEqual(command[command.index("--assets") + 1], "/data with spaces/assets")
            self.assertEqual(command[command.index("--net-session") + 1], "123")
            if index == 0:
                self.assertEqual(command[command.index("--net-host") + 1], "127.0.0.1:40000")
            else:
                start = command.index("--net-local") + 1
                self.assertEqual(command[start:start + 2],
                                 [f"127.0.0.1:{ports[index]}", "127.0.0.1:40000"])
            if index < 2:
                self.assertEqual(command[command.index("--controller") + 1], str(index))
            else:
                self.assertNotIn("--controller", command)

    def test_assets_come_from_the_prepared_installation(self):
        with tempfile.TemporaryDirectory() as temp:
            repo = Path(temp)
            with self.assertRaises(ValueError):
                asset_root(None, repo)
            installation = repo / "data/installations" / ("a" * 32)
            installation.mkdir(parents=True)
            (repo / "data/installation.json").write_text(
                json.dumps({"version": 1, "directory": "installations/" + "a" * 32}))
            self.assertEqual(asset_root(None, repo), installation.resolve() / "assets")


if __name__ == "__main__":
    unittest.main()
