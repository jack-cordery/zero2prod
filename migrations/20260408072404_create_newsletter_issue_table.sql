-- Add migration script here
CREATE TABLE newsletters (
   id uuid PRIMARY KEY,
   user_id uuid NOT NULL REFERENCES users(user_id), 
   title TEXT NOT NULL,
   content_text TEXT NOT NULL,
   content_html TEXT NOT NULL
);

