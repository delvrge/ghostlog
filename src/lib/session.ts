/**
 * session.ts — session folder read/write and date-range browsing.
 *
 * Data lives under the OS app-data dir (never inside the watched repo):
 *   <app-data>/GHLG/<project-name>/<YYYY-MM-DD>/session-NN/
 * All filesystem access goes through Tauri commands (Rust side) — the
 * frontend never touches paths directly.
 */
import { invoke } from "@tauri-apps/api/core";

export interface SessionEntry {
  id: string; // e.g. "entry-001-bugfix"
  timestamp: string; // ISO 8601
  tag: "bugfix" | "update" | "feature";
  title: string;
  summary: string;
  screenshotPath?: string;
  markdownPath: string;
}

export interface SessionMeta {
  date: string; // YYYY-MM-DD
  sessionId: string; // e.g. "session-01"
  entryCount: number;
}

/** List all dates that have at least one session (full archive, any past date). */
export async function listDates(): Promise<string[]> {
  return invoke("list_session_dates");
}

/** List sessions for a given date. */
export async function listSessions(date: string): Promise<SessionMeta[]> {
  return invoke("list_sessions", { date });
}

export interface SearchHit {
  date: string;
  sessionId: string;
  entry: SessionEntry;
}

/**
 * Full-text search across every entry of the project (title, tag, summary),
 * newest dates first. Capped server-side, so an over-broad query returns a
 * manageable page rather than the whole archive.
 */
export async function searchEntries(query: string): Promise<SearchHit[]> {
  return invoke("search_entries", { query });
}

/** Read all entries in a session. */
export async function readSession(
  date: string,
  sessionId: string,
): Promise<SessionEntry[]> {
  return invoke("read_session", { date, sessionId });
}

/** Edit an entry's tag/title/summary in place. */
export async function updateEntry(
  date: string,
  sessionId: string,
  entryId: string,
  fields: { tag: SessionEntry["tag"]; title: string; summary: string },
): Promise<void> {
  await invoke("update_entry", { date, sessionId, entryId, ...fields });
}

/** Delete an entry (and its screenshot, if any). */
export async function deleteEntry(
  date: string,
  sessionId: string,
  entryId: string,
): Promise<void> {
  await invoke("delete_entry", { date, sessionId, entryId });
}

/** Delete an entire session — every entry and screenshot in it. */
export async function deleteSession(date: string, sessionId: string): Promise<void> {
  await invoke("delete_session", { date, sessionId });
}
