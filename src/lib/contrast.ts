// Picks dark or light ink for each part of the dock from how bright the glass behind it is.
// The glass layer (widgets/Glass.tsx) reads back a few lines of what it drew (backdrop plus tint),
// `profile` turns them into brightness buckets along the bar, and the dock gives every cell whose
// stretch of glass is light the `data-bg="light"` attribute (dark ink, see dock.css).

/** Brightness 0..1 of one pixel (sRGB-weighted, on the stored values, which is close enough here). */
export function luma(r: number, g: number, b: number): number {
  return (0.2126 * r + 0.7152 * g + 0.0722 * b) / 255;
}

/**
 * Brightness in `n` buckets along a line of RGBA pixels (premultiplied alpha, as WebGL returns them).
 * Several lines can be passed one after another (`lines`); each bucket averages them all.
 * Fully transparent pixels (outside the rounded glass) are skipped; a bucket with none left is -1.
 */
export function profile(px: Uint8Array, lines: number, n: number): number[] {
  const len = px.length / 4 / lines;
  const sum = new Array<number>(n).fill(0);
  const count = new Array<number>(n).fill(0);
  for (let l = 0; l < lines; l++) {
    for (let i = 0; i < len; i++) {
      const o = (l * len + i) * 4;
      const a = px[o + 3];
      if (a < 8) continue;
      const k = Math.min(n - 1, Math.floor((i / len) * n));
      // Undo premultiplication so a half-transparent edge reads as its real colour.
      const f = 255 / a;
      sum[k] += luma(px[o] * f, px[o + 1] * f, px[o + 2] * f);
      count[k]++;
    }
  }
  return sum.map((s, k) => (count[k] ? Math.min(1, s / count[k]) : -1));
}

/** Average brightness of the buckets between fractions `from` and `to` of the bar (-1 = unknown). */
export function span(buckets: number[], from: number, to: number): number {
  const n = buckets.length;
  if (!n) return -1;
  const a = Math.max(0, Math.min(n - 1, Math.floor(from * n)));
  const b = Math.max(a, Math.min(n - 1, Math.ceil(to * n) - 1));
  let s = 0, c = 0;
  for (let k = a; k <= b; k++) {
    if (buckets[k] >= 0) {
      s += buckets[k];
      c++;
    }
  }
  return c ? s / c : -1;
}

/** Light enough for dark ink? The gap between the two thresholds keeps a cell from flickering
 *  between inks while a video or a scrolling page plays behind it. */
export function isLight(was: boolean, l: number): boolean {
  if (l < 0) return was;
  return was ? l > 0.44 : l > 0.52;
}

/** How strongly to darken glass that always carries white text (flyouts, Settings): 0 when the
 *  backdrop is dark enough already, up to 0.75 so that even a white page behind it comes out at about
 *  0.3 brightness, where white and grey text read well. Uses the brightest quarter of the buckets, since
 *  that's where text gets lost. */
export function shadeFor(buckets: number[]): number {
  const known = buckets.filter((b) => b >= 0).sort((a, b) => b - a);
  if (!known.length) return 0;
  const top = known.slice(0, Math.max(1, Math.ceil(known.length / 4)));
  const m = top.reduce((s, b) => s + b, 0) / top.length;
  return m <= 0.32 ? 0 : Math.min(0.75, 1 - 0.3 / m);
}
