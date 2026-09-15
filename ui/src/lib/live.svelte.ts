// One shared, self-refreshing cluster report.
//
// Six of the seven pages want the same object, and none of them should each be
// polling for it. This is a single subscription with a reference count: the
// first component to call `live.attach()` starts it, the last to detach stops
// it, and every page reads the same `$state`.

import { api, subscribe, currentSource, type ClusterReport, type Source } from './api';
import { recordSnapshot, type Snapshot } from './history';

class Live {
  report = $state<ClusterReport | null>(null);
  error = $state<string | null>(null);
  source = $state<Source>('unknown');
  updatedAt = $state<number>(0);
  paused = $state(false);
  snapshots = $state.raw<Snapshot[]>([]);

  #refs = 0;
  #refresh: Promise<void> | null = null;
  #stop: (() => void) | null = null;

  /** Start (or join) the subscription. Returns the detach function. */
  attach(): () => void {
    this.#refs++;
    if (this.#refs === 1) this.#start();
    let detached = false;
    return () => {
      if (detached) return;
      detached = true;
      this.#refs--;
      if (this.#refs === 0) this.#stopAll();
    };
  }

  refresh(): Promise<void> {
    // Several SSE deltas can arrive together. Share the request, not just the subscription.
    if (this.#refresh) return this.#refresh;
    this.#refresh = (async () => {
      try {
        const report = await api.clusterReport();
        if (!this.paused && this.#refs > 0) {
          this.report = report;
          this.updatedAt = Date.now();
          this.snapshots = recordSnapshot(this.snapshots, report, this.updatedAt);
          this.error = null;
        }
      } catch (e) {
        if (this.#refs > 0) this.error = e instanceof Error ? e.message : String(e);
      } finally {
        this.source = currentSource();
        this.#refresh = null;
      }
    })();
    return this.#refresh;
  }

  #start(): void {
    void this.refresh();
    this.#stop = subscribe((_event, data) => {
      if (this.paused) return;
      // The demo ticker hands back a whole report; a real gateway sends deltas
      // per event, so anything that is not a report just triggers a re-read.
      if (currentSource() === 'demo' && data && typeof data === 'object' && 'nodes' in data) {
        this.report = data as ClusterReport;
        this.updatedAt = Date.now();
        this.error = null;
      } else {
        void this.refresh();
      }
      this.source = currentSource();
    }, 2000, (error) => { this.error = error.message; });
  }

  #stopAll(): void {
    this.#stop?.();
    this.#stop = null;
  }
}

export const live = new Live();
