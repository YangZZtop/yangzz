#!/usr/bin/env node

const { execFileSync, execSync } = require("child_process");
const fs = require("fs");
const os = require("os");
const path = require("path");
const { findPlatformPackage } = require("../platforms");

const IS_WIN = os.platform() === "win32";
const EXT = IS_WIN ? ".exe" : "";
const BIN_DIR = path.join(__dirname);
const NATIVE_BIN = path.join(BIN_DIR, `yangzz${EXT}`);
const PATH_FILE = path.join(BIN_DIR, ".yangzz-path");

function isRealBinary(filePath) {
  try {
    const stat = fs.statSync(filePath);
    if (stat.size < 50000) return false;
    const fd = fs.openSync(filePath, "r");
    const buf = Buffer.alloc(2);
    fs.readSync(fd, buf, 0, 2, 0);
    fs.closeSync(fd);
    if (IS_WIN) return buf[0] === 0x4d && buf[1] === 0x5a;
    return true;
  } catch {
    return false;
  }
}

function resolvePlatformPackageBinary() {
  const platformPkg = findPlatformPackage(os.platform(), os.arch());
  if (!platformPkg) return { binary: null, platformPkg: null };

  try {
    const exported = require(platformPkg.packageName);
    if (typeof exported === "string" && fs.existsSync(exported) && isRealBinary(exported)) {
      return { binary: exported, platformPkg };
    }
  } catch {}

  try {
    const pkgJsonPath = require.resolve(`${platformPkg.packageName}/package.json`);
    const candidate = path.join(path.dirname(pkgJsonPath), "bin", platformPkg.binaryName);
    if (fs.existsSync(candidate) && isRealBinary(candidate)) {
      return { binary: candidate, platformPkg };
    }
  } catch {}

  return { binary: null, platformPkg };
}

function findBinary() {
  if (fs.existsSync(NATIVE_BIN) && isRealBinary(NATIVE_BIN)) {
    return { binary: NATIVE_BIN, source: "local-native", platformPkg: null };
  }

  const { binary: packageBinary, platformPkg } = resolvePlatformPackageBinary();
  if (packageBinary) {
    return { binary: packageBinary, source: "optional-dependency", platformPkg };
  }

  if (fs.existsSync(PATH_FILE)) {
    const recorded = fs.readFileSync(PATH_FILE, "utf8").trim();
    if (recorded && fs.existsSync(recorded) && isRealBinary(recorded)) {
      return { binary: recorded, source: "recorded-path", platformPkg };
    }
  }

  try {
    const cmd = IS_WIN ? "where yangzz 2>nul" : "which yangzz 2>/dev/null";
    const result = execSync(cmd, { encoding: "utf8", timeout: 3000 })
      .trim()
      .split("\n")[0]
      .trim();
    if (
      result &&
      fs.existsSync(result) &&
      path.resolve(result) !== path.resolve(process.argv[1]) &&
      isRealBinary(result)
    ) {
      return { binary: result, source: "path", platformPkg };
    }
  } catch {}

  const cargoPath = path.join(os.homedir(), ".cargo", "bin", `yangzz${EXT}`);
  if (fs.existsSync(cargoPath) && isRealBinary(cargoPath)) {
    return { binary: cargoPath, source: "cargo", platformPkg };
  }

  return { binary: null, source: "missing", platformPkg };
}

const resolved = findBinary();

if (!resolved.binary) {
  console.error("yangzz: Binary not found.");
  console.error("");

  if (resolved.platformPkg) {
    console.error(`Expected platform package: ${resolved.platformPkg.packageName}`);
    console.error(`Current platform: ${os.platform()} ${os.arch()}`);
    console.error("");
  } else {
    console.error(`Unsupported platform: ${os.platform()} ${os.arch()}`);
    console.error("");
  }

  console.error("Try:");
  console.error("  npm install -g yangzz");
  console.error("  # if your package manager skipped optionalDependencies:");
  if (resolved.platformPkg) {
    console.error(`  npm install -g ${resolved.platformPkg.packageName}`);
  }
  console.error("  # or install from source:");
  console.error("  cargo install yangzz");
  process.exit(1);
}

try {
  execFileSync(resolved.binary, process.argv.slice(2), {
    stdio: "inherit",
    env: process.env,
  });
} catch (e) {
  process.exit(e.status || 1);
}
