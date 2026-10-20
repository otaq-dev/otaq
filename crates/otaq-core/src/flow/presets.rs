//! The shipped preset files (FLW-08, d0108), embedded at build time: what **Use a preset…** and `otaq flow
//! presets` list and `flow.propose_from` proposes. One list: each preset file Otaq ships is one entry here.

use crate::naming;

/// One shipped preset: its file name and its contents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Preset {
    pub file: &'static str,
    pub yaml: &'static str,
}

/// Every shipped preset, in the order Use a preset… shows them.
pub const PRESETS: &[Preset] = &[Preset {
    file: naming::DEFAULT_FLOW_FILE,
    yaml: naming::DEFAULT_FLOW_YAML,
}];

/// The shipped preset `file` ("otaq-default.yaml").
pub fn preset(file: &str) -> Option<&'static Preset> {
    PRESETS.iter().find(|p| p.file == file)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_preset_reads_and_passes_the_checks() {
        for preset in PRESETS {
            let file = crate::flow::yaml::read(preset.yaml).expect(preset.file);
            assert_eq!(crate::flow::check(&file.flow), Ok(()), "{}", preset.file);
        }
        assert!(preset(naming::DEFAULT_FLOW_FILE).is_some());
        assert!(preset("nope.yaml").is_none());
    }
}
