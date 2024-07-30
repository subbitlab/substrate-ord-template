// We make sure this pallet uses `no_std` for compiling to Wasm.
#![cfg_attr(not(feature = "std"), no_std)]

use sp_runtime::offchain::http;
use thiserror::Error;
// Re-export pallet items so that they can be accessed from the crate namespace.
use ordinals::{Rune, RuneId};
use sp_std::collections::btree_map::BTreeMap;
use sp_std::vec::Vec;

pub mod index;
mod rpc;
mod runes;
pub mod weights;

use crate::index::event::Event;
use crate::index::lot::Lot;
pub use weights::*;

pub const REQUIRED_CONFIRMATIONS: u32 = 4;
pub const FIRST_HEIGHT: u32 = 839999;
pub const FIRST_BLOCK_HASH: &'static str =
	"0000000000000000000172014ba58d66455762add0512355ad651207918494ab";

pub(crate) type Result<T> = sp_std::result::Result<T, OrdError>;

pub(crate) type OffchainWorkerRpcResult<T> = sp_std::result::Result<T, http::Error>;

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
	#[error("rpc error: {0}")]
	Rpc(#[from] rpc::RpcError),
	#[error("offchain http error")]
	OffchainHttp(http::Error),
}
// All pallet logic is defined in its own module and must be annotated by the `pallet` attribute.
#[frame_support::pallet]
pub mod pallet {
	// Import various useful types required by all FRAME pallets.
	use super::*;
	use crate::index::entry::{Entry, OutPointValue, RuneBalance, TxidValue};
	use crate::index::RuneEntry;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;
	use ordinals::{Artifact, Edict, Etching, Pile, Rune, RuneId, SpacedRune};

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
	#[pallet::getter(fn outpoint_to_rune_balances)]
	pub type OutPointRuneBalances<T: Config> = StorageMap<
		_,
		Twox64Concat,
		OutPointValue,
		BoundedVec<RuneBalance, T::MaxOutPointRuneBalancesLen>,
		OptionQuery,
	>;

	#[pallet::storage]
	#[pallet::getter(fn rune_id_to_rune_entry)]
	pub type RuneIdToRuneEntry<T: Config> =
		StorageMap<_, Blake2_128Concat, RuneId, RuneEntry, OptionQuery>;

	#[pallet::storage]
	#[pallet::getter(fn rune_to_rune_id)]
	pub type RuneToRuneId<T: Config> = StorageMap<_, Blake2_128Concat, u128, RuneId, OptionQuery>;

	#[pallet::storage]
	#[pallet::getter(fn transaction_id_to_rune)]
	pub type TransactionIdToRune<T: Config> =
		StorageMap<_, Blake2_128Concat, TxidValue, u128, OptionQuery>;

	#[pallet::storage]
	#[pallet::getter(fn height_to_block_hash)]
	pub type HeightToBlockHash<T: Config> =
		StorageMap<_, Blake2_128Concat, u32, [u8; 32], OptionQuery>;

	#[pallet::storage]
	#[pallet::getter(fn highest_height)]
	pub type HighestHeight<T: Config> = StorageValue<_, (u32, [u8; 32]), OptionQuery>;

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

	use bitcoin::blockdata::transaction::OutPoint;
	use bitcoin::consensus::Encodable;
	use bitcoin::hashes::Hash;
	use bitcoin::{BlockHash, Transaction};
	use bitcoincore_rpc_json::GetRawTransactionResult;
	use ordinals::{Runestone, Txid};

	impl<T: Config> Pallet<T> {
		pub(crate) fn increase_height(height: u32, hash: [u8; 32]) {
			HeightToBlockHash::<T>::insert(height, hash.clone());
			HighestHeight::<T>::put((height, hash));
		}

		pub(crate) fn highest_block() -> (u32, [u8; 32]) {
			Self::highest_height().unwrap_or_default()
		}

