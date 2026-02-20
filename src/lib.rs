#![recursion_limit = "512"]

mod acp;
mod actor_cli;
mod actor_mcp;
mod agent;
mod api;
mod app;
mod auth;
mod config;
mod db;
mod internal;
mod push;
mod sse;
mod state;
mod team;
mod web;

pub use app::run;
