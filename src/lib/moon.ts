// Moon phase from the mean synodic month: no network, right to within about a day.
const SYNODIC = 29.530588853; // days from one new moon to the next
const NEW_MOON = Date.UTC(2000, 0, 6, 18, 14); // a known new moon

export type Moon = { age: number; lit: number; name: string; waxing: boolean };

/** The moon at a moment: its age in days, the lit fraction (0 to 1), the phase name, and whether it grows. */
export function moonAt(ms: number): Moon {
  const days = (ms - NEW_MOON) / 86_400_000;
  const age = ((days % SYNODIC) + SYNODIC) % SYNODIC;
  const lit = (1 - Math.cos((2 * Math.PI * age) / SYNODIC)) / 2;
  const waxing = age < SYNODIC / 2;
  const names = ["New moon", "Waxing crescent", "First quarter", "Waxing gibbous", "Full moon", "Waning gibbous", "Last quarter", "Waning crescent"];
  const name = names[Math.round((age / SYNODIC) * 8) % 8];
  return { age, lit, name, waxing };
}
