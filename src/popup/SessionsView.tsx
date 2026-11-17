import { relevant, useSessions } from "../widgets/Sessions";

const ago = (s: number) => (s < 60 ? "just now" : s < 3600 ? `${Math.floor(s / 60)} min ago` : `${Math.floor(s / 3600)} h ago`);
const LABEL = { working: "Working", waiting: "Your turn", stuck: "Needs you" } as const;

/** Claude Code sessions from the last 3 hours with their state. */
export default function SessionsView({ opened }: { opened: number }) {
  const all = useSessions(5_000, opened);
  const s = all ? relevant(all, 180) : null;
  return (
    <div className="view sesv">
      <header>
        <b>Claude sessions</b>
        <span className="sp" />
        {s && <small className="ghlogin">{s.filter((x) => x.state === "working").length} working</small>}
      </header>
      {!s && <p className="empty center">Loading…</p>}
      {s && !s.length && <p className="empty center">No sessions in the last 3 hours.</p>}
      {s && s.length > 0 && (
        <div className="ghbody">
          {s.map((x, i) => (
            <div className="sesrow" key={i} title={x.cwd}>
              <span className={`spill ${x.state}`}>{LABEL[x.state]}</span>
              <div className="fmeta">
                <b>{x.project}</b>
                <small>{x.state === "working" ? "active" : `last activity ${ago(x.idle)}`}</small>
              </div>
            </div>
          ))}
        </div>
      )}
      <p className="stnote">Read from the local Claude Code transcripts; "Needs you" usually means an approval prompt.</p>
    </div>
  );
}
