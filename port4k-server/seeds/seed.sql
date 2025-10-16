BEGIN;

-- =========================
-- ACCOUNTS (with balances)
-- =========================
INSERT INTO accounts (username, role, email, password_hash, xp, health, coins)
VALUES
  ('system',  'admin',  'sytsem@port4k.com', '$argon2id$v=19$m=4096,t=3,p=1$QkpwWkJPZGNydlozQ0RickJKcFpCT2RjcnZaM0NEYnI$Sp3/ozKVXV2fupW2jGme8Q', 0, 0, 0),
  ('admin',  'admin',  'admin@port4k.com', '$argon2id$v=19$m=4096,t=3,p=1$YTRucTM0d2JrMmYwMDAwMA$ys7sXXH6ETEFmIVysP4fW6YQo5s6V/hy2VLrNF7CDEM', 100000, 100, 1000),
  ('alice',  'player', 'alice@port4k.com', '$argon2id$v=19$m=4096,t=3,p=1$MWIya3JwNmNnZTQwMDAwMA$jbsb0ayARAcFOHJ+tLIIR/mhd7ocQpOp0gTrW8cKPoQ',  2500, 100, 1000),
  ('bob',    'player', 'bob@port4k.com', '$argon2id$v=19$m=4096,t=3,p=1$ajlzMXB4Nm5sMHIwMDAwMA$msXwjUslddp3j8B7vRcPRXn84cAsXH2oPbqEjwl2yw4',  1500, 100, 1000),
  ('carol',  'player', 'carol@port4k.com', '$argon2id$v=19$m=4096,t=3,p=1$c3NudWZsM3UycWgwMDAwMA$rmhw1AzK4zZtbAEJyzKWaAMV56I5H4fF5qDvOWSGYPM',  500, 100, 1000)
ON CONFLICT (username) DO NOTHING;


INSERT INTO public.zones (id, key, title, kind, created_at) VALUES ('16b896de-7ba8-4d90-bec5-ccf06ca9cc5d', 'hub', 'The Hub', 'live', '2025-10-10 12:00:00.000000 +00:00');

COMMIT;