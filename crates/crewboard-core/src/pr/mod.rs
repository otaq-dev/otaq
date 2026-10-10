//! Pull requests (REV-09…REV-14; d0017, d0041, d0050, d0059): provider detection. Crewboard never watches PRs:
//! nothing here polls or calls GitHub or Azure DevOps on its own.

pub mod provider;
