// Keys pressed while an input method (Chinese, Japanese, Korean...) is composing belong to the IME:
// Enter there picks a candidate, so it must not also launch, send or add. Check this at the top of
// every onKeyDown that acts on Enter or the arrow keys.
import type { KeyboardEvent } from "react";

export const composing = (e: KeyboardEvent) => e.nativeEvent.isComposing || e.keyCode === 229;
