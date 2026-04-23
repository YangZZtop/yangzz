#!/usr/bin/env node

const { execSync } = require("child_process");
const fs = require("fs");
const path = require("path");
const os = require("os");
const https = require("https");

const PACKAGE_VERSION = require("./package.json").version;
const BIN_DIR = path.join(__dirname, "bin");
const EXT = os.platform() === "win32" ? ".exe" : "";
const BIN_PATH = path.join(BIN_DIR, `yangzz${EXT}`);

// Platform mapping for GitHub releases
const PLATFORM_MAP = {
  "darwin-arm64": "aarch64-apple-darwin",
  "darwin-x64": "x86_64-apple-darwin",
  "linux-arm64": "aarch64-unknown-linux-gnu",
  "linux-x64": "x86_64-unknown-linux-gnu",
  "win32-x64": "x86_64-pc-windows-msvc",
};

function getTarget() {
  const key = `${os.platform()}-${os.arch()}`;
  return PLATFORM_MAP[key] || null;
}

// Strategy 1: Download pre-built binary from GitHub Releases
function tryDownload(target) {
  const url = `https://github.com/YangZZtop/yangzz/releases/download/v${PACKAGE_VERSION}/yangzz-${target}${EXT}`;
  console.log(`Downloading yangzz v${PACKAGE_VERSION} for ${target}...`);

  try {
    // Use curl/wget for simplicity and redirect following
    if (os.platform() !== "win32") {
      execSync(`curl -fsSL "${url}" -o "${BIN_PATH}" && chmod +x "${BIN_PATH}"`, {
        stdio: "inherit",
        timeout: 60000,
      });
    } else {
      execSync(`powershell -Command "Invoke-WebRequest -Uri '${url}' -OutFile '${BIN_PATH}'"`, {
        stdio: "inherit",
        timeout: 60000,
      });
    }
    return fs.existsSync(BIN_PATH) && fs.statSync(BIN_PATH).size > 1000;
  } catch {
    return false;
  }
}

// Strategy 2: Check if `yangzz` is already on PATH (e.g. cargo install)
function tryExistingBinary() {
  try {
    const result = execSync("which yangzz 2>/dev/null || where yangzz 2>nul", {
      encoding: "utf8",
      timeout: 5000,
    }).trim();
    if (result) {
      console.log(`Found existing yangzz at: ${result}`);
      // Create a symlink/wrapper to existing binary
      const wrapper = os.platform() === "win32"
        ? `@echo off\n"${result}" %*\n`
        : `#!/bin/sh\nexec "${result}" "$@"\n`;
      fs.writeFileSync(BIN_PATH, wrapper);
      if (os.platform() !== "win32") {
        fs.chmodSync(BIN_PATH, 0o755);
      }
      return true;
    }
  } catch {}
  return false;
}

// Strategy 3: Build from source with cargo
function tryCargoBuild() {
  try {
    execSync("cargo --version", { stdio: "ignore", timeout: 5000 });
  } catch {
    return false;
  }

  console.log("Building yangzz from source (requires Rust toolchain)...");
  try {
    execSync("cargo install yangzz --root .", {
      stdio: "inherit",
      timeout: 300000, // 5 minutes
      cwd: __dirname,
    });
    // cargo install puts binary in ./bin/yangzz
    return fs.existsSync(BIN_PATH) && fs.statSync(BIN_PATH).size > 1000;
  } catch {
    return false;
  }
}

function main() {
  // If binary already exists and is valid, skip
  if (fs.existsSync(BIN_PATH) && fs.statSync(BIN_PATH).size > 1000) {
    console.log("yangzz binary already exists, skipping install.");
    return;
  }

  fs.mkdirSync(BIN_DIR, { recursive: true });

  const target = getTarget();

  // Try download first
  if (target && tryDownload(target)) {
    console.log("✓ yangzz installed successfully (pre-built binary)");
    return;
  }

  // Try existing binary on PATH
  if (tryExistingBinary()) {
    console.log("✓ yangzz linked to existing installation");
    return;
  }

  // Try cargo build
  if (tryCargoBuild()) {
    console.log("✓ yangzz installed successfully (built from source)");
    return;
  }

  // All strategies failed — create helpful stub
  console.log();
  console.log("⚠ Could not install yangzz binary automatically.");
  console.log("  Please install manually:");
  console.log();
  console.log("  Option 1 (推荐): cargo install yangzz");
  console.log("  Option 2: Download from https://github.com/YangZZtop/yangzz/releases");
  console.log();

  const stub = os.platform() === "win32"
    ? `@echo off\necho yangzz: Binary not found. Run: cargo install yangzz\n`
    : `#!/bin/sh\necho "yangzz: Binary not found. Run: cargo install yangzz"\nexit 1\n`;

  fs.writeFileSync(BIN_PATH, stub);
  if (os.platform() !== "win32") {
    fs.chmodSync(BIN_PATH, 0o755);
  }
}

main();
