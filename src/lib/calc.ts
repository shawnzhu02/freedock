// A small calculator: + - * / ^ %, parentheses, unary minus, sqrt/sin/cos/tan/log/ln/abs/round,
// pi and e, "ans" for the last result, and "x% of y". Its own recursive-descent parser, never eval.
// Pure, so `npm test` covers it.

const FUNCS: Record<string, (x: number) => number> = {
  sqrt: Math.sqrt,
  abs: Math.abs,
  round: Math.round,
  floor: Math.floor,
  ceil: Math.ceil,
  ln: Math.log,
  log: Math.log10,
  sin: Math.sin,
  cos: Math.cos,
  tan: Math.tan,
};

type Tok = { k: "num"; v: number } | { k: "op"; v: string } | { k: "id"; v: string };

function lex(s: string): Tok[] {
  const out: Tok[] = [];
  let i = 0;
  while (i < s.length) {
    const c = s[i];
    if (/\s/.test(c) || c === ",") {
      // Commas are thousands separators: "1,200" is 1200.
      i++;
    } else if (/[0-9.]/.test(c)) {
      let j = i;
      while (j < s.length && /[0-9.,]/.test(s[j])) j++;
      const raw = s.slice(i, j).replace(/,/g, "");
      if ((raw.match(/\./g) ?? []).length > 1) throw new Error("bad number");
      out.push({ k: "num", v: parseFloat(raw) });
      i = j;
    } else if (/[a-zA-Z_]/.test(c)) {
      let j = i;
      while (j < s.length && /[a-zA-Z_]/.test(s[j])) j++;
      out.push({ k: "id", v: s.slice(i, j).toLowerCase() });
      i = j;
    } else if ("+-*/^%()×÷".includes(c)) {
      out.push({ k: "op", v: c === "×" ? "*" : c === "÷" ? "/" : c });
      i++;
    } else {
      throw new Error(`unexpected "${c}"`);
    }
  }
  return out;
}

/** The value of `expr`, or throws with a short reason. `ans` is the previous result. */
export function evaluate(expr: string, ans = 0): number {
  const t = lex(expr);
  let p = 0;
  const peek = () => t[p];
  const isOp = (v: string) => peek()?.k === "op" && peek()!.v === v;
  const isId = (v: string) => peek()?.k === "id" && peek()!.v === v;

  // sum := term (("+"|"-") term)*
  const sum = (): number => {
    let v = term();
    while (isOp("+") || isOp("-")) {
      const op = (t[p++] as { v: string }).v;
      const r = term();
      v = op === "+" ? v + r : v - r;
    }
    return v;
  };
  // term := unary (("*"|"/") unary | implicit unary)*
  const term = (): number => {
    let v = unary();
    for (;;) {
      if (isOp("*") || isOp("/")) {
        const op = (t[p++] as { v: string }).v;
        const r = unary();
        v = op === "*" ? v * r : v / r;
      } else if (peek() && (peek()!.k === "num" || peek()!.k === "id" || isOp("(")) && !isId("of")) {
        v *= unary(); // 2pi, 3(4+1)
      } else return v;
    }
  };
  const unary = (): number => {
    if (isOp("-")) return p++, -unary();
    if (isOp("+")) return p++, unary();
    return power();
  };
  // power := postfix ("^" unary)?   (right-associative)
  const power = (): number => {
    const b = postfix();
    if (isOp("^")) {
      p++;
      return Math.pow(b, unary());
    }
    return b;
  };
  // postfix := atom "%"? ("of" unary)?
  const postfix = (): number => {
    let v = atom();
    if (isOp("%")) {
      p++;
      v /= 100;
      if (isId("of")) {
        p++;
        v *= unary();
      }
    }
    return v;
  };
  const atom = (): number => {
    const k = t[p++];
    if (!k) throw new Error("incomplete");
    if (k.k === "num") return k.v;
    if (k.k === "op" && k.v === "(") {
      const v = sum();
      if (!isOp(")")) throw new Error("missing )");
      p++;
      return v;
    }
    if (k.k === "id") {
      if (k.v === "pi") return Math.PI;
      if (k.v === "e") return Math.E;
      if (k.v === "ans") return ans;
      const f = FUNCS[k.v];
      if (f) return f(isOp("(") ? atom() : unary());
      throw new Error(`unknown "${k.v}"`);
    }
    throw new Error(`unexpected "${k.v}"`);
  };

  if (!t.length) throw new Error("empty");
  const v = sum();
  if (p < t.length) throw new Error("unexpected input");
  if (!Number.isFinite(v)) throw new Error(Number.isNaN(v) ? "not a number" : "too big");
  return v;
}

/** 12 significant digits, thousands separators, no float noise: 0.1+0.2 shows 0.3. */
export function show(v: number): string {
  if (Math.abs(v) >= 1e15 || (v !== 0 && Math.abs(v) < 1e-9)) return v.toPrecision(10).replace(/\.?0+e/, "e");
  const r = Number(v.toPrecision(12));
  return r.toLocaleString("en-US", { maximumFractionDigits: 10 });
}
