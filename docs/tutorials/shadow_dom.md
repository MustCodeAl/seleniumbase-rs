# Shadow DOM Guide

Web Components often hide markup inside Shadow DOM trees. SeleniumBase for Rust can pierce through open shadow roots with `::shadow` selectors.

## Basic usage

```rust
sb.shadow_click("my-app ::shadow button").await?;
sb.shadow_type("my-app ::shadow input", "hello").await?;
let text = sb.shadow_get_text("my-app ::shadow div.result").await?;
```

Each `::shadow` fragment moves the search context into the shadow root of the preceding element.

## Nested shadow roots

Chain multiple `::shadow` fragments for nested Web Components.

```rust
sb.shadow_click("app-shell ::shadow nav-menu ::shadow a[href='/settings']").await?;
```

## Find elements inside shadow DOM

```rust
let elements = sb.shadow_find_elements("my-app ::shadow li").await?;
for el in elements {
    println!("{}", el.text().await?);
}
```

## Query the shadow root directly

```rust
let root = sb.get_shadow_root("my-app").await?;
let button = root.find(By::Css("button")).await?;
button.click().await?;
```

## Limitations

- Only **open** shadow roots are accessible. Closed shadow roots cannot be pierced by WebDriver or CDP.
- CSS selectors inside shadow roots must match the local DOM; they do not leak out to the parent tree.
