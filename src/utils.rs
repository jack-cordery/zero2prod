use std::fmt::{Debug, Display};

use actix_web::{
    HttpResponse,
    error::{ErrorBadRequest, ErrorInternalServerError},
    http::header::LOCATION,
};

pub fn see_other(location: &str) -> HttpResponse {
    HttpResponse::SeeOther()
        .insert_header((LOCATION, location))
        .finish()
}

pub fn e500<T>(error: T) -> actix_web::Error
where
    T: Debug + Display + 'static,
{
    ErrorInternalServerError(error)
}

pub fn e400<T>(error: T) -> actix_web::Error
where
    T: Debug + Display + 'static,
{
    ErrorBadRequest(error)
}
