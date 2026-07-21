import { loadConfig } from "../config.js";
import { dfnsClient } from "../dfns.js";
import { provisionWallet, fundWithFriendbot } from "../provision.js";

const name = process.argv[2];
if (!name) {
  console.error("Usage: npm run provision -- <wallet-name>");
  process.exit(1);
}

const cfg = loadConfig();
const client = dfnsClient(cfg);
const result = await provisionWallet(client, name);
await fundWithFriendbot(result.address);
console.log(JSON.stringify(result));
