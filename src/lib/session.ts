/**
 * session.ts — session folder read/write and date-range browsing.
 *
 * Data lives under the OS app-data dir (never inside the watched repo):
 *   <app-data>/GHLG/<project-name>/<YYYY-MM-DD>/session-NN/
 * All filesystem access goes through Tauri commands (Rust side) — the
 * frontend never touches paths directly. Every function takes an explicit
 * `project` argument rather than relying on a shared "active project" —
 * the archive can read several projects at once (its "All projects" view),
 * so there's no single global project to hang this off of.
 */
import { invoke } from "@tauri-apps/api/core";

export interface SessionEntry {
  id: string; // e.g. "entry-001-bugfix"
  timestamp: string; // ISO 8601
  tag:
    | "bugfix"
    | "feature"
    | "refactor"
    | "performance"
    | "ui"
    | "configuration"
    | "experiment"
    | "decision"
    | "question"
    | "note"
    | "update";
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
export async function listDates(project: string): Promise<string[]> {
  return invoke("list_session_dates", { project });
}

/** List sessions for a given date. */
export async function listSessions(project: string, date: string): Promise<SessionMeta[]> {
  return invoke("list_sessions", { project, date });
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
export async function searchEntries(project: string, query: string): Promise<SearchHit[]> {
  return invoke("search_entries", { project, query });
}

/** Read all entries in a session. */
export async function readSession(
  project: string,
  date: string,
  sessionId: string,
): Promise<SessionEntry[]> {
  return invoke("read_session", { project, date, sessionId });
}

/** Edit an entry's tag/title/summary in place. */
export async function updateEntry(
  project: string,
  date: string,
  sessionId: string,
  entryId: string,
  fields: { tag: SessionEntry["tag"]; title: string; summary: string },
): Promise<void> {
  await invoke("update_entry", { project, date, sessionId, entryId, ...fields });
}

/** Delete an entry (and its screenshot, if any). */
export async function deleteEntry(
  project: string,
  date: string,
  sessionId: string,
  entryId: string,
): Promise<void> {
  await invoke("delete_entry", { project, date, sessionId, entryId });
}

/** Delete an entire session — every entry and screenshot in it. */
export async function deleteSession(project: string, date: string, sessionId: string): Promise<void> {
  await invoke("delete_session", { project, date, sessionId });
}

/** Removes every empty session/date folder for this project. Returns how many were removed. */
export async function cleanupEmpty(project: string): Promise<number> {
  return invoke("cleanup_empty", { project });
}

/** Deletes a whole date's folder outright, regardless of content. */
export async function deleteDate(project: string, date: string): Promise<void> {
  await invoke("delete_date", { project, date });
}
