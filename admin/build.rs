use serde::Deserialize;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

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
    page_title: Option<String>,
    #[serde(default)]
    icon: Option<String>,
    #[serde(default)]
    favicon: Option<String>,
    #[serde(default)]
    theme: ThemeConfig,
    #[serde(default)]
    i18n: BrandI18nConfig,
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

#[derive(Debug, Deserialize, Default)]
struct BrandI18nConfig {
    #[serde(default)]
    de: BrandTexts,
    #[serde(default)]
    en: BrandTexts,
}

#[derive(Debug, Deserialize, Default)]
struct BrandTexts {
    #[serde(default)]
    brand_name: Option<String>,
    #[serde(default)]
    login_title: Option<String>,
    #[serde(default)]
    login_subtitle: Option<String>,
    #[serde(default)]
    login_button: Option<String>,
    #[serde(default)]
    community_name: Option<String>,
    #[serde(default)]
    subscriptions_required: Option<String>,
    #[serde(default)]
    email_verification_continue: Option<String>,
}

fn resolve_profile_path(manifest_dir: &Path) -> (String, PathBuf) {
    if let Ok(path_str) = env::var("APP_CONFIG_PATH") {
        if !path_str.trim().is_empty() {
            let path = PathBuf::from(&path_str);
            let path = if path.is_absolute() {
                path
            } else {
                manifest_dir.join(path)
            };
            return (path_str, path);
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

    let filename = if profile_name.ends_with(".toml") {
        profile_name.clone()
    } else {
        format!("{profile_name}.toml")
    };

    let workspace_root = manifest_dir.parent().unwrap_or(manifest_dir);
    let path = workspace_root.join("profiles").join(filename);
    (profile_name, path)
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

fn generate_ftl_terms(texts: &BrandTexts, is_de: bool) -> String {
    let brand_name = texts.brand_name.as_deref().unwrap_or(if is_de {
        "Kommunikationszentrum"
    } else {
        "Communication Center"
    });
    let login_title = texts.login_title.as_deref().unwrap_or(brand_name);
    let login_subtitle = texts.login_subtitle.as_deref().unwrap_or(if is_de {
        "Nachrichtenverwaltung"
    } else {
        "Message Management"
    });
    let login_button =
        texts
            .login_button
            .as_deref()
            .unwrap_or(if is_de { "Anmelden" } else { "Sign in" });
    let community_name =
        texts
            .community_name
            .as_deref()
            .unwrap_or(if is_de { "Gemeinschaft" } else { "Community" });
    let subscriptions_required = texts.subscriptions_required.as_deref().unwrap_or(if is_de {
        "Dieses Thema ist unbedingt notwendig für das Teilnehmen an der Gemeinschaft"
    } else {
        "This topic is required to participate in the community"
    });
    let email_verification_continue =
        texts
            .email_verification_continue
            .as_deref()
            .unwrap_or(if is_de {
                "Weiter zum Kommunikationszentrum"
            } else {
                "Continue to Communication Center"
            });

    format!(
        "-brand-name = {}\n-brand-login-title = {}\n-brand-login-subtitle = {}\n-brand-login-button = {}\n-brand-community-name = {}\n-brand-subscriptions-required = {}\n-brand-verification-continue = {}\n",
        brand_name,
        login_title,
        login_subtitle,
        login_button,
        community_name,
        subscriptions_required,
        email_verification_continue
    )
}

fn main() {
    println!("cargo:rerun-if-env-changed=APP_PROFILE");
    println!("cargo:rerun-if-env-changed=APP_CONFIG_PATH");

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let (profile_name, profile_path) = resolve_profile_path(&manifest_dir);

    println!("cargo:rerun-if-changed={}", profile_path.display());

    let profile_content = fs::read_to_string(&profile_path).unwrap_or_else(|e| {
        panic!(
            "Failed to read profile configuration file at '{}' (selected profile: '{}'): {}.\n\
             Ensure the profile TOML file exists in the 'profiles/' directory or configure APP_CONFIG_PATH.",
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

    let page_title = config
        .brand
        .page_title
        .unwrap_or_else(|| "Kommunikationszentrum Admin".to_string());

    let icon = config
        .brand
        .icon
        .unwrap_or_else(|| "envelope-fill".to_string());

    let favicon = config
        .brand
        .favicon
        .unwrap_or_else(|| "/assets/favicon.ico".to_string());

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    // Generate brand_terms_de.ftl and brand_terms_en.ftl
    let ftl_de = generate_ftl_terms(&config.brand.i18n.de, true);
    let ftl_en = generate_ftl_terms(&config.brand.i18n.en, false);
    fs::write(out_dir.join("brand_terms_de.ftl"), ftl_de)
        .expect("Failed to write brand_terms_de.ftl");
    fs::write(out_dir.join("brand_terms_en.ftl"), ftl_en)
        .expect("Failed to write brand_terms_en.ftl");

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
