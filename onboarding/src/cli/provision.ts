import { loadConfig } from "../config.js";
import { dfnsClient } from "../dfns.js";
import { loadDotenv } from "../env.js";
import { provisionWallet, fundWithFriendbot, type ProvisionedWallet } from "../provision.js";

loadDotenv();

const name = process.argv[2];
if (!name) {
  console.error("Usage: npm run provision -- <wallet-name>");
  process.exit(1);
}

let provisioned: ProvisionedWallet | undefined;
try {
  const cfg = loadConfig();
  const client = dfnsClient(cfg);
  provisioned = await provisionWallet(client, name, cfg.network);
  await fundWithFriendbot(provisioned.address);
} catch (error) {
  console.error(error instanceof Error ? error.message : String(error));
  if (provisioned) {
    console.error(
      `Wallet already provisioned: walletId=${provisioned.walletId} address=${provisioned.address} (no new wallet needed, retry funding: https://friendbot.stellar.org/?addr=${provisioned.address})`,
    );
  }
  process.exit(1);
}
console.log(JSON.stringify(provisioned));
