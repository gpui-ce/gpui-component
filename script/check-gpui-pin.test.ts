import { expect, test } from "bun:test";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const packages = ["gpui-ce", "gpui_ce_platform", "gpui_ce_macros", "gpui_ce_web"];

function check(versions: (string | undefined)[]) {
  const directory = mkdtempSync(join(tmpdir(), "gpui-ce-pin-"));
  try {
    const manifest = join(directory, "Cargo.toml");
    writeFileSync(manifest, "[workspace.dependencies]\n" + versions.flatMap((version, index) =>
      version === undefined ? [] : [`dep${index} = { package = "${packages[index]}", version = "${version}" }`]
    ).join("\n"));
    return Bun.spawnSync([process.execPath, join(import.meta.dir, "check-gpui-pin.ts"), manifest]);
  } finally {
    rmSync(directory, { recursive: true });
  }
}

test("accepts one exact CE release", () => {
  expect(check(packages.map(() => "=0.3.0")).exitCode).toBe(0);
});

test("rejects mismatched CE releases", () => {
  expect(check(["=0.3.0", "=0.3.1", "=0.3.0", "=0.3.0"]).exitCode).toBe(1);
});

test("rejects a floating CE requirement", () => {
  expect(check(["0.3.0", "=0.3.0", "=0.3.0", "=0.3.0"]).exitCode).toBe(1);
});

test("rejects a missing CE crate", () => {
  expect(check(["=0.3.0", undefined, "=0.3.0", "=0.3.0"]).exitCode).toBe(1);
});
