use mammoth_local::gfs::{Error, Event, Settings, Simulation};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("GFS local demonstration — in memory, logical time, no network or disk durability\n");
    let settings = Settings { chunk_size: 8, ..Settings::default() };
    let mut cluster = Simulation::new(
        settings,
        ["rack-a", "rack-b", "rack-c", "rack-a", "rack-b", "rack-c"].map(str::to_owned).to_vec(),
    )?;

    let path = "/videos/example";
    let original = b"abcdefgh01234567TAIL";
    cluster.create_file(path, original)?;
    println!("1. Master index: {path}, {} bytes, 8-byte teaching chunks", original.len());
    for location in cluster.locate(path)? {
        println!(
            "   chunk {}: {} bytes, workers {:?}",
            location.chunk, location.len, location.workers
        );
    }
    assert_eq!(cluster.read_file(path)?, original);
    println!("   Client reads worker chunks directly and reconstructs the file.\n");

    let chunk = cluster.locate(path)?[0].chunk;
    let lease = cluster.lease(chunk)?;
    let alice = cluster.stage(lease, b"AAAA")?;
    let bob = cluster.stage(lease, b"BBBB")?;
    println!("2. Alice and Bob stage overlapping writes on all three replicas.");
    assert_eq!(cluster.read_file(path)?, original);
    println!(
        "   Worker {} holds the primary lease; staging has not changed the file.",
        lease.primary()
    );
    println!("   Primary orders Bob first: sequence {}", cluster.commit(lease, bob, 0)?);
    println!("   Primary orders Alice next: sequence {}", cluster.commit(lease, alice, 0)?);
    let expected = b"AAAAefgh01234567TAIL";
    assert_eq!(cluster.read_file(path)?, expected);
    for worker in cluster.locate(path)?[0].workers.clone() {
        assert_eq!(cluster.replica_bytes(chunk, worker)?, b"AAAAefgh");
    }
    println!("   All replicas contain AAAAefgh.\n");

    cluster.lose_worker_data(lease.primary())?;
    println!("3. Lose primary worker {} and its data; reads retry another copy.", lease.primary());
    assert_eq!(cluster.read_file(path)?, expected);
    for _ in 0..3 {
        let events = cluster.tick()?;
        println!("   t={}s: {events:?}", cluster.now());
    }
    assert_eq!(cluster.locate(path)?[0].workers.len(), 3);
    assert_eq!(cluster.read_file(path)?, expected);
    println!("   Three missed 30-second heartbeats trigger automatic repair.\n");

    let lease = cluster.lease(chunk)?;
    let pending = cluster.stage(lease, b"ZZZZ")?;
    let cached = cluster.locate(path)?;
    cluster.stop_master(0)?;
    assert_eq!(cluster.read_file(path), Err(Error::MasterUnavailable));
    assert_eq!(cluster.read_location(&cached[0])?, b"AAAAefgh");
    println!("4. Stop master 0: new lookups wait, cached worker reads still work.");
    for _ in 0..3 {
        for event in cluster.tick()? {
            if matches!(event, Event::MasterPromoted { .. }) {
                println!("   t={}s: {event:?}", cluster.now());
            }
        }
    }
    assert_eq!(cluster.active_master(), 1);
    assert_eq!(cluster.read_file(path)?, expected);
    assert_eq!(cluster.commit(lease, pending, 0), Err(Error::StaleLease));
    let fresh = cluster.lease(chunk)?;
    let retry = cluster.stage(fresh, b"DONE")?;
    cluster.commit(fresh, retry, 0)?;
    println!("   Standby preserves the index; old authority is fenced. Retry with a fresh lease.");
    println!("   Final file: {}", String::from_utf8(cluster.read_file(path)?)?);
    println!(
        "\nVerified: chunking, replication, ordering, repair, takeover, and stale-lease rejection."
    );
    println!("Real RPC, persistent logs, Raft, DNS integration, and throughput benchmarks remain planned.");
    Ok(())
}
