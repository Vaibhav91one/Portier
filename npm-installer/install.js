#!/usr/bin/env node

"use strict";

const https = require("https");
const fs = require("fs");
const path = require("path");
const zlib = require("zlib");
const { spawnSync } = require("child_process");

const VERSION = "0.1.0";
const REPO = "vaibhavtomar/portier";

const PLATFORM_MAP = {
  "darwin-arm64": "aarch64-apple-darwin",
  "darwin-x64": "x86_64-apple-darwin",
  "linux-x64": "x86_64-unknown-linux-gnu",
  "win32-x64": "x86_64-pc-windows-msvc",
};

function die(msg) {
  console.error("error: " + msg);
  process.exit(1);
}

function platformKey() {
  return process.platform + "-" + process.arch;
}

function targetTriple() {
  const key = platformKey();
  const triple = PLATFORM_MAP[key];
  if (!triple) {
    die(
      "unsupported platform " + key + " — expected one of " +
        Object.keys(PLATFORM_MAP).join(", ")
    );
  }
  return triple;
}

function archiveExt() {
  return process.platform === "win32" ? "zip" : "tar.gz";
}

function downloadUrl() {
  var triple = targetTriple();
  var ext = archiveExt();
  return (
    "https://github.com/" +
    REPO +
    "/releases/download/v" +
    VERSION +
    "/portier-" +
    triple +
    "." +
    ext
  );
}

function download(url) {
  return new Promise(function (resolve, reject) {
    https
      .get(url, function (res) {
        if (res.statusCode >= 300 && res.statusCode < 400 && res.headers.location) {
          download(res.headers.location).then(resolve, reject);
          return;
        }
        if (res.statusCode !== 200) {
          reject(
            new Error(
              "download failed with status " +
                res.statusCode +
                " (expected 200)"
            )
          );
          return;
        }
        var chunks = [];
        res.on("data", function (c) {
          chunks.push(c);
        });
        res.on("end", function () {
          resolve(Buffer.concat(chunks));
        });
      })
      .on("error", reject);
  });
}

function extract(data, destDir) {
  if (process.platform === "win32") {
    // Write zip to temp file and extract
    var zipPath = path.join(destDir, "portier.zip");
    fs.writeFileSync(zipPath, data);
    var result = spawnSync("tar", ["-xf", zipPath, "-C", destDir], {
      stdio: "pipe",
    });
    if (result.error) {
      // fallback: try powershell Expand-Archive
      var ps = spawnSync(
        "powershell",
        [
          "-NoProfile",
          "-Command",
          "Expand-Archive -Path '" +
            zipPath +
            "' -DestinationPath '" +
            destDir +
            "' -Force",
        ],
        { stdio: "pipe" }
      );
      if (ps.error) {
        die(
          "failed to extract zip: " +
            (ps.error.message || "unknown error") +
            ". try manually extracting portier.exe from " +
            zipPath
        );
      }
    }
    fs.unlinkSync(zipPath);
  } else {
    var gunzip = zlib.createGunzip();
    var tar = spawnSync("tar", ["xz", "-C", destDir], {
      input: data,
      stdio: ["pipe", "inherit", "pipe"],
    });
    if (tar.error || tar.status !== 0) {
      die(
        "failed to extract tarball: " +
          (tar.error ? tar.error.message : "exit code " + tar.status)
      );
    }
  }
}

function install() {
  var binDir = path.resolve(__dirname, ".bin");
  if (!fs.existsSync(binDir)) {
    fs.mkdirSync(binDir, { recursive: true });
  }

  var url = downloadUrl();
  console.log("Downloading portier v" + VERSION + " for " + targetTriple() + " ...");

  download(url)
    .then(function (data) {
      console.log("Extracting ...");
      extract(data, binDir);
      var binPath = path.join(
        binDir,
        "portier" + (process.platform === "win32" ? ".exe" : "")
      );
      if (process.platform !== "win32") {
        fs.chmodSync(binPath, 0o755);
      }
      console.log("Installed portier v" + VERSION + " to " + binPath);
    })
    .catch(function (err) {
      die(
        "failed to install portier: " +
          err.message +
          "\n\n" +
          "Ensure the release exists at:\n  " +
          url +
          "\n\n" +
          "You can also install manually from:\n  https://github.com/" +
          REPO +
          "/releases"
      );
    });
}

install();
