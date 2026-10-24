-- Phase 2e: one cycle per imported Apple Note.
CREATE UNIQUE INDEX idx_cycle_note_id ON cycle(note_id) WHERE note_id IS NOT NULL;
