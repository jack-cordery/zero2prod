mod dashboard;
mod issues;
mod logout;
mod newsletter;
mod password;

pub use dashboard::dashboard;
pub use issues::issues;
pub use logout::logout;
pub use newsletter::newsletter_form;
pub use newsletter::publish_newsletter;
pub use password::change_password;
pub use password::password_form;
