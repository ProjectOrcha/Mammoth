use mammoth_storage::BlockStore;
#[test]
fn verifies_unaligned_ranges_partial_tail_and_invalid_lengths() {
    let dir = tempfile::tempdir().unwrap();
    let store = BlockStore::open(dir.path()).unwrap();
    let data: Vec<_> = (0..10_003).map(|n| (n % 251) as u8).collect();
    store.put(1, &data).unwrap();
    let mut reader = store.open_reader(1).unwrap();
    for range in [0..1, 4095..4097, 9999..10003, 0..10003, 10003..10003] {
        assert_eq!(
            reader.read_range(range.clone()).unwrap(),
            data[range.start as usize..range.end as usize]
        );
    }
    assert!(reader.read_range(0..10004).is_err());
    drop(reader);
    std::fs::OpenOptions::new()
        .write(true)
        .open(store.path(1).join("data"))
        .unwrap()
        .set_len(5)
        .unwrap();
    assert!(store.open_reader(1).is_err());
}
