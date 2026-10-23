# Fixtures

Golden test inputs for the Apple Notes parser (Phase 2).

Real workout notes are personal, so they live in `private/`, which is gitignored.
Tests that use them skip when the files are missing.

## Layout
One file per title series, holding one representative week copied straight from
Apple Notes. Don't retype or clean anything up: the odd dashes and spacing are what the
parser has to handle.

```
private/
  log_book.txt        # "Log Book Week N"            upper/lower
  full_body.txt       # "Full Body Week N"           full body
  uwo_upper.txt       # "Uwo Upper Week N" / "Uwo Week N"   upper/lower
  uwo_y3.txt          # "Uwo Y3 Wk N"                full body
  uwo2_y3.txt         # "Uwo2 Y3 Wk N"               anterior/posterior
  uwo_y4.txt          # "Uwo Y4 Wk N"                upper/lower
```

- Line 1 is the note's exact title. The note body follows.
- During Phase 2c, the agent writes `<name>.expected.json` next to each file.
  The owner reviews it, and then it becomes the snapshot the tests compare against.
- Pick weeks that show the awkward cases: missing weights, blank last sets,
  `1-2 RIR` ranges, comments after RIR, and `N sets <exercise>` lines.
