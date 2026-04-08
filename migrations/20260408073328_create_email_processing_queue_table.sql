-- Add migration script here
CREATE TABLE email_processing_tasks(
   subscriber_id uuid NOT NULL REFERENCES subscriptions(id),
   newsletter_id uuid NOT NULL REFERENCES newsletters(id),
   PRIMARY KEY (subscriber_id, newsletter_id)
);
