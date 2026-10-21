/** A state's value: a name ("heating") or a number (80). */
export type ExpectationValue = string | number;
type Maybe<T> = T | Promise<T>;

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
  /** Connects to the runner (http://127.0.0.1:4477 by default) and serves it. */
  start(url?: string): void;
  stop(): void;
}
