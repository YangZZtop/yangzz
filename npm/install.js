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
const PATH_FILE = path.join(BIN_DIR, ".yangzz-path");
const STATE_FILE = path.join(BIN_DIR, ".install-state.json");

function writeState(state) {
  try {
    fs.mkdirSync(BIN_DIR, { recursive: true });
    fs.writeFileSync(
      STATE_FILE,
      JSON.stringify(
        {
          updatedAt: new Date().toISOString(),
          version: PACKAGE_VERSION,
          platform: os.platform(),
          arch: os.arch(),
          ...state,
        },
        null,
        2,
      ),
    );
  } catch {}
}

function readState() {
  try {
    if (!fs.existsSync(STATE_FILE)) return null;
    return JSON.parse(fs.readFileSync(STATE_FILE, "utf8"));
  } catch {
    return null;
  }
}

function clearState() {
  try { fs.unlinkSync(STATE_FILE); } catch {}
}

function clearRecordedPath() {
  try { fs.unlinkSync(PATH_FILE); } catch {}
}

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

function buildDownloadUrls(target) {
  const fileName = `yangzz-${target}${EXT}`;
  const urls = [];
  const customUrl = process.env.YANGZZ_DOWNLOAD_URL;
  const customBase = process.env.YANGZZ_DOWNLOAD_BASE_URL;

  if (customUrl) {
    urls.push(customUrl);
  }
  if (customBase) {
    urls.push(`${customBase.replace(/\/+$/, "")}/${fileName}`);
  }

  urls.push(
    `https://github.com/YangZZtop/yangzz/releases/download/v${PACKAGE_VERSION}/${fileName}`
  );

  return Array.from(new Set(urls));
}

// Validate that a file looks like a real executable (not HTML error page)
function isValidBinary(filePath) {
  try {
    const fd = fs.openSync(filePath, "r");
    const buf = Buffer.alloc(4);
    fs.readSync(fd, buf, 0, 4, 0);
    fs.closeSync(fd);
    const size = fs.statSync(filePath).size;
    if (size < 50000) return false; // too small, probably error page or stub
    if (IS_WIN) {
      return buf[0] === 0x4d && buf[1] === 0x5a;
    }
    return (
      (buf[0] === 0x7f && buf[1] === 0x45) ||
      (buf[0] === 0xcf && buf[1] === 0xfa) ||
      (buf[0] === 0xca && buf[1] === 0xfe)
    );
  } catch {
    return false;
  }
}

// ── Primary download: write a temp .js file and run it with Node ──
// This avoids shell quoting issues on Windows cmd.exe / PowerShell.
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

function finalizeBinarySuccess() {
  if (!IS_WIN) {
    fs.chmodSync(BIN_PATH, 0o755);
  }
  adhocCodesignMac(BIN_PATH);
  clearRecordedPath();
}

// Strategy 1: Download pre-built binary
function tryDownload(target) {
  const urls = buildDownloadUrls(target);
  const errors = [];

  for (const url of urls) {
    console.log(`Downloading yangzz v${PACKAGE_VERSION} for ${target}...`);
    console.log(`  URL: ${url}`);

    if (downloadWithTempScript(url, BIN_PATH) && isValidBinary(BIN_PATH)) {
      finalizeBinarySuccess();
      console.log(
        `  Binary validated: ${(fs.statSync(BIN_PATH).size / 1024 / 1024).toFixed(1)} MB`
      );
      return true;
    }
    errors.push(`node downloader failed: ${url}`);
    try { fs.unlinkSync(BIN_PATH); } catch {}

    try {
      const curlBin = IS_WIN ? "curl.exe" : "curl";
      execSync(`${curlBin} -fsSL -o "${BIN_PATH}" "${url}"`, {
        stdio: "inherit",
        timeout: 120000,
      });
      if (isValidBinary(BIN_PATH)) {
        finalizeBinarySuccess();
        console.log(
          `  Binary validated (curl): ${(fs.statSync(BIN_PATH).size / 1024 / 1024).toFixed(1)} MB`
        );
        return true;
      }
      try { fs.unlinkSync(BIN_PATH); } catch {}
    } catch (e) {
      errors.push(`curl failed for ${url}: ${e.message || "not available"}`);
      console.log(`  curl failed: ${e.message || "not available"}`);
    }

    if (IS_WIN) {
      try {
        const psCmd = `powershell -NoProfile -ExecutionPolicy Bypass -Command "` +
          `[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12; ` +
          `Invoke-WebRequest -Uri '${url}' -OutFile '${BIN_PATH}' -UseBasicParsing -MaximumRedirection 10"`;
        execSync(psCmd, { stdio: "inherit", timeout: 120000 });
        if (isValidBinary(BIN_PATH)) {
          clearRecordedPath();
          console.log(
            `  Binary validated (PowerShell): ${(fs.statSync(BIN_PATH).size / 1024 / 1024).toFixed(1)} MB`
          );
          return true;
        }
        try { fs.unlinkSync(BIN_PATH); } catch {}
      } catch (e) {
        errors.push(`powershell failed for ${url}: ${e.message || "unknown"}`);
        console.log(`  PowerShell download failed: ${e.message || "unknown"}`);
      }
    }
  }

  writeState({
    phase: "download",
    ok: false,
    target,
    attemptedUrls: urls,
    errors,
  });
  return false;
}

