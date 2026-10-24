use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Mutex;

#[derive(Default)]
struct AuthState(Mutex<Option<PendingLogin>>);

struct PendingLogin {
    client_id: String,
    device_code: String,
    interval: u64,
    next_poll: std::time::Instant,
}

#[derive(Serialize)]
struct DeviceCode {
    user_code: String,
    verification_uri: String,
    expires_in: u64,
    interval: u64,
}

#[derive(Deserialize)]
struct DeviceResponse {
    device_code: String,
    user_code: String,
    verification_uri: String,
    expires_in: u64,
    interval: Option<u64>,
    message: Option<String>,
}

#[derive(Serialize, Clone)]
struct Account {
    name: String,
    uuid: String,
}

#[tauri::command]
async fn begin_microsoft_login(client_id: String, state: tauri::State<'_, AuthState>) -> Result<DeviceCode, String> {
    let client_id = client_id.trim().to_owned();
    if client_id.is_empty() || client_id.len() > 100 { return Err("Trage zuerst Rooks eigene Microsoft-App-ID ein.".into()); }
    let response = reqwest::Client::new()
        .post("https://login.microsoftonline.com/consumers/oauth2/v2.0/devicecode")
        .form(&[("client_id", client_id.as_str()), ("scope", "XboxLive.signin offline_access")])
        .send().await.map_err(|e| format!("Microsoft ist nicht erreichbar: {e}"))?;
    let status = response.status();
    let body = response.text().await.map_err(|e| format!("Microsoft-Antwort konnte nicht gelesen werden: {e}"))?;
    let value: Value = serde_json::from_str(&body).map_err(|_| format!("Microsoft antwortete unerwartet (HTTP {status}). Prüfe die Client-ID und die Einstellung 'Öffentliche Clientflows zulassen'."))?;
    if !status.is_success() {
        let detail = value["error_description"].as_str()
            .or_else(|| value["message"].as_str())
            .unwrap_or("keine Fehlerbeschreibung geliefert");
        return Err(format!("Microsoft hat die Anmeldung abgelehnt (HTTP {status}): {detail}"));
    }
    let data: DeviceResponse = serde_json::from_value(value).map_err(|e| format!("Microsoft-Anmeldedaten fehlen: {e}"))?;
    let interval = data.interval.unwrap_or(5).max(5);
    *state.0.lock().map_err(|_| "Anmeldestatus nicht verfügbar.".to_string())? = Some(PendingLogin {
        client_id, device_code: data.device_code, interval,
        next_poll: std::time::Instant::now() + std::time::Duration::from_secs(interval),
    });
    Ok(DeviceCode { user_code: data.user_code, verification_uri: data.verification_uri, expires_in: data.expires_in, interval })
}

