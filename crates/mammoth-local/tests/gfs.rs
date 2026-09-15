use mammoth_local::gfs::{Error, Event, Settings, Simulation};

fn cluster(workers: usize) -> Simulation {
    Simulation::new(
        Settings { chunk_size: 8, ..Settings::default() },
        (0..workers).map(|i| format!("rack-{}", i % 3)).collect(),
    )
    .unwrap()
}

fn three_ticks(cluster: &mut Simulation) -> Vec<Event> {
    (0..3).flat_map(|_| cluster.tick().unwrap()).collect()
}

#[test]
fn settings_reject_invalid_values() {
    for settings in [
        Settings { chunk_size: 0, ..Settings::default() },
        Settings { replication: 0, ..Settings::default() },
        Settings { heartbeat_secs: 0, ..Settings::default() },
        Settings { missed_heartbeats: 0, ..Settings::default() },
        Settings { lease_secs: 0, ..Settings::default() },
        Settings { heartbeat_secs: u64::MAX, ..Settings::default() },
    ] {
        assert!(matches!(Simulation::new(settings, vec!["a".into(); 3]), Err(Error::InvalidInput)));
    }
    assert!(matches!(
        Simulation::new(Settings::default(), vec!["a".into(); 2]),
        Err(Error::NotEnoughWorkers)
    ));
}

#[test]
fn chunk_boundaries_empty_files_and_rack_placement() {
    let mut sim = cluster(6);
    for size in [0_usize, 1, 7, 8, 9, 16, 19, 1024] {
        let bytes: Vec<_> = (0..size).map(|i| (i % 251) as u8).collect();
        let path = format!("/file-{size}");
        sim.create_file(&path, &bytes).unwrap();
        assert_eq!(sim.read_file(&path).unwrap(), bytes);
        let locations = sim.locate(&path).unwrap();
        assert_eq!(locations.len(), size.div_ceil(8));
        assert_eq!(locations.iter().map(|l| l.len).sum::<usize>(), size);
        for location in locations {
            let racks: std::collections::BTreeSet<_> =
                location.workers.iter().map(|i| i % 3).collect();
            assert_eq!(location.workers.len(), 3);
            assert_eq!(racks.len(), 3);
        }
    }
}

#[test]
fn invalid_paths_and_failed_create_leave_no_file() {
    let mut sim = cluster(3);
    for path in ["", "relative", "/", "/a//b", "/a/../b", "/a/./b"] {
        assert_eq!(sim.create_file(path, b"x"), Err(Error::InvalidInput));
    }
    sim.create_file("/existing", b"original").unwrap();
    assert_eq!(sim.create_file("/existing", b"new"), Err(Error::AlreadyExists));
    sim.stop_worker(0).unwrap();
    assert_eq!(sim.create_file("/failed", b"x"), Err(Error::NotEnoughWorkers));
    assert!(matches!(sim.locate("/failed"), Err(Error::NotFound)));
    assert_eq!(sim.read_file("/existing").unwrap(), b"original");
}

#[test]
fn failure_is_detected_on_third_miss_and_repaired_once() {
    let mut sim = cluster(6);
    sim.create_file("/file", b"abcdefghTAIL").unwrap();
    let old = sim.locate("/file").unwrap()[0].clone();
    let victim = old.workers[0];
    sim.lose_worker_data(victim).unwrap();
    assert_eq!(sim.read_file("/file").unwrap(), b"abcdefghTAIL");
    assert!(sim.tick().unwrap().is_empty());
    assert!(sim.tick().unwrap().is_empty());
    assert_eq!(sim.now(), 60);
    let events = sim.tick().unwrap();
    assert_eq!(sim.now(), 90);
    assert!(events.contains(&Event::WorkerDead(victim)));
    assert!(events.iter().any(|e| matches!(e, Event::Repaired { .. })));
    let repaired = &sim.locate("/file").unwrap()[0];
    assert_eq!(repaired.workers.len(), 3);
    assert!(!repaired.workers.contains(&victim));
    assert_eq!(sim.read_file("/file").unwrap(), b"abcdefghTAIL");
    assert!(sim.tick().unwrap().is_empty());
}

