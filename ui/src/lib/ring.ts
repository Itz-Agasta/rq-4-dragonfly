/**
 * Fixed-capacity ring buffer over a pre-allocated `Float64Array`.
 *
 * Sized once at construction and never resized: at 20 Hz a growing array would
 * reallocate every few seconds, and the resulting garbage is what turns a smooth
 * strip chart into a stuttering one.
 */
export class Ring {
  /**
   * Backing store, twice the capacity.
   *
   * Every sample is written at both `head` and `head + capacity`. That costs one
   * extra store per sample and buys a **contiguous** oldest-to-newest window at
   * any head position, which {@link view} can then hand out as a subarray with no
   * copy and no wraparound seam. The obvious alternative, copying the two halves
   * into a scratch array on every read, allocates nothing but does the work again
   * for every chart on every frame.
   */
  private readonly buf: Float64Array;
  private head = -1;
  private filled = 0;

  constructor(readonly capacity: number) {
    this.buf = new Float64Array(capacity * 2);
  }

  /** Append one sample, evicting the oldest once full. */
  push(value: number): void {
    this.head = (this.head + 1) % this.capacity;
    this.buf[this.head] = value;
    this.buf[this.head + this.capacity] = value;
    if (this.filled < this.capacity) this.filled += 1;
  }

  /**
   * Oldest-to-newest window over everything written so far.
   *
   * Borrowed, not owned: the next {@link push} overwrites the newest end of it.
   * Safe to hand straight to a chart that reads it synchronously; never retain it
   * across a frame.
   */
  view(): Float64Array {
    const start = this.head + this.capacity - this.filled + 1;
    return this.buf.subarray(start, start + this.filled);
  }
}
