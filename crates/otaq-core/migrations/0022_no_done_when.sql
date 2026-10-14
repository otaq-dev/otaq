-- T-0175 (TKT-04; d0067): the "Done when" checklist goes away. A ticket's items are appended to its description as
-- a Markdown list under a "## Done when" heading (in position order, after a blank line), so nothing is lost; a
-- ticket without items keeps its body byte for byte. The heading matches `board::batch::CRITERIA_HEADING`.
UPDATE tickets
SET body = CASE WHEN TRIM(body) = '' THEN '' ELSE RTRIM(body, char(10, 13, 32, 9)) || char(10) || char(10) END
        || '## Done when' || char(10) || char(10)
        || (SELECT group_concat('- ' || d.text, char(10) ORDER BY d.position)
            FROM ticket_done_when d WHERE d.ticket_id = tickets.id)
WHERE EXISTS (SELECT 1 FROM ticket_done_when d WHERE d.ticket_id = tickets.id);

DROP TABLE ticket_done_when;
