import { loadConfig } from "../config.js";
import { dfnsClient } from "../dfns.js";
import { provisionWallet, fundWithFriendbot, type ProvisionedWallet } from "../provision.js";

const name = process.argv[2];
if (!name) {
  console.error("Usage: npm run provision -- <wallet-name>");
  process.exit(1);
}

let provisioned: ProvisionedWallet | undefined;
try {
  const cfg = loadConfig();
  const client = dfnsClient(cfg);
  provisioned = await provisionWallet(client, name);
  await fundWithFriendbot(provisioned.address);
} catch (error) {
  console.error(error instanceof Error ? error.message : String(error));
  if (provisioned) {
    console.error(
      `Wallet already provisioned: walletId=${provisioned.walletId} address=${provisioned.address} (retry funding, no new wallet needed)`,
    );
  }
  process.exit(1);
}
console.log(JSON.stringify(provisioned));
