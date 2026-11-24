//! `meridian_setup`: create an account, get an API key, and buy credits with Bitcoin, so an agent can take a
//! user from nothing to a funded account. The password is used for one call and never kept.

use reqwest::Method;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::backend::{Backend, Raw, Reply};
use crate::session::Session;

#[derive(Deserialize, JsonSchema)]
pub struct SetupParams {
    /// What to do. With no account yet and no email: `signup_with_bitcoin`, then `signup_status`. With an email: 
    /// `create_account`, then `request_api_key`. With an account: `buy_credits`, then `purchase_status`.
    pub action: String,
    /// `create_account`, `request_api_key`: the account's username (or its email, if no username was chosen).
    pub username: Option<String>,
    /// `create_account`: the account's email; a verification link is sent to it. `request_api_key`: may be used
    /// in place of `username`.
    pub email: Option<String>,
    /// `create_account`, `request_api_key`: the account password (8-20 characters, at least one special
    /// character). Ask the user to choose it; it is sent once and never stored.
    pub password: Option<String>,
    /// `create_account`: optional phone number.
    pub phone: Option<String>,
    /// `create_account`: the Cloudflare Turnstile token, when the server requires one. Only a person in a browser
    /// can produce it; if sign-up is refused for lack of it, send the user to the Meridian web app to sign up.
    pub turnstile_token: Option<String>,
    /// `request_api_key`: true to revoke the account's current key and issue a new one. Anything using the old key
    /// stops working, so only set it when the user has said so.
    pub replace_existing: Option<bool>,
    /// `signup_with_bitcoin`, `buy_credits`: how many credits to buy (1 to 10000). A new account's first purchase
    /// must be at least the minimum the server states (5 USD at 0.50 USD a credit is 10 credits).
    pub credits: Option<u32>,
    /// `purchase_status`: the `purchaseId` that `buy_credits` returned.
    pub purchase: Option<String>,
    /// `signup_status`: the `signupId` and `claim_secret` that `signup_with_bitcoin` returned. Both are remembered
    /// for this session, so they are only needed after a restart.
    pub signup: Option<String>,
    pub claim_secret: Option<String>,
}

/// The result of a setup call: a JSON value for the agent, or an error message.
pub type Outcome = Result<Value, String>;

pub async fn run(backend: &Backend, session: &Session, p: SetupParams) -> Outcome {
    match p.action.to_ascii_lowercase().as_str() {
        "signup_with_bitcoin" => signup_with_bitcoin(backend, session, p).await,
        "signup_status" => signup_status(backend, session, p).await,
        "create_account" => create_account(backend, p).await,
        "request_api_key" => request_api_key(backend, p).await,
        "buy_credits" => buy_credits(backend, p).await,
        "purchase_status" => purchase_status(backend, p).await,
        other => Err(format!(
            "Unknown action `{other}`. Use `signup_with_bitcoin`, `signup_status`, `create_account`, `request_api_key`, `buy_credits` or `purchase_status`."
        )),
    }
}

fn required(value: &Option<String>, what: &str) -> Result<String, String> {
    value
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(str::to_string)
        .ok_or_else(|| format!("`{what}` is required for this action. Ask the user for it."))
}

fn failure(what: &str, r: &Raw) -> String {
    let v = r.json();
    let message = v.get("message").and_then(Value::as_str).unwrap_or("request failed");
    let code = v.get("code").and_then(Value::as_str).unwrap_or("");
    format!("{what}: {} {code} {message}", r.status).replace("  ", " ")
}

