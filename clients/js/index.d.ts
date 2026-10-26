/** A state's value: a name ("heating") or a number (80). */
export type ExpectationValue = string | number;
type Maybe<T> = T | Promise<T>;

/** Something a person can act on. */
export interface ExpectationControl {
  /** The screen or dialog it is in; the same id in two views is two controls. */
  view: string;
  /** Stable, and independent of display copy. */
  id: string;
  kind?: string;
  label?: string;
  /** The vocabulary event using it makes happen, e.g. "the user presses lock". */
  event?: string;
}

export declare const DEFAULT_URL: string;

export declare class ExpectationDriver {
  constructor(options: { platform: string; app: string });
  observe(subject: string, f: () => Maybe<ExpectationValue>): this;
  set(subject: string, f: (value: ExpectationValue) => Maybe<void>): this;
  arrange(subject: string, f: (value: ExpectationValue) => Maybe<void>): this;
  on(event: string, f: () => Maybe<void>): this;
  onReset(f: () => Maybe<void>): this;
  /** Returns a base64 PNG. Without it, the runner can take its own screenshots. */
  onScreenshot(f: () => Maybe<string>): this;
  /** Lists what a person can act on now, so the report can show which controls no test used. */
  onControls(f: () => Maybe<ExpectationControl[]>): this;
  /** The `{view, id}` of controls `set` and `perform` acted on since the last call; empty it as you return. */
  onUsed(f: () => Maybe<{ view: string; id: string }[]>): this;
  /** Connects to the runner (http://127.0.0.1:4477 by default) and serves it. */
  start(url?: string): void;
  stop(): void;
}
