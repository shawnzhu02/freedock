//! The agenda behind the calendar, meeting, timeline and to-do widgets, from three sources merged
//! into one "today view":
//!  - private calendar links (iCal/ICS: Google, Outlook and iCloud all offer one), read here,
//!    recurring events included;
//!  - a local to-do list in `%APPDATA%\freedock\todos.json`;
//!  - optionally an MCP server (mcp.rs) with a `today_view` tool.
//! The view's shape: { events: [{time, title, place}], today: [item], soon: [item], school: [item] }.

use chrono::{
    DateTime, Duration, Local, NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Timelike, Utc,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

// ------------------------------------------------------------------------------------------- ICS

/// One VEVENT, with the raw values needed to place it in time.
#[derive(Debug, Clone, Default)]
pub struct VEvent {
    pub uid: String,
    pub summary: String,
    pub location: String,
    pub description: String,
    pub start: Option<Stamp>,
    pub rrule: Option<String>,
    pub exdates: Vec<Stamp>,
    pub recurrence_id: Option<Stamp>,
    pub cancelled: bool,
}

/// A DTSTART-like value: the text, its TZID if any, and whether it's a whole day.
#[derive(Debug, Clone, PartialEq)]
pub struct Stamp {
    pub value: String,
    pub tzid: Option<String>,
    pub date_only: bool,
}

/// Lines with their folded continuations joined (RFC 5545 section 3.1).
fn unfold(text: &str) -> Vec<String> {
    let mut out: Vec<String> = vec![];
    for line in text.split('\n') {
        let line = line.strip_suffix('\r').unwrap_or(line);
        if (line.starts_with(' ') || line.starts_with('\t')) && !out.is_empty() {
            out.last_mut().unwrap().push_str(&line[1..]);
        } else {
            out.push(line.to_string());
        }
    }
    out
}

fn unescape(v: &str) -> String {
    v.replace("\\n", " ")
        .replace("\\N", " ")
        .replace("\\,", ",")
        .replace("\\;", ";")
        .replace("\\\\", "\\")
}

/// "DTSTART;TZID=America/New_York:20261116T100000" -> ("DTSTART", [("TZID", ...)], value).
fn split_prop(line: &str) -> Option<(String, Vec<(String, String)>, String)> {
    // The value starts after the first ':' that isn't inside a quoted parameter.
    let mut quoted = false;
    let mut cut = None;
    for (i, c) in line.char_indices() {
        match c {
            '"' => quoted = !quoted,
            ':' if !quoted => {
                cut = Some(i);
                break;
            }
            _ => {}
        }
    }
    let i = cut?;
    let (head, value) = (&line[..i], &line[i + 1..]);
    let mut parts = head.split(';');
    let name = parts.next()?.to_ascii_uppercase();
    let params = parts
        .filter_map(|p| p.split_once('='))
        .map(|(k, v)| (k.to_ascii_uppercase(), v.trim_matches('"').to_string()))
        .collect();
    Some((name, params, value.to_string()))
}

fn stamp(params: &[(String, String)], value: &str) -> Stamp {
    let tzid = params
        .iter()
        .find(|(k, _)| k == "TZID")
        .map(|(_, v)| v.clone());
    let date_only = params
        .iter()
        .any(|(k, v)| k == "VALUE" && v.eq_ignore_ascii_case("DATE"))
        || (value.len() == 8 && !value.contains('T'));
    Stamp {
        value: value.trim().to_string(),
        tzid,
        date_only,
    }
}

pub fn parse_ics(text: &str) -> Vec<VEvent> {
    let mut out = vec![];
    let mut cur: Option<VEvent> = None;
    // How deep we are inside sub-components of the current VEVENT (VALARM, or any other). Their
    // properties describe the sub-component, not the event: Google's reminder VALARM carries its own
    // DESCRIPTION ("This is an event reminder"), SUMMARY and sometimes UID, which would otherwise
    // overwrite the event's.
    let mut nested = 0u32;
    for line in unfold(text) {
        let Some((name, params, value)) = split_prop(&line) else {
            continue;
        };
        if cur.is_some() {
            match name.as_str() {
                "BEGIN" => {
                    nested += 1;
                    continue;
                }
                "END" if nested > 0 => {
                    nested -= 1;
                    continue;
                }
                _ if nested > 0 => continue,
                _ => {}
            }
        }
        match (name.as_str(), &mut cur) {
            ("BEGIN", None) if value.eq_ignore_ascii_case("VEVENT") => {
                cur = Some(VEvent::default())
            }
            ("END", Some(_)) if value.eq_ignore_ascii_case("VEVENT") => {
                out.push(cur.take().unwrap())
            }
            ("UID", Some(e)) => e.uid = value,
            ("SUMMARY", Some(e)) => e.summary = unescape(&value),
            ("LOCATION", Some(e)) => e.location = unescape(&value),
            ("DESCRIPTION", Some(e)) => e.description = unescape(&value),
            ("DTSTART", Some(e)) => e.start = Some(stamp(&params, &value)),
            ("RRULE", Some(e)) => e.rrule = Some(value),
            ("EXDATE", Some(e)) => e
                .exdates
                .extend(value.split(',').map(|v| stamp(&params, v))),
            ("RECURRENCE-ID", Some(e)) => e.recurrence_id = Some(stamp(&params, &value)),
            ("STATUS", Some(e)) => e.cancelled = value.eq_ignore_ascii_case("CANCELLED"),
            _ => {}
        }
    }
    out
}

/// Windows time zone names (Outlook writes these) for the most common zones.
fn iana_of(tzid: &str) -> Option<chrono_tz::Tz> {
    if let Ok(tz) = tzid.parse::<chrono_tz::Tz>() {
        return Some(tz);
    }
    let name = match tzid {
        "Eastern Standard Time" | "US Eastern Standard Time" => "America/New_York",
        "Central Standard Time" => "America/Chicago",
        "Mountain Standard Time" => "America/Denver",
        "US Mountain Standard Time" => "America/Phoenix",
        "Pacific Standard Time" => "America/Los_Angeles",
        "Alaskan Standard Time" => "America/Anchorage",
        "Hawaiian Standard Time" => "Pacific/Honolulu",
        "GMT Standard Time" => "Europe/London",
        "W. Europe Standard Time" => "Europe/Berlin",
        "Romance Standard Time" => "Europe/Paris",
        "Central Europe Standard Time" => "Europe/Budapest",
        "E. Europe Standard Time" => "Europe/Bucharest",
        "India Standard Time" => "Asia/Kolkata",
        "China Standard Time" => "Asia/Shanghai",
        "Tokyo Standard Time" => "Asia/Tokyo",
        "Korea Standard Time" => "Asia/Seoul",
        "Singapore Standard Time" => "Asia/Singapore",
        "AUS Eastern Standard Time" => "Australia/Sydney",
        "UTC" | "Coordinated Universal Time" => "UTC",
        _ => return None,
    };
    name.parse().ok()
}

fn naive(s: &Stamp) -> Option<NaiveDateTime> {
    let v = s.value.trim_end_matches('Z');
    if s.date_only {
        return NaiveDate::parse_from_str(v.get(..8).unwrap_or(v), "%Y%m%d")
            .ok()
            .map(|d| d.and_time(NaiveTime::MIN));
    }
    NaiveDateTime::parse_from_str(v, "%Y%m%dT%H%M%S").ok()
}

/// The stamp as a local time. UTC ("Z") and TZID values are converted; floating ones (and whole
/// days) are taken as local already.
pub fn to_local(s: &Stamp) -> Option<DateTime<Local>> {
    let n = naive(s)?;
    if s.value.ends_with('Z') {
        return Some(Utc.from_utc_datetime(&n).with_timezone(&Local));
    }
    if let Some(tz) = s.tzid.as_deref().filter(|_| !s.date_only).and_then(iana_of) {
        return tz
            .from_local_datetime(&n)
            .earliest()
            .map(|t| t.with_timezone(&Local));
    }
    Local.from_local_datetime(&n).earliest()
}

/// The stamp written the way the rrule crate reads a DTSTART line.
fn dtstart_line(s: &Stamp) -> Option<String> {
    let n = naive(s)?;
    let t = n.format("%Y%m%dT%H%M%S");
    if s.value.ends_with('Z') {
        return Some(format!("DTSTART:{t}Z"));
    }
    match s.tzid.as_deref().filter(|_| !s.date_only).and_then(iana_of) {
        Some(tz) => Some(format!("DTSTART;TZID={}:{t}", tz.name())),
        None => Some(format!("DTSTART:{t}")),
    }
}

/// A placed event: when it starts (local) and whether it's all day.
#[derive(Debug, Clone, PartialEq)]
pub struct Occurrence {
    pub start: DateTime<Local>,
    pub all_day: bool,
    pub title: String,
    pub place: String,
}

/// Video-call services whose links may become a Join button. Anyone can send an invite, so a link
/// only counts when its host is one of these (or a subdomain), checked on the parsed host.
const MEETING_HOSTS: [&str; 5] = [
    "meet.google.com",
    "zoom.us",
    "teams.microsoft.com",
    "teams.live.com",
    "webex.com",
];

/// True for an https link whose host is a meeting service (exact, or a subdomain on a dot boundary).
pub fn is_meeting_link(url: &str) -> bool {
    let Some(rest) = url.strip_prefix("https://") else {
        return false;
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    if authority.contains('@') {
        return false; // "https://zoom.us@evil.example" is evil.example
    }
    let host = authority
        .split(':')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    MEETING_HOSTS
        .iter()
        .any(|h| host == *h || host.ends_with(&format!(".{h}")))
}

fn words(text: &str) -> impl Iterator<Item = &str> {
    text.split(|c: char| c.is_whitespace() || c == '<' || c == '>' || c == '"')
        .map(|w| w.trim_end_matches([',', '.', ';', ')']))
}

/// The place shown for an event: its location with any link that isn't a meeting service removed,
/// plus a meeting link from the description when the location has none.
fn place_with_link(e: &VEvent) -> String {
    let place: Vec<&str> = words(&e.location)
        .filter(|w| !w.contains("://") || is_meeting_link(w))
        .filter(|w| !w.is_empty())
        .collect();
    let place = place.join(" ");
    if words(&e.location).any(is_meeting_link) {
        return place;
    }
    match words(&e.description).find(|w| is_meeting_link(w)) {
        Some(l) if place.is_empty() => l.to_string(),
        Some(l) => format!("{place} {l}"),
        None => place,
    }
}

/// Every occurrence of `events` that starts in [from, to).
pub fn occurrences(
    events: &[VEvent],
    from: DateTime<Local>,
    to: DateTime<Local>,
) -> Vec<Occurrence> {
    // Moved or edited instances of a series: their originals are skipped in the series.
    let moved: Vec<(String, DateTime<Local>)> = events
        .iter()
        .filter_map(|e| Some((e.uid.clone(), to_local(e.recurrence_id.as_ref()?)?)))
        .collect();
    let mut out = vec![];
    for e in events
        .iter()
        .filter(|e| !e.cancelled || e.recurrence_id.is_some())
    {
        let Some(start) = e.start.as_ref() else {
            continue;
        };
        let all_day = start.date_only;
        let mut push = |at: DateTime<Local>| {
            if !e.cancelled && at >= from && at < to {
                out.push(Occurrence {
                    start: at,
                    all_day,
                    title: e.summary.clone(),
                    place: place_with_link(e),
                });
            }
        };
        match (&e.rrule, e.recurrence_id.is_none()) {
            (Some(rule), true) => {
                let starts = expand(start, rule, &e.exdates, from, to);
                for at in starts {
                    if !moved.iter().any(|(uid, m)| *uid == e.uid && *m == at) {
                        push(at);
                    }
                }
            }
            _ => {
                if let Some(at) = to_local(start) {
                    push(at);
                }
            }
        }
    }
    out.sort_by_key(|o| (!o.all_day, o.start));
    out
}

fn expand(
    start: &Stamp,
    rule: &str,
    exdates: &[Stamp],
    from: DateTime<Local>,
    to: DateTime<Local>,
) -> Vec<DateTime<Local>> {
    let Some(line) = dtstart_line(start) else {
        return vec![];
    };
    // A floating UNTIL with a zoned start (or the reverse) is a common mistake the rrule crate
    // refuses; dropping it is the forgiving reading.
    let parsed = format!("{line}\nRRULE:{rule}")
        .parse::<rrule::RRuleSet>()
        .or_else(|_| {
            let rule: String = rule
                .split(';')
                .filter(|p| !p.to_ascii_uppercase().starts_with("UNTIL="))
                .collect::<Vec<_>>()
                .join(";");
            format!("{line}\nRRULE:{rule}").parse::<rrule::RRuleSet>()
        });
    let Ok(set) = parsed else {
        return to_local(start).into_iter().collect();
    };
    let tz = rrule::Tz::LOCAL;
    // Whole-day events are compared by date, so widen the window by a day on each side.
    let (a, b) = if start.date_only {
        (from - Duration::days(1), to + Duration::days(1))
    } else {
        (from - Duration::seconds(1), to)
    };
    let mut set = set.after(a.with_timezone(&tz)).before(b.with_timezone(&tz));
    for x in exdates.iter().filter_map(to_local) {
        set = set.exdate(x.with_timezone(&tz));
    }
    set.all(200)
        .dates
        .into_iter()
        .map(|d| d.with_timezone(&Local))
        .filter(|d| *d >= from && *d < to)
        .collect()
}

/// "9 am", "10:30 am", "2 pm".
pub fn clock(t: &DateTime<Local>) -> String {
    let (pm, h) = t.hour12();
    let ap = if pm { "pm" } else { "am" };
    if t.minute() == 0 {
        format!("{h} {ap}")
    } else {
        format!("{h}:{:02} {ap}", t.minute())
    }
}

// ------------------------------------------------------------------------------------- To-dos

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Todo {
    pub id: String,
    pub title: String,
    /// "2026-11-19", or none for "whenever".
    #[serde(default)]
    pub due: Option<String>,
    #[serde(default)]
    pub done: bool,
    /// When it was ticked (unix seconds); ticked items are dropped after a week.
    #[serde(default)]
    pub done_at: Option<i64>,
}

/// Today's list (undone, due today, overdue or undated) and what's due in the next 7 days.
pub fn todo_view(todos: &[Todo], today: NaiveDate) -> (Vec<Value>, Vec<Value>) {
    let item = |t: &Todo| json!({"id": format!("local:{}", t.id), "title": t.title, "due": t.due, "area": "To-do"});
    let due = |t: &Todo| {
        t.due
            .as_deref()
            .and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())
    };
    let mut now = vec![];
    let mut soon = vec![];
    for t in todos.iter().filter(|t| !t.done) {
        match due(t) {
            None => now.push(item(t)),
            Some(d) if d <= today => now.push(item(t)),
            Some(d) if d <= today + Duration::days(7) => soon.push(item(t)),
            _ => {}
        }
    }
    (now, soon)
}

fn todos_file() -> std::path::PathBuf {
    crate::settings::dir().join("todos.json")
}

/// Held across every load-change-save of the to-do list, so two edits at once can't lose one.
static TODOS: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// The to-do list for showing; empty when it can't be read.
pub fn todos_load() -> Vec<Todo> {
    todos_read().unwrap_or_default()
}

/// The to-do list, or an error when the file exists but can't be read or parsed (then nothing may
/// save over it).
fn todos_read() -> Result<Vec<Todo>, String> {
    let mut l: Vec<Todo> = match std::fs::read_to_string(todos_file()) {
        Ok(s) => serde_json::from_str(&s)
            .map_err(|e| format!("todos.json is damaged ({e}); fix or delete it"))?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => vec![],
        Err(e) => return Err(format!("Couldn't read your to-dos: {e}")),
    };
    let week_ago = Utc::now().timestamp() - 7 * 86400;
    l.retain(|t| !t.done || t.done_at.is_none_or(|at| at > week_ago));
    Ok(l)
}

fn todos_save(l: &[Todo]) -> Result<(), String> {
    let _ = std::fs::create_dir_all(crate::settings::dir());
    let f = todos_file();
    let tmp = f.with_extension("tmp");
    std::fs::write(
        &tmp,
        serde_json::to_vec_pretty(l).map_err(|e| e.to_string())?,
    )
    .and_then(|_| std::fs::rename(&tmp, &f))
    .map_err(|e| e.to_string())
}

pub fn todo_add(title: &str, due: Option<String>) -> Result<String, String> {
    let title = title.trim();
    if title.is_empty() {
        return Err("Type the to-do first".into());
    }
    let due = due.filter(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").is_ok());
    let _one = TODOS.lock().unwrap_or_else(|e| e.into_inner());
    let mut l = todos_read()?;
    let id = format!("{:x}", Utc::now().timestamp_nanos_opt().unwrap_or_default());
    l.push(Todo {
        id: id.clone(),
        title: title.chars().take(300).collect(),
        due,
        done: false,
        done_at: None,
    });
    todos_save(&l)?;
    Ok(id)
}

/// Tick or untick a local to-do ("local:<id>" or "<id>").
pub fn todo_toggle(id: &str) -> Result<(), String> {
    let id = id.strip_prefix("local:").unwrap_or(id);
    let _one = TODOS.lock().unwrap_or_else(|e| e.into_inner());
    let mut l = todos_read()?;
    let t = l
        .iter_mut()
        .find(|t| t.id == id)
        .ok_or("That to-do is gone")?;
    t.done = !t.done;
    t.done_at = t.done.then(|| Utc::now().timestamp());
    todos_save(&l)
}

pub fn todo_delete(id: &str) -> Result<(), String> {
    let id = id.strip_prefix("local:").unwrap_or(id);
    let _one = TODOS.lock().unwrap_or_else(|e| e.into_inner());
    let mut l = todos_read()?;
    l.retain(|t| t.id != id);
    todos_save(&l)
}

// ------------------------------------------------------------------------------------ Calendars

/// The saved calendar links ("webcal://" is the same as https).
pub fn ics_urls() -> Vec<String> {
    crate::secrets::get("ics")
        .unwrap_or_default()
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(String::from)
        .collect()
}

/// Save the calendar links (one per line). Returns how many were kept.
pub fn ics_set(text: &str) -> Result<usize, String> {
    let mut urls = vec![];
    for l in text
        .split(['\n', ',', ' '])
        .map(str::trim)
        .filter(|l| !l.is_empty())
    {
        let u = if let Some(rest) = l.strip_prefix("webcal://") {
            format!("https://{rest}")
        } else {
            l.to_string()
        };
        if !u.starts_with("https://") {
            return Err("Calendar links start with https:// or webcal://".into());
        }
        if !urls.contains(&u) {
            urls.push(u);
        }
    }
    crate::secrets::set("ics", (!urls.is_empty()).then(|| urls.join("\n")))?;
    CACHE.lock().unwrap_or_else(|e| e.into_inner()).clear();
    Ok(urls.len())
}

static CACHE: std::sync::Mutex<Vec<(String, std::time::Instant, Vec<VEvent>)>> =
    std::sync::Mutex::new(Vec::new());

/// One calendar, fetched at most every 10 minutes. Errors never include the link (it's private).
fn calendar(url: &str) -> Result<Vec<VEvent>, String> {
    if let Some((_, at, ev)) = CACHE
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .iter()
        .find(|(u, _, _)| u == url)
    {
        if at.elapsed() < std::time::Duration::from_secs(600) {
            return Ok(ev.clone());
        }
    }
    let resp = ureq::get(url)
        .timeout(std::time::Duration::from_secs(20))
        .set("User-Agent", "freedock-calendar/1")
        .call();
    let text = match resp {
        Ok(r) => r
            .into_string()
            .map_err(|_| "a calendar's reply was cut off".to_string())?,
        Err(ureq::Error::Status(code, _)) => {
            return Err(format!("a calendar link answered HTTP {code}"));
        }
        Err(_) => return Err("couldn't reach a calendar".into()),
    };
    let ev = parse_ics(&text);
    let mut c = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    c.retain(|(u, _, _)| u != url);
    c.push((url.to_string(), std::time::Instant::now(), ev.clone()));
    Ok(ev)
}

/// Everything merged: calendar events today, local to-dos, and the MCP server's view if connected.
/// Blocking (network); run it off the UI thread.
pub fn today() -> Result<Value, String> {
    let now = Local::now();
    let start = Local
        .from_local_datetime(&now.date_naive().and_time(NaiveTime::MIN))
        .earliest()
        .unwrap_or(now);
    let end = start + Duration::days(1);
    let mut events: Vec<(i64, Value)> = vec![];
    let mut errors: Vec<String> = vec![];
    for url in ics_urls() {
        match calendar(&url) {
            Ok(ev) => {
                for o in occurrences(&ev, start, end) {
                    let time = if o.all_day {
                        "All day".to_string()
                    } else {
                        clock(&o.start)
                    };
                    events.push((
                        o.start.timestamp(),
                        json!({"time": time, "title": o.title, "place": o.place}),
                    ));
                }
            }
            Err(e) => errors.push(e),
        }
    }
    let (mut today_items, mut soon) = todo_view(&todos_load(), now.date_naive());
    let mut school: Vec<Value> = vec![];
    if crate::mcp::connected().is_some() {
        match crate::mcp::call("today_view", json!({})).and_then(|t| {
            serde_json::from_str::<Value>(&t)
                .map_err(|_| "the agenda server's today view was unreadable".to_string())
        }) {
            Ok(v) => {
                let v = if v.get("view").is_some() {
                    v["view"].clone()
                } else {
                    v
                };
                let list = |k: &str| v[k].as_array().cloned().unwrap_or_default();
                // Server events have only a clock text; keep their order after the calendar's.
                events.extend(list("events").into_iter().map(|e| (i64::MAX, e)));
                today_items.extend(list("today"));
                soon.extend(list("soon"));
                school.extend(list("school"));
            }
            Err(e) => errors.push(e),
        }
    }
    events.sort_by_key(|(t, _)| *t);
    let events: Vec<Value> = events.into_iter().map(|(_, e)| e).collect();
    Ok(
        json!({"events": events, "today": today_items, "soon": soon, "school": school, "errors": errors}),
    )
}

/// Add a to-do: to the MCP server when one is connected (its own list), else locally.
pub fn add_todo(name: &str, for_date: Option<String>) -> Result<String, String> {
    if crate::mcp::connected().is_some() {
        let mut args = json!({"name": name});
        if let Some(d) = &for_date {
            args["for_date"] = json!(d);
        }
        return crate::mcp::call("add_todo", args);
    }
    todo_add(name, for_date).map(|_| "Added".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn local(y: i32, m: u32, d: u32, h: u32, min: u32) -> DateTime<Local> {
        Local.with_ymd_and_hms(y, m, d, h, min, 0).unwrap()
    }

    const CAL: &str = "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nUID:a\r\nSUMMARY:Stand-up\r\nDTSTART:20261116T090000\r\nRRULE:FREQ=WEEKLY;BYDAY=MO,WE,FR\r\nEXDATE:20261118T090000\r\nLOCATION:https://meet.google.com/abc-defg-hij\r\nEND:VEVENT\r\nBEGIN:VEVENT\r\nUID:b\r\nSUMMARY:Lunch\\, with Sam\r\nDTSTART:20261116T123000\r\nDESCRIPTION:Join https://acme.zoom.us/j/123 \r\n  please\r\nEND:VEVENT\r\nBEGIN:VEVENT\r\nUID:c\r\nSUMMARY:Holiday\r\nDTSTART;VALUE=DATE:20261117\r\nEND:VEVENT\r\nBEGIN:VEVENT\r\nUID:a\r\nRECURRENCE-ID:20261120T090000\r\nSUMMARY:Stand-up (moved)\r\nDTSTART:20261120T140000\r\nEND:VEVENT\r\nBEGIN:VEVENT\r\nUID:d\r\nSUMMARY:Gone\r\nSTATUS:CANCELLED\r\nDTSTART:20261116T150000\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";

    #[test]
    fn parses_and_unfolds() {
        let ev = parse_ics(CAL);
        assert_eq!(ev.len(), 5);
        assert_eq!(ev[1].summary, "Lunch, with Sam");
        assert_eq!(ev[1].description, "Join https://acme.zoom.us/j/123  please");
        assert!(ev[2].start.as_ref().unwrap().date_only);
        assert!(ev[4].cancelled);
    }

    #[test]
    fn alarm_inside_an_event_keeps_its_own_fields() {
        let cal = "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nUID:evt-1@google.com\r\nSUMMARY:Dentist\r\nDESCRIPTION:Bring the form\r\nDTSTART:20261116T090000Z\r\nBEGIN:VALARM\r\nACTION:DISPLAY\r\nDESCRIPTION:This is an event reminder\r\nSUMMARY:Alarm notification\r\nUID:alarm-1\r\nTRIGGER:-P0DT0H30M0S\r\nEND:VALARM\r\nLOCATION:Main St\r\nEND:VEVENT\r\nBEGIN:VEVENT\r\nUID:evt-2\r\nSUMMARY:After\r\nDTSTART:20261116T100000Z\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
        let ev = parse_ics(cal);
        assert_eq!(ev.len(), 2, "the VALARM's END doesn't end the event");
        assert_eq!(ev[0].uid, "evt-1@google.com");
        assert_eq!(ev[0].summary, "Dentist");
        assert_eq!(ev[0].description, "Bring the form");
        assert_eq!(
            ev[0].location, "Main St",
            "properties after the VALARM still count"
        );
        assert_eq!(ev[1].uid, "evt-2");
        assert_eq!(ev[1].summary, "After");
    }

    #[test]
    fn one_day_with_recurrence_links_and_cancellations() {
        let ev = parse_ics(CAL);
        let day = occurrences(&ev, local(2026, 11, 16, 0, 0), local(2026, 11, 17, 0, 0));
        let got: Vec<(String, String, String)> = day
            .iter()
            .map(|o| (clock(&o.start), o.title.clone(), o.place.clone()))
            .collect();
        assert_eq!(
            got,
            [
                (
                    "9 am".to_string(),
                    "Stand-up".to_string(),
                    "https://meet.google.com/abc-defg-hij".to_string()
                ),
                (
                    "12:30 pm".to_string(),
                    "Lunch, with Sam".to_string(),
                    "https://acme.zoom.us/j/123".to_string()
                ),
            ]
        );
    }

    #[test]
    fn a_week_of_the_series() {
        let ev = parse_ics(CAL);
        let week = occurrences(&ev, local(2026, 11, 16, 0, 0), local(2026, 11, 23, 0, 0));
        let standups: Vec<String> = week
            .iter()
            .filter(|o| o.title.starts_with("Stand-up"))
            .map(|o| format!("{} {}", o.start.format("%a"), clock(&o.start)))
            .collect();
        // Wednesday is excluded; Friday's was moved to 2 pm.
        assert_eq!(standups, ["Mon 9 am", "Fri 2 pm"]);
        let holiday: Vec<&Occurrence> = week.iter().filter(|o| o.all_day).collect();
        assert_eq!(holiday.len(), 1);
        assert_eq!(
            holiday[0].start.date_naive(),
            NaiveDate::from_ymd_opt(2026, 11, 17).unwrap()
        );
    }

    #[test]
    fn zones_convert_to_local() {
        let utc = Stamp {
            value: "20261116T150000Z".into(),
            tzid: None,
            date_only: false,
        };
        assert_eq!(
            to_local(&utc).unwrap(),
            Utc.with_ymd_and_hms(2026, 11, 16, 15, 0, 0)
                .unwrap()
                .with_timezone(&Local)
        );
        let ny = Stamp {
            value: "20261116T100000".into(),
            tzid: Some("Eastern Standard Time".into()),
            date_only: false,
        };
        assert_eq!(
            to_local(&ny).unwrap(),
            Utc.with_ymd_and_hms(2026, 11, 16, 15, 0, 0)
                .unwrap()
                .with_timezone(&Local)
        );
        let iana = Stamp {
            value: "20260716T100000".into(),
            tzid: Some("America/New_York".into()),
            date_only: false,
        };
        assert_eq!(
            to_local(&iana).unwrap(),
            Utc.with_ymd_and_hms(2026, 7, 16, 14, 0, 0)
                .unwrap()
                .with_timezone(&Local),
            "summer time"
        );
    }

    #[test]
    fn todo_lists() {
        let t = |id: &str, due: Option<&str>, done| Todo {
            id: id.into(),
            title: id.into(),
            due: due.map(String::from),
            done,
            done_at: None,
        };
        let today = NaiveDate::from_ymd_opt(2026, 11, 16).unwrap();
        let l = [
            t("whenever", None, false),
            t("late", Some("2026-11-10"), false),
            t("fri", Some("2026-11-20"), false),
            t("later", Some("2026-12-30"), false),
            t("done", None, true),
        ];
        let (now, soon) = todo_view(&l, today);
        let ids = |v: &[Value]| {
            v.iter()
                .map(|x| x["id"].as_str().unwrap().to_string())
                .collect::<Vec<_>>()
        };
        assert_eq!(ids(&now), ["local:whenever", "local:late"]);
        assert_eq!(ids(&soon), ["local:fri"]);
    }

    #[test]
    fn only_meeting_hosts_become_links() {
        assert!(is_meeting_link("https://meet.google.com/abc-defg-hij"));
        assert!(is_meeting_link("https://acme.zoom.us/j/123?pwd=x"));
        assert!(
            !is_meeting_link("https://zoom.us.evil.example/j/1"),
            "suffix trick"
        );
        assert!(
            !is_meeting_link("https://evilzoom.us/j/1"),
            "no dot boundary"
        );
        assert!(
            !is_meeting_link("https://zoom.us@evil.example/"),
            "userinfo trick"
        );
        assert!(!is_meeting_link("http://zoom.us/j/1"), "https only");
        let e = VEvent {
            location: "Room 4 https://phish.example/login".into(),
            description: "https://acme.zoom.us/j/9 and https://bad.example".into(),
            ..Default::default()
        };
        assert_eq!(place_with_link(&e), "Room 4 https://acme.zoom.us/j/9");
    }

    #[test]
    fn clock_text() {
        assert_eq!(clock(&local(2026, 1, 1, 0, 5)), "12:05 am");
        assert_eq!(clock(&local(2026, 1, 1, 14, 0)), "2 pm");
    }
}
