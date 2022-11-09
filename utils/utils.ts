import * as anchor from "@project-serum/anchor";
import * as serumAssoToken from "@project-serum/associated-token";
import {
  createMintToInstruction, createSyncNativeInstruction, createWrappedNativeAccount, MintLayout, RawMint, TOKEN_PROGRAM_ID,
} from "@solana/spl-token";
import * as fs from "fs";
import { Keypair, PublicKey } from "@solana/web3.js";
import { SystemProgram, Transaction } from '@solana/web3.js';
import { LAMPORTS_PER_SOL } from "@solana/web3.js";
import { assert } from "chai";

export async function deriveTrackingAddress(
  programId: anchor.web3.PublicKey,
  vault: anchor.web3.PublicKey,
  owner: anchor.web3.PublicKey
): Promise<[anchor.web3.PublicKey, number]> {
  return await anchor.web3.PublicKey.findProgramAddress(
    [Buffer.from("tracking"), vault.toBuffer(), owner.toBuffer()],
    programId
  );
}

export async function deriveTrackingPdaAddress(
  programId: anchor.web3.PublicKey,
  trackingAddress: anchor.web3.PublicKey
): Promise<[anchor.web3.PublicKey, number]> {
  return await anchor.web3.PublicKey.findProgramAddress(
    [trackingAddress.toBuffer()],
    programId
  );
}

export async function deriveTrackingQueueAddress(
  programId: anchor.web3.PublicKey,
  trackingPdaAddress: anchor.web3.PublicKey
): Promise<[anchor.web3.PublicKey, number]> {
  return await anchor.web3.PublicKey.findProgramAddress(
    [Buffer.from("queue"), trackingPdaAddress.toBuffer()],
    programId
  );
}
/**
 * creates associated token account with provider as payer 
 * @param provider 
 * @param owner 
 * @param mint 
 * @returns PublicKey 
 */
export async function createAssociatedTokenAccount(
  provider: anchor.AnchorProvider,
  owner: anchor.web3.PublicKey,
  mint: anchor.web3.PublicKey
): Promise<anchor.web3.PublicKey> {
  let tx = new anchor.web3.Transaction();

  tx.add(
    await serumAssoToken.createAssociatedTokenAccount(
      provider.wallet.publicKey,
      owner,
      mint
    )
  );
  await provider.sendAll([{ tx }]);
  let acct = await serumAssoToken.getAssociatedTokenAddress(owner, mint);
  return acct;
}

export const mintTokens = async (
  provider: anchor.AnchorProvider,
  amount: number,
  mint: anchor.web3.PublicKey,
  authority: anchor.web3.PublicKey,
  userAssociatedTokenAccount: anchor.web3.PublicKey
) => {
  const txFundTokenAccount = new anchor.web3.Transaction();
  txFundTokenAccount.add(createMintToInstruction(
    mint,
    userAssociatedTokenAccount,
    authority,
    amount,
  ));
  await provider.sendAndConfirm(txFundTokenAccount);
}
export const wrapSol = async (
  provider: anchor.AnchorProvider,
  user: Keypair,
  amount: number,
): Promise<PublicKey> => {

  return await createWrappedNativeAccount(
    provider.connection,
    user,
    user.publicKey,
    amount,
    // user
  )
}

export const addToWSolAccount = async (
  provider: anchor.AnchorProvider,
  user: Keypair,
  wSolAccount: PublicKey,
  amount: number,
) => {

  const tx = new Transaction().add(
    SystemProgram.transfer({
      fromPubkey: user.publicKey,
      toPubkey: wSolAccount,
      lamports: amount,
    }),
    createSyncNativeInstruction(wSolAccount, TOKEN_PROGRAM_ID)
  );
  await provider.sendAndConfirm(tx, [user])
}

export const getSecretKey = (name: string) =>
  Uint8Array.from(
    JSON.parse(fs.readFileSync(`test_utils/keys/${name}.json`) as unknown as string)
  );

