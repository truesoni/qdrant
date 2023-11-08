use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use atomic_refcell::AtomicRefCell;
use common::types::PointOffsetType;
use rand::Rng;
use sparse::common::sparse_vector_fixture::random_sparse_vector;
use sparse::index::inverted_index::inverted_index_ram::InvertedIndexRam;
use sparse::index::inverted_index::InvertedIndex;
use tempfile::Builder;

use crate::common::operation_error::OperationResult;
use crate::common::rocksdb_wrapper::{open_db, DB_VECTOR_CF};
use crate::fixtures::payload_context_fixture::FixtureIdTracker;
use crate::index::sparse_index::sparse_vector_index::SparseVectorIndex;
use crate::index::struct_payload_index::StructPayloadIndex;
use crate::index::VectorIndex;
use crate::payload_storage::in_memory_payload_storage::InMemoryPayloadStorage;
use crate::types::Distance;
use crate::vector_storage::simple_sparse_vector_storage::open_simple_sparse_vector_storage;
use crate::vector_storage::VectorStorage;

/// Helper to open a test sparse vector index
pub fn fixture_open_sparse_index<I: InvertedIndex>(
    index_dir: &Path,
    num_vectors: usize, // used to size the id tracker
) -> OperationResult<SparseVectorIndex<I>> {
    // temp dirs
    let payload_dir = Builder::new().prefix("payload_dir").tempdir().unwrap();
    let storage_dir = Builder::new().prefix("storage_dir").tempdir().unwrap();

    // setup
    let id_tracker = Arc::new(AtomicRefCell::new(FixtureIdTracker::new(num_vectors)));
    let payload_storage = InMemoryPayloadStorage::default();
    let wrapped_payload_storage = Arc::new(AtomicRefCell::new(payload_storage.into()));
    let payload_index = StructPayloadIndex::open(
        wrapped_payload_storage,
        id_tracker.clone(),
        payload_dir.path(),
        true,
    )?;
    let wrapped_payload_index = Arc::new(AtomicRefCell::new(payload_index));

    let db = open_db(storage_dir.path(), &[DB_VECTOR_CF]).unwrap();
    let vector_storage = open_simple_sparse_vector_storage(db, DB_VECTOR_CF, Distance::Dot)?;

    let sparse_vector_index: SparseVectorIndex<I> = SparseVectorIndex::open(
        id_tracker,
        vector_storage.clone(),
        wrapped_payload_index,
        index_dir,
    )?;

    Ok(sparse_vector_index)
}

/// Prepares a sparse vector index with random sparse vectors
pub fn fixture_sparse_index_ram<R: Rng + ?Sized>(
    rnd: &mut R,
    num_vectors: usize,
    max_dim: usize,
    stopped: &AtomicBool,
) -> SparseVectorIndex<InvertedIndexRam> {
    let index_dir = Builder::new().prefix("index_dir").tempdir().unwrap();
    let mut sparse_vector_index = fixture_open_sparse_index(index_dir.path(), num_vectors).unwrap();
    let mut borrowed_storage = sparse_vector_index.vector_storage.borrow_mut();

    // add points to storage
    for idx in 0..num_vectors {
        let vec = &random_sparse_vector(rnd, max_dim);
        borrowed_storage
            .insert_vector(idx as PointOffsetType, vec.into())
            .unwrap();
    }
    drop(borrowed_storage);

    // assert all points are in storage
    assert_eq!(
        sparse_vector_index
            .vector_storage
            .borrow()
            .available_vector_count(),
        num_vectors
    );

    // build index to refresh RAM index
    sparse_vector_index.build_index(stopped).unwrap();
    assert_eq!(sparse_vector_index.indexed_vector_count(), num_vectors);
    sparse_vector_index
}