		fn set_beginning_block() {
			let mut hash = [0u8; 32];
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
			let balances_vec = Self::outpoint_to_rune_balances(OutPoint::store(outpoint));
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

								symbol: None, /*entry.symbol*/, //TODO
							},
						);
					}
					Ok(result)
				},
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

	use crate::index::updater::BlockData;

	//updater
	impl<T: Config> Pallet<T> {
		pub fn get_block(height: u32) -> Result<BlockData> {
			let url = "get_url()"; //TODO
			let hash = rpc::get_block_hash(&url, height)?;
			let block = rpc::get_block(&url, hash)?;
			block
				.check_merkle_root()
				.then(|| BlockData::from(block))
				.ok_or(OrdError::BlockVerification(height))
		}

		pub(crate) fn get_raw_tx(txid: ordinals::Txid) -> Result<GetRawTransactionResult> {
			let url = "get_url()"; //TODO
			rpc::get_raw_tx(&url, txid)
		}

		fn unallocated(tx: &Transaction) -> Result<BTreeMap<RuneId, crate::index::lot::Lot>> {
			let mut unallocated: BTreeMap<RuneId, crate::index::lot::Lot> = BTreeMap::new();
			for input in &tx.input {
				let key = OutPoint::store(input.previous_output);
				let r = Self::outpoint_to_rune_balances(key.clone());
				OutPointRuneBalances::<T>::remove(key);

				if let Some(balances) = r {
					for rune in balances.iter() {
						let rune = *rune;
						*unallocated.entry(rune.id).or_default() += rune.balance;
					}
				}
			}
			Ok(unallocated)
		}

		pub fn get_best_from_rpc() -> Result<(u32, BlockHash)> {
			let url = "get_url()"; //TODO
			let hash = rpc::get_best_block_hash(&url)?;
			let header = rpc::get_block_header(&url, hash)?;
			Ok((header.height.try_into().expect("usize to u32"), hash))
		}
	}

	impl<T: Config> Pallet<T> {
		//runes updater integreate
		pub(super) fn index_runes(
			updater: &mut RuneUpdater,
			tx_index: u32,
			tx: &Transaction,
			txid: Txid,
		) -> Result<()> {
			let artifact = Runestone::decipher(tx);

			let mut unallocated = Self::unallocated(tx)?;

			let mut allocated: Vec<BTreeMap<RuneId, Lot>> = vec![BTreeMap::new(); tx.output.len()];

			if let Some(artifact) = &artifact {
				if let Some(id) = artifact.mint() {
					if let Some(amount) = Self::mint(&mut *updater, id)? {
						*unallocated.entry(id).or_default() += amount;
						let bitcoin_txid = bitcoin::Txid::from_slice(txid.0.as_slice()).unwrap();
						if let Some(handler) = &updater.event_handler {
							handler(crate::index::event::Event::RuneMinted {
								block_height: updater.height,
								txid: bitcoin_txid,
								rune_id: id,
								amount: amount.n(),
							});
						}
					}
				}

				let etched = Self::etched(&mut *updater, tx_index, tx, artifact)?;

				if let Artifact::Runestone(runestone) = artifact {
					if let Some((id, ..)) = etched {
						*unallocated.entry(id).or_default() +=
							runestone.etching.unwrap().premine.unwrap_or_default();
					}

					for Edict { id, amount, output } in runestone.edicts.iter().copied() {
						let amount = Lot(amount);

						// edicts with output values greater than the number of outputs
						// should never be produced by the edict parser
						let output = usize::try_from(output).unwrap();
						assert!(output <= tx.output.len());

						let id = if id == RuneId::default() {
							let Some((id, ..)) = etched else {
								continue;
							};

							id
						} else {
							id
						};

						let Some(balance) = unallocated.get_mut(&id) else {
							continue;
						};

						let mut allocate = |balance: &mut Lot, amount: Lot, output: usize| {
							if amount > 0 {
								*balance -= amount;
								*allocated[output].entry(id).or_default() += amount;
							}
						};

						if output == tx.output.len() {
							// find non-OP_RETURN outputs
							let destinations = tx
								.output
								.iter()
								.enumerate()
								.filter_map(|(output, tx_out)| {
									(!tx_out.script_pubkey.is_op_return()).then_some(output)
								})
								.collect::<Vec<usize>>();

							if !destinations.is_empty() {
								if amount == 0 {
									// if amount is zero, divide balance between eligible outputs
									let amount = *balance / destinations.len() as u128;
									let remainder =
										usize::try_from(*balance % destinations.len() as u128)
											.unwrap();

									for (i, output) in destinations.iter().enumerate() {
										allocate(
											balance,
											if i < remainder { amount + 1 } else { amount },
											*output,
										);
									}
								} else {
									// if amount is non-zero, distribute amount to eligible outputs
									for output in destinations {
										allocate(balance, amount.min(*balance), output);
									}
								}
							}
						} else {
							// Get the allocatable amount
							let amount = if amount == 0 { *balance } else { amount.min(*balance) };

							allocate(balance, amount, output);
						}
					}
				}

				if let Some((id, rune)) = etched {
					Self::create_rune_entry(&mut *updater, txid, artifact, id, rune)?;
				}
			}

			let mut burned: BTreeMap<RuneId, Lot> = BTreeMap::new();

			if let Some(Artifact::Cenotaph(_)) = artifact {
				for (id, balance) in unallocated {
					*burned.entry(id).or_default() += balance;
				}
			} else {
				let pointer = artifact
					.map(|artifact| match artifact {
						Artifact::Runestone(runestone) => runestone.pointer,
						Artifact::Cenotaph(_) => unreachable!(),
					})
					.unwrap_or_default();

				// assign all un-allocated runes to the default output, or the first non
				// OP_RETURN output if there is no default
				if let Some(vout) = pointer
					.map(|pointer| pointer as usize)
					.inspect(|&pointer| assert!(pointer < allocated.len()))
					.or_else(|| {
						tx.output
							.iter()
							.enumerate()
							.find(|(_vout, tx_out)| !tx_out.script_pubkey.is_op_return())
							.map(|(vout, _tx_out)| vout)
					}) {
					for (id, balance) in unallocated {
						if balance > 0 {
							*allocated[vout].entry(id).or_default() += balance;
						}
					}
				} else {
					for (id, balance) in unallocated {
						if balance > 0 {
							*burned.entry(id).or_default() += balance;
						}
					}
				}
			}

			// update outpoint balances
			for (vout, balances) in allocated.into_iter().enumerate() {
				if balances.is_empty() {
					continue;
				}

				// increment burned balances
				if tx.output[vout].script_pubkey.is_op_return() {
					for (id, balance) in &balances {
						*burned.entry(*id).or_default() += *balance;
					}
					continue;
				}

				// let mut balances = balances.into_iter().collect::<Vec<(RuneId, Lot)>>();

				// Sort balances by id so tests can assert balances in a fixed order
				// balances.sort();

				let bitcoin_txid = bitcoin::Txid::from_slice(txid.0.as_slice()).unwrap();
				let outpoint = OutPoint { txid: bitcoin_txid, vout: vout.try_into().unwrap() };
				let mut vec = Vec::with_capacity(balances.len());

				for (id, balance) in balances {
					vec.push(RuneBalance { id, balance: balance.0 });
					if let Some(handler) = &updater.event_handler {
						let bitcoin_txid = bitcoin::Txid::from_slice(txid.0.as_slice()).unwrap();
						handler(crate::index::event::Event::RuneTransferred {
							outpoint,
							block_height: updater.height,
							txid: bitcoin_txid,
							rune_id: id,
							amount: balance.0,
						});
					}
				}

				//TODO	outpoint_to_rune_balances(|b| b.insert(outpoint.store(), vec).expect("MemoryOverflow"));
			}

			// increment entries with burned runes
			for (id, amount) in burned {
				*updater.burned.entry(id).or_default() += amount;

				if let Some(handler) = &updater.event_handler {
					let bitcoin_txid = bitcoin::Txid::from_slice(txid.0.as_slice()).unwrap();
					handler(crate::index::event::Event::RuneBurned {
						block_height: updater.height,
						txid: bitcoin_txid,
						rune_id: id,
						amount: amount.n(),
					});
				}
			}

			Ok(())
		}

		fn create_rune_entry(
			updater: &mut RuneUpdater,
			txid: Txid,
			artifact: &Artifact,
			id: RuneId,
			rune: Rune,
		) -> Result<()> {
			// crate::rune_to_rune_id(|r| r.insert(rune.store(), id)).expect("MemoryOverflow");
			TransactionIdToRune::<T>::insert(txid.store(), rune.0);

			let entry = match artifact {
				Artifact::Cenotaph(_) => RuneEntry {
					block: id.block,
					burned: 0,
					divisibility: 0,
					etching: txid,
					//TODO terms: None,
					mints: 0,
					premine: 0,
					spaced_rune: SpacedRune { rune, spacers: 0 },
					//TODO symbol: None,
					timestamp: updater.block_time.into(),
					turbo: false,
				},
				Artifact::Runestone(Runestone { etching, .. }) => {
					let Etching { divisibility, terms, premine, spacers, symbol, turbo, .. } =
						etching.unwrap();

					RuneEntry {
						block: id.block,
						burned: 0,
						divisibility: divisibility.unwrap_or_default(),
						etching: txid,
						//TODO terms,
						mints: 0,
						premine: premine.unwrap_or_default(),
						spaced_rune: SpacedRune { rune, spacers: spacers.unwrap_or_default() },
						//TODO symbol,
						timestamp: updater.block_time.into(),
						turbo,
					}
				},
			};

			RuneIdToRuneEntry::<T>::insert(id, entry);

			let bitcoin_txid = bitcoin::Txid::from_slice(txid.0.as_slice()).unwrap();
			match &updater.event_handler {
				Some(handler) => handler(crate::index::event::Event::RuneEtched {
					block_height: updater.height,
					txid: bitcoin_txid,
					rune_id: id,
				}),
				None => {},
			}
			Ok(())
		}

		fn etched(
			updater: &mut RuneUpdater,
			tx_index: u32,
			_tx: &Transaction,
			artifact: &Artifact,
		) -> Result<Option<(RuneId, Rune)>> {
			let rune = match artifact {
				Artifact::Runestone(runestone) => match runestone.etching {
					Some(etching) => etching.rune,
					None => return Ok(None),
				},
				Artifact::Cenotaph(cenotaph) => match cenotaph.etching {
					Some(rune) => Some(rune),
					None => return Ok(None),
				},
			};

			let rune = if let Some(rune) = rune {
				if rune < updater.minimum || rune.is_reserved()
				// || crate::rune_to_rune_id(|r| r.get(&rune.0).is_some())
				// || !Self::tx_commits_to_rune(tx, rune).await?
				{
					return Ok(None);
				}
				rune
			} else {
				Rune::reserved(updater.height.into(), tx_index)
			};

			Ok(Some((RuneId { block: updater.height.into(), tx: tx_index }, rune)))
		}

		fn mint(updater: &mut RuneUpdater, id: RuneId) -> Result<Option<Lot>> {
			let Some(mut rune_entry) = Self::rune_id_to_rune_entry(&id) else {
				return Ok(None);
			};
			let Ok(amount) = rune_entry.mintable(updater.height.into()) else {
				return Ok(None);
			};
			rune_entry.mints += 1;
			RuneIdToRuneEntry::<T>::insert(id, rune_entry);
			Ok(Some(Lot(amount)))
		}

		pub fn update(updater: RuneUpdater) -> Result<()> {
			for (rune_id, burned) in updater.burned {
				let mut entry = Self::rune_id_to_rune_entry(rune_id.clone()).unwrap();
				entry.burned = entry.burned.checked_add(burned.n()).unwrap();
				RuneIdToRuneEntry::<T>::insert(rune_id, entry);
			}
			Ok(())
		}

		fn tx_commits_to_rune(tx: &Transaction, rune: Rune) -> Result<bool> {
			let commitment = rune.commitment();

			for input in &tx.input {
				// extracting a tapscript does not indicate that the input being spent
				// was actually a taproot output. this is checked below, when we load the
				// output's entry from the database
				let Some(tapscript) = input.witness.tapscript() else {
					continue;
				};

				for instruction in tapscript.instructions() {
					// ignore errors, since the extracted script may not be valid
					let Ok(instruction) = instruction else {
						break;
					};

					let Some(pushbytes) = instruction.push_bytes() else {
						continue;
					};

					if pushbytes.as_bytes() != commitment {
						continue;
					}

					let v = input.previous_output.txid.to_byte_array();
					let mut txid_content = [0u8; 32];
					txid_content.copy_from_slice(v.as_slice());
					let txid = Txid(txid_content);
					let tx_info = Self::get_raw_tx(txid)?;
					let taproot = tx_info.vout[input.previous_output.vout as usize]
						.script_pub_key
						.script()
						.map_err(|e| OrdError::Params(e.to_string()))?
						.is_p2tr();

					if !taproot {
						continue;
					}
					return Ok(true);
				}
			}
			Ok(false)
		}
	}
}

pub(crate) struct RuneUpdater {
	pub(crate) block_time: u32,
	pub(crate) burned: BTreeMap<RuneId, Lot>,
	pub(crate) event_handler: Option<Box<dyn Fn(Event)>>,
	pub(crate) height: u32,
	pub(crate) minimum: Rune,
}
