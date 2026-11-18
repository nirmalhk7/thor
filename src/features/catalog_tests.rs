use super::*;

#[test]
fn catalog_lookup_preserves_display_names_categories_and_maturity() {
    let registry = FeatureRegistry::new();
    let cases = [
        (
            "state-timetravel",
            "State Time-Travel",
            FeatureCategory::Frontend,
            FeatureStatus::Stable,
        ),
        (
            "zindex-visualizer",
            "Z-Index Visualizer",
            FeatureCategory::Frontend,
            FeatureStatus::Stable,
        ),
        (
            "network-component-binding",
            "Network↔Component Binding",
            FeatureCategory::Frontend,
            FeatureStatus::Stable,
        ),
        (
            "soft-nav-vitals",
            "Soft-Nav Vitals (experimental)",
            FeatureCategory::Performance,
            FeatureStatus::Experimental,
        ),
    ];
    for (slug, name, category, status) in cases {
        let descriptor = registry.by_slug(slug).unwrap();
        assert_eq!(descriptor.name, name);
        assert_eq!(descriptor.category, category);
        assert_eq!(descriptor.status, status);
        assert_eq!(registry.get(descriptor.id).unwrap().slug, slug);
    }
}
