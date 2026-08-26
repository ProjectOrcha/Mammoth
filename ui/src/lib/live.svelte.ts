// One shared, self-refreshing cluster report.
//
// Six of the seven pages want the same object, and none of them should each be
// polling for it. This is a single subscription with a reference count: the
// first component to call `live.attach()` starts it, the last to detach stops
// it, and every page reads the same `$state`.

import { api, subscribe, currentSource, type ClusterReport, type Source } from './api';

class Live {
  report = $state<ClusterReport | null>(null);
  error = $state<string | null>(null);
  source = $state<Source>('unknown');
  updatedAt = $state<number>(0);
  paused = $state(false);

  #refs = 0;
  #stop: (() => void) | null = null;

  /** Start (or join) the subscription. Returns the detach function. */
  attach(): () => void {
    this.#refs++;
    if (this.#refs === 1) this.#start();
    return () => {
      this.#refs--;
      if (this.#refs === 0) this.#stopAll();
    };
  }

  async refresh(): Promise<void> {
    try {
      this.report = await api.clusterReport();
      this.source = currentSource();
      this.updatedAt = Date.now();
      this.error = null;
    } catch (e) {
      this.error = e instanceof Error ? e.message : String(e);
    }
  }

  #start(): void {
    void this.refresh();
    this.#stop = subscribe((_event, data) => {
      if (this.paused) return;
      // The demo ticker hands back a whole report; a real gateway sends deltas
      // per event, so anything that is not a report just triggers a re-read.
      if (data && typeof data === 'object' && 'nodes' in data) {
        this.report = data as ClusterReport;
        this.updatedAt = Date.now();
      } else {
        void this.refresh();
      }
      this.source = currentSource();
    });
  }

  #stopAll(): void {
    this.#stop?.();
    this.#stop = null;
  }
}

export const live = new Live();
