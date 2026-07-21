import { describe, it, expect } from "vitest";
import { loadConfig } from "../src/config.js";

const FULL_ENV = {
  DFNS_AUTH_TOKEN: "t", DFNS_CRED_ID: "c", DFNS_PRIVATE_KEY: "k",
};

describe("loadConfig", () => {
  it.each(["DFNS_AUTH_TOKEN", "DFNS_CRED_ID", "DFNS_PRIVATE_KEY"])(
    "throws when %s is missing",
    (key) => {
      expect(() => loadConfig({ ...FULL_ENV, [key]: undefined }))
        .toThrow(new RegExp(key));
    },
  );

  it("returns an explicitly set env var over the default", () => {
    const cfg = loadConfig({ ...FULL_ENV, STELLAR_RPC_URL: "https://rpc.example.test" });
    expect(cfg.rpcUrl).toBe("https://rpc.example.test");
  });

  it("applies public defaults for network endpoints", () => {
    const cfg = loadConfig(FULL_ENV);
    expect(cfg.rpcUrl).toBe("https://soroban-testnet.stellar.org");
    expect(cfg.network).toBe("StellarTestnet");
  });
});
