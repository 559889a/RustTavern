mod app;
mod infrastructure;
mod observability_targets;
mod presentation;
mod product;
mod server;

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

pub fn run() {
    server::run();
}
