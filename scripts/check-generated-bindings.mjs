import { execFileSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const repoRoot = dirname(scriptDir);
const bindingsPath = join(repoRoot, "src", "generated", "bindings.ts");
const bindingsPrettierPath = "src/generated/bindings.ts";
const EXPECTED_HOME_USAGE_PERIOD_LITERALS = ["last7", "last15", "last30", "month"];

function parseHomeUsagePeriodLiterals(source) {
  const match = source.match(/export type HomeUsagePeriod = ([^;]+);/);
  if (!match) {
    throw new Error("Missing HomeUsagePeriod export in generated bindings.");
  }
  return Array.from(match[1].matchAll(/"([^"]+)"/g), (part) => part[1]);
}

function assertHomeUsagePeriodContract(source) {
  const actual = parseHomeUsagePeriodLiterals(source);
  if (JSON.stringify(actual) === JSON.stringify(EXPECTED_HOME_USAGE_PERIOD_LITERALS)) return;

  throw new Error(
    `HomeUsagePeriod contract drifted. Expected ${EXPECTED_HOME_USAGE_PERIOD_LITERALS.join(
      ", "
    )}; received ${actual.join(", ")}.`
  );
}
const before = existsSync(bindingsPath) ? readFileSync(bindingsPath, "utf8") : null;

function quoteForCmd(arg) {
  return /^[A-Za-z0-9_./:-]+$/.test(arg) ? arg : `"${arg.replaceAll('"', '""')}"`;
}

function runPnpm(args) {
  const options = {
    cwd: repoRoot,
    stdio: "inherit",
  };

  if (process.platform === "win32") {
    const command = `pnpm ${args.map(quoteForCmd).join(" ")}`;
    execFileSync(process.env.ComSpec ?? "cmd.exe", ["/d", "/s", "/c", command], options);
    return;
  }

  execFileSync("pnpm", args, options);
}

runPnpm(["tauri:gen-types"]);

// Format the freshly generated file so the comparison uses the same style
// as the committed version (which passes through prettier on pre-commit).
runPnpm(["exec", "prettier", "--write", bindingsPrettierPath]);

const after = existsSync(bindingsPath) ? readFileSync(bindingsPath, "utf8") : null;
if (after == null) {
  console.error("Generated bindings file is missing: src/generated/bindings.ts");
  process.exit(1);
}

if (after != null) {
  try {
    assertHomeUsagePeriodContract(after);
  } catch (error) {
    console.error(String(error));
    process.exit(1);
  }
}

if (before !== after) {
  console.error("Generated bindings were outdated. Review and commit src/generated/bindings.ts.");
  process.exit(1);
}
