// Ping samples -> what the cell shows. Pure, so `npm test` covers it. A sample is the round trip in
// ms, or null for a lost ping.

export type PingStats = { last: number | null; avg: number | null; jitter: number | null; loss: number; n: number };

export function stats(samples: (number | null)[]): PingStats {
  const ok = samples.filter((x): x is number => x !== null);
  const n = samples.length;
  const avg = ok.length ? ok.reduce((a, b) => a + b, 0) / ok.length : null;
  // Jitter as the mean difference between consecutive replies, as call apps report it.
  let jitter: number | null = null;
  if (ok.length > 1) {
    let d = 0;
    for (let i = 1; i < ok.length; i++) d += Math.abs(ok[i] - ok[i - 1]);
    jitter = d / (ok.length - 1);
  }
  return { last: n ? samples[n - 1] : null, avg, jitter, loss: n ? (n - ok.length) / n : 0, n };
}

/** good under 60 ms with no loss, poor over 150 ms or 5%+ loss, else ok. */
export function quality(s: PingStats): "good" | "ok" | "poor" | "none" {
  if (!s.n) return "none";
  if (s.last === null && s.loss > 0.5) return "poor";
  if (s.loss >= 0.05 || (s.avg ?? 0) > 150) return "poor";
  if (s.loss > 0 || (s.avg ?? 0) > 60 || (s.jitter ?? 0) > 30) return "ok";
  return "good";
}
