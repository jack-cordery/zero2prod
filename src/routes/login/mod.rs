mod get;
mod oidc;
mod post;

pub use get::login_form;
pub use oidc::CallbackQuery;
pub use oidc::callback;
pub use oidc::initiate_google_login;
pub use post::LoginError;
pub use post::REDIRECT_FAILED_LOGIN;
pub use post::REDIRECT_SUCCESSFUL_LOGIN;
pub use post::login;
pub use post::login_redirect;
