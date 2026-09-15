use mammoth_local::gfs::{Error, Settings, Simulation};
use mammoth_testkit::Seeded;

#[test]
fn seeded_gfs_teaching_scenario() {
    let seed = std::env::var("MAMMOTH_SIM_SEED")
        .map(|v| v.parse::<u64>().expect("MAMMOTH_SIM_SEED must be u64"))
        .unwrap_or(20260915);
    let count = std::env::var("MAMMOTH_SIM_COUNT")
        .map(|v| v.parse::<u64>().expect("MAMMOTH_SIM_COUNT must be u64"))
        .unwrap_or(1);
    assert!((1..=100_000).contains(&count));
    for offset in 0..count {
        let current = seed.wrapping_add(offset);
        if let Err(error) = std::panic::catch_unwind(|| scenario(current)) {
            eprintln!("failing_seed={current}");
            std::panic::resume_unwind(error);
        }
    }
}

fn scenario(seed: u64) {
    let mut rng = Seeded::new(seed);
    let mut sim = Simulation::new(
        Settings { chunk_size: 8, ..Settings::default() },
        (0..6).map(|i| format!("rack-{}", i % 3)).collect(),
    )
    .unwrap();
    let mut expected = vec![];
    for file in 0..8 {
        let data: Vec<_> =
            (0..(rng.next_u64() % 32 + 1)).map(|_| (rng.next_u64() % 251) as u8).collect();
        let path = format!("/seed-{seed}/file-{file}");
        sim.create_file(&path, &data).unwrap();
        expected.push((path, data));
    }
    for step in 0..64 {
        match rng.next_u64() % 3 {
            0 => {
                let worker = (rng.next_u64() % 6) as usize;
                sim.lose_worker_data(worker).unwrap();
                for _ in 0..3 {
                    sim.tick().unwrap();
                }
                sim.start_worker(worker).unwrap();
                sim.tick().unwrap();
            }
            1 => {
                let leader = sim.active_master();
                sim.stop_master(leader).unwrap();
                assert_eq!(sim.read_file(&expected[0].0), Err(Error::MasterUnavailable));
                for _ in 0..3 {
                    sim.tick().unwrap();
                }
                assert_ne!(sim.active_master(), leader, "seed={seed}, step={step}");
                sim.start_master(leader).unwrap();
            }
            _ => {
                sim.tick().unwrap();
            }
        }
        for (path, data) in &expected {
            assert_eq!(
                &sim.read_file(path).unwrap(),
                data,
                "seed={seed}, step={step}, path={path}"
            );
        }
    }
}
