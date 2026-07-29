use super::*;

#[derive(Serialize)]
pub struct DeviceCodeInfo {
    pub user_code: String,
    pub verification_uri: String,
    pub message: String,
}

async fn run_auth_flow(
    network: std::sync::Arc<crate::network::NetworkManager>,
    db: Db,
    device_code: String,
    interval: u64,
) -> Result<AccountView> {
    let mut interval = interval.max(1);
    let token = loop {
        tokio::time::sleep(Duration::from_secs(interval)).await;
        match microsoft::poll_token(&network, &device_code).await? {
            PollOutcome::Pending => continue,
            PollOutcome::SlowDown => {
                interval += 5;
                continue;
            }
            PollOutcome::Token(token) => break token,
        }
    };

    let mc = microsoft::authenticate_minecraft(&network, &token.access_token).await?;
    let account = Account {
        id: mc.uuid.clone(),
        name: mc.name,
        mc_access_token: mc.access_token,
        refresh_token: token.refresh_token,
        expires_at: chrono::Utc::now().timestamp() + mc.expires_in,
    };

    tracing::info!(account = %account.name, uuid = %account.id, "microsoft sign-in completed");
    let mut store = db.load_accounts()?;
    store.upsert_active(account);
    db.save_accounts(&store)?;

    store
        .views()
        .into_iter()
        .find(|v| v.id == mc.uuid)
        .ok_or_else(|| Error::other("account vanished after save"))
}

#[tauri::command]
#[tracing::instrument(skip_all, err)]
pub async fn auth_begin(app: AppHandle, state: State<'_, AppState>) -> Result<DeviceCodeInfo> {
    let device = microsoft::request_device_code(&state.network).await?;
    tracing::info!(
        verification_uri = %device.verification_uri,
        interval = device.interval,
        "device code issued"
    );
    let info = DeviceCodeInfo {
        user_code: device.user_code.clone(),
        verification_uri: device.verification_uri.clone(),
        message: device.message.clone(),
    };

    let network = state.network.clone();
    let db = state.db.clone();
    let device_code = device.device_code.clone();
    let interval = device.interval;

    tokio::spawn(async move {
        match run_auth_flow(network, db, device_code, interval).await {
            Ok(view) => {
                let _ = app.emit(
                    "auth:state",
                    json!({ "status": "success", "account": view }),
                );
            }
            Err(e) => {
                tracing::error!(error = %e, "microsoft sign-in failed");
                let _ = app.emit(
                    "auth:state",
                    json!({ "status": "error", "message": e.to_string() }),
                );
            }
        }
    });

    Ok(info)
}

#[tauri::command]
#[tracing::instrument(skip_all, err)]
pub fn list_accounts(state: State<AppState>) -> Result<Vec<AccountView>> {
    Ok(state.db.load_accounts()?.views())
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub fn set_active_account(state: State<AppState>, account_id: String) -> Result<()> {
    let mut store = state.db.load_accounts()?;
    if store.accounts.iter().any(|a| a.id == account_id) {
        store.active_id = Some(account_id);
        state.db.save_accounts(&store)?;
        tracing::info!("active account changed");
    }
    Ok(())
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub fn remove_account(state: State<AppState>, account_id: String) -> Result<()> {
    let mut store = state.db.load_accounts()?;
    store.accounts.retain(|a| a.id != account_id);
    if store.active_id.as_deref() == Some(account_id.as_str()) {
        store.active_id = store.accounts.first().map(|a| a.id.clone());
    }
    state.db.save_accounts(&store)?;
    tracing::info!(remaining = store.accounts.len(), "account removed");
    Ok(())
}
