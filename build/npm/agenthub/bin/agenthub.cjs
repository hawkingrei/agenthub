#!/usr/bin/env node

const path = require("node:path");
const { spawnSync } = require("node:child_process");
const { createRequire } = require("node:module");

const PLATFORM_PACKAGE_NAMES = {
  "darwin:arm64": "@linkerdao/agenthub-darwin-arm64",
  "linux:arm64": "@linkerdao/agenthub-linux-arm64",
  "linux:x64": "@linkerdao/agenthub-linux-x64",
};

function resolvePlatformPackageName(platform = process.platform, arch = process.arch) {
  return PLATFORM_PACKAGE_NAMES[`${platform}:${arch}`] ?? null;
}

function buildUnsupportedPlatformMessage(platform = process.platform, arch = process.arch) {
  return [
    `No published @linkerdao/agenthub binary is available for ${platform}/${arch}.`,
    "Supported targets currently are: darwin/arm64, linux/arm64, linux/x64.",
  ].join(" ");
}

function buildMissingPackageMessage(packageName) {
  return [
    `The platform package ${packageName} is not installed.`,
    "Reinstall @linkerdao/agenthub or verify that npm optional dependencies are enabled for this platform.",
  ].join(" ");
}

function resolveBinaryPath(
  platform = process.platform,
  arch = process.arch,
  resolvePackageJson = (packageName) => createRequire(__filename).resolve(`${packageName}/package.json`)
) {
  const packageName = resolvePlatformPackageName(platform, arch);
  if (!packageName) {
    const error = new Error(buildUnsupportedPlatformMessage(platform, arch));
    error.code = "UNSUPPORTED_PLATFORM";
    throw error;
  }

  let packageJsonPath;
  try {
    packageJsonPath = resolvePackageJson(packageName);
  } catch (error) {
    const wrapped = new Error(buildMissingPackageMessage(packageName));
    wrapped.code = "MISSING_PLATFORM_PACKAGE";
    wrapped.cause = error;
    throw wrapped;
  }

  return path.join(path.dirname(packageJsonPath), "bin", "agenthub");
}

function runCli(argv = process.argv.slice(2), options = {}) {
  const binaryPath = resolveBinaryPath(options.platform, options.arch, options.resolvePackageJson);
  const result = spawnSync(binaryPath, argv, {
    stdio: "inherit",
    env: process.env,
  });

  if (result.error) {
    throw result.error;
  }

  if (typeof result.status === "number") {
    return result.status;
  }

  return 1;
}

if (require.main === module) {
  try {
    process.exit(runCli());
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    console.error(message);
    process.exit(1);
  }
}

module.exports = {
  buildMissingPackageMessage,
  buildUnsupportedPlatformMessage,
  resolveBinaryPath,
  resolvePlatformPackageName,
  runCli,
};
