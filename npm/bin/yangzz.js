#!/usr/bin/env node

const { execFileSync, execSync } = require("child_process");
const path = require("path");
const fs = require("fs");
const os = require("os");

const IS_WIN = os.platform() === "win32";
const EXT = IS_WIN ? ".exe" : "";
const BIN_DIR = path.join(__dirname);
const NATIVE_BIN = path.join(BIN_DIR, `yangzz${EXT}`);
const PATH_FILE = path.join(BIN_DIR, ".yangzz-path");

// Validate binary is a real executable (not a text stub)
function isRealBinary(filePath) {
  try {
    const stat = fs.statSync(filePath);
    if (stat.size < 50000) return false;
    const fd = fs.openSync(filePath, 'r');
    const buf = Buffer.alloc(2);
    fs.readSync(fd, buf, 0, 2, 0);
    fs.closeSync(fd);
    if (IS_WIN) return buf[0] === 0x4D && buf[1] === 0x5A; // MZ
    return true;
  } catch { return false; }
}

function findBinary() {
  // 1. Check native binary next to this script (downloaded by install.js)
  if (fs.existsSync(NATIVE_BIN) && isRealBinary(NATIVE_BIN)) {
    return NATIVE_BIN;
  }

  // 2. Check .yangzz-path file (written by install.js on Windows)
  if (fs.existsSync(PATH_FILE)) {
    const recorded = fs.readFileSync(PATH_FILE, "utf8").trim();
    if (recorded && fs.existsSync(recorded)) {
      return recorded;
    }
  }

  // 3. Check if yangzz is on PATH (e.g. cargo install)
  try {
    const cmd = IS_WIN ? "where yangzz 2>nul" : "which yangzz 2>/dev/null";
    const result = execSync(cmd, { encoding: "utf8", timeout: 3000 }).trim().split("\n")[0].trim();
    // Avoid finding ourselves (this wrapper script)
    if (result && fs.existsSync(result) && path.resolve(result) !== path.resolve(process.argv[1])) {
      return result;
    }
  } catch {}

  // 4. Check cargo bin
  const cargoPath = path.join(os.homedir(), ".cargo", "bin", `yangzz${EXT}`);
  if (fs.existsSync(cargoPath) && isRealBinary(cargoPath)) {
    return cargoPath;
  }

  return null;
}

const bin = findBinary();

if (!bin) {
  console.error("yangzz: Binary not found.");
  console.error("");
  if (IS_WIN) {
    console.error("Install options:");
    console.error("  1. Download from: https://github.com/YangZZtop/yangzz/releases");
    console.error("     (yangzz-x86_64-pc-windows-msvc.exe)");
    console.error(`     Put it at: ${NATIVE_BIN}`);
    console.error("");
    console.error("  2. cargo install yangzz");
    console.error("");
    console.error("  3. npm rebuild yangzz  (retry postinstall download)");
    console.error("");
    console.error("If you see EBUSY errors, close all terminals first.");
  } else {
    console.error("The postinstall script may not have run. Try:");
    console.error("  npm rebuild yangzz");
    console.error("  # or install from source:");
    console.error("  cargo install yangzz");
  }
  process.exit(1);
}

try {
  execFileSync(bin, process.argv.slice(2), {
    stdio: "inherit",
    env: process.env,
  });
} catch (e) {
  process.exit(e.status || 1);
}
