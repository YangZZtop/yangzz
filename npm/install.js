#!/usr/bin/env node

const { execSync } = require("child_process");
const fs = require("fs");
const path = require("path");
const os = require("os");

const PACKAGE_VERSION = require("./package.json").version;
const BIN_DIR = path.join(__dirname, "bin");
const IS_WIN = os.platform() === "win32";
const IS_MAC = os.platform() === "darwin";
const EXT = IS_WIN ? ".exe" : "";
const BIN_PATH = path.join(BIN_DIR, `yangzz${EXT}`);

// macOS Sequoia (15+) silently blocks unsigned arm64 binaries — they hang with
// no error. Ad-hoc signing fixes it locally without needing an Apple Developer
// account. Safe to run on already-signed binaries too (replaces signature).
function adhocCodesignMac(binPath) {
  if (!IS_MAC) return;
  try {
    execSync(`codesign --force --sign - "${binPath}"`, {
      stdio: "ignore",
      timeout: 10000,
    });
  } catch {
    // codesign unavailable or failed — not fatal; CI-built binaries should
    // already be signed. Local cargo builds may hang on Sequoia; user can
    // manually run: codesign --force --sign - <path>
  }
}

// Platform mapping for GitHub releases
const PLATFORM_MAP = {
  "darwin-arm64": "aarch64-apple-darwin",
  "darwin-x64": "x86_64-apple-darwin",
  "linux-arm64": "aarch64-unknown-linux-gnu",
  "linux-x64": "x86_64-unknown-linux-gnu",
  "win32-x64": "x86_64-pc-windows-msvc",
  "win32-arm64": "aarch64-pc-windows-msvc",
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
    if (size < 50000) return false; // too small, probably error page or stub
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

// ── Primary download: write a temp .js file and run it with Node ──
// This avoids ALL shell quoting issues on Windows cmd.exe / PowerShell.
function downloadWithTempScript(url, dest) {
  const tmpScript = path.join(os.tmpdir(), `yangzz-dl-${Date.now()}.js`);
  const code = [
    `const https = require("https");`,
    `const http = require("http");`,
    `const fs = require("fs");`,
    `const url = ${JSON.stringify(url)};`,
    `const dest = ${JSON.stringify(dest)};`,
    `function dl(u, n) {`,
    `  if (n > 10) { console.error("Too many redirects"); process.exit(1); }`,
    `  const mod = u.startsWith("https") ? https : http;`,
    `  mod.get(u, { headers: { "User-Agent": "yangzz-installer/${PACKAGE_VERSION}" } }, (res) => {`,
    `    if (res.statusCode >= 300 && res.statusCode < 400 && res.headers.location) {`,
    `      dl(res.headers.location, n + 1);`,
    `      return;`,
    `    }`,
    `    if (res.statusCode !== 200) {`,
    `      console.error("HTTP " + res.statusCode + " for " + u);`,
    `      process.exit(1);`,
    `    }`,
    `    const total = parseInt(res.headers["content-length"] || "0", 10);`,
    `    let downloaded = 0;`,
    `    const file = fs.createWriteStream(dest);`,
    `    res.on("data", (chunk) => {`,
    `      downloaded += chunk.length;`,
    `      if (total > 0) {`,
    `        const pct = Math.round(downloaded / total * 100);`,
    `        process.stdout.write("\\r  Downloading... " + pct + "% (" + (downloaded / 1024 / 1024).toFixed(1) + " MB)");`,
    `      }`,
    `    });`,
    `    res.pipe(file);`,
    `    file.on("finish", () => { file.close(); console.log("\\n  Download complete."); process.exit(0); });`,
    `    file.on("error", (e) => { console.error("Write error: " + e.message); process.exit(1); });`,
    `  }).on("error", (e) => { console.error("Request error: " + e.message); process.exit(1); });`,
    `}`,
    `dl(url, 0);`,
  ].join("\n");

  try {
    fs.writeFileSync(tmpScript, code);
    execSync(`node "${tmpScript}"`, { stdio: "inherit", timeout: 120000 });
    try { fs.unlinkSync(tmpScript); } catch {}
    return fs.existsSync(dest) && fs.statSync(dest).size > 50000;
  } catch (e) {
    console.log(`  Node download failed: ${e.message || "unknown"}`);
    try { fs.unlinkSync(tmpScript); } catch {}
    return false;
  }
}

// Strategy 1: Download pre-built binary from GitHub Releases
function tryDownload(target) {
  const url = `https://github.com/YangZZtop/yangzz/releases/download/v${PACKAGE_VERSION}/yangzz-${target}${EXT}`;
  console.log(`Downloading yangzz v${PACKAGE_VERSION} for ${target}...`);
  console.log(`  URL: ${url}`);

  // Method 1: Node.js temp script (works on ALL platforms, no shell quoting issues)
  if (downloadWithTempScript(url, BIN_PATH) && isValidBinary(BIN_PATH)) {
    if (!IS_WIN) fs.chmodSync(BIN_PATH, 0o755);
    adhocCodesignMac(BIN_PATH);
    console.log(`  Binary validated: ${(fs.statSync(BIN_PATH).size / 1024 / 1024).toFixed(1)} MB`);
    return true;
  }
  try { fs.unlinkSync(BIN_PATH); } catch {}

  // Method 2: curl (macOS/Linux usually have it; Windows 10+ has curl.exe)
  try {
    const curlBin = IS_WIN ? "curl.exe" : "curl";
    execSync(`${curlBin} -fsSL -o "${BIN_PATH}" "${url}"`, { stdio: "inherit", timeout: 120000 });
    if (isValidBinary(BIN_PATH)) {
      if (!IS_WIN) fs.chmodSync(BIN_PATH, 0o755);
      adhocCodesignMac(BIN_PATH);
      console.log(`  Binary validated (curl): ${(fs.statSync(BIN_PATH).size / 1024 / 1024).toFixed(1)} MB`);
      return true;
    }
    try { fs.unlinkSync(BIN_PATH); } catch {}
  } catch (e) {
    console.log(`  curl failed: ${e.message || "not available"}`);
  }

  // Method 3: PowerShell (Windows fallback)
  if (IS_WIN) {
    try {
      const psCmd = `powershell -NoProfile -ExecutionPolicy Bypass -Command "` +
        `[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12; ` +
        `Invoke-WebRequest -Uri '${url}' -OutFile '${BIN_PATH}' -UseBasicParsing -MaximumRedirection 10"`;
      execSync(psCmd, { stdio: "inherit", timeout: 120000 });
      if (isValidBinary(BIN_PATH)) {
        console.log(`  Binary validated (PowerShell): ${(fs.statSync(BIN_PATH).size / 1024 / 1024).toFixed(1)} MB`);
        return true;
      }
      try { fs.unlinkSync(BIN_PATH); } catch {}
    } catch (e) {
      console.log(`  PowerShell download failed: ${e.message || "unknown"}`);
    }
  }

  return false;
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
    if (fs.existsSync(BIN_PATH) && fs.statSync(BIN_PATH).size > 1000) {
      adhocCodesignMac(BIN_PATH);
      return true;
    }
    return false;
  } catch {
    return false;
  }
}

function main() {
  // If binary already exists and is a REAL executable, skip
  if (fs.existsSync(BIN_PATH) && isValidBinary(BIN_PATH)) {
    const sizeMB = (fs.statSync(BIN_PATH).size / 1024 / 1024).toFixed(1);
    console.log(`yangzz binary already exists (${sizeMB} MB), skipping install.`);
    return;
  }
  // Clean up any broken/fake binary from previous install
  if (fs.existsSync(BIN_PATH) && !isValidBinary(BIN_PATH)) {
    console.log("  Removing invalid binary from previous install...");
    try { fs.unlinkSync(BIN_PATH); } catch {}
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

// Allow YANGZZ_FORCE_DOWNLOAD=1 to skip existing check (for npm rebuild)
if (process.env.YANGZZ_FORCE_DOWNLOAD === "1") {
  try { fs.unlinkSync(BIN_PATH); } catch {}
}

main();
