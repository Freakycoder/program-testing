import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { EscrowProgram } from "../target/types/escrow_program";

describe("escrow_program", () => {
  // Configure the client to use the local cluster.
  anchor.setProvider(anchor.AnchorProvider.env());

  const program = anchor.workspace.escrowProgram as Program<EscrowProgram>;
  const provider = anchor.AnchorProvider.env();

  it("initialize fee account", async () => {

    const [feePda] = anchor.web3.PublicKey.findProgramAddressSync([Buffer.from("fee_account"), provider.wallet.publicKey.toBuffer()], program.programId)
    
    const tx = await program.methods
      .initializeFeeAccount().rpc();

    console.log("Your transaction signature account", tx);
    console.log("fee account PDA : ", feePda.toBase58())
    
    const feeAccountData = await program.account.feeAccount.fetch(feePda);
    console.log("fee acount PDA data : {}", feeAccountData);
  });
});
