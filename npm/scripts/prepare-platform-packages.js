#!/usr/bin/env node

const fs = require("fs");
const path = require("path");
const {
  PLATFORM_PACKAGES,
  optionalDependencyMap,
} = require("../platforms");

const REPO_ROOT = path.resolve(__dirname, "..", "..");
const ROOT_PACKAGE_JSON = path.resolve(__dirname, "..", "package.json");

function parseArgs(argv) {
  const args = {
    artifactsDir: path.resolve(process.cwd()),
    outputDir: path.resolve(process.cwd(), "npm-platform-dist"),
    syncRoot: false,
  };

  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === "--artifacts-dir") {
      args.artifactsDir = path.resolve(argv[++i]);
    } else if (arg === "--output-dir") {
      args.outputDir = path.resolve(argv[++i]);
    } else if (arg === "--sync-root-optional-deps") {
      args.syncRoot = true;
    } else {
      throw new Error(`Unknown argument: ${arg}`);
    }
  }

  return args;
}

function ensureDir(dir) {
  fs.mkdirSync(dir, { recursive: true });
}

function removeDir(dir) {
  fs.rmSync(dir, { recursive: true, force: true });
}

function readJson(filePath) {
  return JSON.parse(fs.readFileSync(filePath, "utf8"));
}

function writeJson(filePath, value) {
  fs.writeFileSync(filePath, `${JSON.stringify(value, null, 2)}\n`);
}

function packageDirName(packageName) {
  return packageName.replace(/^@/, "").replace("/", "__");
}

function platformReadme(pkg) {
  return `# ${pkg.packageName}

Platform binary package for \`yangzz\`.

- target: \`${pkg.target}\`
- os: \`${pkg.os}\`
- cpu: \`${pkg.cpu}\`

This package is installed automatically as an \`optionalDependency\` of the main \`yangzz\` package.
`;
}

function platformIndexJs(binaryName) {
  return `const path = require("path");
module.exports = path.join(__dirname, "bin", ${JSON.stringify(binaryName)});
`;
}

function platformPackageJson(pkg, version) {
  return {
    name: pkg.packageName,
    version,
    description: `Prebuilt ${pkg.target} binary for yangzz`,
    os: [pkg.os],
    cpu: [pkg.cpu],
    files: ["README.md", "bin", "index.js", "package.json"],
    main: "index.js",
    license: "MIT",
    repository: {
      type: "git",
      url: "https://github.com/YangZZtop/yangzz.git",
    },
    homepage: "https://github.com/YangZZtop/yangzz",
    publishConfig: {
      access: "public",
    },
  };
}

function copyBinary(src, dest) {
  ensureDir(path.dirname(dest));
  fs.copyFileSync(src, dest);
  if (!dest.endsWith(".exe")) {
    fs.chmodSync(dest, 0o755);
  }
}

function syncRootOptionalDependencies(version) {
  const rootPackage = readJson(ROOT_PACKAGE_JSON);
  rootPackage.optionalDependencies = optionalDependencyMap(version);
  writeJson(ROOT_PACKAGE_JSON, rootPackage);
}

function main() {
  const args = parseArgs(process.argv.slice(2));
  const rootPackage = readJson(ROOT_PACKAGE_JSON);
  const version = rootPackage.version;

  if (args.syncRoot) {
    syncRootOptionalDependencies(version);
  }

  removeDir(args.outputDir);
  ensureDir(args.outputDir);

  const generated = [];
  const missing = [];

  for (const pkg of PLATFORM_PACKAGES) {
    const src = path.join(args.artifactsDir, pkg.assetName);
    if (!fs.existsSync(src)) {
      missing.push({ packageName: pkg.packageName, expectedAsset: src });
      continue;
    }

    const dir = path.join(args.outputDir, packageDirName(pkg.packageName));
    const binaryDest = path.join(dir, "bin", pkg.binaryName);

    ensureDir(dir);
    copyBinary(src, binaryDest);
    fs.writeFileSync(path.join(dir, "index.js"), platformIndexJs(pkg.binaryName));
    fs.writeFileSync(path.join(dir, "README.md"), platformReadme(pkg));
    writeJson(path.join(dir, "package.json"), platformPackageJson(pkg, version));

    generated.push({
      packageName: pkg.packageName,
      dir,
      binary: binaryDest,
    });
  }

  if (missing.length > 0) {
    console.error("Missing release artifacts for platform packages:");
    for (const item of missing) {
      console.error(`  - ${item.packageName}: ${item.expectedAsset}`);
    }
    process.exit(1);
  }

  console.log(`Generated ${generated.length} platform npm packages in ${args.outputDir}`);
  for (const item of generated) {
    console.log(`  - ${item.packageName}`);
  }
}

main();
