//! `meridian_setup`: create an account, get an API key, and buy credits with Bitcoin, so an agent can take a
//! user from nothing to a funded account. The password is used for one call and never kept.

use reqwest::Method;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::backend::{Backend, Raw, Reply};
use crate::session::Session;
use crate::store::{PendingSignup, Store};

#[derive(Deserialize, JsonSchema)]
pub struct SetupParams {
    /// What to do. With no account yet and no email: `signup_with_bitcoin`, then `signup_status`. With an email: 
    /// `create_account`, then `request_api_key`. `request_api_key` with no login details asks for the pay-first account's key: the key once paid, 402 until then. With an account: `buy_credits`, then `purchase_status`.
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
    /// `signup_with_bitcoin`: optional. The first payment must be worth at least 5 USD, which is what is invoiced when
    /// this is left out. It is a minimum, not a price: a Bitcoin payment of more buys more credits (at the credit rate,
    /// rounded down to whole credits). `buy_credits`: how many credits to buy (1 to 10000).
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

pub async fn run(backend: &Backend, session: &Session, store: &Store, p: SetupParams) -> Outcome {
    match p.action.to_ascii_lowercase().as_str() {
        "signup_with_bitcoin" => signup_with_bitcoin(backend, session, store, p).await,
        "signup_status" => signup_status(backend, session, store, p).await,
        "create_account" => create_account(backend, p).await,
        "request_api_key" => request_api_key(backend, session, store, p).await,
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
    let mut text = format!("{what}: {} {code} {message}", r.status).replace("  ", " ");
    if let Some(details) = v.get("details").filter(|d| d.as_object().is_some_and(|o| !o.is_empty())) {
        text.push_str(&format!(" details: {details}"));
    }
    text
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

async fn request_api_key(backend: &Backend, session: &Session, store: &Store, p: SetupParams) -> Outcome {
    let has_login = [&p.username, &p.email, &p.password].iter().any(|v| v.as_deref().is_some_and(|s| !s.trim().is_empty()));
    if !has_login && (session.signup().is_some() || p.signup.is_some()) {
        return signup_api_key(backend, session, store, &p).await;
    }
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
    let stored = keep_key(store, &raw_key);
    Ok(merge(json!({
        "apiKey": raw_key,
        "inUse": "This server is now using the key; the other Meridian tools work for the rest of this session.",
        "next": KEY_NEXT
    }), stored))
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
async fn signup_with_bitcoin(backend: &Backend, session: &Session, store: &Store, p: SetupParams) -> Outcome {
    let body = match p.credits {
        Some(credits) => json!({ "credits": credits }),
        None => json!({}),
    };
    let r = backend.raw(Method::POST, "/mcp/v1/signup", Some(body), None, &[]).await?;
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
    session.set_signup(id.clone(), secret.clone());
    // Saved before the invoice is shown to anyone, so a crash or restart between "pay this" and "payment arrived" cannot
    // lose the only way back into the sign-up.
    let saved = store.save_signup(PendingSignup {
        id,
        claim_secret: secret,
        address: v.get("address").and_then(Value::as_str).map(str::to_string),
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
    });
    match saved {
        Ok(s) => {
            // Not echoed: the agent does not need the secret, and the transcript should not hold it.
            if let Some(o) = v.as_object_mut() {
                o.remove("claimSecret");
            }
            v["stored"] = json!(s.path.display().to_string());
            v["encrypted"] = json!(s.encrypted);
        }
        Err(e) => {
            v["storageWarning"] = json!(format!(
                "The sign-up was NOT saved locally: {e}. The claimSecret in this result is the only way back into it if this \
                 server stops; tell the user to keep it."
            ));
        }
    }
    v["next"] = json!(
        "Show the user the address, the amount in sats (or the bitcoinUri) and when it expires. They pay from their own \
         wallet; this tool cannot send Bitcoin. The amount is the minimum, not a fixed price: if they send more, they \
         get more credits (usdPerCredit each, rounded down), so ask whether they want to send more. Sending less than \
         the minimum buys nothing. The account does not exist for use until the payment confirms. The sign-up is saved \
         locally (see `stored`), so it survives this server restarting. Then call `meridian_setup` with action \
         `request_api_key` or `signup_status` until it reports the API key."
    );
    Ok(v)
}

/// Checks a pay-first sign-up; once paid, collects the API key and starts using it.
async fn signup_status(backend: &Backend, session: &Session, store: &Store, p: SetupParams) -> Outcome {
    let (id, secret) = signup_credentials(session, &p)?;
    let headers = [("X-Claim-Secret", secret.as_str())];
    let r = backend.raw(Method::GET, &format!("/mcp/v1/signup/{id}"), None, None, &headers).await?;
    if !r.ok() {
        return Err(failure("Sign-up status", &r));
    }
    let mut status = r.json();
    if status.get("status").and_then(Value::as_str) != Some("PAID") {
        status["next"] = json!("Not paid yet. If `payment.status` is PENDING or RECEIVED the payment has been seen and is \
            waiting for confirmations; otherwise wait for the user to pay. Check again, or call `request_api_key`, \
            which answers 402 until the payment is confirmed. If the payment request has expired, start a new sign-up.");
        return Ok(status);
    }
    let (key, stored) = fetch_signup_key(backend, session, store, &id, &secret).await?;
    Ok(merge(json!({
        "status": "PAID",
        "credits": status.get("credits"),
        "apiKey": key,
        "inUse": "This server is now using the key; the other Meridian tools work for the rest of this session.",
        "next": KEY_NEXT_CLAIM
    }), stored))
}

/// The sign-up's ID and claim secret: from the call, else the one this session started.
fn signup_credentials(session: &Session, p: &SetupParams) -> Result<(String, String), String> {
    let held = session.signup();
    let id = p.signup.clone().or(held.as_ref().map(|h| h.0.clone()))
        .ok_or("No sign-up in progress. Call `signup_with_bitcoin` first, or pass `signup` and `claim_secret`.")?;
    let secret = p.claim_secret.clone().or(held.map(|h| h.1))
        .ok_or("`claim_secret` is required: it was returned by `signup_with_bitcoin`.")?;
    if uuid::Uuid::parse_str(&id).is_err() {
        return Err("`signup` must be the signupId that `signup_with_bitcoin` returned.".into());
    }
    Ok((id, secret))
}

/// Asks for the pay-first account's API key. Paid: the key, now in use. Not paid: an error carrying `402
/// PAYMENT_REQUIRED` and how far the payment has got (for example `PENDING` with its transaction).
async fn fetch_signup_key(
    backend: &Backend,
    session: &Session,
    store: &Store,
    id: &str,
    secret: &str,
) -> Result<(String, Value), String> {
    let headers = [("X-Claim-Secret", secret)];
    let k = backend.raw(Method::POST, &format!("/mcp/v1/signup/{id}/api-key"), None, None, &headers).await?;
    if k.status == 402 {
        return Err(failure("Payment required", &k));
    }
    if !k.ok() {
        return Err(failure("API key was not issued", &k));
    }
    let Some(key) = k.json().get("apiKey").and_then(Value::as_str).map(str::to_string) else {
        return Err("Meridian returned no API key.".into());
    };
    backend.set_api_key(key.clone());
    session.clear_signup();
    // Kept before the key is shown to anyone. The sign-up's claim secret stays in the store too: it can issue a
    // replacement key if this one is ever lost, until the account is claimed in the web app.
    let stored = keep_key(store, &key);
    Ok((key, stored))
}

async fn signup_api_key(backend: &Backend, session: &Session, store: &Store, p: &SetupParams) -> Outcome {
    let (id, secret) = signup_credentials(session, p)?;
    let (key, stored) = fetch_signup_key(backend, session, store, &id, &secret).await?;
    Ok(merge(json!({
        "apiKey": key,
        "inUse": "This server is now using the key; the other Meridian tools work for the rest of this session.",
        "next": KEY_NEXT_CLAIM
    }), stored))
}

/// Keeps the API key locally. Returns what to tell the agent: where it was kept, or why it could not be.
fn keep_key(store: &Store, key: &str) -> Value {
    match store.save_key(key) {
        Ok(saved) => json!({ "stored": saved.path.display().to_string(), "encrypted": saved.encrypted }),
        Err(e) => json!({
            "storageWarning": format!("The API key was NOT saved: {e}. Tell the user to keep it, or it is lost when this server stops.")
        }),
    }
}

fn merge(mut v: Value, extra: Value) -> Value {
    if let (Some(into), Some(from)) = (v.as_object_mut(), extra.as_object()) {
        for (k, val) in from {
            into.insert(k.clone(), val.clone());
        }
    }
    v
}

const KEY_NEXT: &str = "The key is saved locally (see `stored`), so future sessions pick it up without MERIDIAN_API_KEY. \
    It is shown here once; do not paste it anywhere else or write it into the repository.";

const KEY_NEXT_CLAIM: &str = "The key is saved locally (see `stored`), so future sessions pick it up without MERIDIAN_API_KEY. \
    It is shown here once, for the web app: this account has no email or password yet, so to sign in there the user opens \
    its 'Claim your account' page and enters the key. Do not paste it anywhere else or write it into the repository.";
