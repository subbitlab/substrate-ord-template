// We make sure this pallet uses `no_std` for compiling to Wasm.
#![cfg_attr(not(feature = "std"), no_std)]

use thiserror::Error;
// Re-export pallet items so that they can be accessed from the crate namespace.
use sp_std::vec::Vec;
mod index;
mod runes;
pub mod weights;

pub use weights::*;

pub const REQUIRED_CONFIRMATIONS: u32 = 4;
pub const FIRST_HEIGHT: u32 = 839999;
pub const FIRST_BLOCK_HASH: &'static str =
	"0000000000000000000172014ba58d66455762add0512355ad651207918494ab";

pub(crate) type Result<T> = sp_std::result::Result<T, OrdError>;

#[derive(Debug, Error)]
pub enum OrdError {
	#[error("params: {0}")]
	Params(String),
	#[error("overflow")]
	Overflow,
	#[error("block verification")]
	BlockVerification(u32),
	#[error("index error: {0}")]
	Index(runes::MintError),
	/*	#[error("rpc error: {0}")]
	Rpc(#[from] rpc::RpcError),*/
}
// All pallet logic is defined in its own module and must be annotated by the `pallet` attribute.
#[frame_support::pallet]
pub mod pallet {
	use std::collections::BTreeMap;
	use std::ptr::hash;
	// Import various useful types required by all FRAME pallets.
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;
	use ordinals::{Pile, Rune, RuneId, SpacedRune};
	use crate::index::entry::{Entry, OutPointValue, RuneBalance, TxidValue};
	use crate::index::RuneEntry;

	// The `Pallet` struct serves as a placeholder to implement traits, methods and dispatchables
	// (`Call`s) in this pallet.
	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// The overarching runtime event type.
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
		/// A type representing the weights required by the dispatchables of this pallet.
		type WeightInfo: WeightInfo;
		type MaxOutPointRuneBalancesLen: Get<u32>;
	}

	#[pallet::storage]
	pub type Something<T> = StorageValue<_, u32>;

	#[pallet::storage]
	#[pallet::getter(fn outpoint_to_rune_blances)]
	pub type OutPointRuneBalances<T: Config> =
		StorageMap<_, Twox64Concat, OutPointValue, BoundedVec<RuneBalance, T::MaxOutPointRuneBalancesLen>, OptionQuery>;

	#[pallet::storage]
	#[pallet::getter(fn rune_id_to_rune_entry)]
	pub type RuneIdToRuneEntry<T: Config> =
		StorageMap<_, Blake2_128Concat, RuneId, RuneEntry, OptionQuery>;

	#[pallet::storage]
	#[pallet::getter(fn rune_to_rune_id)]
	pub type RuneToRuneId<T: Config> =
		StorageMap<_, Blake2_128Concat, u128, RuneId, OptionQuery>;

	#[pallet::storage]
	#[pallet::getter(fn transaction_id_to_rune)]
	pub type TransactionIdToRune<T: Config> =
		StorageMap<_, Blake2_128Concat, TxidValue, u128, OptionQuery>;

	#[pallet::storage]
	#[pallet::getter(fn height_to_block_hash)]
	pub type HeightToBlockHash<T: Config> =
		StorageMap<_, Blake2_128Concat, u32, [u8;32], OptionQuery>;

	#[pallet::storage]
	#[pallet::getter(fn highest_height)]
	pub type HighestHeight<T: Config> =
	StorageValue<_, (u32, [u8;32]), OptionQuery>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A user has successfully set a new value.
		SomethingStored {
			/// The new value set.
			something: u32,
			/// The account who set the new value.
			who: T::AccountId,
		},
	}

	#[pallet::error]
	pub enum Error<T> {
		/// The value retrieved was `None` as no value was previously set.
		NoneValue,
		/// There was an attempt to increment the value in storage over `u32::MAX`.
		StorageOverflow,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::call_index(0)]
		#[pallet::weight(T::WeightInfo::do_something())]
		pub fn do_something(origin: OriginFor<T>, something: u32) -> DispatchResult {
			Ok(())
		}
	}

	use bitcoin::BlockHash;
	use bitcoin::consensus::Encodable;
	use bitcoin::hashes::sha256d::Hash;
	use ordinals::{Txid};
	use bitcoin::blockdata::transaction::OutPoint;
	impl<T: Config> Pallet<T> {

		pub(crate) fn increase_height(height: u32, hash: [u8;32]) {
			HeightToBlockHash::<T>::insert(height, hash.clone());
			HighestHeight::<T>::put((height, hash));
		}

		pub(crate) fn highest_block() -> (u32, [u8;32]) {
			Self::highest_height().unwrap_or_default()
		}

		fn set_beginning_block() {
			let mut hash = [0u8;32];
			let hex = hex::decode(FIRST_BLOCK_HASH).unwrap();
			hash.copy_from_slice(hex.as_slice());
			Self::increase_height(FIRST_HEIGHT, hash);
		}

		#[allow(dead_code)]
		pub(crate) fn get_etching(txid: Txid) -> Result<Option<SpacedRune>> {
			let Some(rune) = Self::transaction_id_to_rune(Txid::store(txid)) else {
				return Ok(None);
			};
			let id = Self::rune_to_rune_id(rune).unwrap();
			let entry = Self::rune_id_to_rune_entry(id).unwrap();
			Ok(Some(entry.spaced_rune))
		}

		pub(crate) fn get_rune_balances_for_output(
			outpoint: OutPoint,
		) -> Result<BTreeMap<SpacedRune, Pile>> {
			let balances_vec = Self::outpoint_to_rune_blances(OutPoint::store(outpoint));
			match balances_vec {
				Some(balances) => {
					let mut result = BTreeMap::new();
					for rune in balances.iter() {
						let rune = *rune;

						let entry = Self::rune_id_to_rune_entry(rune.id).unwrap();

						result.insert(
							entry.spaced_rune,
							Pile {
								amount: rune.balance,
								divisibility: entry.divisibility,

								symbol: None/*entry.symbol*/, //TODO
							},
						);
					}
					Ok(result)
				}
				None => Ok(BTreeMap::new()),
			}
		}

		pub fn init_rune() {
			Self::set_beginning_block();
			let rune = Rune(2055900680524219742);
			let id = RuneId { block: 1, tx: 0 };
			let etching = Txid([0u8; 32]);
			RuneToRuneId::<T>::insert(rune.store(), id);
			RuneIdToRuneEntry::<T>::insert(
					id,
					RuneEntry {
						block: id.block,
						burned: 0,
						divisibility: 0,
						etching,
				/*		terms: Some(Terms {
							amount: Some(1),
							cap: Some(u128::MAX),
							height: (
								Some((SUBSIDY_HALVING_INTERVAL * 4).into()),
								Some((SUBSIDY_HALVING_INTERVAL * 5).into()),
							),
							offset: (None, None),
						}),*/ //TODO

						mints: 0,
						premine: 0,
						spaced_rune: SpacedRune { rune, spacers: 128 },
						/*symbol: Some('\u{29C9}'),*/ //TODO
						timestamp: 0,
						turbo: true,
					},
				);
				TransactionIdToRune::<T>::insert(Txid::store(etching), rune.store());
		}



	}

	/*use bitcoin::BlockHash;
	impl<T: Config> Pallet<T> {

		pub(crate) async fn get_best_from_rpc() -> Result<(u32, BlockHash)> {
			let url = get_url();
			let hash = rpc::get_best_block_hash(&url).await?;
			let header = rpc::get_block_header(&url, hash).await?;
			Ok((header.height.try_into().expect("usize to u32"), hash))
		}
	}*/
}
