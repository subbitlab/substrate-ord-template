use bitcoin::{Address, Amount, ScriptBuf};
use bitcoin::address::NetworkUnchecked;
use bitcoin::block::Version;
use codec::alloc;
use sp_std::vec::Vec;
use alloc::string::String;

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct GetRawTransactionResult {
	pub in_active_chain: Option<bool>,
	pub hex: Vec<u8>,
	pub txid: bitcoin::Txid,
	pub hash: bitcoin::Wtxid,
	pub size: usize,
	pub vsize: usize,
	pub version: u32,
	pub locktime: u32,
	pub vin: Vec<GetRawTransactionResultVin>,
	pub vout: Vec<GetRawTransactionResultVout>,
	pub blockhash: Option<bitcoin::BlockHash>,
	pub confirmations: Option<u32>,
	pub time: Option<usize>,
	pub blocktime: Option<usize>,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct GetRawTransactionResultVin {
	pub sequence: u32,
	/// The raw scriptSig in case of a coinbase tx.
	pub coinbase: Option<Vec<u8>>,
	/// Not provided for coinbase txs.
	pub txid: Option<bitcoin::Txid>,
	/// Not provided for coinbase txs.
	pub vout: Option<u32>,
	/// The scriptSig in case of a non-coinbase tx.
	pub script_sig: Option<GetRawTransactionResultVinScriptSig>,
	/// Not provided for coinbase txs.
	pub txinwitness: Option<Vec<Vec<u8>>>,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct GetRawTransactionResultVout {
	pub value: Amount,
	pub n: u32,
	pub script_pub_key: GetRawTransactionResultVoutScriptPubKey,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct GetRawTransactionResultVoutScriptPubKey {
	pub asm: String,
	pub hex: Vec<u8>,
	pub req_sigs: Option<usize>,
	pub type_: Option<ScriptPubkeyType>,
	// Deprecated in Bitcoin Core 22
	pub addresses: Vec<Address<NetworkUnchecked>>,
	// Added in Bitcoin Core 22
	pub address: Option<Address<NetworkUnchecked>>,
}



#[allow(non_camel_case_types)]
#[derive(Copy, Clone, PartialEq, Eq, Debug, )]
pub enum ScriptPubkeyType {
	Nonstandard,
	Pubkey,
	PubkeyHash,
	ScriptHash,
	MultiSig,
	NullData,
	Witness_v0_KeyHash,
	Witness_v0_ScriptHash,
	Witness_v1_Taproot,
	Witness_Unknown,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct GetRawTransactionResultVinScriptSig {
	pub asm: String,
	pub hex: Vec<u8>,
}

impl GetRawTransactionResultVinScriptSig {
	pub fn script(&self) -> ScriptBuf {
		ScriptBuf::from(self.hex.clone())
	}
}

impl GetRawTransactionResultVoutScriptPubKey {
	pub fn script(&self) -> ScriptBuf {
		ScriptBuf::from(self.hex.clone())
	}
}

#[derive(Clone, PartialEq, Debug)]
pub struct GetBlockHeaderResult {
	pub hash: bitcoin::BlockHash,
	pub confirmations: i32,
	pub height: usize,
	pub version: Version,
	pub version_hex: Option<Vec<u8>>,
	pub merkle_root: bitcoin::hash_types::TxMerkleNode,
	pub time: usize,
	pub median_time: Option<usize>,
	pub nonce: u32,
	pub bits: String,
	pub difficulty: f64,
	pub chainwork: Vec<u8>,
	pub n_tx: usize,
	pub previous_block_hash: Option<bitcoin::BlockHash>,
	pub next_block_hash: Option<bitcoin::BlockHash>,
}
