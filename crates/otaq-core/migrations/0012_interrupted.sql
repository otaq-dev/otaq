-- Startup reconcile (REC-01; d0018, d0045): an orange ticket (Asks you, Needs you, Idle) whose session ended with a
-- restart becomes Interrupted but keeps its label and question. The orange status it had is kept here, next to the
-- signal text it already stores; read only while the ticket is Interrupted.
ALTER TABLE tickets ADD COLUMN interrupted_from TEXT;

-- Every start of the background service (REC-01, REC-06; d0022). A run without `stopped_at` ended without a clean
-- stop (a crash or a reboot); the next start says so in its summary. The sessions a run started are the ones its
-- end interrupted.
CREATE TABLE service_runs (
    id         TEXT    NOT NULL PRIMARY KEY,
    pid        INTEGER NOT NULL,
    build_id   TEXT    NOT NULL,
    started_at BIGINT  NOT NULL,
    stopped_at BIGINT
);
CREATE INDEX service_runs_started ON service_runs (started_at);
