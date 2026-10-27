//! Explicit, benchmark-only options. The ordinary desk does not read these variables.
use serde::Serialize;

pub(super) const LAYOUT_ENV: &str = "LIGHT_STAGE_PACKAGED_BENCH_LAYOUT";
const QUALITY_ENV: &str = "LIGHT_STAGE_PACKAGED_BENCH_QUALITY";
const VIEW_ENV: &str = "LIGHT_STAGE_PACKAGED_BENCH_VIEW";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct BaselineOptions {
    pub baseline_layout: Option<String>,
    pub baseline_quality: String,
    pub baseline_view: String,
}

impl BaselineOptions {
    pub fn from_env() -> Result<Self, String> {
        Self::parse(
            std::env::var(LAYOUT_ENV).ok().as_deref(),
            std::env::var(QUALITY_ENV).ok().as_deref(),
            std::env::var(VIEW_ENV).ok().as_deref(),
        )
    }

    fn parse(
        layout: Option<&str>,
        quality: Option<&str>,
        view: Option<&str>,
    ) -> Result<Self, String> {
        if layout.is_none() && (quality.is_some() || view.is_some()) {
            return Err(format!("{QUALITY_ENV} and {VIEW_ENV} require {LAYOUT_ENV}"));
        }
        Ok(Self {
            baseline_layout: layout
                .map(|value| choice(LAYOUT_ENV, value, &["single", "dual"]))
                .transpose()?,
            baseline_quality: choice(
                QUALITY_ENV,
                quality.unwrap_or("draft"),
                &["draft", "standard", "high", "ultra"],
            )?,
            baseline_view: choice(
                VIEW_ENV,
                view.unwrap_or("lines_3d"),
                &["full_3d", "lines_3d"],
            )?,
        })
    }
}

fn choice(name: &str, value: &str, allowed: &[&str]) -> Result<String, String> {
    if allowed.contains(&value) {
        Ok(value.to_string())
    } else {
        Err(format!("{name} must be one of {}", allowed.join(", ")))
    }
}

#[cfg(test)]
mod tests {
    use super::BaselineOptions;

    #[test]
    fn default_preserves_lifecycle_and_fixed_layout_allows_explicit_matrix() {
        let default = BaselineOptions::parse(None, None, None).unwrap();
        assert_eq!(default.baseline_layout, None);
        assert_eq!(default.baseline_quality, "draft");
        assert_eq!(default.baseline_view, "lines_3d");
        let fixed = BaselineOptions::parse(Some("dual"), Some("ultra"), Some("full_3d")).unwrap();
        assert_eq!(fixed.baseline_layout.as_deref(), Some("dual"));
        assert_eq!(fixed.baseline_quality, "ultra");
        assert_eq!(fixed.baseline_view, "full_3d");
    }

    #[test]
    fn invalid_matrix_cannot_silently_fall_back() {
        assert!(BaselineOptions::parse(None, Some("ultra"), None).is_err());
        assert!(BaselineOptions::parse(Some("triple"), None, None).is_err());
        assert!(BaselineOptions::parse(Some("single"), Some("best"), None).is_err());
        assert!(BaselineOptions::parse(Some("single"), None, Some("3d")).is_err());
    }
}
