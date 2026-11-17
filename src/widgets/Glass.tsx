import { useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { profile, shadeFor } from "../lib/contrast";

/** The dock styles drawn as liquid glass. */
export const GLASS_STYLES = ["glass", "tinted", "midnight"];

type Geometry = { mon_w: number; mon_h: number; left: number; top: number; width: number; height: number };
type Wallpaper = { data: string | null; fit: string };
/** A live grab of the screen around the dock (glass.rs): where it sits on the monitor, and its pixels. */
type Live = { x0: number; y0: number; coverW: number; coverH: number };

const TINTS: Record<string, [number, number, number, number]> = {
  glass: [1, 1, 1, 0.1],
  tinted: [0.47, 0.63, 1, 0.24],
  midnight: [0.04, 0.055, 0.12, 0.62],
};
const FITS: Record<string, number> = { fill: 0, fit: 1, stretch: 2, center: 3, tile: 4, span: 0 };

const VERT = `attribute vec2 p; void main() { gl_Position = vec4(p, 0.0, 1.0); }`;

// What's behind the dock (the live screen, or the wallpaper), bent at the rounded edges like thick
// glass, with the colours splitting slightly where it bends most, softened, a little more saturated,
// tinted, lit along the top edge and shaded along the bottom. Drawn only when something changes.
const FRAG = `
precision highp float;
uniform sampler2D img;
uniform vec2 res, win0, winSize, mon, imgSize, cap0, capSize;
uniform int fit, live;
uniform float radius, band, blur;
uniform vec4 tint;

vec2 uvAt(vec2 s) {
  if (live == 1) return (s - cap0) / capSize;
  if (fit == 2) return s / mon;
  if (fit == 4) return fract(s / imgSize);
  float k = fit == 3 ? 1.0 : (fit == 1 ? min(mon.x / imgSize.x, mon.y / imgSize.y) : max(mon.x / imgSize.x, mon.y / imgSize.y));
  vec2 size = imgSize * k;
  return (s - (mon - size) * 0.5) / size;
}
vec3 tap(vec2 s) {
  vec2 uv = uvAt(s);
  if (live == 1) return texture2D(img, clamp(uv, 0.0, 1.0)).rgb;
  if (uv.x < 0.0 || uv.y < 0.0 || uv.x > 1.0 || uv.y > 1.0) return vec3(0.0);
  return texture2D(img, uv).rgb;
}
float sdRound(vec2 p, vec2 b, float r) {
  vec2 q = abs(p) - b + r;
  return length(max(q, 0.0)) + min(max(q.x, q.y), 0.0) - r;
}
void main() {
  vec2 frag = vec2(gl_FragCoord.x, res.y - gl_FragCoord.y);
  vec2 c = res * 0.5, p = frag - c;
  float d = sdRound(p, c, radius);
  vec2 e = vec2(1.0, 0.0);
  vec2 n = normalize(vec2(sdRound(p + e.xy, c, radius) - sdRound(p - e.xy, c, radius),
                          sdRound(p + e.yx, c, radius) - sdRound(p - e.yx, c, radius)) + 1e-5);
  // t: 0 in the flat middle, 1 at the rim. The lens pulls in content from outside the edge.
  float t = clamp(1.0 + d / band, 0.0, 1.0);
  float bend = pow(t, 2.2) * band * 1.1;
  vec2 scale = winSize / res;
  vec2 s = win0 + (frag - n * bend) * scale;
  vec3 col = vec3(0.0);
  for (int i = 0; i < 16; i++) {
    float f = float(i);
    float a = f * 2.39996;
    col += tap(s + vec2(cos(a), sin(a)) * sqrt((f + 0.5) / 16.0) * blur * scale);
  }
  col /= 16.0;
  // Chromatic split near the rim: red bends a little less, blue a little more.
  float split = pow(t, 3.0);
  vec2 sr = win0 + (frag - n * bend * 0.86) * scale;
  vec2 sb = win0 + (frag - n * bend * 1.14) * scale;
  col.r = mix(col.r, tap(sr).r, split * 0.55);
  col.b = mix(col.b, tap(sb).b, split * 0.55);
  float l = dot(col, vec3(0.299, 0.587, 0.114));
  col = mix(vec3(l), col, 1.35);
  col = mix(col, tint.rgb, tint.a);
  // Light from above: a bright rim and a soft sheen along the top, a faint shade along the bottom.
  float up = clamp(-n.y, 0.0, 1.0), down = clamp(n.y, 0.0, 1.0);
  float rim = smoothstep(1.8, 0.0, abs(d + 1.0));
  col += rim * (0.08 + 0.34 * up + 0.06 * down);
  col += pow(t, 4.0) * up * 0.16;
  col += pow(clamp(1.0 - frag.y / res.y, 0.0, 1.0), 6.0) * 0.05;
  col *= 1.0 - pow(t, 3.0) * down * 0.16;
  float alpha = smoothstep(0.8, -0.8, d);
  gl_FragColor = vec4(col * alpha, alpha);
}`;

function program(gl: WebGLRenderingContext) {
  const sh = (type: number, src: string) => {
    const s = gl.createShader(type)!;
    gl.shaderSource(s, src);
    gl.compileShader(s);
    if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) throw new Error(gl.getShaderInfoLog(s) || "shader");
    return s;
  };
  const pr = gl.createProgram()!;
  gl.attachShader(pr, sh(gl.VERTEX_SHADER, VERT));
  gl.attachShader(pr, sh(gl.FRAGMENT_SHADER, FRAG));
  gl.linkProgram(pr);
  gl.useProgram(pr);
  const buf = gl.createBuffer();
  gl.bindBuffer(gl.ARRAY_BUFFER, buf);
  gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 1, -1, -1, 1, 1, 1]), gl.STATIC_DRAW);
  const loc = gl.getAttribLocation(pr, "p");
  gl.enableVertexAttribArray(loc);
  gl.vertexAttribPointer(loc, 2, gl.FLOAT, false, 0, 0);
  return pr;
}

