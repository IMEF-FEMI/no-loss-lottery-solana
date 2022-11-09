import * as anchor from "@project-serum/anchor";
import { Program, } from "@project-serum/anchor";
import { NoLossLottery } from "../target/types/no_loss_lottery";
import {
  deriveLotteryInfoPDA,
  deriveVaultSignerPDA,
  sleep,
  mintInfo,
  transferLamports,
  wrapSol,
  rpcWithBalanceChange
} from "../utils/utils";
import * as serumAssoToken from "@project-serum/associated-token";
import { NATIVE_MINT, TOKEN_PROGRAM_ID, } from "@solana/spl-token";
import { LAMPORTS_PER_SOL, SYSVAR_RECENT_BLOCKHASHES_PUBKEY } from "@solana/web3.js";
import {
  AnchorWallet,
  Callback,
  PermissionAccount,
  ProgramStateAccount,
  SwitchboardPermission,
  VrfAccount,
} from "@switchboard-xyz/switchboard-v2";
import { SwitchboardTestContext } from "@switchboard-xyz/sbv2-utils";
import { RawMint } from "@solana/spl-token";
import assert from "assert";
import { getOrCreateAssociatedTokenAccount } from "@solana/spl-token";
import { createCloseAccountInstruction } from "@solana/spl-token";


