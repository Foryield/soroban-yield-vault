export type Config = {
  dfnsApiUrl: string;
  dfnsAuthToken: string;
  dfnsCredId: string;
  dfnsPrivateKey: string;
  rpcUrl: string;
  horizonUrl: string;
  vaultContractId: string;
  network: "StellarTestnet";
};

type Env = Record<string, string | undefined>;

function required(env: Env, key: string): string {
  const value = env[key];
  if (!value) throw new Error(`Missing required env var: ${key}`);
  return value;
}

export function loadConfig(env: Env = process.env): Config {
  return {
    dfnsApiUrl: env.DFNS_API_URL ?? "https://api.dfns.io",
    dfnsAuthToken: required(env, "DFNS_AUTH_TOKEN"),
    dfnsCredId: required(env, "DFNS_CRED_ID"),
    dfnsPrivateKey: required(env, "DFNS_PRIVATE_KEY"),
    rpcUrl: env.STELLAR_RPC_URL ?? "https://soroban-testnet.stellar.org",
    horizonUrl: env.STELLAR_HORIZON_URL ?? "https://horizon-testnet.stellar.org",
    vaultContractId: env.VAULT_CONTRACT_ID ?? "CCKW7NFKDCOTOVUODLJ6K734ZEYT4TZLQGLIVFZZR6DLUHO6UOTENWQ6",
    network: "StellarTestnet",
  };
}
