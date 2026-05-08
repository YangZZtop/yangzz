const NPM_SCOPE = "@yangzz123";

const PLATFORM_PACKAGES = [
  {
    key: "darwin-arm64",
    os: "darwin",
    cpu: "arm64",
    target: "aarch64-apple-darwin",
    packageName: `${NPM_SCOPE}/yangzz-darwin-arm64`,
    binaryName: "yangzz",
    assetName: "yangzz-aarch64-apple-darwin",
  },
  {
    key: "darwin-x64",
    os: "darwin",
    cpu: "x64",
    target: "x86_64-apple-darwin",
    packageName: `${NPM_SCOPE}/yangzz-darwin-x64`,
    binaryName: "yangzz",
    assetName: "yangzz-x86_64-apple-darwin",
  },
  {
    key: "linux-arm64",
    os: "linux",
    cpu: "arm64",
    target: "aarch64-unknown-linux-gnu",
    packageName: `${NPM_SCOPE}/yangzz-linux-arm64-gnu`,
    binaryName: "yangzz",
    assetName: "yangzz-aarch64-unknown-linux-gnu",
  },
  {
    key: "linux-x64",
    os: "linux",
    cpu: "x64",
    target: "x86_64-unknown-linux-gnu",
    packageName: `${NPM_SCOPE}/yangzz-linux-x64-gnu`,
    binaryName: "yangzz",
    assetName: "yangzz-x86_64-unknown-linux-gnu",
  },
  {
    key: "win32-arm64",
    os: "win32",
    cpu: "arm64",
    target: "aarch64-pc-windows-msvc",
    packageName: `${NPM_SCOPE}/yangzz-win32-arm64-msvc`,
    binaryName: "yangzz.exe",
    assetName: "yangzz-aarch64-pc-windows-msvc.exe",
  },
  {
    key: "win32-x64",
    os: "win32",
    cpu: "x64",
    target: "x86_64-pc-windows-msvc",
    packageName: `${NPM_SCOPE}/yangzz-win32-x64-msvc`,
    binaryName: "yangzz.exe",
    assetName: "yangzz-x86_64-pc-windows-msvc.exe",
  },
];

function currentPlatformKey(platform, arch) {
  return `${platform}-${arch}`;
}

function findPlatformPackage(platform, arch) {
  return PLATFORM_PACKAGES.find(
    (item) => item.key === currentPlatformKey(platform, arch)
  ) || null;
}

function optionalDependencyMap(version) {
  return Object.fromEntries(
    PLATFORM_PACKAGES.map((item) => [item.packageName, version])
  );
}

module.exports = {
  NPM_SCOPE,
  PLATFORM_PACKAGES,
  currentPlatformKey,
  findPlatformPackage,
  optionalDependencyMap,
};
