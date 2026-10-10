// `sqlx::migrate!` embeds ./migrations at compile time; rebuild whenever a
// migration is added or edited so new tables are picked up without `cargo clean`.
fn main() {
    println!("cargo:rerun-if-changed=migrations");
}
