import { describe, it, expect, afterEach } from "vitest";
import { mkdtempSync, writeFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { pathToFileURL } from "node:url";
import { loadDotenv } from "../src/env.js";

// Every test passes an explicit temp path: the real onboarding/.env (default
// parameter) is never read by this suite.
const tmpDirs: string[] = [];
const setKeys: string[] = [];

function tempDir(): string {
  const dir = mkdtempSync(path.join(tmpdir(), "onboarding-env-"));
  tmpDirs.push(dir);
  return dir;
}

function writeEnvFile(content: string): string {
  const file = path.join(tempDir(), ".env");
  writeFileSync(file, content);
  return file;
}

// Track keys the fixtures may set so afterEach restores process.env exactly.
function track(...keys: string[]): void {
  setKeys.push(...keys);
}

afterEach(() => {
  for (const key of setKeys) delete process.env[key];
  setKeys.length = 0;
  for (const dir of tmpDirs) rmSync(dir, { recursive: true, force: true });
  tmpDirs.length = 0;
});

describe("loadDotenv", () => {
  it("does nothing when the file does not exist", () => {
    const missing = path.join(tempDir(), ".env");
    const before = { ...process.env };
    expect(() => loadDotenv(missing)).not.toThrow();
    expect(process.env).toEqual(before);
  });

  it("sets absent keys and never overrides an existing env var", () => {
    track("ENV_TEST_EXISTING", "ENV_TEST_NEW");
    process.env.ENV_TEST_EXISTING = "from-real-env";
    const file = writeEnvFile("ENV_TEST_EXISTING=from-file\nENV_TEST_NEW=hello\n");
    loadDotenv(file);
    expect(process.env.ENV_TEST_NEW).toBe("hello");
    expect(process.env.ENV_TEST_EXISTING).toBe("from-real-env");
  });

  it("unquotes double-quoted values and expands \\n into real newlines", () => {
    track("ENV_TEST_PEM");
    // FAKE PEM-shaped fixture, not a real key.
    const file = writeEnvFile(
      'ENV_TEST_PEM="-----BEGIN PRIVATE KEY-----\\nAAAA\\n-----END PRIVATE KEY-----"\n',
    );
    loadDotenv(pathToFileURL(file));
    expect(process.env.ENV_TEST_PEM).toBe(
      "-----BEGIN PRIVATE KEY-----\nAAAA\n-----END PRIVATE KEY-----",
    );
  });

  it("skips comments and blank lines", () => {
    track("ENV_TEST_ONLY");
    const before = new Set(Object.keys(process.env));
    const file = writeEnvFile("# a comment\n\n   \nENV_TEST_ONLY=v\n# ENV_TEST_COMMENTED=x\n");
    loadDotenv(file);
    const added = Object.keys(process.env).filter((k) => !before.has(k));
    expect(added).toEqual(["ENV_TEST_ONLY"]);
    expect(process.env.ENV_TEST_ONLY).toBe("v");
  });
});
