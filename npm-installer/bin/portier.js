#!/usr/bin/env node
"use strict";

// Launcher shim: forwards args to the platform binary that postinstall
// downloaded into ../.bin. (The `bin` entry must NOT point at install.js,
// or invoking `portier` would re-run the installer instead of the tool.)

const path = require("path");
const fs = require("fs");
const { spawnSync } = require("child_process");

const bin = path.join(
  __dirname,
  "..",
  ".bin",
  "portier" + (process.platform === "win32" ? ".exe" : "")
);

if (!fs.existsSync(bin)) {
  console.error(
    "portier binary not found. Reinstall it with: npm rebuild @portier/cli"
  );
  process.exit(1);
}

const res = spawnSync(bin, process.argv.slice(2), { stdio: "inherit" });
process.exit(res.status === null ? 1 : res.status);
