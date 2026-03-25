-- Add migration script here
INSERT INTO users (user_id, username, password_hash) VALUES  (
   '50a43b97-8862-4d46-8748-4003ca597393',
   'admin',
   '$argon2id$v=19$m=15000,t=2,p=1$ksKnrr4S3BtAH6Qz6ERyEw$e9hlc8pIuJ9MyYX1DUTBBh7dlhU0YslMvFjLe/VR3oI'
   );
