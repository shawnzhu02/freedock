// Customisable flyouts: each flyout's sections in the user's order, some hidden. Saved per flyout in
// settings.layouts as a list like ["cpu", "-gpu", "memory"]: a leading "-" means hidden. Sections
// a later version adds (not in the saved list at all) show up at the end, visible.

export type Resolved = { order: string[]; hidden: Set<string> };

export function resolve(saved: string[] | undefined | null, all: string[], hiddenByDefault: string[] = []): Resolved {
  const order: string[] = [];
  const hidden = new Set<string>();
  if (!saved) {
    for (const id of all) {
      order.push(id);
      if (hiddenByDefault.includes(id)) hidden.add(id);
    }
    return { order, hidden };
  }
  for (const raw of saved) {
    const off = raw.startsWith("-");
    const id = off ? raw.slice(1) : raw;
    if (!all.includes(id) || order.includes(id)) continue;
    order.push(id);
    if (off) hidden.add(id);
  }
  for (const id of all) {
    if (!order.includes(id)) {
      order.push(id);
      if (hiddenByDefault.includes(id)) hidden.add(id);
    }
  }
  return { order, hidden };
}

/** The saved form of an order and hidden set. */
export function serialize(order: string[], hidden: Set<string>): string[] {
  return order.map((id) => (hidden.has(id) ? `-${id}` : id));
}

/** `list` with the item at `from` moved to `to`. */
export function moveItem<T>(list: T[], from: number, to: number): T[] {
  const out = list.slice();
  const [x] = out.splice(from, 1);
  out.splice(to, 0, x);
  return out;
}
