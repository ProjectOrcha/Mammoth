//! 14 · Progress bars that disappear when nobody is watching.
//!
//!     cargo run -q -p mammoth-parts --example 14-progress
//!     cargo run -q -p mammoth-parts --example 14-progress 2>/dev/null
//!     cargo run -q -p mammoth-parts --example 14-progress | cat
//!
//! Design principle 4 from `crates/mammoth-cli/src/main.rs`: *progress bars on
//! anything over a second, auto-disabled when piped.*
//!
//! Two rules, and the second one is the one people get wrong:
//!
//!   1. **Progress goes to stderr.** `mammoth put big.log /data/ > receipt.txt`
//!      must leave a clean receipt, and the human must still see the bar. Both
//!      only work if the bar is on stderr.
//!   2. **Draw nothing when stderr is not a terminal.** A CI log with four
//!      thousand redraw frames in it is worse than no progress at all.
//!
//! indicatif does (2) for you if you let it: `ProgressDrawTarget::stderr()`
//! becomes a hidden target automatically when stderr is not a tty.

use std::time::Duration;

use indicatif::{HumanBytes, MultiProgress, ProgressBar, ProgressDrawTarget, ProgressStyle};

fn main() {
    println!("stdout: this line is the command's actual output");

    determinate();
    spinner();
    replicas();

    // The point of rule 1: this went to stdout, uninterrupted by any bar.
    println!("stdout:   ✔ /data/big.log   350.0 MB · 3 blocks · replication 3");
    println!();
    println!("Now run it again as `... 2>/dev/null` — the bars vanish, the two");
    println!("stdout lines remain. Then as `... | cat` — the bars still draw (they");
    println!("are on stderr, which is still your terminal) and stdout is clean.");
}

/// A bar for work whose size you know: bytes of a file, blocks of a write.
fn determinate() {
    let total: u64 = 350 * 1024 * 1024;

    let pb = ProgressBar::new(total);
    // stderr(), not stdout(). And indicatif hides itself when stderr is a pipe.
    pb.set_draw_target(ProgressDrawTarget::stderr());
    pb.set_style(
        ProgressStyle::with_template(
            "  {msg:<18} {bar:28.cyan/blue} {bytes:>10}/{total_bytes:<10} {bytes_per_sec:>11}  eta {eta}",
        )
        .expect("template")
        // The same eighth-blocks as `mammoth-viz::bar`, so the CLI looks like
        // one program rather than three libraries stapled together.
        .progress_chars("█▉▊▋▌▍▎▏░"),
    );
    pb.set_message("put big.log");

    let mut done = 0;
    while done < total {
        let step = 12 * 1024 * 1024;
        done = (done + step).min(total);
        pb.set_position(done);
        std::thread::sleep(Duration::from_millis(30));
    }
    // `finish_and_clear` leaves no wreckage on the screen. `finish_with_message`
    // leaves a one-line summary. Prefer clear, and print your own summary to
    // stdout — that way the summary survives a pipe and the bar does not.
    pb.finish_and_clear();
    eprintln!("  wrote {} in 0.9s", HumanBytes(total));
}

/// A spinner for work whose size you do not know: a network call, a scan.
fn spinner() {
    let pb = ProgressBar::new_spinner();
    pb.set_draw_target(ProgressDrawTarget::stderr());
    pb.set_style(
        ProgressStyle::with_template("  {spinner:.green} {msg}")
            .expect("template")
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏", "✔"]),
    );
    pb.set_message("waiting for block reports from 6 workers");
    // `enable_steady_tick` animates on its own thread, so the spinner keeps
    // moving while your code is blocked on something slow.
    pb.enable_steady_tick(Duration::from_millis(80));
    std::thread::sleep(Duration::from_millis(900));
    pb.finish_and_clear();
    eprintln!("  ✔ 6/6 workers reported");
}

/// Several bars at once — one per replica, which is what a real pipelined write
/// looks like from the client's side.
fn replicas() {
    let multi = MultiProgress::with_draw_target(ProgressDrawTarget::stderr());
    let style = ProgressStyle::with_template("  {msg:<22} {bar:24.green/black} {percent:>3}%")
        .expect("template")
        .progress_chars("█▉▊▋▌▍▎▏░");

    let bars: Vec<ProgressBar> = ["w1  /dc1/rack-a", "w4  /dc1/rack-b", "w5  /dc1/rack-b"]
        .iter()
        .map(|name| {
            let pb = multi.add(ProgressBar::new(100));
            pb.set_style(style.clone());
            pb.set_message(name.to_string());
            pb
        })
        .collect();

    // Replicas do not finish together; the pipeline is only as fast as its
    // slowest hop. Showing that is the point.
    let speeds = [7u64, 5, 3];
    for _ in 0..40 {
        for (pb, speed) in bars.iter().zip(speeds) {
            if !pb.is_finished() {
                pb.inc(speed);
                if pb.position() >= 100 {
                    pb.finish();
                }
            }
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    for pb in &bars {
        pb.finish_and_clear();
    }
    eprintln!("  ✔ 3/3 replicas acknowledged");
}
