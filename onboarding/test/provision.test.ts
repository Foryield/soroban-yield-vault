import { describe, it, expect, vi } from "vitest";
import { provisionWallet, fundWithFriendbot } from "../src/provision.js";

describe("provisionWallet", () => {
  it("creates a StellarTestnet wallet and returns id + address", async () => {
    const client = { wallets: { createWallet: vi.fn().mockResolvedValue({ id: "wa-1", address: "GABC" }) } };
    const result = await provisionWallet(client as never, "user@example.com");
    expect(client.wallets.createWallet).toHaveBeenCalledWith({
      body: { network: "StellarTestnet", name: "user@example.com" },
    });
    expect(result).toEqual({ walletId: "wa-1", address: "GABC" });
  });

  it("fails loudly when DFNS returns no address", async () => {
    const client = { wallets: { createWallet: vi.fn().mockResolvedValue({ id: "wa-1" }) } };
    await expect(provisionWallet(client as never, "user@example.com")).rejects.toThrow(/address/i);
  });
});

describe("fundWithFriendbot", () => {
  it("fails loudly when friendbot rejects", async () => {
    const fetchImpl = vi.fn().mockResolvedValue({ ok: false, status: 400 });
    await expect(fundWithFriendbot("GABC", fetchImpl as never)).rejects.toThrow(/friendbot/i);
  });

  it("resolves when friendbot accepts", async () => {
    const fetchImpl = vi.fn().mockResolvedValue({ ok: true, status: 200 });
    await expect(fundWithFriendbot("GABC", fetchImpl as never)).resolves.toBeUndefined();
    expect(fetchImpl).toHaveBeenCalledWith("https://friendbot.stellar.org/?addr=GABC");
  });
});
