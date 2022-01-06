use cargo_metadata::DependencyKind;

#[derive(Debug, PartialEq)]
pub enum ExtraDeps {
    All,
    Build,
    Dev,
    NoMore,
}

impl ExtraDeps {
    // This clippy recommendation is valid, but makes this function much harder to read
    #[allow(clippy::match_like_matches_macro)]
    pub fn allows(&self, dependency_kind: DependencyKind) -> bool {
        match (self, dependency_kind) {
            (ExtraDeps::All, _) => true,
            (ExtraDeps::NoMore, _) => false,
            (_, DependencyKind::Normal) => true,
            (ExtraDeps::Build, DependencyKind::Build) => true,
            (ExtraDeps::Dev, DependencyKind::Development) => true,
            _ => false,
        }
    }
}

#[cfg(test)]
mod extra_deps_tests {
    use super::*;
    use rstest::*;

    #[rstest(
        input_extra_deps,
        input_dependency_kind,
        expected_allows,
        case(ExtraDeps::All, DependencyKind::Normal, true),
        case(ExtraDeps::Build, DependencyKind::Normal, true),
        case(ExtraDeps::Dev, DependencyKind::Normal, true),
        case(ExtraDeps::NoMore, DependencyKind::Normal, false),
        case(ExtraDeps::All, DependencyKind::Build, true),
        case(ExtraDeps::All, DependencyKind::Development, true),
        case(ExtraDeps::Build, DependencyKind::Build, true),
        case(ExtraDeps::Build, DependencyKind::Development, false),
        case(ExtraDeps::Dev, DependencyKind::Build, false),
        case(ExtraDeps::Dev, DependencyKind::Development, true)
    )]
    fn extra_deps_allows_test(
        input_extra_deps: ExtraDeps,
        input_dependency_kind: DependencyKind,
        expected_allows: bool,
    ) {
        assert_eq!(
            input_extra_deps.allows(input_dependency_kind),
            expected_allows
        );
    }
}
