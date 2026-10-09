//! The `crewboard` command: launches the app and provides the `daemon`, `attach`, `hook` and agent-facing
//! subcommands (d0002, d0031).

fn main() {}

#[cfg(test)]
mod tests {
    #[test]
    fn crate_builds() {
        assert_eq!(env!("CARGO_BIN_NAME"), "crewboard");
    }
}
