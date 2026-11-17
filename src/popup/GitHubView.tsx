import { invoke } from "@tauri-apps/api/core";
import { checkState, reasonLabel, useGitHub, type Pr } from "../lib/github";
import { press } from "../lib/press";
import { since } from "../lib/projects";

const open = (url: string) => invoke("open_link", { url }).then(() => invoke("popup_close"));
const secs = (iso: string) => (iso ? Date.parse(iso) / 1000 : 0);

function PrRow({ p, now }: { p: Pr; now: number }) {
  const c = checkState(p.checks);
  const review = p.review === "APPROVED" ? "approved" : p.review === "CHANGES_REQUESTED" ? "changes requested" : "";
  return (
    <div className="ghrow" {...press(() => open(p.url))} title={p.url}>
      <i className={`ghdot ${c}`} title={c === "ok" ? "Checks passed" : c === "fail" ? "Checks failing" : c === "wait" ? "Checks running" : "No checks"} />
      <div className="fmeta">
        <b>{p.title}</b>
        <small>
          {p.repo}
          {p.draft && " · draft"}
          {review && ` · ${review}`}
          {p.updated && ` · ${since(secs(p.updated), now)}`}
        </small>
      </div>
    </div>
  );
}

/** What's waiting on GitHub: review requests, the user's open PRs with checks, unread notifications. Read-only. */
export default function GitHubView({ opened }: { opened: number }) {
  const g = useGitHub(5 * 60_000, opened);
  const now = Date.now() / 1000;
  return (
    <div className="view ghv">
      <header>
        <b>GitHub</b>
        {g?.login && <small className="ghlogin">{g.login}</small>}
        <span className="sp" />
        <button className="ghost" onClick={() => open("https://github.com/notifications")}>
          Inbox
        </button>
      </header>
      {!g && <p className="empty center">Loading…</p>}
      {g?.error && <p className="note">{g.error}</p>}
      {g && !g.error && (
        <div className="ghbody">
          {g.reviews.length > 0 && (
            <section>
              <h4>Waiting on your review</h4>
              {g.reviews.map((p) => (
                <PrRow key={p.url} p={p} now={now} />
              ))}
            </section>
          )}
          <section>
            <h4>Your pull requests</h4>
            {g.prs.length ? g.prs.map((p) => <PrRow key={p.url} p={p} now={now} />) : <p className="empty">None open.</p>}
          </section>
          <section>
            <h4>Notifications</h4>
            {g.notes.length ? (
              g.notes.slice(0, 12).map((n, i) => (
                <div className="ghrow" key={i} {...press(() => open(n.url))} title={n.url}>
                  <i className="ghdot note" />
                  <div className="fmeta">
                    <b>{n.title}</b>
                    <small>
                      {n.repo} · {reasonLabel(n.reason)}
                      {n.updated && ` · ${since(secs(n.updated), now)}`}
                    </small>
                  </div>
                </div>
              ))
            ) : (
              <p className="empty">All caught up.</p>
            )}
          </section>
        </div>
      )}
    </div>
  );
}