/**
 * gets KeyPair from file
 * @param name name of secretKey file
 * @returns KeyPair
 */
export const getKeypair = (name: string) =>
  Keypair.fromSecretKey(getSecretKey(name));

export function sleep(ms) {
  return new Promise((resolve) => {
    setTimeout(resolve, ms);
  });
}

export async function deriveVaultSignerPDA(
  programId: anchor.web3.PublicKey,
): Promise<[anchor.web3.PublicKey, number]> {
  const pda = await anchor.web3.PublicKey.findProgramAddress(
    [Buffer.from("vault_signer")],
    programId
  );

  return pda
}
export async function deriveLotteryInfoPDA(
  programId: anchor.web3.PublicKey,
): Promise<[anchor.web3.PublicKey, number]> {
  const pda = await anchor.web3.PublicKey.findProgramAddress(
    [Buffer.from("lottery_info")],
    programId
  );

  return pda
}

export async function requestAirdrop(
  provider: anchor.AnchorProvider,
  user: anchor.web3.PublicKey,
  amount: number
) {
  if (provider.connection.rpcEndpoint == "https://api.devnet.solana.com") {
    await provider.connection.requestAirdrop(
      user,
      amount * LAMPORTS_PER_SOL
    )
  } else {
    const tx = new Transaction().add(
      SystemProgram.transfer({
        fromPubkey: provider.wallet.publicKey,
        toPubkey: user,
        lamports: amount * LAMPORTS_PER_SOL,
      }),
    );
    await provider.sendAndConfirm(tx,)
  }
}

export async function transferLamports(
  provider: anchor.AnchorProvider,
  user: anchor.web3.PublicKey,
  amount: number
) {
  const tx = new Transaction().add(
    SystemProgram.transfer({
      fromPubkey: provider.wallet.publicKey,
      toPubkey: user,
      lamports: amount,
    }),
  );
  await provider.sendAndConfirm(tx,)
}

export async function getProviderKeypair(): Promise<Keypair> {
  return Keypair.fromSecretKey(Buffer.from(JSON.parse(require("fs").readFileSync(process.env.ANCHOR_WALLET, {
    encoding: "utf-8",
  }))));
}

export async function rpcWithBalanceChange(
  provider: anchor.AnchorProvider,
  addresses: anchor.web3.PublicKey[],
  decimals: number[],
  rpcFunction: Function
) {
  assert.equal(decimals.length, addresses.length);
  const beforeBalances = [];
  for (let k = 0; k < addresses.length; k += 1) {
    beforeBalances.push(
      Number((await provider.connection.getTokenAccountBalance(addresses[k])).value.amount)
      );
    }


  await rpcFunction();
  await sleep(100)
  const afterBalances = [];
  for (let k = 0; k < addresses.length; k += 1) {
    afterBalances.push(
      Number((await provider.connection.getTokenAccountBalance(addresses[k])).value.amount)
    );
  }

  const deltas: number[] = [];
  for (let k = 0; k < addresses.length; k += 1) {
    const delta = (afterBalances[k] - beforeBalances[k]) / (10 ** decimals[k]);
    
    deltas.push(delta);
  }

  return deltas;
}

export const mintInfo = async (provider: anchor.Provider, mintPublicKey: anchor.web3.PublicKey,): Promise<RawMint> => {
  const tokenInfo = await provider.connection.getAccountInfo(mintPublicKey);
  const data = Buffer.from(tokenInfo.data);
  const accountInfo = MintLayout.decode(data);
  return {
    ...accountInfo,
    mintAuthority: accountInfo.mintAuthority == null ? null : anchor.web3.PublicKey.decode(accountInfo.mintAuthority.toBuffer()),
    freezeAuthority: accountInfo.freezeAuthority == null ? null : anchor.web3.PublicKey.decode(accountInfo.freezeAuthority.toBuffer()),
  }
}