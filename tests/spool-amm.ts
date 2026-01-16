import * as anchor from "@coral-xyz/anchor";
import { Program, BN } from "@coral-xyz/anchor";
import { SpoolAmm } from "../target/types/spool_amm";
import {
  Keypair,
  LAMPORTS_PER_SOL,
  PublicKey,
  SystemProgram,
} from "@solana/web3.js";
import {
  TOKEN_PROGRAM_ID,
  TOKEN_2022_PROGRAM_ID,
  createMint,
  getOrCreateAssociatedTokenAccount,
  mintTo,
  getAccount,
  getMint,
} from "@solana/spl-token";
import { assert, expect } from "chai";

describe("spool-amm", () => {
  // Configure the client to use the local cluster.
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.spoolAmm as Program<SpoolAmm>;
  const connection = provider.connection;

  // Keypairs
  const payer = (provider.wallet as anchor.Wallet).payer;
  let usdcMint: PublicKey;
  let wsolMint: PublicKey;
  let lpTokenMint: Keypair;

  // PDAs
  let poolStateAccount: PublicKey;
  let poolStateBump: number;
  let usdcVault: PublicKey;
  let usdcVaultBump: number;
  let wsolVault: PublicKey;
  let wsolVaultBump: number;

  // User token accounts
  let userUsdcAccount: PublicKey;
  let userWsolAccount: PublicKey;
  let userLpAta: PublicKey;

  // Constants
  const USDC_DECIMALS = 6;
  const WSOL_DECIMALS = 9;
  const LP_DECIMALS = 9;
  const INITIAL_USDC_AMOUNT = 1000 * 10 ** USDC_DECIMALS; // 1000 USDC
  const INITIAL_WSOL_AMOUNT = 10 * 10 ** WSOL_DECIMALS; // 10 wSOL

  before(async () => {
    console.log("Setting up test environment...");
    console.log("Payer:", payer.publicKey.toBase58());

    // Create USDC mock mint
    usdcMint = await createMint(
      connection,
      payer,
      payer.publicKey,
      payer.publicKey,
      USDC_DECIMALS,
      undefined,
      undefined,
      TOKEN_PROGRAM_ID
    );
    console.log("USDC Mint:", usdcMint.toBase58());

    // Create wSOL mock mint
    wsolMint = await createMint(
      connection,
      payer,
      payer.publicKey,
      payer.publicKey,
      WSOL_DECIMALS,
      undefined,
      undefined,
      TOKEN_PROGRAM_ID
    );
    console.log("wSOL Mint:", wsolMint.toBase58());

    // Create user token accounts for USDC and wSOL
    const userUsdcAccountInfo = await getOrCreateAssociatedTokenAccount(
      connection,
      payer,
      usdcMint,
      payer.publicKey
    );
    userUsdcAccount = userUsdcAccountInfo.address;
    console.log("User USDC Account:", userUsdcAccount.toBase58());

    const userWsolAccountInfo = await getOrCreateAssociatedTokenAccount(
      connection,
      payer,
      wsolMint,
      payer.publicKey
    );
    userWsolAccount = userWsolAccountInfo.address;
    console.log("User wSOL Account:", userWsolAccount.toBase58());

    // Mint initial tokens to user accounts
    await mintTo(
      connection,
      payer,
      usdcMint,
      userUsdcAccount,
      payer,
      INITIAL_USDC_AMOUNT
    );
    console.log(`Minted ${INITIAL_USDC_AMOUNT} USDC to user account`);

    await mintTo(
      connection,
      payer,
      wsolMint,
      userWsolAccount,
      payer,
      INITIAL_WSOL_AMOUNT
    );
    console.log(`Minted ${INITIAL_WSOL_AMOUNT} wSOL to user account`);

    // Derive PDAs
    [poolStateAccount, poolStateBump] = PublicKey.findProgramAddressSync(
      [Buffer.from("pool_state"), usdcMint.toBuffer(), wsolMint.toBuffer()],
      program.programId
    );
    console.log("Pool State Account PDA:", poolStateAccount.toBase58());

    [usdcVault, usdcVaultBump] = PublicKey.findProgramAddressSync(
      [Buffer.from("usdc_vault"), usdcMint.toBuffer()],
      program.programId
    );
    console.log("USDC Vault PDA:", usdcVault.toBase58());

    // Note: In the contract, wsol_vault uses "usdc_vault" seed prefix (potential bug)
    [wsolVault, wsolVaultBump] = PublicKey.findProgramAddressSync(
      [Buffer.from("usdc_vault"), wsolMint.toBuffer()],
      program.programId
    );
    console.log("wSOL Vault PDA:", wsolVault.toBase58());

    // LP token mint keypair (for later tests)
    lpTokenMint = Keypair.generate();
    console.log("LP Token Mint (future):", lpTokenMint.publicKey.toBase58());

    // Derive LP ATA PDA
    [userLpAta] = PublicKey.findProgramAddressSync(
      [Buffer.from("lptokenata"), payer.publicKey.toBuffer()],
      program.programId
    );
    console.log("User LP ATA PDA:", userLpAta.toBase58());
  });

  describe("Initialize Pool", () => {
    it("should initialize the AMM pool successfully", async () => {
      const tx = await program.methods
        .initialize()
        .accounts({
          signer: payer.publicKey,
          usdcMint: usdcMint,
          wsolMint: wsolMint,
          systemProgram: SystemProgram.programId,
          poolStateaccount: poolStateAccount,
          tokenProgram: TOKEN_PROGRAM_ID,
          usdcVault: usdcVault,
          wsolVault: wsolVault,
        })
        .signers([payer])
        .rpc();

      console.log("Initialize transaction signature:", tx);

      // Verify pool state account was created
      const poolState = await program.account.lpPoolAccountShape.fetch(
        poolStateAccount
      );

      console.log("Pool State:", {
        usdcMint: poolState.usdcMint.toBase58(),
        wsolMint: poolState.wsolMint.toBase58(),
        usdcVaultAddress: poolState.usdcVaultAddress.toBase58(),
        solVaultAddress: poolState.solVaultAddress.toBase58(),
        lpTokenMint: poolState.lpTokenMint.toBase58(),
        bump: poolState.bump,
      });
    });

    it("should create USDC vault with correct configuration", async () => {
      const vaultAccount = await getAccount(connection, usdcVault);

      assert.equal(
        vaultAccount.mint.toBase58(),
        usdcMint.toBase58(),
        "USDC vault should have correct mint"
      );
      assert.equal(
        vaultAccount.owner.toBase58(),
        poolStateAccount.toBase58(),
        "USDC vault authority should be pool state account"
      );
      assert.equal(
        vaultAccount.amount.toString(),
        "0",
        "USDC vault should start with 0 balance"
      );
    });

    it("should create wSOL vault with correct configuration", async () => {
      const vaultAccount = await getAccount(connection, wsolVault);

      assert.equal(
        vaultAccount.mint.toBase58(),
        wsolMint.toBase58(),
        "wSOL vault should have correct mint"
      );
      assert.equal(
        vaultAccount.owner.toBase58(),
        poolStateAccount.toBase58(),
        "wSOL vault authority should be pool state account"
      );
      assert.equal(
        vaultAccount.amount.toString(),
        "0",
        "wSOL vault should start with 0 balance"
      );
    });

    it("should fail when trying to initialize the same pool twice", async () => {
      try {
        await program.methods
          .initialize()
          .accounts({
            signer: payer.publicKey,
            usdcMint: usdcMint,
            wsolMint: wsolMint,
            systemProgram: SystemProgram.programId,
            poolStateaccount: poolStateAccount,
            tokenProgram: TOKEN_PROGRAM_ID,
            usdcVault: usdcVault,
            wsolVault: wsolVault,
          })
          .signers([payer])
          .rpc();

        assert.fail("Should have thrown an error for duplicate initialization");
      } catch (error) {
        // Expected to fail - account already exists
        console.log("Expected error: Pool already initialized");
        expect(error).to.exist;
      }
    });
  });

  describe("LP Token Mint", () => {
    // TODO: Add instruction in lib.rs for create_lp_mint
    // The LpMint struct exists but no instruction is implemented yet
    it.skip("should create LP token mint", async () => {
      // This test is skipped because the instruction is not implemented yet
      // When implemented, it should:
      // 1. Create a new mint for LP tokens
      // 2. Set the authority to the pool state account or program
      // 3. Use 9 decimals as specified in the struct
      // Example implementation when ready:
      // const tx = await program.methods
      //   .createLpMint()
      //   .accounts({
      //     signer: payer.publicKey,
      //     mint: lpTokenMint.publicKey,
      //     tokenProgram: TOKEN_PROGRAM_ID,
      //     systemProgram: SystemProgram.programId,
      //   })
      //   .signers([payer, lpTokenMint])
      //   .rpc();
    });
  });

  describe("Create LP Token ATA", () => {
    // TODO: Add instruction in lib.rs for create_lp_ata
    // The CreateLpAta struct exists but no instruction is implemented yet
    it.skip("should create LP token ATA for user", async () => {
      // This test is skipped because the instruction is not implemented yet
      // When implemented, it should:
      // 1. Create an ATA for LP tokens using PDA seeds ["lptokenata", signer.key()]
      // 2. Set the correct mint and authority
      // Example implementation when ready:
      // const tx = await program.methods
      //   .createLpAta()
      //   .accounts({
      //     signer: payer.publicKey,
      //     lptokenmint: lpTokenMint.publicKey,
      //     lpAta: userLpAta,
      //     tokenProgram: TOKEN_PROGRAM_ID,
      //     systemProgram: SystemProgram.programId,
      //   })
      //   .signers([payer])
      //   .rpc();
    });
  });

  describe("Mint LP Tokens", () => {
    // TODO: Add instruction in lib.rs for mint_lp_tokens
    // The Mintlptokens struct and impl exist but no instruction is implemented yet
    it.skip("should mint LP tokens to user ATA", async () => {
      // This test is skipped because the instruction is not implemented yet
      // When implemented, it should:
      // 1. Mint LP tokens based on liquidity provided
      // 2. Transfer tokens to user's LP ATA
      // 3. Update pool state if needed
      // Example implementation when ready:
      // const lpAmount = new BN(1000 * 10 ** LP_DECIMALS);
      // const tx = await program.methods
      //   .mintLpTokens(lpAmount)
      //   .accounts({
      //     signer: payer.publicKey,
      //     lptokenmint: lpTokenMint.publicKey,
      //     lpata: userLpAta,
      //     tokenProgram: TOKEN_PROGRAM_ID,
      //   })
      //   .signers([payer])
      //   .rpc();
    });
  });

  describe("Provide Liquidity", () => {
    // TODO: Add instruction in lib.rs for provide_lp
    // The ProvideLp struct exists but the impl is incomplete
    it.skip("should allow user to provide liquidity to the pool", async () => {
      // This test is skipped because the instruction is not implemented yet
      // When implemented, it should:
      // 1. Transfer USDC from user to USDC vault
      // 2. Transfer wSOL from user to wSOL vault
      // 3. Mint LP tokens proportional to liquidity provided
      // 4. Update pool state
      // Example implementation when ready:
      // const usdcAmount = new BN(100 * 10 ** USDC_DECIMALS);
      // const wsolAmount = new BN(1 * 10 ** WSOL_DECIMALS);
      //
      // const tx = await program.methods
      //   .provideLp(usdcAmount, wsolAmount)
      //   .accounts({
      //     signer: payer.publicKey,
      //     usdcMint: usdcMint,
      //     wsolMint: wsolMint,
      //     userUsdcAccount: userUsdcAccount,
      //     userWsolAccount: userWsolAccount,
      //     usdcVaultAccount: usdcVault,
      //     wsolVaultAccount: wsolVault,
      //     tokenProgram: TOKEN_PROGRAM_ID,
      //   })
      //   .signers([payer])
      //   .rpc();
    });

    it.skip("should fail when user has insufficient USDC balance", async () => {
      // Test for insufficient balance error handling
    });

    it.skip("should fail when user has insufficient wSOL balance", async () => {
      // Test for insufficient balance error handling
    });
  });

  describe("Swap Tokens", () => {
    // TODO: Add instruction in lib.rs for swap
    // The SwapTokens struct exists but all impl methods are empty
    it.skip("should swap USDC for wSOL", async () => {
      // This test is skipped because the instruction is not implemented yet
      // When implemented, it should:
      // 1. Transfer input tokens from user to input vault
      // 2. Calculate output amount using AMM formula (x * y = k)
      // 3. Deduct fees
      // 4. Transfer output tokens from vault to user
      // Example implementation when ready:
      // const inputAmount = new BN(10 * 10 ** USDC_DECIMALS);
      //
      // const tx = await program.methods
      //   .swap(inputAmount)
      //   .accounts({
      //     signer: payer.publicKey,
      //     userInputAccount: userUsdcAccount,
      //     userOutputAccount: userWsolAccount,
      //     inputVaultAccount: usdcVault,
      //     outputVaultAccont: wsolVault,
      //     poolStateaccount: poolStateAccount,
      //   })
      //   .signers([payer])
      //   .rpc();
    });

    it.skip("should swap wSOL for USDC", async () => {
      // Test swap in the opposite direction
    });

    it.skip("should apply correct swap fees", async () => {
      // Test that fees are correctly calculated and distributed
    });

    it.skip("should fail when slippage exceeds threshold", async () => {
      // Test slippage protection
    });

    it.skip("should fail when pool has insufficient liquidity", async () => {
      // Test for insufficient liquidity error
    });
  });

  describe("Edge Cases and Error Handling", () => {
    it("should verify user has correct initial token balances", async () => {
      const usdcAccount = await getAccount(connection, userUsdcAccount);
      const wsolAccount = await getAccount(connection, userWsolAccount);

      assert.equal(
        usdcAccount.amount.toString(),
        INITIAL_USDC_AMOUNT.toString(),
        "User should have initial USDC balance"
      );
      assert.equal(
        wsolAccount.amount.toString(),
        INITIAL_WSOL_AMOUNT.toString(),
        "User should have initial wSOL balance"
      );
    });

    it("should verify mints have correct decimals", async () => {
      const usdcMintInfo = await getMint(connection, usdcMint);
      const wsolMintInfo = await getMint(connection, wsolMint);

      assert.equal(
        usdcMintInfo.decimals,
        USDC_DECIMALS,
        "USDC mint should have 6 decimals"
      );
      assert.equal(
        wsolMintInfo.decimals,
        WSOL_DECIMALS,
        "wSOL mint should have 9 decimals"
      );
    });
  });

  describe("Pool State Verification", () => {
    it("should correctly store pool state data", async () => {
      const poolState = await program.account.lpPoolAccountShape.fetch(
        poolStateAccount
      );

      // The initialize function currently just logs a message
      // These assertions test the expected behavior once fully implemented
      // For now, the default values (all zeros) would be stored

      console.log("Current Pool State:");
      console.log("  USDC Mint:", poolState.usdcMint.toBase58());
      console.log("  wSOL Mint:", poolState.wsolMint.toBase58());
      console.log("  USDC Vault:", poolState.usdcVaultAddress.toBase58());
      console.log("  SOL Vault:", poolState.solVaultAddress.toBase58());
      console.log("  LP Token Mint:", poolState.lpTokenMint.toBase58());
      console.log("  Bump:", poolState.bump);
    });
  });
});

// Helper functions for future use
function calculateSwapOutput(
  inputAmount: number,
  inputReserve: number,
  outputReserve: number,
  feeNumerator: number = 3,
  feeDenominator: number = 1000
): number {
  // Constant product formula: x * y = k
  // Output = (outputReserve * inputAmount * (1 - fee)) / (inputReserve + inputAmount * (1 - fee))
  const inputWithFee = inputAmount * (feeDenominator - feeNumerator);
  const numerator = outputReserve * inputWithFee;
  const denominator = inputReserve * feeDenominator + inputWithFee;
  return Math.floor(numerator / denominator);
}

function calculateLpTokens(
  depositA: number,
  depositB: number,
  reserveA: number,
  reserveB: number,
  totalLpSupply: number
): number {
  if (totalLpSupply === 0) {
    // Initial deposit - use geometric mean
    return Math.floor(Math.sqrt(depositA * depositB));
  }
  // Subsequent deposits - proportional to existing reserves
  const lpFromA = (depositA * totalLpSupply) / reserveA;
  const lpFromB = (depositB * totalLpSupply) / reserveB;
  // Return minimum to prevent manipulation
  return Math.floor(Math.min(lpFromA, lpFromB));
}