function texParams(gl: WebGLRenderingContext) {
  // Any size of image: clamp and linear filtering only (no mipmaps).
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
}

/** The liquid glass behind the dock's tiles (or behind the settings panel or the agents' popup, by
 *  `window` label). Calls `onReady(false)` if WebGL is missing, or there is neither a live picture nor
 *  a wallpaper to show. */
export default function Glass({ style, radius, tint, blur, live, onReady, onLuma, autoShade = false, window: label = "dock" }: {
  style: string;
  radius: number;
  /** 0 to 1; 0.5 is the style's own tint. */
  tint: number;
  blur: number;
  /** Show the live screen behind the dock (falls back to the wallpaper until a frame arrives). */
  live: boolean;
  onReady: (ok: boolean) => void;
  /** Gets how bright the drawn glass is, in buckets along its long side (left to right, or top to
   *  bottom), so text over light glass can switch to dark ink. Sent at most 4 times a second. */
  onLuma?: (buckets: number[]) => void;
  /** Darken the glass by itself over a bright backdrop, for cards whose text is always white. */
  autoShade?: boolean;
  /** Which window this glass sits in: "dock", "settings" or "ask". */
  window?: string;
}) {
  const ref = useRef<HTMLCanvasElement>(null);
  // The look is read at draw time, so dragging a Tint or Blur slider only redraws; rebuilding the GL
  // program and textures (and re-reading the wallpaper) on every step piled up GPU memory.
  const look = useRef({ style, radius, tint, blur });
  look.current = { style, radius, tint, blur };
  const redraw = useRef<() => void>(() => {});
  const lumaTo = useRef(onLuma);
  lumaTo.current = onLuma;
  const shading = useRef(autoShade);
  shading.current = autoShade;
  useEffect(() => redraw.current(), [style, radius, tint, blur]);
  useEffect(() => {
    const canvas = ref.current!;
    const gl = canvas.getContext("webgl", { premultipliedAlpha: true, alpha: true, antialias: false });
    if (!gl) return onReady(false);
    let pr: WebGLProgram;
    try {
      pr = program(gl);
    } catch (e) {
      console.error(e);
      return onReady(false);
    }
    const wallTex = gl.createTexture();
    const liveTex = gl.createTexture();
    let img: HTMLImageElement | null = null;
    let fit = 0;
    let geo: Geometry | null = null;
    let frame: Live | null = null;
    let disposed = false;

    const draw = () => {
      if (disposed || !geo || (!img && !frame)) return;
      const { style, radius, tint, blur } = look.current;
      const dpr = window.devicePixelRatio || 1;
      const w = Math.round(canvas.clientWidth * dpr), h = Math.round(canvas.clientHeight * dpr);
      if (canvas.width !== w || canvas.height !== h) {
        canvas.width = w;
        canvas.height = h;
      }
      gl.viewport(0, 0, w, h);
      const u = (n: string) => gl.getUniformLocation(pr, n);
      const useLive = frame !== null;
      gl.bindTexture(gl.TEXTURE_2D, useLive ? liveTex : wallTex);
      gl.uniform1i(u("live"), useLive ? 1 : 0);
      if (frame) {
        gl.uniform2f(u("cap0"), frame.x0, frame.y0);
        gl.uniform2f(u("capSize"), frame.coverW, frame.coverH);
      }
      gl.uniform2f(u("res"), w, h);
      gl.uniform2f(u("win0"), geo.left, geo.top);
      gl.uniform2f(u("winSize"), geo.width, geo.height);
      gl.uniform2f(u("mon"), geo.mon_w, geo.mon_h);
      gl.uniform2f(u("imgSize"), img?.naturalWidth ?? 1, img?.naturalHeight ?? 1);
      gl.uniform1i(u("fit"), fit);
      gl.uniform1f(u("radius"), Math.min(radius * dpr, Math.min(w, h) / 2));
      gl.uniform1f(u("band"), Math.min(Math.min(w, h) * 0.45, 22 * dpr));
      const [r, g, b, a] = TINTS[style] ?? TINTS.glass;
      const paint = (shade: number) => {
        // Shade: darken the glass over a bright backdrop (cards with autoShade), so white text stays
        // readable. It blends the tint toward near-black and raises its strength to at least `shade`.
        const k = shade > 0 ? Math.min(1, shade / Math.max(shade, a * tint * 2)) : 0;
        gl.uniform4f(u("tint"), r * (1 - k) + 0.05 * k, g * (1 - k) + 0.055 * k, b * (1 - k) + 0.07 * k, Math.min(1, Math.max(a * tint * 2, shade)));
        gl.clearColor(0, 0, 0, 0);
        gl.clear(gl.COLOR_BUFFER_BIT);
        gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4);
      };
      gl.uniform1f(u("blur"), blur);
      const due = (lumaTo.current || shading.current) && dueToMeasure();
      if (!due) return paint(shade);
      // Measure the unshaded glass (the backdrop as the style shows it), then shade on top if needed.
      // Both happen before the frame is shown, so the unshaded one is never seen.
      paint(0);
      const buckets = measure(w, h);
      lumaTo.current?.(buckets);
      if (shading.current) {
        shade = shadeFor(buckets);
        if (shade > 0) paint(shade);
      }
    };
    // Measures at most 4 times a second; a change in between is measured once the wait is over.
    let lastMeasure = 0;
    let measureLater = 0;
    let shade = 0;
    const dueToMeasure = () => {
      const now = performance.now();
      if (now - lastMeasure < 250) {
        clearTimeout(measureLater);
        measureLater = window.setTimeout(() => redraw.current(), 260 - (now - lastMeasure));
        return false;
      }
      lastMeasure = now;
      return true;
    };
    // Reads three lines through the middle of what was just drawn (along the long side) and returns
    // their brightness. Must run right after drawing: the buffer is cleared once it is shown.
    const measure = (w: number, h: number) => {
      // A bar is read along its length; a card (autoShade) across its full width, wherever it's tall.
      const wide = w >= h || shading.current;
      const len = wide ? w : h;
      const px = new Uint8Array(len * 4 * 3);
      [0.3, 0.5, 0.7].forEach((f, i) => {
        const at = Math.min((wide ? h : w) - 1, Math.floor((wide ? h : w) * f));
        const part = new Uint8Array(px.buffer, len * 4 * i, len * 4);
        if (wide) gl.readPixels(0, at, w, 1, gl.RGBA, gl.UNSIGNED_BYTE, part);
        // A column comes back bottom-up; flip it so bucket 0 is the top of the bar.
        else {
          gl.readPixels(at, 0, 1, h, gl.RGBA, gl.UNSIGNED_BYTE, part);
          new Uint32Array(px.buffer, len * 4 * i, len).reverse();
        }
      });
      return profile(px, 3, 48);
    };
    const loadGeometry = () =>
      invoke<Geometry | null>("geometry", { window: label }).then((g) => {
        if (disposed) return;
        geo = g;
        draw();
      });
    const loadWallpaper = () =>
      invoke<Wallpaper>("wallpaper").then((w) => {
        if (disposed) return;
        if (!w.data) {
          if (!live) onReady(false);
          return;
        }
        fit = FITS[w.fit] ?? 0;
        const im = new Image();
        im.onload = () => {
          if (disposed) return;
          gl.bindTexture(gl.TEXTURE_2D, wallTex);
          gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGB, gl.RGB, gl.UNSIGNED_BYTE, im);
          texParams(gl);
          img = im;
          onReady(true);
          draw();
        };
        im.onerror = () => onReady(false);
        im.src = w.data;
      });
    // One request in flight at a time; a frame that lands meanwhile is picked up by the next event.
    let fetching = false, again = false;
    const loadFrame = async () => {
      if (!live) return;
      if (fetching) {
        again = true;
        return;
      }
      fetching = true;
      try {
        do {
          again = false;
          const buf = await invoke<ArrayBuffer>("glass_frame", { window: label });
          // The panel and popup move without a geometry event; their place is re-read with each frame.
          if (label !== "dock") geo = await invoke<Geometry | null>("geometry", { window: label });
          if (disposed) return;
          if (!buf || buf.byteLength < 24) {
            frame = null;
          } else {
            const hd = new Int32Array(buf, 0, 6);
            const [x0, y0, coverW, coverH, fw, fh] = Array.from(hd);
            gl.bindTexture(gl.TEXTURE_2D, liveTex);
            gl.pixelStorei(gl.UNPACK_ALIGNMENT, 1);
            gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, fw, fh, 0, gl.RGBA, gl.UNSIGNED_BYTE, new Uint8Array(buf, 24, fw * fh * 4));
            texParams(gl);
            frame = { x0, y0, coverW, coverH };
            onReady(true);
          }
          draw();
        } while (again);
      } catch {
        // No frame yet: the wallpaper stands in.
      } finally {
        fetching = false;
      }
    };
    // Debug aid for `freedock --snap`: save what the glass layer draws.
    const snap = () => {
      draw();
      invoke<string>("glass_snap_save", { data: canvas.toDataURL("image/png"), window: label }).then(console.log).catch(console.error);
    };

    loadGeometry();
    loadWallpaper();
    loadFrame();
    const offs = [
      listen("wallpaper-changed", loadWallpaper),
      listen("geometry-changed", loadGeometry),
      listen<string | null>("glass-frame", (e) => (e.payload == null || e.payload === label) && loadFrame()),
      listen("glass-snap", snap),
    ];
    // Repaint at the new size in the same frame (until then the old bitmap is stretched, which
    // distorts the corners for a moment), then fetch the window's new place.
    const ro = new ResizeObserver(() => {
      draw();
      loadGeometry();
    });
    ro.observe(canvas);
    // Shown again somewhere else (the panel and popup open next to the dock wherever it is).
    const shown = () => document.visibilityState === "visible" && loadGeometry();
    document.addEventListener("visibilitychange", shown);
    window.addEventListener("focus", shown);
    redraw.current = draw;
    return () => {
      disposed = true;
      clearTimeout(measureLater);
      redraw.current = () => {};
      gl.deleteTexture(wallTex);
      gl.deleteTexture(liveTex);
      gl.deleteProgram(pr);
      ro.disconnect();
      document.removeEventListener("visibilitychange", shown);
      window.removeEventListener("focus", shown);
      offs.forEach((o) => o.then((f) => f()));
    };
  }, [live, onReady, label]);
  return <canvas ref={ref} className="glass" aria-hidden="true" />;
}
