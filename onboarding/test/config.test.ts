import { describe, it, expect } from "vitest";
import { loadConfig } from "../src/config.js";

const FULL_ENV = {
  DFNS_AUTH_TOKEN: "t", DFNS_CRED_ID: "c", DFNS_PRIVATE_KEY: "k",
};

describe("loadConfig", () => {
  it("throws when a required DFNS var is missing", () => {
    expect(() => loadConfig({ ...FULL_ENV, DFNS_AUTH_TOKEN: undefined }))
      .toThrow(/DFNS_AUTH_TOKEN/);
  });

  it("applies public defaults for network endpoints", () => {
    const cfg = loadConfig(FULL_ENV);
    expect(cfg.rpcUrl).toBe("https://soroban-testnet.stellar.org");
    expect(cfg.network).toBe("StellarTestnet");
  });
});
