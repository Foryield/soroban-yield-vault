// Ingestion des events de conformite (D6c, plan docs/plans/2026-10-23-d6c-
// dashboard.md, section 3).
//
// Le RPC ne garde les events qu'environ 7 jours ; MiCA impose 5 ans. Ce script
// les recopie dans une archive JSON par contrat, en ajout seul, que la tache
// quotidienne commite sur la branche `data/events` et que le dashboard lit.
//
// Chaque event garde sa forme brute (XDR base64, la seule qui ne depende
// d'aucun decodeur) et son decodage generique, pour qu'un systeme tiers
// l'exploite sans le SDK Stellar. Identifiant unique : l'id RPC de l'event
// (ledger, transaction, operation, index), sur le modele de `VaultEvent` de
// foryield-v2 (unicite par transaction et index).
//
// Un ledger sorti de la fenetre du RPC avant d'avoir ete lu est consigne dans
// `missed` : l'archive dit ce qu'elle n'a pas pu voir, plutot que de le taire.
//
// Usage : node scripts/ingest-events.mjs <dossier_archive> [contrat...]
//   sans contrat : ceux de l'allocateur D5 et de l'instance D6b, testnet.
//   RPC_URL surcharge le RPC testnet public.

import { mkdir, readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { rpc, scValToNative, xdr } from "@stellar/stellar-sdk";
import { fetchContractEvents } from "../lib/rpc-events.mjs";

const RPC_URL = process.env.RPC_URL || "https://soroban-testnet.stellar.org";
const NETWORK = "testnet";

// Allocateur D5 testnet : le vault et ses deux strategies (prix de reference
// de la strategie Aquarius, depots et retraits vus par chaque strategie) ;
// instance EURC a frais D6b.
const DEFAULT_CONTRACTS = [
  "CBESWJ4CXLWEFU33HEEMJFT4SAYV5PW6LPEBAYRQBAPKIGRFSVBWXVOV",
  "CCDZZ4NXLWFXWIJIFSOG37K3NIMITZPMUJAHLELRSHO7XO5VA5ZYDEV5",
  "CACZE525I6OY7ASL2UUSHCWY45QA4PXZ3BB3ZEUT6UUUYCFHDVQESEDF",
  "CCXPS66KJHVMXRFHYXVPGOJKPH3LWBIAA4WVNV6BYJXFYKKEBCCLYVMP",
];

const [outDir, ...args] = process.argv.slice(2);
if (!outDir) {
  console.error("usage: node scripts/ingest-events.mjs <archive_dir> [contract...]");
  process.exit(2);
}
const contracts = args.length > 0 ? args : DEFAULT_CONTRACTS;

// Decodage generique en JSON : les i128 et u64 deviennent des chaines, seule
// forme exacte au-dela de 2^53.
function decode(base64) {
  return JSON.parse(
    JSON.stringify(scValToNative(xdr.ScVal.fromXDR(base64, "base64")), (_, v) =>
      typeof v === "bigint" ? v.toString() : v,
    ),
  );
}

async function loadArchive(path, contract) {
  try {
    return JSON.parse(await readFile(path, "utf8"));
  } catch (e) {
    if (e.code !== "ENOENT") throw e;
    return { contract, network: NETWORK, lastLedger: 0, missed: [], events: [] };
  }
}

async function ingest(server, contract) {
  const path = join(outDir, `${contract}.json`);
  const archive = await loadArchive(path, contract);
  const from = archive.lastLedger + 1;
  const page = await fetchContractEvents(server, [contract], from);

  // Premiere ingestion (lastLedger 0) : rien n'a ete manque, on part du plus
  // ancien ledger disponible. Ensuite, tout ledger saute est consigne.
  if (archive.lastLedger > 0 && page.startLedger > from) {
    archive.missed.push({ fromLedger: from, toLedger: page.startLedger - 1 });
  }

  const known = new Set(archive.events.map((e) => e.id));
  const fresh = page.events
    .filter((e) => !known.has(e.id))
    .map((e) => ({
      ...e,
      decoded: { topics: e.topics.map(decode), value: decode(e.value) },
    }));
  archive.events.push(...fresh);
  archive.events.sort((a, b) => (a.id < b.id ? -1 : a.id > b.id ? 1 : 0));
  archive.lastLedger = Math.max(archive.lastLedger, page.latestLedger);

  await writeFile(path, `${JSON.stringify(archive, null, 2)}\n`);
  console.log(
    `${contract}: +${fresh.length} events (${archive.events.length} in archive), ledgers ${page.startLedger}-${page.latestLedger}`,
  );
}

// Un contrat en echec n'empeche pas d'ecrire les autres : la tache commite
// ce qui a ete lu, puis echoue pour qu'on le voie. Sans cela, des echecs
// repetes sur un seul contrat feraient sortir les autres de la fenetre du RPC.
await mkdir(outDir, { recursive: true });
const server = new rpc.Server(RPC_URL);
let failed = 0;
for (const contract of contracts) {
  try {
    await ingest(server, contract);
  } catch (e) {
    failed += 1;
    console.error(`${contract}: ingestion failed`, e);
  }
}
if (failed > 0) process.exit(1);
