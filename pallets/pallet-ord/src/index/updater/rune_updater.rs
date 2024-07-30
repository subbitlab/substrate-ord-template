use crate::index::lot::Lot;
use crate::index::{entry::RuneBalance, *};
use bitcoin::{OutPoint, Transaction, Txid};
use ordinals::{Artifact, Edict, Etching, Rune, RuneId, Runestone, SpacedRune};
use std::collections::HashMap;

/*
pub(super) struct RuneUpdater {
	pub(super) block_time: u32,
	pub(super) burned: HashMap<RuneId, Lot>,
	pub(super) event_handler: Option<Box<dyn Fn(Event)>>,
	pub(super) height: u32,
	pub(super) minimum: Rune,
}

impl RuneUpdater {






}*/