#[tauri::command]
async fn poll_microsoft_login(state: tauri::State<'_, AuthState>) -> Result<Option<Account>, String> {
    let (client_id, device_code, interval) = {
        let mut guard = state.0.lock().map_err(|_| "Anmeldestatus nicht verfügbar.".to_string())?;
        let pending = guard.as_mut().ok_or("Starte zuerst die Microsoft-Anmeldung.")?;
        if std::time::Instant::now() < pending.next_poll { return Ok(None); }
        pending.next_poll = std::time::Instant::now() + std::time::Duration::from_secs(pending.interval);
        (pending.client_id.clone(), pending.device_code.clone(), pending.interval)
    };
    let client = reqwest::Client::new();
    let response = client.post("https://login.microsoftonline.com/consumers/oauth2/v2.0/token")
        .form(&[("grant_type", "urn:ietf:params:oauth:grant-type:device_code"), ("client_id", client_id.as_str()), ("device_code", device_code.as_str())])
        .send().await.map_err(|e| format!("Microsoft-Anmeldung fehlgeschlagen: {e}"))?;
    let status = response.status();
    let token: Value = response.json().await.map_err(|e| format!("Microsoft-Antwort konnte nicht gelesen werden: {e}"))?;
    if !status.is_success() {
        match token["error"].as_str().unwrap_or("") {
            "authorization_pending" => return Ok(None),
            "slow_down" => {
                if let Ok(mut guard) = state.0.lock() { if let Some(p) = guard.as_mut() { p.interval = interval + 5; } }
                return Ok(None);
            }
            "expired_token" | "authorization_declined" | "bad_verification_code" => {
                if let Ok(mut guard) = state.0.lock() { *guard = None; }
                return Err("Die Anmeldung ist abgelaufen oder wurde abgelehnt. Bitte starte sie erneut.".into());
            }
            _ => return Err(format!("Microsoft-Login: {}", token["error_description"].as_str().unwrap_or("unbekannter Fehler"))),
        }
    }
    let access = token["access_token"].as_str().ok_or("Microsoft hat kein Zugriffstoken geliefert.")?;
    let xbl: Value = client.post("https://user.auth.xboxlive.com/user/authenticate")
        .json(&json!({"Properties":{"AuthMethod":"RPS","SiteName":"user.auth.xboxlive.com","RpsTicket":format!("d={access}")},"RelyingParty":"http://auth.xboxlive.com","TokenType":"JWT"}))
        .send().await.map_err(|e| format!("Xbox-Anmeldung fehlgeschlagen: {e}"))?.error_for_status().map_err(|_| "Xbox konnte dieses Microsoft-Konto nicht authentifizieren. Prüfe, ob die App-ID XboxLive.signin erlaubt.".to_string())?.json::<Value>().await.map_err(|e| e.to_string())?;
    let xsts: Value = client.post("https://xsts.auth.xboxlive.com/xsts/authorize")
        .json(&json!({"Properties":{"SandboxId":"RETAIL","UserTokens":[xbl["Token"]]},"RelyingParty":"rp://api.minecraftservices.com/","TokenType":"JWT"}))
        .send().await.map_err(|e| format!("Xbox-Berechtigung fehlgeschlagen: {e}"))?.error_for_status().map_err(|_| "Xbox XSTS hat die Anmeldung abgelehnt. Prüfe Xbox-Konto und Jugendschutzeinstellungen.".to_string())?.json::<Value>().await.map_err(|e| e.to_string())?;
    let uhs = xsts["DisplayClaims"]["xui"][0]["uhs"].as_str().ok_or("Xbox hat keine Benutzerkennung geliefert.")?;
    let mc_response = client.post("https://api.minecraftservices.com/authentication/login_with_xbox")
        .json(&json!({"identityToken":format!("XBL3.0 x={uhs};{}", xsts["Token"].as_str().unwrap_or(""))}))
        .send().await.map_err(|e| format!("Minecraft-Anmeldung fehlgeschlagen: {e}"))?;
    let mc_status = mc_response.status();
    let mc_body = mc_response.text().await.map_err(|e| format!("Minecraft-Antwort konnte nicht gelesen werden: {e}"))?;
    if !mc_status.is_success() {
        let details: Value = serde_json::from_str(&mc_body).unwrap_or(Value::Null);
        let reason = details["errorMessage"].as_str()
            .or_else(|| details["details"]["reason"].as_str())
            .or_else(|| details["error"].as_str())
            .unwrap_or("Minecraft Services hat keinen Grund mitgeliefert.");
        if mc_status.as_u16() == 403 && reason.to_ascii_lowercase().contains("invalid app registration") {
            return Err("Microsoft-Login war erfolgreich, aber Minecraft Services meldet HTTP 403: Invalid app registration. Rooks Client-ID muss erst von Minecraft für diesen Dienst zugelassen werden. Das ist kein Beweis, dass deinem Konto Java fehlt. Hinweise: https://aka.ms/AppRegInfo".into());
        }
        return Err(format!("Minecraft Services hat die Anmeldung abgelehnt (HTTP {mc_status}): {reason}. Prüfe, ob du das richtige Microsoft-Konto mit Minecraft: Java Edition verwendest."));
    }
    let mc: Value = serde_json::from_str(&mc_body).map_err(|e| format!("Minecraft-Antwort konnte nicht gelesen werden: {e}"))?;
    let mc_token = mc["access_token"].as_str().ok_or("Minecraft Services hat kein Spieltoken geliefert.")?;
    let profile: Value = client.get("https://api.minecraftservices.com/minecraft/profile").bearer_auth(mc_token)
        .send().await.map_err(|e| format!("Minecraft-Profil konnte nicht geladen werden: {e}"))?.error_for_status().map_err(|_| "Dieses Microsoft-Konto hat kein verfügbares Java-Edition-Profil.".to_string())?.json().await.map_err(|e| e.to_string())?;
    let account = Account { name: profile["name"].as_str().ok_or("Minecraft-Profilname fehlt.")?.to_owned(), uuid: profile["id"].as_str().ok_or("Minecraft-Profil-ID fehlt.")?.to_owned() };
    if let Ok(mut guard) = state.0.lock() { *guard = None; }
    Ok(Some(account))
}

#[cfg(target_os = "windows")]
#[tauri::command]
fn open_verification_page(url: String) -> Result<(), String> {
    let parsed = reqwest::Url::parse(&url).map_err(|_| "Ungültige Microsoft-Anmeldeseite.".to_string())?;
    let trusted_host = matches!(parsed.host_str(), Some("microsoft.com" | "www.microsoft.com"));
    let trusted_path = matches!(parsed.path(), "/link" | "/devicelogin");
    if parsed.scheme() != "https" || !trusted_host || !trusted_path || parsed.query().is_some() {
        return Err("Microsoft hat eine nicht unterstützte Anmeldeseite zurückgegeben.".into());
    }
    std::process::Command::new("rundll32").args(["url.dll,FileProtocolHandler", &url]).spawn().map(|_| ()).map_err(|e| e.to_string())
}

pub fn run() {
    tauri::Builder::default()
        .manage(AuthState::default())
        .invoke_handler(tauri::generate_handler![begin_microsoft_login, poll_microsoft_login, open_verification_page])
        .run(tauri::generate_context!())
        .expect("Rook's Client could not start");
}