async fn create_account(backend: &Backend, p: SetupParams) -> Outcome {
    let email = required(&p.email, "email")?;
    let password = required(&p.password, "password")?;
    let username = p.username.as_deref().map(str::trim).filter(|u| !u.is_empty()).unwrap_or(&email).to_string();
    let body = json!({
        "username": username, "password": password, "email": email,
        "phone": p.phone, "turnstileToken": p.turnstile_token,
    });
    let r = backend.raw(Method::POST, "/user/create", Some(body), None, &[]).await?;
    if r.ok() {
        return Ok(json!({
            "created": true,
            "username": username,
            "next": "Meridian emailed a verification link to the address given. Ask the user to open it, then call \
                     `meridian_setup` with action `request_api_key`. The account cannot sign in until it is verified."
        }));
    }
    let mut msg = failure("Account was not created", &r);
    if r.status == 400 && p.turnstile_token.as_deref().is_none_or(str::is_empty) {
        msg.push_str(
            ". If the server requires a Turnstile check, this call cannot pass it: ask the user to sign up in the \
             Meridian web app, verify their email, and then call `request_api_key`.",
        );
    }
    Err(msg)
}

async fn request_api_key(backend: &Backend, p: SetupParams) -> Outcome {
    let password = required(&p.password, "password")?;
    let login = match (p.username.as_deref().map(str::trim).filter(|u| !u.is_empty()), p.email.as_deref().map(str::trim).filter(|u| !u.is_empty())) {
        (Some(u), _) => json!({ "username": u, "password": password }),
        (None, Some(e)) => json!({ "email": e, "password": password }),
        _ => return Err("`username` or `email` is required for this action. Ask the user for it.".into()),
    };
    let r = backend.raw(Method::POST, "/security/login", Some(login), None, &[]).await?;
    if !r.ok() {
        let v = r.json();
        if v.get("code").and_then(Value::as_str) == Some("EMAIL_VERIFICATION_REQUIRED") {
            return Err("The account's email is not verified yet. Ask the user to open the verification link Meridian \
                        emailed them, then try again.".into());
        }
        return Err(failure("Sign-in failed", &r));
    }
    let Some(session) = r.json().get("sessionId").and_then(Value::as_str).map(|s| format!("m_session_{s}")) else {
        return Err("Sign-in succeeded but Meridian returned no session.".into());
    };
    let stomp = p.replace_existing.unwrap_or(false);
    let path = format!("/user/apiKey?stomp={stomp}");
    let key = backend.raw(Method::GET, &path, None, Some(&session), &[]).await;
    // The sign-in session was only needed for this; end it whatever happened.
    let _ = backend.raw(Method::GET, "/security/logout", None, Some(&session), &[]).await;
    let key = key?;
    if key.status == 409 {
        return Err("This account already has an active API key, and Meridian shows a key only once. If the user no \
                    longer has it, ask whether to replace it (that revokes the old key), then call again with \
                    `replace_existing` true.".into());
    }
    if !key.ok() {
        return Err(failure("API key was not issued", &key));
    }
    let raw_key = key.text.trim().trim_matches('"').to_string();
    backend.set_api_key(raw_key.clone());
    Ok(json!({
        "apiKey": raw_key,
        "inUse": "This server is now using the key; the other Meridian tools work for the rest of this session.",
        "next": "Tell the user to keep the key: it is shown only once and is not saved anywhere. To use it in future \
                 sessions they set MERIDIAN_API_KEY in this server's MCP configuration. Treat it as a secret and \
                 do not write it into the repository."
    }))
}

async fn buy_credits(backend: &Backend, p: SetupParams) -> Outcome {
    let credits = p.credits.ok_or("`credits` is required for this action. Ask the user how many to buy.")?;
    match backend.call(Method::POST, "/credits/purchases", Some(json!({ "credits": credits })), None, false, None).await {
        Reply::Ok(mut v) => {
            v["next"] = json!(
                "Show the user the address, the amount in sats (or the bitcoinUri) and when the request expires. They \
                 pay from their own wallet; this tool cannot send Bitcoin. After they pay, call `meridian_setup` with \
                 action `purchase_status` and this purchaseId until status is PAID (it waits for confirmations)."
            );
            Ok(v)
        }
        other => Err(reply_message(other)),
    }
}

