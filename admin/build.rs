use fluent_bundle::{FluentBundle, FluentResource};
use serde::Deserialize;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use unic_langid::langid;

#[derive(Debug, Deserialize, Default)]
struct ProfileConfig {
    #[serde(default)]
    spacetimedb: SpacetimeDbConfig,
    #[serde(default)]
    oauth: OAuthConfig,
    #[serde(default)]
    brand: BrandConfig,
}

#[derive(Debug, Deserialize, Default)]
struct SpacetimeDbConfig {
    #[serde(default)]
    uri: Option<String>,
    #[serde(default)]
    module_name: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct OAuthConfig {
    #[serde(default)]
    issuer_url: Option<String>,
    #[serde(default)]
    client_id: Option<String>,
    #[serde(default)]
    redirect_uri: Option<String>,
    #[serde(default)]
    scope: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct BrandConfig {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    page_title: Option<String>,
    #[serde(default)]
    icon: Option<String>,
    #[serde(default)]
    favicon: Option<String>,
    #[serde(default)]
    theme: ThemeConfig,
}

#[derive(Debug, Deserialize, Default)]
struct ThemeConfig {
    #[serde(default)]
    colors: ColorTokens,
    #[serde(default)]
    surfaces: SurfaceTokens,
    #[serde(default)]
    dark: Option<DarkThemeConfig>,
}

#[derive(Debug, Deserialize, Default)]
struct DarkThemeConfig {
    #[serde(default)]
    colors: ColorTokens,
    #[serde(default)]
    surfaces: SurfaceTokens,
}

#[derive(Debug, Deserialize, Default, Clone)]
struct ColorTokens {
    #[serde(default)]
    primary: Option<String>,
    #[serde(default)]
    secondary: Option<String>,
    #[serde(default)]
    success: Option<String>,
    #[serde(default)]
    info: Option<String>,
    #[serde(default)]
    warning: Option<String>,
    #[serde(default)]
    danger: Option<String>,
    #[serde(default)]
    light: Option<String>,
    #[serde(default)]
    dark: Option<String>,
}

#[derive(Debug, Deserialize, Default, Clone)]
struct SurfaceTokens {
    #[serde(default)]
    body_bg: Option<String>,
    #[serde(default)]
    body_color: Option<String>,
    #[serde(default)]
    secondary_bg: Option<String>,
    #[serde(default)]
    secondary_color: Option<String>,
    #[serde(default)]
    border_color: Option<String>,
    #[serde(default)]
    link_color: Option<String>,
    #[serde(default)]
    link_hover_color: Option<String>,
}

fn resolve_profile_dir(manifest_dir: &Path) -> (String, PathBuf) {
    if let Ok(path_str) = env::var("APP_CONFIG_PATH") {
        if !path_str.trim().is_empty() {
            let path = PathBuf::from(&path_str);
            let path = if path.is_absolute() {
                path
            } else {
                manifest_dir.join(path)
            };
            let dir = if path.is_dir() {
                path
            } else {
                path.parent().unwrap_or(&path).to_path_buf()
            };
            return (path_str, dir);
        }
    }

    let profile = env::var("APP_PROFILE")
        .unwrap_or_else(|_| "example".to_string())
        .trim()
        .to_string();
    let profile_name = if profile.is_empty() {
        "example".to_string()
    } else {
        profile
    };

    let workspace_root = manifest_dir.parent().unwrap_or(manifest_dir);
    let dir = workspace_root.join("profiles").join(&profile_name);
    (profile_name, dir)
}

fn format_opt_str(val: &Option<String>) -> String {
    match val {
        Some(s) => format!("Some({:?})", s),
        None => "None".to_string(),
    }
}

fn format_colors(colors: &ColorTokens) -> String {
    format!(
        "GeneratedThemeColors {{\n    primary: {},\n    secondary: {},\n    success: {},\n    info: {},\n    warning: {},\n    danger: {},\n    light: {},\n    dark: {},\n}}",
        format_opt_str(&colors.primary),
        format_opt_str(&colors.secondary),
        format_opt_str(&colors.success),
        format_opt_str(&colors.info),
        format_opt_str(&colors.warning),
        format_opt_str(&colors.danger),
        format_opt_str(&colors.light),
        format_opt_str(&colors.dark),
    )
}

fn format_surfaces(surfaces: &SurfaceTokens) -> String {
    format!(
        "GeneratedThemeSurfaces {{\n    body_bg: {},\n    body_color: {},\n    secondary_bg: {},\n    secondary_color: {},\n    border_color: {},\n    link_color: {},\n    link_hover_color: {},\n}}",
        format_opt_str(&surfaces.body_bg),
        format_opt_str(&surfaces.body_color),
        format_opt_str(&surfaces.secondary_bg),
        format_opt_str(&surfaces.secondary_color),
        format_opt_str(&surfaces.border_color),
        format_opt_str(&surfaces.link_color),
        format_opt_str(&surfaces.link_hover_color),
    )
}

fn main() {
    println!("cargo:rerun-if-env-changed=APP_PROFILE");
    println!("cargo:rerun-if-env-changed=APP_CONFIG_PATH");

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let (profile_name, profile_dir) = resolve_profile_dir(&manifest_dir);
    let profile_path = profile_dir.join("profile.toml");

    println!("cargo:rerun-if-changed={}", profile_path.display());

    let profile_content = fs::read_to_string(&profile_path).unwrap_or_else(|e| {
        panic!(
            "Failed to read profile configuration file at '{}' (selected profile: '{}'): {}.\n\
             Ensure the profile directory exists under 'profiles/<name>/' and contains 'profile.toml'.",
            profile_path.display(),
            profile_name,
            e
        );
    });

    let config: ProfileConfig = toml::from_str(&profile_content).unwrap_or_else(|e| {
        panic!(
            "Failed to parse TOML configuration from '{}': {}",
            profile_path.display(),
            e
        );
    });

    // Profile configuration values
    let spacetimedb_uri = config
        .spacetimedb
        .uri
        .unwrap_or_else(|| "http://localhost:3000".to_string());

    let spacetimedb_module_name = config
        .spacetimedb
        .module_name
        .unwrap_or_else(|| "kommunikation".to_string());

    let oidc_issuer_url = config
        .oauth
        .issuer_url
        .unwrap_or_else(|| "http://127.0.0.1:8000/o".to_string());

    let oidc_client_id = config
        .oauth
        .client_id
        .unwrap_or_else(|| "admin-app".to_string());

    let oidc_redirect_uri = config
        .oauth
        .redirect_uri
        .unwrap_or_else(|| "http://127.0.0.1:8080/callback".to_string());

    let oauth_scope = config
        .oauth
        .scope
        .unwrap_or_else(|| "openid profile email".to_string());

    let brand_name = config
        .brand
        .name
        .unwrap_or_else(|| "Kommunikationszentrum".to_string());

    let page_title = config
        .brand
        .page_title
        .unwrap_or_else(|| format!("{brand_name} Admin"));

    let icon = config
        .brand
        .icon
        .unwrap_or_else(|| "envelope-fill".to_string());

    let favicon = config
        .brand
        .favicon
        .unwrap_or_else(|| "/assets/favicon.ico".to_string());

    let locales_dir = profile_dir.join("locales");
    let profile_de_path = locales_dir.join("de.ftl");
    let profile_en_path = locales_dir.join("en.ftl");

    println!("cargo:rerun-if-changed={}", profile_de_path.display());
    println!("cargo:rerun-if-changed={}", profile_en_path.display());

    let base_de_path = manifest_dir.join("src/locales/de.ftl");
    let base_en_path = manifest_dir.join("src/locales/en.ftl");

    let base_de_content =
        fs::read_to_string(&base_de_path).expect("Failed to read admin/src/locales/de.ftl");
    let base_en_content =
        fs::read_to_string(&base_en_path).expect("Failed to read admin/src/locales/en.ftl");

    let profile_de_content = fs::read_to_string(&profile_de_path).unwrap_or_else(|e| {
        panic!(
            "Failed to read profile German locale at '{}' (selected profile: '{}'): {}.\n\
             Every profile must contain 'locales/de.ftl'.",
            profile_de_path.display(),
            profile_name,
            e
        );
    });
    let profile_en_content = fs::read_to_string(&profile_en_path).unwrap_or_else(|e| {
        panic!(
            "Failed to read profile English locale at '{}' (selected profile: '{}'): {}.\n\
             Every profile must contain 'locales/en.ftl'.",
            profile_en_path.display(),
            profile_name,
            e
        );
    });

    // Validate German bundle with add_resource_overriding
    let base_de_res = FluentResource::try_new(base_de_content)
        .map_err(|(_, errs)| format!("Parse error in base de.ftl: {:?}", errs))
        .unwrap();
    let profile_de_res = FluentResource::try_new(profile_de_content.clone())
        .map_err(|(_, errs)| {
            format!(
                "Parse error in profile locale '{}': {:?}",
                profile_de_path.display(),
                errs
            )
        })
        .unwrap();
    let mut bundle_de = FluentBundle::new(vec![langid!("de")]);
    bundle_de
        .add_resource(base_de_res)
        .map_err(|errs| format!("Error adding base de.ftl to bundle: {:?}", errs))
        .unwrap();
    bundle_de.add_resource_overriding(profile_de_res);

    // Validate English bundle with add_resource_overriding
    let base_en_res = FluentResource::try_new(base_en_content)
        .map_err(|(_, errs)| format!("Parse error in base en.ftl: {:?}", errs))
        .unwrap();
    let profile_en_res = FluentResource::try_new(profile_en_content.clone())
        .map_err(|(_, errs)| {
            format!(
                "Parse error in profile locale '{}': {:?}",
                profile_en_path.display(),
                errs
            )
        })
        .unwrap();
    let mut bundle_en = FluentBundle::new(vec![langid!("en")]);
    bundle_en
        .add_resource(base_en_res)
        .map_err(|errs| format!("Error adding base en.ftl to bundle: {:?}", errs))
        .unwrap();
    bundle_en.add_resource_overriding(profile_en_res);

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    // Write validated profile FTL files to OUT_DIR
    fs::write(out_dir.join("profile_de.ftl"), profile_de_content)
        .expect("Failed to write profile_de.ftl");
    fs::write(out_dir.join("profile_en.ftl"), profile_en_content)
        .expect("Failed to write profile_en.ftl");

    // Generate generated_config.rs
    let dark_section = if let Some(dark) = &config.brand.theme.dark {
        format!(
            "Some(GeneratedThemeTokens {{\n    colors: {},\n    surfaces: {},\n}})",
            format_colors(&dark.colors),
            format_surfaces(&dark.surfaces)
        )
    } else {
        "None".to_string()
    };

    let generated_code = format!(
        r#"// Auto-generated by admin/build.rs from profile: {profile_name}
// DO NOT EDIT MANUALLY.

pub struct GeneratedSpacetimeDbConfig {{
    pub uri: &'static str,
    pub module_name: &'static str,
}}

pub struct GeneratedOAuthConfig {{
    pub issuer_url: &'static str,
    pub client_id: &'static str,
    pub redirect_uri: &'static str,
    pub scope: &'static str,
}}

pub struct GeneratedBrandConfig {{
    pub name: &'static str,
    pub page_title: &'static str,
    pub icon: &'static str,
    pub favicon: &'static str,
}}

#[derive(Clone, Copy)]
pub struct GeneratedThemeColors {{
    pub primary: Option<&'static str>,
    pub secondary: Option<&'static str>,
    pub success: Option<&'static str>,
    pub info: Option<&'static str>,
    pub warning: Option<&'static str>,
    pub danger: Option<&'static str>,
    pub light: Option<&'static str>,
    pub dark: Option<&'static str>,
}}

#[derive(Clone, Copy)]
pub struct GeneratedThemeSurfaces {{
    pub body_bg: Option<&'static str>,
    pub body_color: Option<&'static str>,
    pub secondary_bg: Option<&'static str>,
    pub secondary_color: Option<&'static str>,
    pub border_color: Option<&'static str>,
    pub link_color: Option<&'static str>,
    pub link_hover_color: Option<&'static str>,
}}

#[derive(Clone, Copy)]
pub struct GeneratedThemeTokens {{
    pub colors: GeneratedThemeColors,
    pub surfaces: GeneratedThemeSurfaces,
}}

pub struct GeneratedThemeConfig {{
    pub colors: GeneratedThemeColors,
    pub surfaces: GeneratedThemeSurfaces,
    pub dark: Option<GeneratedThemeTokens>,
}}

pub const GENERATED_SPACETIMEDB: GeneratedSpacetimeDbConfig = GeneratedSpacetimeDbConfig {{
    uri: {spacetimedb_uri:?},
    module_name: {spacetimedb_module_name:?},
}};

pub const GENERATED_OAUTH: GeneratedOAuthConfig = GeneratedOAuthConfig {{
    issuer_url: {oidc_issuer_url:?},
    client_id: {oidc_client_id:?},
    redirect_uri: {oidc_redirect_uri:?},
    scope: {oauth_scope:?},
}};

pub const GENERATED_BRAND: GeneratedBrandConfig = GeneratedBrandConfig {{
    name: {brand_name:?},
    page_title: {page_title:?},
    icon: {icon:?},
    favicon: {favicon:?},
}};

pub const GENERATED_THEME: GeneratedThemeConfig = GeneratedThemeConfig {{
    colors: {light_colors},
    surfaces: {light_surfaces},
    dark: {dark_section},
}};
"#,
        profile_name = profile_name,
        spacetimedb_uri = spacetimedb_uri,
        spacetimedb_module_name = spacetimedb_module_name,
        oidc_issuer_url = oidc_issuer_url,
        oidc_client_id = oidc_client_id,
        oidc_redirect_uri = oidc_redirect_uri,
        oauth_scope = oauth_scope,
        brand_name = brand_name,
        page_title = page_title,
        icon = icon,
        favicon = favicon,
        light_colors = format_colors(&config.brand.theme.colors),
        light_surfaces = format_surfaces(&config.brand.theme.surfaces),
        dark_section = dark_section,
    );

    fs::write(out_dir.join("generated_config.rs"), generated_code)
        .expect("Failed to write generated_config.rs");
}
