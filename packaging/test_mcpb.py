import json
import os
import subprocess
import sys
import tempfile
import unittest
import zipfile

sys.path.insert(0, os.path.dirname(__file__))
import mcpb


class BundleTest(unittest.TestCase):
    def build(self, label):
        d = tempfile.mkdtemp()
        binary = os.path.join(d, "bin")
        open(binary, "wb").write(b"#!/bin/sh\necho meridian-mcp test\n")
        lic = os.path.join(d, "LICENSE")
        open(lic, "w").write("MIT")
        out = os.path.join(d, "x.mcpb")
        mcpb.build("1.2.3", label, binary, lic, out)
        return zipfile.ZipFile(out)

    def test_it_is_a_zip_with_a_manifest_at_its_root_and_the_server_under_server(self):
        z = self.build("macos-arm64")
        self.assertEqual(sorted(z.namelist()), ["LICENSE", "manifest.json", "server/meridian-mcp"])
        m = json.loads(z.read("manifest.json"))
        self.assertEqual(m["manifest_version"], "0.2")
        self.assertEqual(m["version"], "1.2.3")
        self.assertEqual(m["server"]["type"], "binary")
        self.assertEqual(m["server"]["entry_point"], "server/meridian-mcp")
        self.assertEqual(m["server"]["mcp_config"], {"command": "${__dirname}/server/meridian-mcp", "args": ["serve"]})
        self.assertEqual(m["license"], "MIT")

    def test_the_entry_point_exists_in_the_bundle_and_is_executable(self):
        for label in ("macos-arm64", "macos-x86_64", "windows-x86_64"):
            z = self.build(label)
            m = json.loads(z.read("manifest.json"))
            info = z.getinfo(m["server"]["entry_point"])
            self.assertEqual(info.external_attr >> 16 & 0o777, 0o755, label)

    def test_windows_uses_the_exe_and_each_bundle_names_only_its_own_platform(self):
        win = json.loads(self.build("windows-x86_64").read("manifest.json"))
        self.assertEqual(win["server"]["entry_point"], "server/meridian-mcp.exe")
        self.assertEqual(win["server"]["mcp_config"]["command"], "${__dirname}/server/meridian-mcp.exe")
        self.assertEqual(win["compatibility"]["platforms"], ["win32"])
        mac = json.loads(self.build("macos-x86_64").read("manifest.json"))
        self.assertEqual(mac["compatibility"]["platforms"], ["darwin"])

    def test_every_tool_the_server_offers_is_listed_and_nothing_else(self):
        src = open(os.path.join(os.path.dirname(__file__), "..", "src", "tools.rs")).read()
        import re
        offered = sorted(set(re.findall(r"async fn (meridian_[a-z_]+)\(", src)))
        listed = sorted(t["name"] for t in json.loads(self.build("macos-arm64").read("manifest.json"))["tools"])
        self.assertEqual(listed, offered)

    def test_an_unknown_platform_is_refused(self):
        with self.assertRaises(SystemExit):
            mcpb.platform_of("plan9-x86_64")


if __name__ == "__main__":
    unittest.main()
