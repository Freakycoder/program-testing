import fs from "fs"
import path from "path"
import { Keypair } from "@solana/web3.js";

const keypairPath = path.resolve(
    process.env.HOME,
    ".config",
    "solana",
    "id.json"
)

const secretkeystring = fs.readFileSync(keypairPath, "utf-8");
const secretkey = Uint8Array.from(JSON.parse(secretkeystring));

export const payer_keypair = Keypair.fromSecretKey(secretkey);