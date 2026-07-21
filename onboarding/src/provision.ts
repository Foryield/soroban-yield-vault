import type { DfnsApiClient } from "@dfns/sdk";

export type ProvisionedWallet = { walletId: string; address: string };

export async function provisionWallet(client: DfnsApiClient, name: string): Promise<ProvisionedWallet> {
  const wallet = await client.wallets.createWallet({
    body: { network: "StellarTestnet", name },
  });
  // The SDK types address as optional; a wallet without one is unusable downstream.
  if (!wallet.address) throw new Error(`DFNS returned wallet ${wallet.id} without an address`);
  return { walletId: wallet.id, address: wallet.address };
}

// Testnet only: Friendbot creates and funds the on-chain account (10k test XLM).
export async function fundWithFriendbot(address: string, fetchImpl: typeof fetch = fetch): Promise<void> {
  const response = await fetchImpl(`https://friendbot.stellar.org/?addr=${encodeURIComponent(address)}`);
  if (!response.ok) throw new Error(`friendbot funding failed for ${address}: HTTP ${response.status}`);
}
