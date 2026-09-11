# Admin App Guidelines

## Dioxus RSX & `dioxus-i18n` Rules

### 1. RSX Child Expression Formatting
- **Always** render `tid!` calls as unquoted RSX child expression blocks `{tid!("key")}` rather than quoted strings `"{tid!(\"key\")}"`:
  ```rust
  // Correct
  h4 { class: "mt-2 mb-0", {tid!("app-login-title")} }
  Badge { color: Color::Success, {tid!("category-status-active")} }

  // Avoid
  h4 { class: "mt-2 mb-0", "{tid!(\"app-login-title\")}" }
  ```
- **Never** place parameterized macro calls inside quoted string interpolation (e.g. avoid `"{tid!(\"key\", param: val)}"`), as Dioxus interprets colons as format specifiers.
- **Do not** create unnecessary temporary `let` variable bindings just to hold formatted strings. Render them directly in RSX:
  ```rust
  FormText {
      {tid!(
          "members-filter-count",
          filtered: filtered_accounts.len(),
          total: available_accounts.len()
      )}
  }
  ```
- **Do not** place a trailing comma after the last argument of `tid!` (the `dioxus-i18n` macro pattern `($id:expr, $( $name:ident : $value:expr ),*)` does not allow trailing commas).
- For component props and element attributes, pass `tid!(...)` directly as the expression value without string interpolation:
  ```rust
  FormGroup { label: tid!("category-form-name"), ... }
  Input { placeholder: tid!("category-form-name-placeholder"), ... }
  ```

### 2. Internationalization (`dioxus-i18n` & Fluent)
- When adding user-visible strings in `admin/`, do not use hardcoded strings.
- **Prefer single parameterized keys** over splicing multiple translated sentence fragments together in RSX, to allow proper grammar and word order in each locale:
  ```ftl
  # Preferred in de.ftl / en.ftl:
  management-config-sync-success = Synchronisierung abgeschlossen: { $found } gefunden, { $added } hinzugefügt, { $updated } aktualisiert, { $removed } entfernt.
  ```
- Always provide corresponding keys in both:
  - `src/locales/de.ftl` (German, default/fallback)
  - `src/locales/en.ftl` (English)
- Use standard naming prefixes for keys:
  - `general-*` for generic UI actions/labels (e.g., `general-cancel`, `general-none`, `general-unknown`)
  - `<section>-*` for specific views (e.g., `members-*`, `subscriptions-*`, `subscriber-*`, `management-config-*`)
