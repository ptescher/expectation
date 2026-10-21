// The kettle itself, the same on every platform: the dial has to stay above 55 % for 0.6 s
// before it starts, starting takes 0.22 s, and while the child lock is on the dial and the
// buttons (all but LOCK and RESET) do nothing.

export const PRESETS = ['WARM', 'HOT', 'BOIL'] as const;
const TARGET = [70, 85, 100];

export type Phase = 'off' | 'starting' | 'heating' | 'dry';

export class Kettle {
  phase: Phase = 'off';
  dial = 0;
  preset = 0;
  lock = false;
  temperature = 20;
  private hold = 0;
  private startingFor = 0;

  tick(dt: number) {
    const dial = this.lock ? 0 : this.dial;
    switch (this.phase) {
      case 'off':
        if (dial > 55) {
          this.hold += dt;
          if (this.hold > 0.6) {
            this.phase = 'starting';
            this.startingFor = 0;
            this.hold = 0;
          }
        } else {
          this.hold = 0;
        }
        break;
      case 'starting':
        this.startingFor += dt;
        if (this.startingFor >= 0.22) this.phase = 'heating';
        break;
      case 'heating':
        this.temperature = Math.min(this.temperature + dt * 8, TARGET[this.preset]);
        break;
      case 'dry':
        break;
    }
  }

  start() {
    if (!this.lock && this.phase === 'off') {
      this.phase = 'starting';
      this.startingFor = 0;
    }
  }

  reset() {
    this.phase = 'off';
    this.hold = 0;
    this.temperature = 20;
  }

  pressPreset() {
    if (!this.lock) this.preset = (this.preset + 1) % PRESETS.length;
  }

  pressLock() {
    this.lock = !this.lock;
  }

  runOut() {
    if (this.phase === 'heating' || this.phase === 'starting') this.phase = 'dry';
  }

  /** For tests: put the kettle in a phase directly. */
  force(phase: string) {
    if (phase === 'starting') {
      this.phase = 'starting';
      this.startingFor = 0;
    } else if (phase === 'heating') {
      this.phase = 'heating';
      this.temperature = 60;
    } else if (phase === 'dry') {
      this.phase = 'dry';
    } else {
      this.reset();
    }
  }

  restore() {
    this.reset();
    this.dial = 0;
    this.preset = 0;
    this.lock = false;
  }
}