#[test]
fn a_heartbeat_resets_the_failure_deadline() {
    let mut sim = cluster(3);
    sim.stop_worker(0).unwrap();
    assert!(sim.tick().unwrap().is_empty());
    sim.start_worker(0).unwrap();
    assert!(sim.tick().unwrap().is_empty());
    sim.stop_worker(0).unwrap();
    assert!(sim.tick().unwrap().is_empty());
    assert!(sim.tick().unwrap().is_empty());
    assert!(sim.tick().unwrap().contains(&Event::WorkerDead(0)));
    assert_eq!(sim.now(), 150);
}

#[test]
fn insufficient_destinations_are_reported_and_retried_after_return() {
    let mut sim = cluster(3);
    sim.create_file("/file", b"abcdefgh").unwrap();
    sim.lose_worker_data(0).unwrap();
    assert!(three_ticks(&mut sim).contains(&Event::UnderReplicated {
        chunk: 0,
        available: 2,
        wanted: 3,
    }));
    assert_eq!(sim.read_file("/file").unwrap(), b"abcdefgh");
    assert_eq!(sim.lease(0), Err(Error::NotEnoughWorkers));
    sim.start_worker(0).unwrap();
    assert!(sim.tick().unwrap().iter().any(|e| matches!(e, Event::Repaired { to: 0, .. })));
    assert_eq!(sim.replica_bytes(0, 0).unwrap(), b"abcdefgh");
    sim.lease(0).unwrap();
}

#[test]
fn no_replica_means_unavailable_and_never_fabricated_bytes() {
    let mut sim = cluster(6);
    sim.create_file("/file", b"abcdefgh").unwrap();
    for worker in sim.locate("/file").unwrap()[0].workers.clone() {
        sim.lose_worker_data(worker).unwrap();
    }
    assert!(three_ticks(&mut sim).contains(&Event::ChunkUnavailable(0)));
    assert_eq!(sim.read_file("/file"), Err(Error::DataUnavailable));
    assert_eq!(sim.lease(0), Err(Error::NotEnoughWorkers));
}

#[test]
fn concurrent_clients_follow_primary_order_for_every_interleaving() {
    // Stage A, B, C before any commit, then explore all six primary schedules.
    // This tests interleaved clients, not host-thread timing or network delivery.
    for order in [[0, 1, 2], [0, 2, 1], [1, 0, 2], [1, 2, 0], [2, 0, 1], [2, 1, 0]] {
        let mut sim = cluster(3);
        sim.create_file("/file", b"abcdefgh").unwrap();
        let lease = sim.lease(0).unwrap();
        let data = [b"AAAA", b"BBBB", b"CCCC"];
        let mutations = data.map(|bytes| sim.stage(lease, bytes).unwrap());
        assert_eq!(sim.read_file("/file").unwrap(), b"abcdefgh");
        for (sequence, &client) in order.iter().enumerate() {
            assert_eq!(sim.commit(lease, mutations[client], 2).unwrap(), sequence as u64 + 1);
            let mut expected = b"abcdefgh".to_vec();
            expected[2..6].copy_from_slice(data[client]);
            for worker in 0..3 {
                assert_eq!(sim.replica_bytes(0, worker).unwrap(), expected);
            }
        }
    }
}

