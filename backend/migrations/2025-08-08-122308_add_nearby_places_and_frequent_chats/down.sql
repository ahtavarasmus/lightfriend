-- This file should undo anything in `up.sql`
alter table user_info drop column nearby_places;
alter table user_info drop column recent_contacts;
alter table user_info drop column blocker_password_vault;
alter table user_info drop column lockbox_password_vault;
