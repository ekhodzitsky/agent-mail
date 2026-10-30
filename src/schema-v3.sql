DROP TRIGGER reconciliations_unknown_only;
CREATE TRIGGER reconciliations_attempt_only BEFORE INSERT ON reconciliations
WHEN NOT EXISTS(SELECT 1 FROM attempts WHERE id=NEW.attempt_id AND package_id=NEW.package_id AND outcome IN ('unknown','accepted'))
BEGIN SELECT RAISE(ABORT,'not an accepted or unknown attempt'); END;
