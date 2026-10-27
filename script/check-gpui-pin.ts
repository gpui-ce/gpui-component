#!/usr/bin/env bun
/** Verify that the GPUI CE release crates stay on one exact version.
 * Usage: bun script/check-gpui-pin.ts [path/to/Cargo.toml]
 */
import { readFileSync } from "node:fs";
import { join, resolve } from "node:path";

const REPO_ROOT = resolve(import.meta.dir, "..");
const MANIFEST = "Cargo.toml";
const RELEASE_PACKAGES = new Set(["gpui-ce", "gpui_ce_platform", "gpui_ce_macros", "gpui_ce_web"]);
const EXACT_VERSION = /^=(\d+\.\d+\.\d+)$/;

interface Dependency {
  alias: string;
  package: string;
  version: string | undefined;
}

function releaseDependencies(manifestPath: string): Dependency[] {
  const manifest = Bun.TOML.parse(readFileSync(manifestPath, "utf8")) as {
    workspace?: { dependencies?: Record<string, unknown> };
  };
  const dependencies: Dependency[] = [];
  for (const [alias, spec] of Object.entries(manifest.workspace?.dependencies ?? {})) {
    if (typeof spec !== "object" || spec === null) continue;
    const { package: pkg, version } = spec as { package?: unknown; version?: unknown };
    if (typeof pkg !== "string" || !RELEASE_PACKAGES.has(pkg)) continue;
    dependencies.push({ alias, package: pkg, version: typeof version === "string" ? version : undefined });
  }
  return dependencies;
}

function describe(dependency: Dependency): string {
  return `${dependency.alias} = { package = "${dependency.package}", version = ${JSON.stringify(dependency.version ?? "")} }`;
}

function problems(dependencies: Dependency[]): string[] {
  const found: string[] = [];
  for (const pkg of RELEASE_PACKAGES) {
    if (!dependencies.some(dependency => dependency.package === pkg)) {
      found.push(`missing ${pkg} in [workspace.dependencies]`);
    }
  }
  const releaseVersions = new Map<string, string[]>();
  for (const dependency of dependencies) {
    const exact = dependency.version === undefined ? null : EXACT_VERSION.exec(dependency.version);
    if (exact === null) {
      found.push(`${describe(dependency)} is not an exact \`=x.y.z\` requirement`);
      continue;
    }
    const aliases = releaseVersions.get(exact[1]) ?? [];
    aliases.push(dependency.package);
    releaseVersions.set(exact[1], aliases);
  }
  if (releaseVersions.size > 1) {
    const detail = [...releaseVersions].map(([version, names]) => `${version} (${names.join(", ")})`).join(", ");
    found.push(`the GPUI CE crates are pinned to more than one version: ${detail}`);
  }
  return found;
}

const manifestPath = process.argv[2] ?? join(REPO_ROOT, MANIFEST);
const found = problems(releaseDependencies(manifestPath));
if (found.length > 0) {
  for (const problem of found) console.error(`::error file=${MANIFEST}::${problem}`);
  console.error(`\n${MANIFEST} must pin every GPUI CE crate to the exact release gpui-kit was built against.`);
  process.exit(1);
}
console.log(`${MANIFEST} pins the GPUI CE crates exactly`);