async fn purchase_status(backend: &Backend, p: SetupParams) -> Outcome {
    let id = required(&p.purchase, "purchase")?;
    if uuid::Uuid::parse_str(&id).is_err() {
        return Err("`purchase` must be the purchaseId that `buy_credits` returned.".into());
    }
    match backend.call(Method::GET, &format!("/credits/purchases/{id}"), None, None, false, None).await {
        Reply::Ok(v) => Ok(v),
        other => Err(reply_message(other)),
    }
}

fn reply_message(reply: Reply) -> String {
    match reply {
        Reply::Failed(m) => m,
        _ => "Meridian asked for a passphrase, which this action does not use.".into(),
    }
}

/// Pay-first: no email, password or captcha. The Bitcoin payment is what proves the sign-up is real.
async fn signup_with_bitcoin(backend: &Backend, session: &Session, p: SetupParams) -> Outcome {
    let credits = p.credits.ok_or("`credits` is required for this action. Ask the user how many to buy.")?;
    let r = backend.raw(Method::POST, "/mcp/v1/signup", Some(json!({ "credits": credits })), None, &[]).await?;
    if !r.ok() {
        return Err(failure("Sign-up was not started", &r));
    }
    let mut v = r.json();
    let (Some(id), Some(secret)) = (
        v.get("signupId").and_then(Value::as_str).map(str::to_string),
        v.get("claimSecret").and_then(Value::as_str).map(str::to_string),
    ) else {
        return Err("Meridian returned no sign-up.".into());
    };
    session.set_signup(id, secret);
    v["next"] = json!(
        "Show the user the address, the amount in sats (or the bitcoinUri) and when it expires. They pay from their own \
         wallet; this tool cannot send Bitcoin. The account does not exist for use until the payment confirms. Keep the \
         claimSecret: it is the only way back into this sign-up, so tell the user to store it safely if this session may \
         end. Then call `meridian_setup` with action `signup_status` until it reports the API key."
    );
    Ok(v)
}

/// Checks a pay-first sign-up; once paid, collects the API key and starts using it.
async fn signup_status(backend: &Backend, session: &Session, p: SetupParams) -> Outcome {
    let held = session.signup();
    let id = p.signup.clone().or(held.as_ref().map(|h| h.0.clone()))
        .ok_or("No sign-up in progress. Call `signup_with_bitcoin` first, or pass `signup` and `claim_secret`.")?;
    let secret = p.claim_secret.clone().or(held.map(|h| h.1))
        .ok_or("`claim_secret` is required: it was returned by `signup_with_bitcoin`.")?;
    if uuid::Uuid::parse_str(&id).is_err() {
        return Err("`signup` must be the signupId that `signup_with_bitcoin` returned.".into());
    }
    let headers = [("X-Claim-Secret", secret.as_str())];
    let r = backend.raw(Method::GET, &format!("/mcp/v1/signup/{id}"), None, None, &headers).await?;
    if !r.ok() {
        return Err(failure("Sign-up status", &r));
    }
    let mut status = r.json();
    if status.get("status").and_then(Value::as_str) != Some("PAID") {
        status["next"] = json!("Not paid yet. Wait for the user to pay and for the network to confirm, then check again. \
            If the payment request has expired, start a new sign-up.");
        return Ok(status);
    }
    let k = backend.raw(Method::POST, &format!("/mcp/v1/signup/{id}/api-key"), None, None, &headers).await?;
    if !k.ok() {
        return Err(failure("Paid, but the API key was not issued", &k));
    }
    let Some(key) = k.json().get("apiKey").and_then(Value::as_str).map(str::to_string) else {
        return Err("Meridian returned no API key.".into());
    };
    backend.set_api_key(key.clone());
    session.clear_signup();
    Ok(json!({
        "status": "PAID",
        "credits": status.get("credits"),
        "apiKey": key,
        "inUse": "This server is now using the key; the other Meridian tools work for the rest of this session.",
        "next": "Tell the user to keep the key: it is not saved anywhere, and to use it in future sessions they set \
                 MERIDIAN_API_KEY in this server's MCP configuration. This account has no email or password yet; to \
                 sign in to the Meridian web app they open its 'Claim your account' page and enter the key."
    }))
}
