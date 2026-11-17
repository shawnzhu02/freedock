import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

/** A plain-text scratch pad, saved to %APPDATA%\freedock\notes.md half a second after typing stops. */
export default function NotesView({ opened }: { opened: number }) {
  const [text, setText] = useState<string | null>(null);
  const [state, setState] = useState<"" | "saving" | "saved" | string>("");
  const pending = useRef(0);
  const latest = useRef<string | null>(null);
  const box = useRef<HTMLTextAreaElement>(null);
  useEffect(() => {
    invoke<string>("notes_get").then((t) => {
      setText(t);
      setState("");
      requestAnimationFrame(() => {
        const el = box.current;
        if (el) {
          el.focus();
          el.setSelectionRange(el.value.length, el.value.length);
        }
      });
    }).catch((e) => {
      // No pad (and so no autosave) until the file reads cleanly.
      setText(null);
      setState(String(e));
    });
  }, [opened]);
  const save = (t: string) => {
    latest.current = t;
    setState("saving");
    clearTimeout(pending.current);
    pending.current = window.setTimeout(
      () => {
        pending.current = 0;
        invoke("notes_set", { text: t }).then(() => setState("saved")).catch((e) => setState(String(e)));
      },
      500,
    );
  };
  // Saves straight away if the flyout closes mid-debounce.
  // (The text is kept in a ref: React has already detached box.current when this cleanup runs.)
  useEffect(() => () => {
    if (pending.current && latest.current !== null) {
      clearTimeout(pending.current);
      invoke("notes_set", { text: latest.current }).catch(() => {});
    }
  }, []);
  return (
    <div className="view notesv">
      <header>
        <b>Notes</b>
        <span className="sp" />
        <small className="nstate">{state === "saving" ? "Saving…" : state === "saved" ? "Saved" : state}</small>
      </header>
      {text !== null && (
        <textarea
          ref={box}
          spellCheck
          value={text}
          placeholder="Anything. Saved on this PC as you type."
          onChange={(e) => {
            setText(e.target.value);
            save(e.target.value);
          }}
          onKeyDown={(e) => e.key === "Escape" && invoke("popup_close")}
        />
      )}
    </div>
  );
}