describe("no_loss_lottery", () => {

  let provider = anchor.AnchorProvider.env();


  anchor.setProvider(provider);
  const program = anchor.workspace.NoLossLottery as Program<NoLossLottery>;
  const programId = program.programId;



  const lendingProgram = new anchor.web3.PublicKey("pdQ2rQQU5zH2rDgZ7xH2azMBJegUzUyunJ5Jd637hC4")
  const lendingMarket = new anchor.web3.PublicKey("H27Quk3DSbu55T4dCr1NddTTSAezXwHU67FPCZVKLhSW")

  const WSOL_PTOKEN_MINT = new anchor.web3.PublicKey("Hk4Rp3kaPssB6hnjah3Mrqpt5CAXWGoqFT5dVsWA3TaM");
  const SOL_RESERVE = new anchor.web3.PublicKey("6FeVStQAGPWvfWijDHF7cTWRCi7He6vTT3ubfNhe9SPt")
  const SOL_RESERVE_LIQUIDITY_SUPPLY = new anchor.web3.PublicKey("AbKeR7nQdHPDddiDQ71YUsz1F138a7cJMfJVtpdYUSvE")
  const SOL_ORACLE = new anchor.web3.PublicKey("J83w4HKfqxwcq3BEMMkPFSppX3gqekLyLJBexebFVkix")

  // let user: anchor.web3.Keypair;
  // let userTokenAccount: anchor.web3.PublicKey;
  let sourceLiquidityVault: anchor.web3.PublicKey;
  let destinationCollateralVault: anchor.web3.PublicKey;
  let vaultSigner: anchor.web3.PublicKey;
  let vaultSignerBump: number;

  let users: anchor.web3.Keypair[] = [];
  let LOTTERY_ENTRY_FEE: number = 1 * LAMPORTS_PER_SOL;


  let sourceLiquidityMintInfo: RawMint;
  let destinationCollateralMintInfo: RawMint;
  let lotteryPDA: anchor.web3.PublicKey;

  // Switchboard VRF
  let switchboard: SwitchboardTestContext;
  const vrfSecret = anchor.web3.Keypair.generate();

  const payer = (provider.wallet as AnchorWallet).payer;

  let [vrfClientKey, vrfClientBump] =
    anchor.web3.PublicKey.findProgramAddressSync(
      [
        Buffer.from("STATE"),
        vrfSecret.publicKey.toBytes(),
        payer.publicKey.toBytes(),
      ],
      programId
    );

  const vrfIxCoder = new anchor.BorshInstructionCoder(program.idl);
  const vrfClientCallback: Callback = {
    programId: programId,
    accounts: [
      // ensure all accounts in updateResult are populated
      { pubkey: vrfClientKey, isSigner: false, isWritable: true },
      { pubkey: vrfSecret.publicKey, isSigner: false, isWritable: false },
    ],
    ixData: vrfIxCoder.encode("updateResult", ""), // pass any params for instruction here
  };

  let lotteryWinner: anchor.web3.PublicKey;


  before(async () => {
    sourceLiquidityMintInfo = await mintInfo(provider, NATIVE_MINT);
    destinationCollateralMintInfo = await mintInfo(provider, WSOL_PTOKEN_MINT);

    lotteryPDA = (await deriveLotteryInfoPDA(programId))[0]
    //generate users and send lamports to them
    for (let i = 0; i < 5; i++) {
      const user = anchor.web3.Keypair.generate()
      try {
        // try to get some airdrop
        await provider.connection.requestAirdrop(
          user.publicKey,
          LAMPORTS_PER_SOL
        )

        await sleep(1000)

      } catch (e) {
        console.log(e);

        // if it fails, send LOTTERY_ENTRY_FEE from provider wallet
        await transferLamports(
          provider,
          user.publicKey,
          LOTTERY_ENTRY_FEE,
        )
      }
      //send additional 0.1 sol from provider wallet for other transaction fees
      await transferLamports(
        provider,
        user.publicKey,
        0.1 * LAMPORTS_PER_SOL,
      )


      try {
        await wrapSol(provider, user, LOTTERY_ENTRY_FEE)
      } catch (error) {

      }
      users.push(user)
    }

    // First, attempt to load the switchboard devnet PID
    try {
      switchboard = await SwitchboardTestContext.loadDevnetQueue(
        provider,
        "F8ce7MsckeZAbAGmxjJNetxYXQa9mKr9nnrC3qKubyYy",
        5_000_000 // .005 wSOL
      );
      console.log("devnet detected");
      return;
    } catch (error: any) {
      console.log(`Error: SBV2 Devnet - ${error.message}`);
      // console.error(error);
    }
    // If fails, fallback to looking for a local env file
    try {
      switchboard = await SwitchboardTestContext.loadFromEnv(
        provider,
        undefined,
        5_000_000 // .005 wSOL
      );
      console.log("localnet detected");
      return;
    } catch (error: any) {
      console.log(`Error: SBV2 Localnet - ${error.message}`);
    }
    // If fails, throw error
    throw new Error(
      `Failed to load the SwitchboardTestContext from devnet or from a switchboard.env file`
    );

  })

  let vrfAccount: VrfAccount;
  let permissionAccount: PermissionAccount;

  it("initializes Lottery and VRF accounts", async () => {


    [vaultSigner, vaultSignerBump] = await deriveVaultSignerPDA(programId)
    sourceLiquidityVault = await serumAssoToken.getAssociatedTokenAddress(
      vaultSigner,
      NATIVE_MINT
    )

    destinationCollateralVault = await serumAssoToken.getAssociatedTokenAddress(
      vaultSigner,
      WSOL_PTOKEN_MINT,
    )

    const queue = switchboard.queue;
    const { unpermissionedVrfEnabled, authority, dataBuffer } =
      await queue.loadData();

    // Create Switchboard VRF and Permission account
    vrfAccount = await VrfAccount.create(switchboard.program, {
      queue,
      callback: vrfClientCallback,
      authority: vrfClientKey, // vrf authority
      keypair: vrfSecret,
    });


    console.log(`Created VRF Account: ${vrfAccount.publicKey}`);

    permissionAccount = await PermissionAccount.create(
      switchboard.program,
      {
        authority,
        granter: queue.publicKey,
        grantee: vrfAccount.publicKey,
      }
    );
    console.log(`Created Permission Account: ${permissionAccount.publicKey}`);
    // If queue requires permissions to use VRF, check the correct authority was provided
    if (!unpermissionedVrfEnabled) {
      if (!payer.publicKey.equals(authority)) {
        throw new Error(
          `queue requires PERMIT_VRF_REQUESTS and wrong queue authority provided`
        );
      }

      await permissionAccount.set({
        authority: payer,
        permission: SwitchboardPermission.PERMIT_VRF_REQUESTS,
        enable: true,
      });
      console.log(`Set VRF Permissions`);
    }


    try {

      await program.methods
        .initializeLottery({
          entryFee: new anchor.BN(LOTTERY_ENTRY_FEE),
          maxParticipants: new anchor.BN(users.length)
        })
        .accounts({
          state: vrfClientKey,
          vrf: vrfAccount.publicKey,
          sourceLiquidityMint: NATIVE_MINT,
          destinationCollateralMint: WSOL_PTOKEN_MINT,
          sourceLiquidityVault,
          destinationCollateralVault,
          user: provider.wallet.publicKey,
          authority: provider.wallet.publicKey,
          vaultSigner,
          lotteryAcct: lotteryPDA,
        })
        .rpc()
    } catch (error) {

      console.log("Account has been successfully initialized...",);
    }
  })




  it("users enter lottery", async () => {

    for (let i = 0; i < users.length; i++) {
      const user = users[i]
      const userTokenAccount = await serumAssoToken.getAssociatedTokenAddress(user.publicKey, NATIVE_MINT,)



      const [userTokenDifference, sourceLiquidityVaultDifference] = await rpcWithBalanceChange(
        provider,
        [userTokenAccount, sourceLiquidityVault],
        [sourceLiquidityMintInfo.decimals, sourceLiquidityMintInfo.decimals],
        async () => {

          await program.methods
            .enterLottery()
            .accounts({
              sourceLiquidityMint: NATIVE_MINT,
              destinationCollateralMint: WSOL_PTOKEN_MINT,
              userTokenAccount,
              sourceLiquidityVault,
              destinationCollateralVault,
              user: user.publicKey,
              vaultSigner,
              lotteryAcct: lotteryPDA,
              systemProgram: anchor.web3.SystemProgram.programId,
              rent: anchor.web3.SYSVAR_RENT_PUBKEY,
              associatedTokenProgram: serumAssoToken.ASSOCIATED_TOKEN_PROGRAM_ID,
              tokenProgram: TOKEN_PROGRAM_ID,
            })
            .signers([user])
            .rpc()
            .catch(e => console.log(e))

        }
      )
      assert.ok(userTokenDifference == -1) // 0 - 1_000_000_000 / 1_000_000_000 = -1

      assert.ok(sourceLiquidityVaultDifference == LOTTERY_ENTRY_FEE / 10 ** sourceLiquidityMintInfo.decimals)
    }

  })

  it("deposit to Port finance Lending Pool ", async () => {

    const [sourceLiquidityVaultDifference, destinationCollateralVaultDifference] = await rpcWithBalanceChange(
      provider,
      [sourceLiquidityVault, destinationCollateralVault],
      [sourceLiquidityMintInfo.decimals, destinationCollateralMintInfo.decimals],
      async () => {
        await program.methods
          .deposit(vaultSignerBump)
          .accounts({
            lendingProgram: lendingProgram,
            sourceLiquidityVault,
            destinationCollateralVault,
            reserve: SOL_RESERVE,
            reserveLiquiditySupply: SOL_RESERVE_LIQUIDITY_SUPPLY,
            reserveCollateralMint: WSOL_PTOKEN_MINT,
            sourceLiquidityMint: NATIVE_MINT,
            lendingMarket: lendingMarket,
            destinationCollateralMint: WSOL_PTOKEN_MINT,
            authority: vaultSigner,
            clock: anchor.web3.SYSVAR_CLOCK_PUBKEY,
            reserveLiquidityOracle: SOL_ORACLE
          })
          .remainingAccounts([
            {
              pubkey: new anchor.web3.PublicKey("9CGs1nDsrPZ6wYm16UaPTipRGFz5SgxMWtGpDwJoYu5A"),
              isSigner: false,
              isWritable: false
            },
            {
              pubkey: TOKEN_PROGRAM_ID,
              isSigner: false,
              isWritable: false
            }
          ])
          .rpc()
          .catch(e => console.log(e))
      })

    assert.ok(sourceLiquidityVaultDifference < 0)
    assert.ok(destinationCollateralVaultDifference > 0)
  })

  it("requests for random number", async () => {
    const queue = switchboard.queue;
    const { authority, dataBuffer } =
      await queue.loadData();
    // Get required switchboard accounts
    const [programStateAccount, programStateBump] =
      ProgramStateAccount.fromSeed(switchboard.program);
    const [_, permissionBump] = PermissionAccount.fromSeed(
      switchboard.program,
      authority,
      queue.publicKey,
      vrfAccount.publicKey
    );
    const mint = await programStateAccount.getTokenMint();
    const payerTokenAccount = await getOrCreateAssociatedTokenAccount(
      provider.connection,
      payer,
      mint.address,
      payer.publicKey
    );

    const { escrow } = await vrfAccount.loadData();


    await program.methods
      .requestResult({
        switchboardStateBump: programStateBump,
        permissionBump,
      })
      .accounts({
        state: vrfClientKey,
        authority: payer.publicKey,
        switchboardProgram: switchboard.program.programId,
        vrf: vrfAccount.publicKey,
        oracleQueue: queue.publicKey,
        queueAuthority: authority,
        dataBuffer,
        permission: permissionAccount.publicKey,
        escrow,
        payerWallet: payerTokenAccount.address,
        payerAuthority: payer.publicKey,
        recentBlockhashes: SYSVAR_RECENT_BLOCKHASHES_PUBKEY,
        programState: programStateAccount.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .rpc();
  })

  it("chooses winner after vrf has been fulfilled", async () => {
    await sleep(30000); //30 secs
    await program.methods
      .chooseWinner()
      .accounts({
        lotteryAcct: lotteryPDA,
        state: vrfClientKey,
        vrf: vrfAccount.publicKey
      })
      .rpc()
      .catch(error => console.log(error));
    const lotteryState = await program.account.lotteryInfo.fetch(lotteryPDA)
    lotteryWinner = lotteryState.winner;
    console.log("And the winner is: ", lotteryState.winner.toBase58());

  })

  it("withdraws tokens from lending pool", async () => {


    const [sourceLiquidityVaultDifference, destinationCollateralVaultDifference] = await rpcWithBalanceChange(
      provider,
      [sourceLiquidityVault, destinationCollateralVault],
      [sourceLiquidityMintInfo.decimals, destinationCollateralMintInfo.decimals],
      async () => {
        await program.methods
          .withdraw(vaultSignerBump)
          .accounts({
            lendingProgram: lendingProgram,
            sourceLiquidityVault,
            destinationCollateralVault,
            reserve: SOL_RESERVE,
            reserveLiquiditySupply: SOL_RESERVE_LIQUIDITY_SUPPLY,
            reserveCollateralMint: WSOL_PTOKEN_MINT,
            sourceLiquidityMint: NATIVE_MINT,
            lendingMarket: lendingMarket,
            destinationCollateralMint: WSOL_PTOKEN_MINT,
            authority: vaultSigner,
            clock: anchor.web3.SYSVAR_CLOCK_PUBKEY,
            reserveLiquidityOracle: SOL_ORACLE
          })
          .remainingAccounts([
            {
              pubkey: new anchor.web3.PublicKey("9CGs1nDsrPZ6wYm16UaPTipRGFz5SgxMWtGpDwJoYu5A"),
              isSigner: false,
              isWritable: false
            },
            {
              pubkey: TOKEN_PROGRAM_ID,
              isSigner: false,
              isWritable: false
            }
          ])
          .rpc()
          .catch(e => console.log(e))
      })

    assert.ok(sourceLiquidityVaultDifference * 10 ** sourceLiquidityMintInfo.decimals > LOTTERY_ENTRY_FEE * users.length)
    console.log(destinationCollateralVaultDifference);

    assert.ok(destinationCollateralVaultDifference < 0)
  })

  it("withdraw user tokens", async () => {

    for (let i = 0; i < users.length; i++) {
      const user = users[i];
      const userTokenAccount = await serumAssoToken.getAssociatedTokenAddress(user.publicKey, NATIVE_MINT,)
      console.log(user.publicKey.toBase58());
      const balanceBefore = await provider.connection.getTokenAccountBalance(userTokenAccount)

      try {
        await program.methods
          .withdrawUserTokens()
          .accounts({
            sourceLiquidityMint: NATIVE_MINT,
            userTokenAccount,
            sourceLiquidityVault,
            user: user.publicKey,
            vaultSigner,
            lotteryAcct: lotteryPDA,
            systemProgram: anchor.web3.SystemProgram.programId,
            rent: anchor.web3.SYSVAR_RENT_PUBKEY,
            associatedTokenProgram: serumAssoToken.ASSOCIATED_TOKEN_PROGRAM_ID,
            tokenProgram: TOKEN_PROGRAM_ID,
          })
          .signers([user])
          .rpc()
      } catch (error) {

      }
      const balanceAfter = await provider.connection.getTokenAccountBalance(userTokenAccount)

      if (lotteryWinner.toString() == user.publicKey.toString()) {
        console.log("Winner new balance", balanceAfter.value.amount);
        assert.ok(Number(balanceAfter.value.amount) > LOTTERY_ENTRY_FEE)
      } else {
        console.log(balanceAfter.value.amount);
        assert.ok(Number(balanceAfter.value.amount) == LOTTERY_ENTRY_FEE)

      }
    }
  })

  it("transfers back tokens to payer wallet for some more testing", async () => {
    for (let i = 0; i < users.length; i++) {
      const user = users[i];
      const userWsolAccount = await serumAssoToken.getAssociatedTokenAddress(
        user.publicKey,
        NATIVE_MINT
      )
      // SOL
      try {
        const tx1 = new anchor.web3.Transaction().add(
          createCloseAccountInstruction(
            userWsolAccount,
            provider.wallet.publicKey,
            user.publicKey
          )
        )
        await provider.sendAndConfirm(tx1, [user])
      } catch (error) {

      }
      try {

        const balance = await provider.connection.getBalance(user.publicKey)

        const tx2 = new anchor.web3.Transaction().add(
          anchor.web3.SystemProgram.transfer({
            fromPubkey: user.publicKey,
            toPubkey: provider.wallet.publicKey,
            lamports: balance,
          }),
        );
        await provider.sendAndConfirm(tx2, [user])
      } catch (error) {

      }


    }
  })
  it("closes accounts", async () => {
    await program.methods
      .closeAccounts()
      .accounts({
        state: vrfClientKey,
        lotteryAcct: lotteryPDA,
        vrf: vrfAccount.publicKey,
        user: payer.publicKey,
      })
      .rpc()
      .catch(err => console.log(err));

  })
});