#[test]
fn duplicate_unknown_and_out_of_bounds_mutations_cannot_change_data() {
    let mut sim = cluster(3);
    sim.create_file("/file", b"abcdefgh").unwrap();
    let lease = sim.lease(0).unwrap();
    let mutation = sim.stage(lease, b"XX").unwrap();
    assert_eq!(sim.commit(lease, mutation, usize::MAX), Err(Error::OutOfBounds));
    assert_eq!(sim.commit(lease, mutation, 7), Err(Error::OutOfBounds));
    assert_eq!(sim.commit(lease, mutation + 1, 0), Err(Error::NotStaged));
    assert_eq!(sim.read_file("/file").unwrap(), b"abcdefgh");
    assert_eq!(sim.commit(lease, mutation, 0).unwrap(), 1);
    assert_eq!(sim.commit(lease, mutation, 2), Err(Error::NotStaged));
    assert_eq!(sim.read_file("/file").unwrap(), b"XXcdefgh");
}

#[test]
fn replica_failure_before_commit_never_partially_applies_a_model_event() {
    let mut sim = cluster(6);
    sim.create_file("/file", b"abcdefgh").unwrap();
    let lease = sim.lease(0).unwrap();
    let mutation = sim.stage(lease, b"XXXX").unwrap();
    let secondary = sim.locate("/file").unwrap()[0].workers[1];
    sim.stop_worker(secondary).unwrap();
    assert_eq!(sim.commit(lease, mutation, 0), Err(Error::NotEnoughWorkers));
    assert_eq!(sim.read_file("/file").unwrap(), b"abcdefgh");
    three_ticks(&mut sim);
    assert_eq!(sim.commit(lease, mutation, 0), Err(Error::StaleLease));
    let fresh = sim.lease(0).unwrap();
    assert_eq!(sim.commit(fresh, mutation, 0), Err(Error::NotStaged));
}

#[test]
fn expired_lease_rejects_staged_writes_at_exact_boundary() {
    let mut sim = cluster(3);
    sim.create_file("/file", b"abcdefgh").unwrap();
    let old = sim.lease(0).unwrap();
    let mutation = sim.stage(old, b"XXXX").unwrap();
    sim.tick().unwrap();
    assert_eq!(sim.lease(0).unwrap(), old);
    sim.tick().unwrap();
    assert_eq!(sim.now(), old.expires_at());
    assert_eq!(sim.commit(old, mutation, 0), Err(Error::StaleLease));
    let fresh = sim.lease(0).unwrap();
    assert_ne!(old, fresh);
    assert_eq!(sim.stage(old, b"XXXX"), Err(Error::StaleLease));
    assert_eq!(sim.commit(fresh, mutation, 0), Err(Error::NotStaged));
}

#[test]
fn stopped_primary_is_not_replaced_before_lease_expiry() {
    let mut sim = cluster(6);
    sim.create_file("/file", b"abcdefgh").unwrap();
    let old = sim.lease(0).unwrap();
    sim.stop_worker(old.primary()).unwrap();
    sim.tick().unwrap();
    assert_eq!(sim.lease(0), Err(Error::PrimaryUnavailable));
    sim.tick().unwrap();
    assert_eq!(sim.lease(0), Err(Error::NotEnoughWorkers));
    sim.tick().unwrap();
    assert_ne!(sim.lease(0).unwrap().primary(), old.primary());
}

#[test]
fn stale_returning_worker_cannot_serve_cached_old_bytes() {
    let mut sim = cluster(6);
    sim.create_file("/file", b"abcdefgh").unwrap();
    let old_lease = sim.lease(0).unwrap();
    let mut stale = sim.locate("/file").unwrap()[0].clone();
    let victim = stale.workers[0];
    stale.workers = vec![victim];
    sim.stop_worker(victim).unwrap();
    three_ticks(&mut sim);
    let fresh = sim.lease(0).unwrap();
    let mutation = sim.stage(fresh, b"XXXX").unwrap();
    sim.commit(fresh, mutation, 0).unwrap();
    sim.start_worker(victim).unwrap();
    assert_eq!(sim.read_location(&stale), Err(Error::DataUnavailable));
    sim.tick().unwrap();
    assert_eq!(sim.read_location(&stale), Err(Error::DataUnavailable));
    assert_eq!(sim.stage(old_lease, b"OOOO"), Err(Error::StaleLease));
    assert_eq!(sim.read_file("/file").unwrap(), b"XXXXefgh");
}

