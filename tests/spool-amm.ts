import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { SpoolAmm } from "../target/types/spool_amm";
import { LAMPORTS_PER_SOL, PublicKey, SystemProgram, Transaction, Keypair } from "@solana/web3.js";
import {
  getMint,
  TOKEN_PROGRAM_ID,
  getAccount,
  createAssociatedTokenAccountInstruction,
  getAssociatedTokenAddress,
  createSyncNativeInstruction,
  getOrCreateAssociatedTokenAccount
} from "@solana/spl-token"
import { assert } from "chai";
import { BN } from "bn.js";
import { base58 } from "@scure/base";

describe("spool-amm", () => {
  // Configure the client to use the local cluster.
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.spoolAmm as Program<SpoolAmm>;

  // Keypair for user
  const user_keypair = anchor.web3.Keypair.fromSecretKey(base58.decode("3cJruu3vu3ym1U6dxuYivwyMqrPBnWJFtap26DSMqP56DjjTRpF8mpPMKJgep7o8v9sLhzJCdDLW9vU1PW1r9X9P"));

  // USDC and SOL mint addresses
  const usdc_mint_address = "Gh9ZwEmdLJ8DscKNTkTqPbNwLNNBjuSzaG9Vp2KGtKJr"
  const sol_mint_address = "So11111111111111111111111111111111111111112"

  const usdcMintPubkey = new PublicKey(usdc_mint_address)
  const wsolMintPubkey = new PublicKey(sol_mint_address)

  // LP mint keypair - generated for initialization
  const lpMintKeypair = Keypair.generate();

  // Correct seeds matching lib.rs
  const usdc_vault_seed = [Buffer.from("usdc_vault"), usdcMintPubkey.toBuffer()];
  const wsol_vault_seed = [Buffer.from("usdc_vault"), wsolMintPubkey.toBuffer()]; // Note: program uses same seed prefix
  const pool_state_seed = [Buffer.from("pool_state"), usdcMintPubkey.toBuffer(), wsolMintPubkey.toBuffer()];

  // Find PDAs
  const [usdcVaultPda] = PublicKey.findProgramAddressSync(usdc_vault_seed, program.programId);
  const [wsolVaultPda] = PublicKey.findProgramAddressSync(wsol_vault_seed, program.programId);
  const [poolStatePda] = PublicKey.findProgramAddressSync(pool_state_seed, program.programId);

  // Helper function to get or create ATA
  const getOrCreateATA = async (mint: PublicKey, owner: PublicKey, isWrappedSol = false, solAmount = 0) => {
    const ata = await getAssociatedTokenAddress(mint, owner);
    const tx = new Transaction();
    let shouldSend = false;

    // Check if account exists
    const info = await provider.connection.getAccountInfo(ata);

    if (!info) {
      console.log(`Creating ATA for ${mint.toString().slice(0, 8)}...`);
      tx.add(createAssociatedTokenAccountInstruction(user_keypair.publicKey, ata, owner, mint));
      shouldSend = true;
    }

    // If it's Wrapped SOL, transfer SOL and sync
    if (isWrappedSol && solAmount > 0) {
      console.log(`Wrapping ${solAmount} SOL...`);
      tx.add(
        SystemProgram.transfer({
          fromPubkey: user_keypair.publicKey,
          toPubkey: ata,
          lamports: solAmount * LAMPORTS_PER_SOL
        }),
        createSyncNativeInstruction(ata)
      );
      shouldSend = true;
    }

    if (shouldSend) {
      await provider.sendAndConfirm(tx, [user_keypair]);
    }
    return ata;
  };

  it("Initialize pool", async () => {
    // Check if pool already exists
    const accountInfo = await provider.connection.getAccountInfo(poolStatePda);
    if (accountInfo !== null) {
      console.log("⚠️ Pool already initialized. Skipping init.");
      return;
    }

    console.log("Initializing pool...");
    console.log("Pool State PDA:", poolStatePda.toString());
    console.log("USDC Vault PDA:", usdcVaultPda.toString());
    console.log("WSOL Vault PDA:", wsolVaultPda.toString());
    console.log("LP Mint:", lpMintKeypair.publicKey.toString());

    const tx = await program.methods.initialize()
      .accounts({
        signer: user_keypair.publicKey,
        usdcMint: usdcMintPubkey,
        wsolMint: wsolMintPubkey,
        mint: lpMintKeypair.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .signers([user_keypair, lpMintKeypair])
      .rpc({ commitment: "confirmed" });

    console.log("✅ Initialize tx:", tx);

    // Verify accounts were created
    const usdcVaultAccount = await getAccount(provider.connection, usdcVaultPda, "confirmed", TOKEN_PROGRAM_ID);
    const wsolVaultAccount = await getAccount(provider.connection, wsolVaultPda, "confirmed", TOKEN_PROGRAM_ID);
    const poolStateAccount = await program.account.lpPoolAccountShape.fetch(poolStatePda);

    assert.ok(usdcVaultAccount, "USDC vault should exist");
    assert.ok(wsolVaultAccount, "WSOL vault should exist");
    assert.equal(poolStateAccount.usdcMint.toString(), usdcMintPubkey.toString());
    assert.equal(poolStateAccount.wsolMint.toString(), wsolMintPubkey.toString());

    console.log("✅ Pool initialized successfully!");
  });

  it("Provide liquidity", async () => {
    // Fetch pool state to get LP mint
    const poolStateAccount = await program.account.lpPoolAccountShape.fetch(poolStatePda);
    const lpMintPubkey = poolStateAccount.lpTokenMint;

    // Get user token accounts
    const userUsdcAccount = await getAssociatedTokenAddress(usdcMintPubkey, user_keypair.publicKey);
    const userWsolAccount = await getOrCreateATA(wsolMintPubkey, user_keypair.publicKey, true, 1);

    // LP ATA seed
    const lpAtaSeed = [Buffer.from("lptokenata"), user_keypair.publicKey.toBuffer()];
    const [lpAtaPda] = PublicKey.findProgramAddressSync(lpAtaSeed, program.programId);

    // Amounts to provide
    const usdcAmount = new BN(100).mul(new BN(10).pow(new BN(6))); // 100 USDC (6 decimals)
    const wsolAmount = new BN(1).mul(new BN(10).pow(new BN(9))); // 1 SOL (9 decimals)

    console.log("Providing liquidity...");
    console.log("USDC amount:", usdcAmount.toString());
    console.log("WSOL amount:", wsolAmount.toString());

    try {
      const tx = await program.methods.providelp(wsolAmount, usdcAmount)
        .accountsPartial({
          signer: user_keypair.publicKey,
          usdcMint: usdcMintPubkey,
          wsolMint: wsolMintPubkey,
          userUsdcAccount: userUsdcAccount,
          userWsolAccount: userWsolAccount,
          usdcVaultAccount: usdcVaultPda,
          wsolVaultAccount: wsolVaultPda,
          tokenProgram: TOKEN_PROGRAM_ID,
          lptokenmint: lpMintPubkey,
          lpAta: lpAtaPda,
          mintAuthority: poolStatePda,
        })
        .signers([user_keypair])
        .rpc({ commitment: "confirmed" });

      console.log("✅ Provide LP tx:", tx);

      // Verify LP tokens were minted
      const lpAccount = await getAccount(provider.connection, lpAtaPda, "confirmed");
      console.log("LP token balance:", lpAccount.amount.toString());
      assert.ok(lpAccount.amount > 0, "Should have received LP tokens");
    } catch (error) {
      console.log("❌ Failed to provide liquidity:", error);
      throw error;
    }
  });

  it("Swap USDC to wSOL", async () => {
    // Get user token accounts
    const userUsdcAccount = await getAssociatedTokenAddress(usdcMintPubkey, user_keypair.publicKey);
    const userWsolAccount = await getOrCreateATA(wsolMintPubkey, user_keypair.publicKey, false, 0);

    // Check initial balances
    const initialUsdcBalance = await getAccount(provider.connection, userUsdcAccount, "confirmed");
    console.log("Initial USDC balance:", initialUsdcBalance.amount.toString());

    // Amount to swap
    const amountToSwap = new BN(10).mul(new BN(10).pow(new BN(6))); // 10 USDC

    console.log("Swapping", amountToSwap.toString(), "USDC for wSOL...");

    try {
      const tx = await program.methods.swap(amountToSwap)
        .accountsPartial({
          signer: user_keypair.publicKey,
          inputMint: usdcMintPubkey,
          outputMint: wsolMintPubkey,
          poolStateaccount: poolStatePda,
          inputVaultAccount: usdcVaultPda,
          outputVaultAccount: wsolVaultPda,
          userInputAccount: userUsdcAccount,
          userOutputAccount: userWsolAccount,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .signers([user_keypair])
        .rpc({ commitment: "confirmed" });

      console.log("✅ Swap tx:", tx);

      // Wait for confirmation
      await new Promise(resolve => setTimeout(resolve, 2000));

      // Check final balances
      const finalUsdcBalance = await getAccount(provider.connection, userUsdcAccount, "confirmed");
      const finalWsolBalance = await getAccount(provider.connection, userWsolAccount, "confirmed");

      console.log("Final USDC balance:", finalUsdcBalance.amount.toString());
      console.log("Final wSOL balance:", finalWsolBalance.amount.toString());

      assert.ok(finalWsolBalance.amount > 0, "Should have received wSOL");
    } catch (error) {
      console.log("❌ Failed to swap:", error);
      throw error;
    }
  });

  it("Swap wSOL to USDC", async () => {
    // Get user token accounts
    const userUsdcAccount = await getAssociatedTokenAddress(usdcMintPubkey, user_keypair.publicKey);
    const userWsolAccount = await getAssociatedTokenAddress(wsolMintPubkey, user_keypair.publicKey);

    // Amount to swap
    const amountToSwap = new BN(0.1 * LAMPORTS_PER_SOL); // 0.1 SOL

    console.log("Swapping", amountToSwap.toString(), "wSOL for USDC...");

    try {
      const tx = await program.methods.swap(amountToSwap)
        .accountsPartial({
          signer: user_keypair.publicKey,
          inputMint: wsolMintPubkey,
          outputMint: usdcMintPubkey,
          poolStateaccount: poolStatePda,
          inputVaultAccount: wsolVaultPda,
          outputVaultAccount: usdcVaultPda,
          userInputAccount: userWsolAccount,
          userOutputAccount: userUsdcAccount,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .signers([user_keypair])
        .rpc({ commitment: "confirmed" });

      console.log("✅ Swap tx:", tx);
    } catch (error) {
      console.log("❌ Failed to swap:", error);
      throw error;
    }
  });

  it("Remove liquidity", async () => {
    // Fetch pool state to get LP mint
    const poolStateAccount = await program.account.lpPoolAccountShape.fetch(poolStatePda);
    const lpMintPubkey = poolStateAccount.lpTokenMint;

    // Get user token accounts
    const userUsdcAccount = await getAssociatedTokenAddress(usdcMintPubkey, user_keypair.publicKey);
    const userWsolAccount = await getAssociatedTokenAddress(wsolMintPubkey, user_keypair.publicKey);

    // LP ATA seed
    const lpAtaSeed = [Buffer.from("lptokenata"), user_keypair.publicKey.toBuffer()];
    const [lpAtaPda] = PublicKey.findProgramAddressSync(lpAtaSeed, program.programId);

    // Check LP balance
    let lpBalance;
    try {
      const lpAccount = await getAccount(provider.connection, lpAtaPda, "confirmed");
      lpBalance = lpAccount.amount;
      console.log("LP token balance before burn:", lpBalance.toString());
    } catch {
      console.log("⚠️ No LP tokens to burn. Skipping remove liquidity test.");
      return;
    }

    if (Number(lpBalance) <= 0) {
      console.log("⚠️ No LP tokens to burn. Skipping remove liquidity test.");
      return;
    }

    // Amount to burn (burn half of LP tokens)
    const burnAmount = new BN((Number(lpBalance) / 2).toString());

    console.log("Removing liquidity, burning", burnAmount.toString(), "LP tokens...");

    try {
      const tx = await program.methods.removeLiquidity(burnAmount)
        .accounts({
          signer: user_keypair.publicKey,
          usdcMint: usdcMintPubkey,
          wsolMint: wsolMintPubkey,
          userUsdcAccount: userUsdcAccount,
          userWsolAccount: userWsolAccount,
          usdcVaultAccount: usdcVaultPda,
          wsolVaultAccount: wsolVaultPda,
          tokenProgram: TOKEN_PROGRAM_ID,
          poolStateAccount: poolStatePda,
          lpMint: lpMintPubkey,
          userLpAta: lpAtaPda,
        })
        .signers([user_keypair])
        .rpc({ commitment: "confirmed" });

      console.log("✅ Remove liquidity tx:", tx);

      // Verify LP tokens were burned
      const lpAccountAfter = await getAccount(provider.connection, lpAtaPda, "confirmed");
      console.log("LP token balance after burn:", lpAccountAfter.amount.toString());
      assert.ok(lpAccountAfter.amount < lpBalance, "LP tokens should be burned");
    } catch (error) {
      console.log("❌ Failed to remove liquidity:", error);
      throw error;
    }
  });
});
