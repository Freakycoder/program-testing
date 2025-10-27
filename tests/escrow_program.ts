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
      .initializeFeeAccount().accounts({systemProgram : anchor.web3.SystemProgram.programId, admin : provider.wallet.publicKey})
      .rpc();

    console.log("Your transaction signature", tx);

  });
});
