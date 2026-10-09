//! The `crewboard` command: launches the app and provides the `daemon`, `attach`, `hook` and agent-facing
//! subcommands (d0002, d0031).

fn main() {}

#[cfg(test)]
mod tests {
    #[test]
    fn binary_is_named_after_the_command() {
        assert_eq!(env!("CARGO_BIN_NAME"), crewboard_core::naming::COMMAND_NAME);
    }
}
