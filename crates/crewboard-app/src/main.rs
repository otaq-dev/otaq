//! Crewboard desktop app: the GPUI client of the background service (d0001, d0022).

fn main() {}

#[cfg(test)]
mod tests {
    #[test]
    fn links_the_core() {
        assert!(!crewboard_core::naming::PRODUCT_NAME.is_empty());
    }
}
