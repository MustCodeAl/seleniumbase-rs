//! Cookies, `localStorage` and `sessionStorage`.

use serde_json::Value;

use super::Cdp;
use crate::error::SeleniumBaseError;
use crate::mcp::support::{cookie_file, json_output};
use crate::mcp::{Args, Ctx, Effect, Output, Prop, Schema, ToolDef, ToolError};

pub(super) fn tools() -> Vec<ToolDef<Cdp>> {
    vec![
        ToolDef::new(
            "manage_cookies",
            "Manage Cookies",
            "List, clear, save or load the session's cookies. Saved files live in the \
             server's output directory, under saved_cookies. Cookies can hold logins; \
             handle them with care.",
            Effect::Mixed,
            Schema::new()
                .optional(
                    "action",
                    Prop::choice(&["get_all", "clear", "save", "load"], "What to do")
                        .default("get_all"),
                )
                .optional(
                    "filename",
                    Prop::string("File name for save and load; only its last path part is used")
                        .default("cookies.txt"),
                ),
            manage_cookies,
        ),
        ToolDef::new(
            "manage_storage",
            "Manage Storage",
            "Read or write one key of localStorage or sessionStorage for the current page's \
             origin.",
            Effect::Mixed,
            Schema::new()
                .required("key", Prop::string("The storage key"))
                .optional(
                    "value",
                    Prop::string("The value to store; required for set"),
                )
                .optional(
                    "storage",
                    Prop::choice(&["local", "session"], "Which storage").default("local"),
                )
                .optional(
                    "action",
                    Prop::choice(&["get", "set"], "Read or write").default("get"),
                ),
            manage_storage,
        ),
    ]
}

async fn manage_cookies(ctx: Ctx<Cdp>, args: Args) -> Result<Output, ToolError> {
    let action = args.choice("action", &["get_all", "clear", "save", "load"], "get_all")?;
    let session = ctx.session().await?;
    let cookies = session.page.cookies();

    match action {
        "get_all" => json_output(cookies.all().await?),
        "clear" => {
            cookies.clear().await?;
            Ok("Cookies cleared.".into())
        }
        _ => {
            let (path, name) =
                cookie_file(ctx.settings(), args.str_or("filename", "cookies.txt")?)?;

            if action == "save" {
                if let Some(dir) = path.parent() {
                    tokio::fs::create_dir_all(dir)
                        .await
                        .map_err(SeleniumBaseError::from)?;
                }
                let saved = cookies.save(&path, None).await?;
                Ok(format!("Saved {saved} cookies to {name}").into())
            } else {
                if !path.exists() {
                    return Err(ToolError::Refused(format!(
                        "{name} does not exist in the cookie directory"
                    )));
                }
                let loaded = cookies.load(&path, None).await?;
                Ok(format!("Loaded {loaded} cookies from {name}").into())
            }
        }
    }
}

async fn manage_storage(ctx: Ctx<Cdp>, args: Args) -> Result<Output, ToolError> {
    let key = args.str("key")?;
    let which = args.choice("storage", &["local", "session"], "local")?;
    let action = args.choice("action", &["get", "set"], "get")?;
    let session = ctx.session().await?;
    let storage = if which == "local" {
        session.page.local_storage()
    } else {
        session.page.session_storage()
    };

    if action == "get" {
        return Ok(Output::Json(
            storage.get(key).await?.map_or(Value::Null, Value::String),
        ));
    }
    let value = args
        .opt_str("value")?
        .ok_or_else(|| ToolError::invalid("value", "required when action is set"))?;
    storage.set(key, value).await?;
    Ok(format!("Set {which}Storage[{key:?}]").into())
}
