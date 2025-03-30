-- This file should undo anything in `up.sql`
alter table imap_connection drop column imap_server;
alter table imap_connection drop column imap_port;
