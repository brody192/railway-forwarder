//! Railway authentication, typed GraphQL requests and catalog mapping.
mod account;
pub(crate) mod auth;
mod client;
mod graphql;
pub(crate) use auth::OAuth;
pub(crate) use client::Railway;
