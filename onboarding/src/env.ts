import { readFileSync } from "node:fs";

// Minimal .env loader: no dependency, no logging of keys or values.
// A real environment variable always wins over the file.
export function loadDotenv(filePath: string | URL = new URL("../.env", import.meta.url)): void {
  let content: string;
  try {
    content = readFileSync(filePath, "utf8");
  } catch (error) {
    // No .env file is a supported setup (exported env vars only).
    if ((error as NodeJS.ErrnoException).code === "ENOENT") return;
    throw error;
  }

  for (const line of content.split("\n")) {
    const trimmed = line.trim();
    if (trimmed === "" || trimmed.startsWith("#")) continue;
    const eq = trimmed.indexOf("=");
    if (eq <= 0) continue;
    const key = trimmed.slice(0, eq).trim();
    let value = trimmed.slice(eq + 1).trim();
    if (value.length >= 2 && value.startsWith('"') && value.endsWith('"')) {
      // Double-quoted form: a PEM key fits on one line via literal \n escapes.
      value = value.slice(1, -1).replaceAll("\\n", "\n");
    }
    if (process.env[key] === undefined) process.env[key] = value;
  }
}
