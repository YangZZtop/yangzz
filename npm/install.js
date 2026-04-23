#!/usr/bin/env node

const { execSync } = require("child_process");
const fs = require("fs");
const path = require("path");
const os = require("os");
const https = require("https");

const PACKAGE_VERSION = require("./package.json").version;
const BIN_DIR = path.join(__dirname, "bin");
const IS_WIN = os.platform() === "win32";
const EXT = IS_WIN ? ".exe" : "";
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

// Validate that a file looks like a real executable (not HTML error page)
function isValidBinary(filePath) {
  try {
    const fd = fs.openSync(filePath, 'r');
    const buf = Buffer.alloc(4);
    fs.readSync(fd, buf, 0, 4, 0);
    fs.closeSync(fd);
    const size = fs.statSync(filePath).size;
    if (size < 50000) return false; // too small, probably error page
    if (IS_WIN) {
      // PE executable starts with 'MZ'
      return buf[0] === 0x4D && buf[1] === 0x5A;
    } else {
      // ELF starts with 0x7F 'ELF', Mach-O starts with 0xCF 0xFA
      return (buf[0] === 0x7F && buf[1] === 0x45) || // ELF
             (buf[0] === 0xCF && buf[1] === 0xFA) || // Mach-O 64
             (buf[0] === 0xCA && buf[1] === 0xFE);   // Mach-O universal
    }
  } catch { return false; }
}

// Strategy 1: Download pre-built binary from GitHub Releases
function tryDownload(target) {
  const url = `https://github.com/YangZZtop/yangzz/releases/download/v${PACKAGE_VERSION}/yangzz-${target}${EXT}`;
  console.log(`Downloading yangzz v${PACKAGE_VERSION} for ${target}...`);
  console.log(`  URL: ${url}`);

  // Use Node.js https for download (works everywhere, no external deps)
  const downloaded = downloadWithNode(url, BIN_PATH);
  if (downloaded && isValidBinary(BIN_PATH)) {
    if (!IS_WIN) fs.chmodSync(BIN_PATH, 0o755);
    return true;
  }

  // Fallback: try curl/powershell
  const cmds = IS_WIN
    ? [
        `curl.exe -fsSL "${url}" -o "${BIN_PATH}"`,
        `powershell -Command "[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12; Invoke-WebRequest -Uri '${url}' -OutFile '${BIN_PATH}' -UseBasicParsing -MaximumRedirection 5"`,
      ]
    : [
        `curl -fsSL "${url}" -o "${BIN_PATH}" && chmod +x "${BIN_PATH}"`,
      ];

  for (const cmd of cmds) {
    try {
      execSync(cmd, { stdio: "inherit", timeout: 120000 });
      if (isValidBinary(BIN_PATH)) {
        if (!IS_WIN) fs.chmodSync(BIN_PATH, 0o755);
        return true;
      } else {
        console.log(`  Downloaded file is not a valid executable, retrying...`);
        try { fs.unlinkSync(BIN_PATH); } catch {}
      }
    } catch (e) {
      console.log(`  Download attempt failed: ${e.message || "unknown error"}`);
    }
  }
  return false;
}

// Download using Node.js built-in https (follows redirects)
function downloadWithNode(url, dest) {
  try {
    // Synchronous download using child_process + node -e
    const script = `
      const https = require('https');
      const http = require('http');
      const fs = require('fs');
      function download(url, dest, redirects) {
        if (redirects > 5) { process.exit(1); }
        const mod = url.startsWith('https') ? https : http;
        mod.get(url, { headers: { 'User-Agent': 'yangzz-installer' } }, (res) => {
          if (res.statusCode >= 300 && res.statusCode < 400 && res.headers.location) {
            download(res.headers.location, dest, redirects + 1);
            return;
          }
          if (res.statusCode !== 200) { process.exit(1); }
          const file = fs.createWriteStream(dest);
          res.pipe(file);
          file.on('finish', () => { file.close(); process.exit(0); });
        }).on('error', () => { process.exit(1); });
      }
      download(${JSON.stringify(url)}, ${JSON.stringify(dest)}, 0);
    `;
    execSync(`node -e "${script.replace(/"/g, '\\"').replace(/\n/g, ' ')}"`, {
      stdio: 'inherit',
      timeout: 120000,
    });
    return fs.existsSync(dest) && fs.statSync(dest).size > 50000;
  } catch { return false; }
}

// Strategy 2: Check if `yangzz` is already on PATH (e.g. cargo install)
function tryExistingBinary() {
  try {
    const cmd = IS_WIN ? "where yangzz 2>nul" : "which yangzz 2>/dev/null";
    const result = execSync(cmd, {
      encoding: "utf8",
      timeout: 5000,
    }).trim().split("\n")[0].trim();
    if (result) {
      console.log(`Found existing yangzz at: ${result}`);
      // Create a wrapper to existing binary
      if (IS_WIN) {
        // On Windows, write a .cmd wrapper (JS wrapper handles this)
        // Just record the path for yangzz.js to find
        const infoPath = path.join(BIN_DIR, ".yangzz-path");
        fs.writeFileSync(infoPath, result);
      } else {
        const wrapper = `#!/bin/sh\nexec "${result}" "$@"\n`;
        fs.writeFileSync(BIN_PATH, wrapper);
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
  if (IS_WIN) {
    console.log("  Option 1: Download from https://github.com/YangZZtop/yangzz/releases");
    console.log("           (download yangzz-x86_64-pc-windows-msvc.exe)");
    console.log(`           Put it at: ${BIN_PATH}`);
    console.log();
    console.log("  Option 2: cargo install yangzz");
    console.log();
    console.log("  Note: If you see EBUSY errors, close all cmd/PowerShell windows and retry.");
    console.log("  If NVM is locking files, try: nvm use <version> && npm install -g yangzz");
  } else {
    console.log("  Option 1 (推荐): cargo install yangzz");
    console.log("  Option 2: Download from https://github.com/YangZZtop/yangzz/releases");
  }
  console.log();

  // Don't write a fake .exe on Windows — it will show "not compatible" error
  if (!IS_WIN) {
    const stub = `#!/bin/sh\necho "yangzz: Binary not found. Run: cargo install yangzz"\nexit 1\n`;
    fs.writeFileSync(BIN_PATH, stub);
    fs.chmodSync(BIN_PATH, 0o755);
  }
}

main();