// Strategy 2: Reuse an existing real binary (e.g. cargo install)
function tryExistingBinary() {
  try {
    const cmd = IS_WIN ? "where yangzz 2>nul" : "which yangzz 2>/dev/null";
    const result = execSync(cmd, {
      encoding: "utf8",
      timeout: 5000,
    }).trim().split("\n")[0].trim();

    if (result && fs.existsSync(result) && isValidBinary(result)) {
      console.log(`Found existing yangzz binary at: ${result}`);
      fs.writeFileSync(PATH_FILE, result);
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
      timeout: 300000,
      cwd: __dirname,
    });
    if (fs.existsSync(BIN_PATH) && fs.statSync(BIN_PATH).size > 1000) {
      finalizeBinarySuccess();
      return true;
    }
    return false;
  } catch (e) {
    writeState({
      phase: "cargo",
      ok: false,
      error: e.message || "cargo install failed",
    });
    return false;
  }
}

function main() {
  clearState();

  if (fs.existsSync(BIN_PATH) && isValidBinary(BIN_PATH)) {
    const sizeMB = (fs.statSync(BIN_PATH).size / 1024 / 1024).toFixed(1);
    console.log(`yangzz binary already exists (${sizeMB} MB), skipping install.`);
    writeState({
      phase: "reuse-native",
      ok: true,
      path: BIN_PATH,
    });
    return;
  }

  if (fs.existsSync(BIN_PATH) && !isValidBinary(BIN_PATH)) {
    console.log("  Removing invalid binary from previous install...");
    try { fs.unlinkSync(BIN_PATH); } catch {}
  }

  fs.mkdirSync(BIN_DIR, { recursive: true });

  const target = getTarget();

  if (target && tryDownload(target)) {
    writeState({
      phase: "download",
      ok: true,
      target,
      path: BIN_PATH,
    });
    console.log("✓ yangzz installed successfully (pre-built binary)");
    return;
  }

  if (tryExistingBinary()) {
    writeState({
      phase: "reuse-path",
      ok: true,
      path: fs.readFileSync(PATH_FILE, "utf8").trim(),
    });
    console.log("✓ yangzz linked to existing installation");
    return;
  }

  if (tryCargoBuild()) {
    writeState({
      phase: "cargo",
      ok: true,
      path: BIN_PATH,
    });
    console.log("✓ yangzz installed successfully (built from source)");
    return;
  }

  const previous = readState() || {};
  writeState({
    ...previous,
    phase: "failed",
    ok: false,
    target,
    message: "No usable binary after download / PATH reuse / cargo fallback",
    hints: {
      customDownloadUrl: "Set YANGZZ_DOWNLOAD_URL to a direct binary URL",
      customDownloadBaseUrl: "Set YANGZZ_DOWNLOAD_BASE_URL to your release mirror base URL",
      foregroundScripts: "npm install -g yangzz --foreground-scripts",
    },
  });

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
  console.log("  Option 3: Set YANGZZ_DOWNLOAD_BASE_URL and rerun npm rebuild yangzz");
  console.log();

  if (!IS_WIN) {
    const stub = `#!/bin/sh\necho "yangzz: Binary not found. Run: cargo install yangzz"\nexit 1\n`;
    fs.writeFileSync(BIN_PATH, stub);
    fs.chmodSync(BIN_PATH, 0o755);
  }
}

if (process.env.YANGZZ_FORCE_DOWNLOAD === "1") {
  try { fs.unlinkSync(BIN_PATH); } catch {}
}

main();
