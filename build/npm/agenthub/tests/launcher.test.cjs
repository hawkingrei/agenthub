const test = require("node:test");
const assert = require("node:assert/strict");

const {
  buildMissingPackageMessage,
  buildUnsupportedPlatformMessage,
  resolveBinaryPath,
  resolvePlatformPackageName,
} = require("../bin/agenthub.cjs");

test("resolvePlatformPackageName maps supported targets", () => {
  assert.equal(resolvePlatformPackageName("darwin", "arm64"), "@linkerdog/agenthub-darwin-arm64");
  assert.equal(resolvePlatformPackageName("linux", "arm64"), "@linkerdog/agenthub-linux-arm64");
  assert.equal(resolvePlatformPackageName("linux", "x64"), "@linkerdog/agenthub-linux-x64");
  assert.equal(resolvePlatformPackageName("darwin", "x64"), null);
});

test("resolveBinaryPath joins the package directory with the staged binary path", () => {
  const binaryPath = resolveBinaryPath("linux", "x64", () => "/tmp/platform-package/package.json");
  assert.equal(binaryPath, "/tmp/platform-package/bin/agenthub");
});

test("resolveBinaryPath reports unsupported targets clearly", () => {
  assert.throws(
    () => resolveBinaryPath("win32", "x64"),
    (error) =>
      error instanceof Error &&
      error.message === buildUnsupportedPlatformMessage("win32", "x64") &&
      error.code === "UNSUPPORTED_PLATFORM"
  );
});

test("resolveBinaryPath reports missing optional platform packages clearly", () => {
  assert.throws(
    () =>
      resolveBinaryPath("linux", "arm64", () => {
        throw new Error("not found");
      }),
    (error) =>
      error instanceof Error &&
      error.message === buildMissingPackageMessage("@linkerdog/agenthub-linux-arm64") &&
      error.code === "MISSING_PLATFORM_PACKAGE"
  );
});
