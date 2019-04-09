// Copyright 2017, 2018 Parity Technologies
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use hash_db::{HashDB, Hasher};
use super::{Result, DBValue, TrieDBMut, TrieMut, TrieLayOut, TrieHash, CError};
use node_codec::NodeCodec;

/// A mutable `Trie` implementation which hashes keys and uses a generic `HashDB` backing database.
/// Additionaly it stores inserted hash-key mappings for later retrieval.
///
/// Use it as a `Trie` or `TrieMut` trait object.
pub struct FatDBMut<'db, L>
where
	L: TrieLayOut,
{
	raw: TrieDBMut<'db, L>,
}

impl<'db, L> FatDBMut<'db, L>
where
	L: TrieLayOut,
{
	/// Create a new trie with the backing database `db` and empty `root`
	/// Initialise to the state entailed by the genesis block.
	/// This guarantees the trie is built correctly.
	pub fn new(db: &'db mut HashDB<L::H, DBValue>, root: &'db mut TrieHash<L>, layout: L) -> Self {
		FatDBMut { raw: TrieDBMut::new(db, root, layout) }
	}

	/// Create a new trie with the backing database `db` and `root`.
	///
	/// Returns an error if root does not exist.
	pub fn from_existing(db: &'db mut HashDB<L::H, DBValue>, root: &'db mut TrieHash<L>, layout: L) -> Result<Self, TrieHash<L>, CError<L>> {
		Ok(FatDBMut { raw: TrieDBMut::from_existing(db, root, layout)? })
	}

	/// Get the backing database.
	pub fn db(&self) -> &HashDB<L::H, DBValue> {
		self.raw.db()
	}

	/// Get the backing database.
	pub fn db_mut(&mut self) -> &mut HashDB<L::H, DBValue> {
		self.raw.db_mut()
	}
}

impl<'db, L> TrieMut<L::H, L::C> for FatDBMut<'db, L>
where
	L: TrieLayOut,
{
	fn root(&mut self) -> &TrieHash<L> { self.raw.root() }

	fn is_empty(&self) -> bool { self.raw.is_empty() }

	fn contains(&self, key: &[u8]) -> Result<bool, TrieHash<L>, CError<L>> {
		self.raw.contains(L::H::hash(key).as_ref())
	}

	fn get<'a, 'key>(&'a self, key: &'key [u8]) -> Result<Option<DBValue>, TrieHash<L>, CError<L>>
		where 'a: 'key
	{
		self.raw.get(L::H::hash(key).as_ref())
	}

	fn insert(&mut self, key: &[u8], value: &[u8]) -> Result<Option<DBValue>, TrieHash<L>, CError<L>> {
		let hash = L::H::hash(key);
		let out = self.raw.insert(hash.as_ref(), value)?;
		let db = self.raw.db_mut();

		// insert if it doesn't exist.
		if out.is_none() {
			let aux_hash = L::H::hash(hash.as_ref());
			db.emplace(aux_hash, &[], DBValue::from_slice(key));
		}
		Ok(out)
	}

  fn remove(&mut self, key: &[u8]) -> Result<Option<DBValue>, TrieHash<L>, CError<L>> {
		let hash = L::H::hash(key);
		let out = self.raw.remove(hash.as_ref())?;

		// remove if it already exists.
		if out.is_some() {
			let aux_hash = L::H::hash(hash.as_ref());
			self.raw.db_mut().remove(&aux_hash, &[]);
		}

		Ok(out)
	}
}

#[cfg(test)]
mod test {
	use DBValue;
	use memory_db::{MemoryDB, HashKey};
	use hash_db::Hasher;
	use keccak_hasher::KeccakHasher;
	use reference_trie::{RefFatDBMut, RefTrieDB, Trie, TrieMut};

	#[test]
	fn fatdbmut_to_trie() {
		let mut memdb = MemoryDB::<KeccakHasher, HashKey<_>, _>::default();
		let mut root = Default::default();
		{
			let mut t = RefFatDBMut::new(&mut memdb, &mut root, Default::default());
			t.insert(&[0x01u8, 0x23], &[0x01u8, 0x23]).unwrap();
		}
		let t = RefTrieDB::new(&memdb, &root).unwrap();
		assert_eq!(t.get(&KeccakHasher::hash(&[0x01u8, 0x23])), Ok(Some(DBValue::from_slice(&[0x01u8, 0x23]))));
	}

	#[test]
	fn fatdbmut_insert_remove_key_mapping() {
		let mut memdb = MemoryDB::<KeccakHasher, HashKey<_>, _>::default();
		let mut root = Default::default();
		let key = [0x01u8, 0x23];
		let val = [0x01u8, 0x24];
		let key_hash = KeccakHasher::hash(&key);
		let aux_hash = KeccakHasher::hash(&key_hash);
		let mut t = RefFatDBMut::new(&mut memdb, &mut root, Default::default());
		t.insert(&key, &val).unwrap();
		assert_eq!(t.get(&key), Ok(Some(DBValue::from_slice(&val))));
		assert_eq!(t.db().get(&aux_hash, &[]), Some(DBValue::from_slice(&key)));
		t.remove(&key).unwrap();
		assert_eq!(t.db().get(&aux_hash, &[]), None);
	}
}
