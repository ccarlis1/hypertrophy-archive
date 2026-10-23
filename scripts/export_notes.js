// Export workout-log notes from Apple Notes to JSON (Phase 2a).
//
// Usage: osascript -l JavaScript scripts/export_notes.js [out_path]
//        (default out_path: exports/notes.json, which is gitignored)
//
// Exports two kinds of notes, with their full text:
//   - "series": the title matches a known title series (see SERIES)
//   - "candidate": the title doesn't match, but the body looks like a workout log
// Every other note is only counted, never written out.
// Run scripts/notes_report.py afterwards for coverage and gap checks.

ObjC.import('Foundation');

const WK = 'w(?:ee)?k';
const SERIES = [
  { block: 'log_book', re: new RegExp(`^log\\s*book\\s*${WK}\\s*(\\d+)$`, 'i') },
  { block: 'full_body', re: new RegExp(`^full\\s*body\\s*${WK}\\s*(\\d+)$`, 'i') },
  { block: 'uwo_upper', re: new RegExp(`^uwo\\s*upper\\s*(?:lower\\s*)?${WK}\\s*(\\d+)$`, 'i') },
  { block: 'uwo_upper', re: new RegExp(`^uwo\\s*${WK}\\s*(\\d+)$`, 'i') },
  { block: 'uwo_y3', re: new RegExp(`^uwo\\s*y3\\s*${WK}\\s*(\\d+)$`, 'i') },
  { block: 'uwo2_y3', re: new RegExp(`^uwo\\s*2\\s*y3\\s*${WK}\\s*(\\d+)$`, 'i') },
  { block: 'uwo_y4', re: new RegExp(`^uwo\\s*y4\\s*${WK}\\s*(\\d+)$`, 'i') },
];

// A line that looks like a logged exercise.
const WORKOUT_LINE = /set\s*\d|\d\s*reps?\b|\brir\b/i;
// A title that looks related even though it matches no series.
const RELATED_TITLE = new RegExp(`log\\s*book|full\\s*body|\\buwo|upper|lower|anterior|posterior|${WK}\\s*\\d`, 'i');

function matchSeries(title) {
  for (const s of SERIES) {
    const m = title.trim().match(s.re);
    if (m) return { block: s.block, week: parseInt(m[1], 10) };
  }
  return null;
}

function workoutLines(text) {
  return text.split(/\r?\n/).filter((l) => WORKOUT_LINE.test(l)).length;
}

function writeFile(path, text) {
  const dir = $(path).stringByDeletingLastPathComponent.js;
  if (dir) {
    $.NSFileManager.defaultManager.createDirectoryAtPathWithIntermediateDirectoriesAttributesError(dir, true, $(), null);
  }
  const ok = $(text).writeToFileAtomicallyEncodingError(path, true, $.NSUTF8StringEncoding, null);
  if (!ok) throw new Error(`could not write ${path}`);
}

function run(argv) {
  const outPath = argv[0] || 'exports/notes.json';
  const Notes = Application('Notes');
  const all = Notes.notes;

  // Bulk property reads: one Apple Event each instead of one per note.
  const ids = all.id();
  const names = all.name();
  const created = all.creationDate();
  const modified = all.modificationDate();
  const locked = all.passwordProtected();

  let plain = null;
  try {
    plain = all.plaintext();
  } catch (e) {
    // A locked note can break the bulk read. Fall back to one note at a time below.
  }

  const exported = [];
  const lockedTitles = [];
  const unreadable = [];
  const relatedNoWorkout = [];

  for (let i = 0; i < ids.length; i++) {
    const title = (names[i] || '').trim();
    if (locked[i]) {
      lockedTitles.push(title);
      continue;
    }

    let text = plain ? plain[i] : null;
    if (text == null) {
      try {
        text = Notes.notes.byId(ids[i]).plaintext();
      } catch (e) {
        unreadable.push(title);
        continue;
      }
    }

    const series = matchSeries(title);
    const lines = workoutLines(text);
    let kind = null;
    if (series) kind = 'series';
    else if (lines >= 3 || (lines >= 1 && RELATED_TITLE.test(title))) kind = 'candidate';
    else if (RELATED_TITLE.test(title)) relatedNoWorkout.push(title);
    if (!kind) continue;

    const note = Notes.notes.byId(ids[i]);
    let folder = null;
    try {
      folder = note.container().name();
    } catch (e) {}

    exported.push({
      id: ids[i],
      title,
      kind,
      block: series ? series.block : null,
      week: series ? series.week : null,
      folder,
      created: created[i].toISOString(),
      modified: modified[i].toISOString(),
      workout_lines: lines,
      plaintext: text,
      body_html: note.body(),
    });
  }

  const out = {
    exported_at: new Date().toISOString(),
    total_notes: ids.length,
    locked: lockedTitles,
    unreadable,
    related_titles_without_workout_lines: relatedNoWorkout,
    notes: exported,
  };
  writeFile(outPath, JSON.stringify(out, null, 2));

  const seriesCount = exported.filter((n) => n.kind === 'series').length;
  return `Scanned ${ids.length} notes. Exported ${seriesCount} series notes and ` +
    `${exported.length - seriesCount} candidates to ${outPath}. ` +
    `Locked: ${lockedTitles.length}. Unreadable: ${unreadable.length}.`;
}
