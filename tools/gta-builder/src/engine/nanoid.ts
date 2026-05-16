/**
 * Minimal ID generator — does not need crypto, just needs to be
 * unique within a single generation session.
 */
let _idCounter = 0;
export function nanoid(): string {
  return (++_idCounter).toString(36).padStart(6, '0');
}
export function resetNanoid(): void {
  _idCounter = 0;
}