#[test]
fn repair_does_not_shorten_an_unexpired_primary_lease() {
    let mut sim = Simulation::new(
        Settings { chunk_size: 8, lease_secs: 300, ..Settings::default() },
        (0..6).map(|i| format!("rack-{}", i % 3)).collect(),
    )
    .unwrap();
    sim.create_file("/file", b"abcdefgh").unwrap();
    let old = sim.lease(0).unwrap();
    sim.stop_worker(old.primary()).unwrap();
    three_ticks(&mut sim);
    assert_eq!(sim.locate("/file").unwrap()[0].workers.len(), 3);
    assert_eq!(sim.lease(0), Err(Error::PrimaryUnavailable));
    sim.start_worker(old.primary()).unwrap();
    sim.tick().unwrap();
    assert_eq!(sim.lease(0), Err(Error::PrimaryUnavailable));
    while sim.now() < old.expires_at() {
        sim.tick().unwrap();
    }
    assert_ne!(sim.lease(0).unwrap().primary(), old.primary());
}

#[test]
fn master_takeover_preserves_index_and_fences_an_unexpired_lease() {
    let mut sim = Simulation::new(
        Settings { chunk_size: 8, lease_secs: 300, ..Settings::default() },
        vec!["a".into(), "b".into(), "c".into()],
    )
    .unwrap();
    sim.create_file("/file", b"abcdefghTAIL").unwrap();
    let old = sim.lease(0).unwrap();
    let committed = sim.stage(old, b"AAAA").unwrap();
    sim.commit(old, committed, 0).unwrap();
    let pending = sim.stage(old, b"BBBB").unwrap();
    let cached = sim.locate("/file").unwrap();
    sim.stop_master(0).unwrap();
    assert_eq!(sim.read_file("/file"), Err(Error::MasterUnavailable));
    assert_eq!(sim.read_location(&cached[0]).unwrap(), b"AAAAefgh");
    assert!(sim.tick().unwrap().is_empty());
    assert!(sim.tick().unwrap().is_empty());
    assert_eq!(sim.active_master(), 0);
    assert!(sim.tick().unwrap().contains(&Event::MasterPromoted { master: 1, epoch: 2 }));
    assert!(sim.now() < old.expires_at());
    assert_eq!(sim.read_file("/file").unwrap(), b"AAAAefghTAIL");
    assert_eq!(sim.commit(old, pending, 0), Err(Error::StaleLease));
    let fresh = sim.lease(0).unwrap();
    let retry = sim.stage(fresh, b"DONE").unwrap();
    assert_eq!(sim.commit(fresh, retry, 0).unwrap(), 2);
    sim.start_master(0).unwrap();
    assert_eq!(sim.active_master(), 1);
    sim.stop_master(1).unwrap();
    assert!(three_ticks(&mut sim).contains(&Event::MasterPromoted { master: 0, epoch: 3 }));
    assert_eq!(sim.read_file("/file").unwrap(), b"DONEefghTAIL");
}

#[test]
fn both_masters_down_cannot_accept_operations_or_invent_a_leader() {
    let mut sim = cluster(3);
    sim.create_file("/file", b"abcdefgh").unwrap();
    let cached = sim.locate("/file").unwrap();
    sim.stop_master(0).unwrap();
    sim.stop_master(1).unwrap();
    assert!(three_ticks(&mut sim).is_empty());
    assert_eq!(sim.create_file("/new", b"x"), Err(Error::MasterUnavailable));
    assert_eq!(sim.lease(0), Err(Error::MasterUnavailable));
    assert_eq!(sim.read_location(&cached[0]).unwrap(), b"abcdefgh");
}
