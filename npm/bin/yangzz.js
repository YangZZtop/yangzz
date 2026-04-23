#!/usr/bin/env node

const { execFileSync, execSync } = require("child_process");
const path = require("path");
const fs = require("fs");
const os = require("os");

const EXT = os.platform() === "win32" ? ".exe" : "";
const BIN_DIR = path.join(__dirname);
const NATIVE_BIN = path.join(BIN_DIR, `yangzz${EXT}`);

function findBinary() {
  // 1. Check native binary next to this script (downloaded by install.js)
  if (fs.existsSync(NATIVE_BIN) && fs.statSync(NATIVE_BIN).size > 1000) {
    return NATIVE_BIN;
  }

  // 2. Check if yangzz is on PATH (e.g. cargo install)
  try {
    const cmd = os.platform() === "win32" ? "where yangzz" : "which yangzz";
    const result = execSync(cmd, { encoding: "utf8", timeout: 3000 }).trim().split("\n")[0];
    if (result && fs.existsSync(result)) {
      return result;
    }
  } catch {}

  // 3. Check cargo bin
  const cargoPath = path.join(os.homedir(), ".cargo", "bin", `yangzz${EXT}`);
  if (fs.existsSync(cargoPath)) {
    return cargoPath;
  }

  return null;
}

const bin = findBinary();

if (!bin) {
  console.error("yangzz: Binary not found.");
  console.error("");
  console.error("The postinstall script may not have run. Try:");
  console.error("  npm rebuild yangzz");
  console.error("  # or");
  console.error("  pnpm approve-builds -g && pnpm install -g yangzz");
  console.error("  # or install from source:");
  console.error("  cargo install yangzz");
  process.exit(1);
}

try {
  const result = execFileSync(bin, process.argv.slice(2), {
    stdio: "inherit",
    env: process.env,
  });
} catch (e) {
  process.exit(e.status || 1);
}
